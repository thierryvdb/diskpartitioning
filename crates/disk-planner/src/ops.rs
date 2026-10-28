//! Operações de catálogo avançado (simuladas): mesclar, dividir, rótulo, tipo e conversão GPT/MBR.

use crate::{find_mut, ALIGN_SECTORS};
use disk_domain::*;

const DEP: &str = "Esta tarefa depende de uma operação anterior (partição alvo inexistente).";

type Res = Result<Vec<String>, String>;

pub(crate) fn apply(disks: &mut [Disk], op: &Op) -> Option<Res> {
    Some(match &op.kind {
        OpKind::Merge { part_id, other_id } => merge(disks, part_id, other_id),
        OpKind::Split {
            part_id,
            first_length_sectors,
            new_name,
            fs,
        } => split(disks, &op.id, part_id, *first_length_sectors, new_name, *fs),
        OpKind::SetLabel { part_id, label } => set_label(disks, part_id, label),
        OpKind::SetKind { part_id, kind } => set_kind(disks, part_id, *kind),
        OpKind::ConvertTable { disk_id, to } => convert(disks, disk_id, *to),
        _ => return None,
    })
}

fn check_editable(p: &Partition) -> Result<(), String> {
    if p.kind.is_protected() {
        return Err(format!("Partição {:?} é protegida.", p.kind).to_uppercase());
    }
    if p.fs == Fs::Unknown {
        return Err(
            "Sistema de arquivos desconhecido ou não suportado (ex.: criptografado, LVM, RAID)."
                .into(),
        );
    }
    Ok(())
}

fn merge(disks: &mut [Disk], a_id: &str, b_id: &str) -> Res {
    if a_id == b_id {
        return Err("Escolha duas partições diferentes.".into());
    }
    let (d, ia) = find_mut(disks, a_id).ok_or(DEP)?;
    let ib = d
        .partitions
        .iter()
        .position(|p| p.id == b_id)
        .ok_or("A segunda partição deve estar no mesmo disco.")?;
    let (k, r) = if d.partitions[ia].start_lba <= d.partitions[ib].start_lba {
        (ia, ib)
    } else {
        (ib, ia)
    };
    let (keep, gone) = (d.partitions[k].clone(), d.partitions[r].clone());
    check_editable(&keep)?;
    check_editable(&gone)?;
    if keep.end() != Some(gone.start_lba) {
        return Err("As partições não são adjacentes.".into());
    }
    let mut warn = vec![
        "Mesclar migra os arquivos da segunda para a primeira partição; não é atômico. Faça backup."
            .to_string(),
    ];
    if keep.fs != gone.fs {
        warn.push(format!(
            "Sistemas de arquivos diferentes ({:?} e {:?}): os arquivos são copiados para o da primeira.",
            keep.fs, gone.fs
        ));
    }
    let used = match (keep.used_sectors, gone.used_sectors) {
        (Some(a), Some(b)) => {
            if b > keep.length_sectors.saturating_sub(a) {
                return Err("Espaço livre da primeira partição insuficiente para receber os dados da segunda.".into());
            }
            Some(a + b)
        }
        _ => {
            warn.push("Uso desconhecido: não é possível garantir que os dados caibam.".into());
            None
        }
    };
    let total = keep
        .length_sectors
        .checked_add(gone.length_sectors)
        .ok_or("Tamanho excede o limite.")?;
    d.partitions[k].length_sectors = total;
    d.partitions[k].used_sectors = used;
    d.partitions.remove(r);
    Ok(warn)
}

