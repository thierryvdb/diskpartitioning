//! Inventário Windows SOMENTE LEITURA via cmdlets `Get-*` do módulo Storage.
//! `parse` é puro (testado com fixtures). `WindowsProvider::discover_with_errors` executa o PowerShell:
//! NÃO deve ser chamado na máquina de trabalho sem autorização específica (AGENTS.md).

use disk_domain::{Discovery, Disk, Fs, Kind, Partition, StorageProvider, Table};
use serde::Deserialize;
use std::collections::HashMap;
use storage_common::{kind_from_gpt, kind_from_mbr, run_readonly, usable_range};

/// Script fixo, sem interpolação. Apenas cmdlets `Get-*`; strings forçadas para evitar enums/char numéricos.
const SCRIPT: &str = "[Console]::OutputEncoding=[Text.Encoding]::UTF8; \
@{ \
disks=@(Get-Disk | Select-Object Number,FriendlyName,Size,LogicalSectorSize,IsReadOnly,\
@{n='PartitionStyle';e={[string]$_.PartitionStyle}},@{n='BusType';e={[string]$_.BusType}}); \
partitions=@(Get-Partition | Select-Object DiskNumber,PartitionNumber,Offset,Size,GptType,MbrType,\
@{n='DriveLetter';e={[string]$_.DriveLetter}}); \
volumes=@(Get-Volume | Select-Object Size,SizeRemaining,\
@{n='DriveLetter';e={[string]$_.DriveLetter}},@{n='FileSystem';e={[string]$_.FileSystem}}) \
} | ConvertTo-Json -Depth 4 -Compress";

#[derive(Deserialize, Default)]
#[serde(default)]
struct Raw {
    disks: Vec<RDisk>,
    partitions: Vec<RPart>,
    volumes: Vec<RVol>,
}
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "PascalCase")]
struct RDisk {
    number: u32,
    friendly_name: Option<String>,
    size: u64,
    logical_sector_size: u64,
    partition_style: String,
    bus_type: String,
}
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "PascalCase")]
struct RPart {
    disk_number: u32,
    partition_number: u32,
    offset: u64,
    size: u64,
    gpt_type: Option<String>,
    mbr_type: Option<u8>,
    drive_letter: Option<String>,
}
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "PascalCase")]
struct RVol {
    size: Option<u64>,
    size_remaining: Option<u64>,
    drive_letter: Option<String>,
    file_system: Option<String>,
}

fn letter(s: &Option<String>) -> Option<char> {
    s.as_deref()?
        .trim()
        .chars()
        .next()
        .filter(char::is_ascii_alphabetic)
}

fn fs_from(name: &str) -> Fs {
    match name.to_ascii_lowercase().as_str() {
        "ntfs" => Fs::Ntfs,
        "fat32" => Fs::Fat32,
        "exfat" => Fs::Exfat,
        _ => Fs::Unknown, // RAW, ReFS, FAT16, vazio…: não presumir
    }
}

/// Converte a saída JSON do script em discos. Falhas por dispositivo viram `errors`, não abortam o resto.
pub fn parse(json: &str) -> Result<Discovery, String> {
    let raw: Raw = serde_json::from_str(json.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("JSON inválido: {e}"))?;
    let vols: HashMap<char, &RVol> = raw
        .volumes
        .iter()
        .filter_map(|v| letter(&v.drive_letter).map(|l| (l.to_ascii_uppercase(), v)))
        .collect();
    let mut out = Discovery::default();

    for d in &raw.disks {
        let id = format!("disk{}", d.number);
        let table = match d.partition_style.as_str() {
            "GPT" => Table::Gpt,
            "MBR" => Table::Mbr,
            other => {
                out.errors.push(format!(
                    "{id}: sem tabela GPT/MBR reconhecida ({other}); ignorado."
                ));
                continue;
            }
        };
        let ss = d.logical_sector_size;
        if ss != 512 && ss != 4096 {
            out.errors.push(format!(
                "{id}: tamanho de setor lógico inesperado ({ss}); ignorado."
            ));
            continue;
        }
        let total = d.size / ss;
        let Some((first, last)) = usable_range(table, total, ss) else {
            out.errors
                .push(format!("{id}: disco pequeno demais; ignorado."));
            continue;
        };
        let mut disk = Disk {
            id: id.clone(),
            model: d
                .friendly_name
                .clone()
                .unwrap_or_else(|| format!("Disco {}", d.number)),
            bus: d.bus_type.clone(),
            table,
            sector_size: ss as u32,
            total_sectors: total,
            first_usable_lba: first,
            last_usable_excl_lba: last,
            partitions: Vec::new(),
        };
        for p in raw.partitions.iter().filter(|p| p.disk_number == d.number) {
            let pid = format!("d{}p{}", d.number, p.partition_number);
            if !p.offset.is_multiple_of(ss) || !p.size.is_multiple_of(ss) {
                out.errors.push(format!(
                    "{pid}: offset/tamanho não alinhados ao setor; ignorada."
                ));
                continue;
            }
            let kind = match (&p.gpt_type, p.mbr_type) {
                (Some(g), _) if table == Table::Gpt => kind_from_gpt(g),
                (_, Some(m)) if table == Table::Mbr => kind_from_mbr(m),
                _ => Kind::Normal,
            };
            let vol =
                letter(&p.drive_letter).and_then(|l| vols.get(&l.to_ascii_uppercase()).copied());
            let (fs, used) = match vol {
                Some(v) => (
                    fs_from(v.file_system.as_deref().unwrap_or("")),
                    v.size
                        .zip(v.size_remaining)
                        .map(|(s, r)| s.saturating_sub(r) / ss),
                ),
                None if kind == Kind::Msr => (Fs::None, None),
                None => (Fs::Unknown, None),
            };
            let name = match (kind, letter(&p.drive_letter)) {
                (Kind::Efi, _) => "EFI".to_string(),
                (Kind::Msr, _) => "MSR".to_string(),
                (Kind::Recovery, _) => "Recovery".to_string(),
                (_, Some(l)) => format!("{}:", l.to_ascii_uppercase()),
                _ => format!("Partição {}", p.partition_number),
            };
            disk.partitions.push(Partition {
                id: pid,
                name,
                start_lba: p.offset / ss,
                length_sectors: p.size / ss,
                fs,
                kind,
                used_sectors: used,
                r#virtual: false,
            });
        }
        out.disks.push(disk);
    }
    Ok(out)
}

