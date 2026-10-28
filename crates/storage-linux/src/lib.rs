//! Inventário Linux SOMENTE LEITURA via `lsblk --json` (util-linux).
//! `parse` é puro (testado com fixtures). `LinuxProvider::discover_with_errors` executa o `lsblk`:
//! NÃO deve ser chamado na máquina de trabalho sem autorização específica (AGENTS.md).
//! Sem serial/WWN (privacidade). `Disk.id` é o nome do kernel (transitório, ex.: `sda`).

use disk_domain::{Discovery, Disk, Fs, Kind, Partition, StorageProvider, Table};
use serde_json::Value;
use storage_common::{kind_from_gpt, kind_from_mbr, run_readonly, usable_range};

/// Colunas fixas, sem texto vindo de fora.
const COLUMNS: &str = "NAME,KNAME,TYPE,SIZE,RM,RO,MODEL,TRAN,LOG-SEC,PHY-SEC,PTTYPE,PARTTYPE,PARTLABEL,PARTN,START,FSTYPE,LABEL";
/// FSUSED só existe em util-linux recente (e só para FS montado); se faltar, o lsblk falha por inteiro: tenta sem.
const COLUMNS_WITH_USAGE: &str = "NAME,KNAME,TYPE,SIZE,RM,RO,MODEL,TRAN,LOG-SEC,PHY-SEC,PTTYPE,PARTTYPE,PARTLABEL,PARTN,START,FSTYPE,LABEL,FSUSED";

// lsblk varia entre versões: números podem vir como string, booleanos como "0"/"1".
fn num(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str()?.trim().parse().ok())
}
fn text(v: &Value) -> Option<&str> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty())
}

fn fs_from(fstype: Option<&str>) -> Fs {
    match fstype.map(str::to_ascii_lowercase).as_deref() {
        Some("ntfs") => Fs::Ntfs,
        Some("vfat") => Fs::Fat32,
        Some("exfat") => Fs::Exfat,
        Some("ext4") => Fs::Ext4,
        Some("swap") => Fs::Swap,
        None => Fs::None,
        _ => Fs::Unknown, // LUKS, LVM2_member, btrfs, xfs…: identificar sem presumir suporte
    }
}

pub fn parse(json: &str) -> Result<Discovery, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| format!("JSON inválido: {e}"))?;
    let devices = root["blockdevices"]
        .as_array()
        .ok_or("campo blockdevices ausente")?;
    let mut out = Discovery::default();

    for d in devices.iter().filter(|d| text(&d["type"]) == Some("disk")) {
        let kname = text(&d["kname"]).or(text(&d["name"])).unwrap_or("?");
        let table = match text(&d["pttype"]) {
            Some("gpt") => Table::Gpt,
            Some("dos") => Table::Mbr,
            other => {
                out.errors.push(format!(
                    "{kname}: sem tabela GPT/MBR reconhecida ({}); ignorado.",
                    other.unwrap_or("nenhuma")
                ));
                continue;
            }
        };
        let ss = num(&d["log-sec"]).unwrap_or(0);
        let size = num(&d["size"]).unwrap_or(0);
        if (ss != 512 && ss != 4096) || size == 0 || !size.is_multiple_of(ss) {
            out.errors
                .push(format!("{kname}: tamanho/setor inválido; ignorado."));
            continue;
        }
        let total = size / ss;
        let Some((first, last)) = usable_range(table, total, ss) else {
            out.errors
                .push(format!("{kname}: disco pequeno demais; ignorado."));
            continue;
        };
        let mut disk = Disk {
            id: kname.to_string(),
            model: text(&d["model"]).unwrap_or(kname).to_string(),
            bus: text(&d["tran"])
                .map(str::to_ascii_uppercase)
                .unwrap_or_else(|| "?".into()),
            table,
            sector_size: ss as u32,
            total_sectors: total,
            first_usable_lba: first,
            last_usable_excl_lba: last,
            partitions: Vec::new(),
        };
        let empty = Vec::new();
        for p in d["children"]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .filter(|p| text(&p["type"]) == Some("part"))
        {
            let pk = text(&p["kname"]).unwrap_or("?");
            // `START` do lsblk vem do sysfs, sempre em unidades de 512 bytes.
            let (Some(start512), Some(bytes)) = (num(&p["start"]), num(&p["size"])) else {
                out.errors
                    .push(format!("{pk}: início/tamanho ausente; ignorada."));
                continue;
            };
            let start_bytes = start512 * 512;
            if !start_bytes.is_multiple_of(ss) || !bytes.is_multiple_of(ss) {
                out.errors.push(format!(
                    "{pk}: offset/tamanho não alinhados ao setor; ignorada."
                ));
                continue;
            }
            let ptype = text(&p["parttype"]);
            let kind = match (table, ptype) {
                (Table::Gpt, Some(g)) => kind_from_gpt(g),
                (Table::Mbr, Some(m)) => u8::from_str_radix(m.trim_start_matches("0x"), 16)
                    .map(kind_from_mbr)
                    .unwrap_or(Kind::Normal),
                _ => Kind::Normal,
            };
            let fs = fs_from(text(&p["fstype"]));
            disk.partitions.push(Partition {
                id: pk.to_string(),
                name: text(&p["partlabel"])
                    .or(text(&p["label"]))
                    .unwrap_or(pk)
                    .to_string(),
                start_lba: start_bytes / ss,
                length_sectors: bytes / ss,
                fs,
                kind,
                used_sectors: num(&p["fsused"]).map(|b| b / ss),
                r#virtual: false,
            });
        }
        out.disks.push(disk);
    }
    Ok(out)
}

pub struct LinuxProvider;