fn split(
    disks: &mut [Disk],
    op_id: &str,
    part_id: &str,
    first: u64,
    new_name: &str,
    fs: Fs,
) -> Res {
    let (d, i) = find_mut(disks, part_id).ok_or(DEP)?;
    let p = d.partitions[i].clone();
    check_editable(&p)?;
    if first == 0 || first >= p.length_sectors {
        return Err(
            "Tamanho inválido para dividir: use um valor entre 0 e o tamanho atual.".into(),
        );
    }
    let old_end = p.end().ok_or("Tamanho excede o limite.")?;
    let new_start = (p.start_lba + first).div_ceil(ALIGN_SECTORS) * ALIGN_SECTORS;
    if new_start >= old_end {
        return Err("Sem espaço para a nova partição após o alinhamento.".into());
    }
    let new_len = old_end - new_start;
    let mut warn = vec![
        "Dividir copia os arquivos escolhidos; atributos e ACLs não são garantidos. Faça backup."
            .to_string(),
    ];
    if !(p.start_lba + first).is_multiple_of(ALIGN_SECTORS) {
        warn.push("Fim da primeira parte não alinhado a 1 MiB: sobra espaço não alocado.".into());
    }
    let excess = p.used_sectors.map_or(0, |u| u.saturating_sub(first));
    if excess > new_len {
        return Err("Não há espaço para os arquivos excedentes na nova partição.".into());
    }
    if excess > 0 {
        warn.push("Parte dos arquivos precisaria ir para a nova partição.".into());
    }
    d.partitions[i].length_sectors = first;
    d.partitions[i].used_sectors = p.used_sectors.map(|u| u.min(first));
    d.partitions.push(Partition {
        id: format!("new-{op_id}"),
        name: new_name.to_string(),
        start_lba: new_start,
        length_sectors: new_len,
        fs,
        kind: Kind::Normal,
        used_sectors: Some(excess),
        r#virtual: true,
    });
    Ok(warn)
}

fn set_label(disks: &mut [Disk], part_id: &str, label: &str) -> Res {
    let (d, i) = find_mut(disks, part_id).ok_or(DEP)?;
    let p = &mut d.partitions[i];
    check_editable(p)?;
    let (limit, in_bytes) = match p.fs {
        Fs::Ntfs => (32, false),
        Fs::Fat32 => (11, false),
        Fs::Exfat => (15, false),
        Fs::Ext4 => (16, true),
        Fs::Swap => (15, true),
        _ => return Err("Este sistema de arquivos não suporta rótulo.".into()),
    };
    let len = if in_bytes {
        label.len()
    } else {
        label.chars().count()
    };
    if len > limit {
        return Err(format!(
            "Rótulo longo demais: máximo {limit} {}.",
            if in_bytes { "bytes" } else { "caracteres" }
        ));
    }
    if label.chars().any(char::is_control) {
        return Err("Rótulo contém caracteres de controle.".into());
    }
    let mut warn = Vec::new();
    let mut label = label.to_string();
    if p.fs == Fs::Fat32 {
        if label.chars().any(|c| "*?.,;:/\\|+=<>[]\"".contains(c)) {
            return Err("Rótulo FAT32 contém caracteres inválidos.".into());
        }
        if label != label.to_uppercase() {
            warn.push("FAT32 grava o rótulo em maiúsculas.".into());
            label = label.to_uppercase();
        }
    }
    p.name = if label.is_empty() {
        "(sem rótulo)".into()
    } else {
        label
    };
    Ok(warn)
}

fn set_kind(disks: &mut [Disk], part_id: &str, kind: Kind) -> Res {
    let (d, i) = find_mut(disks, part_id).ok_or(DEP)?;
    let p = &mut d.partitions[i];
    if p.kind.is_protected() {
        return Err(format!("Partição {:?} é protegida.", p.kind).to_uppercase());
    }
    match kind {
        Kind::Normal => return Err("A partição já é normal.".into()),
        Kind::Msr => return Err("A MSR não pode ser atribuída manualmente.".into()),
        Kind::Efi if p.fs != Fs::Fat32 => {
            return Err("A partição EFI precisa ser FAT32.".into());
        }
        _ => {}
    }
    p.kind = kind;
    Ok(vec![
        "Alterar o tipo pode impedir a inicialização do sistema; a partição passa a ser protegida."
            .into(),
    ])
}