pub struct WindowsProvider;

impl WindowsProvider {
    /// EXECUTA o PowerShell (somente `Get-*`). Requer autorização do proprietário na máquina de trabalho.
    pub fn discover_with_errors(&self) -> Result<Discovery, String> {
        parse(&run_readonly(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", SCRIPT],
        )?)
    }
}

impl StorageProvider for WindowsProvider {
    fn discover(&self) -> Vec<Disk> {
        self.discover_with_errors()
            .map(|d| d.disks)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Saída sintética no formato do script (nenhum dado real).
    const FIXTURE: &str = r#"{"disks":[
      {"Number":0,"FriendlyName":"Mock NVMe","Size":512110190592,"LogicalSectorSize":512,"IsReadOnly":false,"PartitionStyle":"GPT","BusType":"NVMe"},
      {"Number":1,"FriendlyName":"Mock USB","Size":64023257088,"LogicalSectorSize":512,"IsReadOnly":false,"PartitionStyle":"MBR","BusType":"USB"},
      {"Number":2,"FriendlyName":"Disco bruto","Size":1000204886016,"LogicalSectorSize":512,"IsReadOnly":false,"PartitionStyle":"RAW","BusType":"SATA"}],
     "partitions":[
      {"DiskNumber":0,"PartitionNumber":1,"Offset":1048576,"Size":104857600,"GptType":"{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}","MbrType":null,"DriveLetter":"\u0000"},
      {"DiskNumber":0,"PartitionNumber":2,"Offset":105906176,"Size":16777216,"GptType":"{e3c9e316-0b5c-4db8-817d-f92df00215ae}","MbrType":null,"DriveLetter":"\u0000"},
      {"DiskNumber":0,"PartitionNumber":3,"Offset":122683392,"Size":400000000000,"GptType":"{ebd0a0a2-b9e5-4433-87c0-68b6b72699c7}","MbrType":null,"DriveLetter":"C"},
      {"DiskNumber":0,"PartitionNumber":4,"Offset":400122683392,"Size":1000,"GptType":"{de94bba4-06d1-4d40-a16a-bfd50179d6ac}","MbrType":null,"DriveLetter":""},
      {"DiskNumber":1,"PartitionNumber":1,"Offset":1048576,"Size":8589934592,"GptType":null,"MbrType":7,"DriveLetter":"E"}],
     "volumes":[
      {"Size":400000000000,"SizeRemaining":150000000000,"DriveLetter":"C","FileSystem":"NTFS"},
      {"Size":8589934592,"SizeRemaining":8000000000,"DriveLetter":"E","FileSystem":"exFAT"}]}"#;

    #[test]
    fn converte_fixture_em_discos_validos() {
        let d = parse(FIXTURE).unwrap();
        assert_eq!(d.disks.len(), 2, "RAW é ignorado");
        assert!(d
            .errors
            .iter()
            .any(|e| e.contains("disk2") && e.contains("RAW")));
        // offset/tamanho 1000 B não alinha a 512 => erro por partição, sem derrubar o disco
        assert!(d.errors.iter().any(|e| e.contains("d0p4")));
        let nvme = &d.disks[0];
        assert_eq!((nvme.table, nvme.bus.as_str()), (Table::Gpt, "NVMe"));
        let kinds: Vec<_> = nvme.partitions.iter().map(|p| p.kind).collect();
        assert_eq!(kinds, [Kind::Efi, Kind::Msr, Kind::Normal]);
        let c = &nvme.partitions[2];
        assert_eq!(
            (c.name.as_str(), c.fs, c.start_lba),
            ("C:", Fs::Ntfs, 239_616)
        );
        assert_eq!(c.used_sectors, Some(250_000_000_000 / 512));
        assert_eq!(nvme.partitions[1].fs, Fs::None);
        let usb = &d.disks[1];
        assert_eq!((usb.table, usb.partitions[0].fs), (Table::Mbr, Fs::Exfat));
        for disk in &d.disks {
            assert!(
                disk_planner::check_layout(disk).is_empty(),
                "{:?}",
                disk_planner::check_layout(disk)
            );
        }
    }

    #[test]
    fn entrada_invalida_e_vazia() {
        assert!(parse("não é json").is_err());
        let vazio = parse(r#"{"disks":[],"partitions":[],"volumes":[]}"#).unwrap();
        assert!(vazio.disks.is_empty() && vazio.errors.is_empty());
    }

    #[test]
    fn script_usa_somente_cmdlets_de_leitura() {
        for proibido in [
            "Set-",
            "New-",
            "Remove-",
            "Resize-",
            "Format-",
            "Clear-",
            "Initialize-",
            "Start-",
            "Invoke-",
            "Repair-",
            "diskpart",
        ] {
            assert!(!SCRIPT.contains(proibido), "script contém {proibido}");
        }
        assert!(
            SCRIPT.contains("Get-Disk")
                && SCRIPT.contains("Get-Partition")
                && SCRIPT.contains("Get-Volume")
        );
    }
}