impl LinuxProvider {
    /// EXECUTA o `lsblk` (leitura). Requer autorização do proprietário na máquina de trabalho.
    pub fn discover_with_errors(&self) -> Result<Discovery, String> {
        let run = |cols| run_readonly("lsblk", &["--json", "--bytes", "--output", cols]);
        parse(&run(COLUMNS_WITH_USAGE).or_else(|_| run(COLUMNS))?)
    }
}

impl StorageProvider for LinuxProvider {
    fn discover(&self) -> Vec<Disk> {
        self.discover_with_errors()
            .map(|d| d.disks)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Saída sintética no formato do lsblk (util-linux recente e antigo misturados de propósito).
    const FIXTURE: &str = r#"{"blockdevices":[
      {"name":"sda","kname":"sda","type":"disk","size":512110190592,"rm":false,"ro":false,"model":"Mock SATA ","tran":"sata","log-sec":512,"phy-sec":4096,"pttype":"gpt",
       "children":[
        {"name":"sda1","kname":"sda1","type":"part","size":536870912,"start":2048,"partn":1,"parttype":"c12a7328-f81f-11d2-ba4b-00a0c93ec93b","partlabel":"EFI System Partition","fstype":"vfat","label":null},
        {"name":"sda2","kname":"sda2","type":"part","size":"107374182400","start":"1050624","partn":2,"parttype":"0fc63daf-8483-4772-8e79-3d69d8477de4","partlabel":null,"fstype":"ext4","label":"raiz","fsused":"21474836480",
         "children":[{"name":"x","kname":"dm-0","type":"crypt","size":1}]},
        {"name":"sda3","kname":"sda3","type":"part","size":8589934592,"start":210765824,"partn":3,"parttype":"0657fd6d-a4ab-43c4-84e5-0933c84b4f4f","partlabel":null,"fstype":"swap","label":null},
        {"name":"sda4","kname":"sda4","type":"part","size":1073741824,"start":227543040,"partn":4,"parttype":"ca7d7ccb-63ed-4c53-861c-1742536059cc","partlabel":null,"fstype":"crypto_LUKS","label":null}]},
      {"name":"sdb","kname":"sdb","type":"disk","size":"64023257088","rm":"1","ro":"0","model":null,"tran":"usb","log-sec":"512","pttype":"dos",
       "children":[{"name":"sdb1","kname":"sdb1","type":"part","size":8589934592,"start":2048,"parttype":"0x7","fstype":"exfat","label":"PEN"}]},
      {"name":"sdc","kname":"sdc","type":"disk","size":1000204886016,"log-sec":512,"pttype":null,"children":null},
      {"name":"loop0","kname":"loop0","type":"loop","size":65536,"log-sec":512,"pttype":null}]}"#;

    #[test]
    fn converte_fixture_em_discos_validos() {
        let d = parse(FIXTURE).unwrap();
        assert_eq!(
            d.disks.len(),
            2,
            "loop ignorado e disco sem tabela vira erro"
        );
        assert!(d
            .errors
            .iter()
            .any(|e| e.contains("sdc") && e.contains("nenhuma")));
        let sda = &d.disks[0];
        assert_eq!(
            (sda.table, sda.bus.as_str(), sda.model.as_str()),
            (Table::Gpt, "SATA", "Mock SATA")
        );
        let p: Vec<_> = sda.partitions.iter().map(|p| (p.kind, p.fs)).collect();
        assert_eq!(
            p,
            [
                (Kind::Efi, Fs::Fat32),
                (Kind::Normal, Fs::Ext4),
                (Kind::Normal, Fs::Swap),
                (Kind::Normal, Fs::Unknown)
            ]
        );
        assert_eq!(sda.partitions[0].name, "EFI System Partition");
        assert_eq!(sda.partitions[1].name, "raiz");
        assert_eq!(sda.partitions[1].length_sectors, 107_374_182_400 / 512);
        assert_eq!(sda.partitions[1].used_sectors, Some(21_474_836_480 / 512));
        assert_eq!(
            sda.partitions[0].used_sectors, None,
            "FS sem FSUSED: uso desconhecido"
        );
        let sdb = &d.disks[1];
        assert_eq!(
            (sdb.table, sdb.model.as_str(), sdb.partitions[0].fs),
            (Table::Mbr, "sdb", Fs::Exfat)
        );
        for disk in &d.disks {
            assert!(
                disk_planner::check_layout(disk).is_empty(),
                "{:?}",
                disk_planner::check_layout(disk)
            );
        }
    }

    #[test]
    fn setor_4096_converte_start_de_512() {
        let j = r#"{"blockdevices":[{"kname":"sdz","type":"disk","size":1073741824,"log-sec":4096,"pttype":"gpt",
          "children":[{"kname":"sdz1","type":"part","size":4194304,"start":2048,"fstype":"ext4"}]}]}"#;
        let p = &parse(j).unwrap().disks[0].partitions[0];
        assert_eq!((p.start_lba, p.length_sectors), (256, 1024)); // 1 MiB / 4096, 4 MiB / 4096
    }

    #[test]
    fn entrada_invalida_e_parcial() {
        assert!(parse("lsblk: erro").is_err());
        assert!(parse("{}").is_err());
        let j = r#"{"blockdevices":[{"kname":"a","type":"disk","size":1073741824,"log-sec":512,"pttype":"gpt",
          "children":[{"kname":"a1","type":"part","size":1000,"start":2048},{"kname":"a2","type":"part","size":1048576,"start":4096}]}]}"#;
        let d = parse(j).unwrap();
        assert_eq!(
            (d.disks[0].partitions.len(), d.errors.len()),
            (1, 1),
            "partição ruim não derruba o disco"
        );
    }

    #[test]
    fn colunas_fixas_sem_texto_externo() {
        assert!(COLUMNS
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == ',' || c == '-'));
    }
}