fn convert(disks: &mut [Disk], disk_id: &str, to: Table) -> Res {
    let d = disks
        .iter_mut()
        .find(|d| d.id == disk_id)
        .ok_or("Disco não encontrado.")?;
    if d.table == to {
        return Err("O disco já usa esta tabela de partições.".into());
    }
    let mut warn =
        vec!["Converter a tabela de partições é arriscado; faça backup antes.".to_string()];
    match to {
        Table::Mbr => {
            if d.partitions.iter().any(|p| p.kind == Kind::Msr) {
                return Err("A MSR não existe em MBR: exclua-a antes de converter.".into());
            }
            if d.partitions.len() > 4 {
                return Err("MBR suporta no máximo 4 partições primárias.".into());
            }
            let max = u64::from(u32::MAX);
            if d.partitions
                .iter()
                .any(|p| p.start_lba > max || p.length_sectors > max)
            {
                return Err(
                    "Há partição além do limite de 32 bits do MBR (2 TiB com setor de 512).".into(),
                );
            }
            if d.partitions.iter().any(|p| p.kind == Kind::Efi) {
                warn.push("Em MBR a partição EFI não inicia sistemas UEFI (exige GPT).".into());
            }
            d.first_usable_lba = 1;
            d.last_usable_excl_lba = d.total_sectors.min(max + 1);
        }
        Table::Gpt => {
            let entries = 128 * 128 / u64::from(d.sector_size);
            let first = 2 + entries;
            let last = d
                .total_sectors
                .checked_sub(1 + entries)
                .ok_or("Disco pequeno demais para GPT.")?;
            let fits = d
                .partitions
                .iter()
                .all(|p| p.start_lba >= first && p.end().is_some_and(|e| e <= last));
            if !fits {
                return Err("A GPT precisa de espaço no início e no fim do disco; as partições atuais não deixam esse espaço.".into());
            }
            d.first_usable_lba = first;
            d.last_usable_excl_lba = last;
        }
    }
    d.table = to;
    Ok(warn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(id: &str, start: u64, len: u64, fs: Fs, used: Option<u64>) -> Partition {
        Partition {
            id: id.into(),
            name: id.into(),
            start_lba: start,
            length_sectors: len,
            fs,
            kind: Kind::Normal,
            used_sectors: used,
            r#virtual: false,
        }
    }
    fn disk(parts: Vec<Partition>) -> Vec<Disk> {
        vec![Disk {
            id: "d".into(),
            model: "m".into(),
            bus: "USB".into(),
            table: Table::Gpt,
            sector_size: 512,
            total_sectors: 1 << 24,
            first_usable_lba: 34,
            last_usable_excl_lba: (1 << 24) - 34,
            partitions: parts,
        }]
    }

    #[test]
    fn merge_soma_tamanho_e_uso_e_valida_espaco() {
        let mut d = disk(vec![
            part("a", 2048, 4096, Fs::Ntfs, Some(1000)),
            part("b", 6144, 2048, Fs::Ntfs, Some(500)),
        ]);
        merge(&mut d, "b", "a").unwrap(); // ordem dos argumentos não importa: vale a posição
        assert_eq!(d[0].partitions.len(), 1);
        let m = &d[0].partitions[0];
        assert_eq!(
            (m.id.as_str(), m.length_sectors, m.used_sectors),
            ("a", 6144, Some(1500))
        );

        let mut d = disk(vec![
            part("a", 2048, 4096, Fs::Ntfs, Some(4000)), // livre 96
            part("b", 6144, 2048, Fs::Ntfs, Some(500)),
        ]);
        assert!(merge(&mut d, "a", "b")
            .unwrap_err()
            .contains("insuficiente"));
        let mut d = disk(vec![
            part("a", 2048, 10, Fs::Ntfs, None),
            part("b", 4096, 10, Fs::Ntfs, None),
        ]);
        assert!(merge(&mut d, "a", "b").unwrap_err().contains("adjacentes"));
        assert!(merge(&mut d, "a", "a").is_err());
    }

    #[test]
    fn split_conserva_capacidade_e_alinha_nova_partição() {
        let mut d = disk(vec![part("a", 2048, 10_000, Fs::Ext4, Some(1000))]);
        split(&mut d, "s", "a", 3000, "nova", Fs::Ext4).unwrap();
        let (a, n) = (&d[0].partitions[0], &d[0].partitions[1]);
        assert_eq!(a.length_sectors, 3000);
        assert_eq!(n.start_lba % ALIGN_SECTORS, 0);
        assert_eq!(
            n.start_lba + n.length_sectors,
            2048 + 10_000,
            "mesmo fim da original"
        );
        assert!(n.r#virtual);
        assert!(split(&mut d, "t", "a", 0, "x", Fs::Ext4).is_err());
        let mut d = disk(vec![part("a", 2048, 10_000, Fs::Ext4, Some(9000))]);
        assert!(split(&mut d, "s", "a", 1000, "nova", Fs::Ext4)
            .unwrap_err()
            .contains("excedentes"));
    }

    #[test]
    fn rotulo_respeita_limites_do_fs() {
        let mut d = disk(vec![
            part("f", 2048, 10, Fs::Fat32, None),
            part("e", 4096, 10, Fs::Ext4, None),
            part("u", 8192, 10, Fs::Unknown, None),
        ]);
        assert!(set_label(&mut d, "f", "ABCDEFGHIJKL").is_err()); // 12 > 11
        assert!(set_label(&mut d, "f", "a:b").is_err());
        assert_eq!(set_label(&mut d, "f", "dados").unwrap().len(), 1);
        assert_eq!(d[0].partitions[0].name, "DADOS");
        assert!(set_label(&mut d, "e", "ééééééééé").is_err()); // 18 bytes > 16
        assert!(set_label(&mut d, "e", "ok").is_ok());
        assert!(set_label(&mut d, "u", "x").is_err());
    }

    #[test]
    fn tipo_exige_fat32_para_efi_e_protege() {
        let mut d = disk(vec![
            part("n", 2048, 10, Fs::Ntfs, None),
            part("f", 4096, 10, Fs::Fat32, None),
        ]);
        assert!(set_kind(&mut d, "n", Kind::Efi)
            .unwrap_err()
            .contains("FAT32"));
        assert!(set_kind(&mut d, "n", Kind::Msr).is_err());
        assert!(set_kind(&mut d, "f", Kind::Efi).is_ok());
        assert!(
            set_kind(&mut d, "f", Kind::Recovery).is_err(),
            "já protegida"
        );
    }

    #[test]
    fn conversao_gpt_mbr_valida_limites() {
        let mut d = disk(vec![part("a", 2048, 100, Fs::Ntfs, None)]);
        convert(&mut d, "d", Table::Mbr).unwrap();
        assert_eq!((d[0].table, d[0].first_usable_lba), (Table::Mbr, 1));
        assert!(convert(&mut d, "d", Table::Mbr).is_err(), "já é MBR");
        convert(&mut d, "d", Table::Gpt).unwrap();
        assert_eq!(d[0].first_usable_lba, 34);
        let mut d = disk(vec![part("a", 2048, 100, Fs::Ntfs, None)]);
        d[0].partitions[0].start_lba = u64::from(u32::MAX) + 1;
        assert!(convert(&mut d, "d", Table::Mbr)
            .unwrap_err()
            .contains("32 bits"));
        let mut m = disk(vec![part("a", 1, 100, Fs::Ntfs, None)]);
        m[0].table = Table::Mbr;
        assert!(convert(&mut m, "d", Table::Gpt)
            .unwrap_err()
            .contains("espaço"));
    }
}
