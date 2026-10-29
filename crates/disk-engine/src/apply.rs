//! Tradução pura (sem E/S) do plano da UI para passos do motor: criar/excluir/formatar (FAT32 e NTFS) em MBR
//! ou GPT de 512 B, e converter MBR→GPT preservando as partições. Qualquer outra operação é recusada.
use crate::gpt::{GOp, GptEntry};
use crate::{plan_hash_str, Entry, Fs, Op as E};
use disk_domain::{Disk, Fs as DFs, Op, OpKind, Table};

const BASIC_DATA: [u8; 16] = [
    0xA2, 0xA0, 0xD0, 0xEB, 0xE5, 0xB9, 0x33, 0x44, 0x87, 0xC0, 0x68, 0xB6, 0xB7, 0x26, 0x99, 0xC7,
];
const EFI: [u8; 16] = [
    0x28, 0x73, 0x2A, 0xC1, 0x1F, 0xF8, 0xD2, 0x11, 0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B,
];

/// Tabela atual lida do disco real.
#[derive(Clone)]
pub enum Layout {
    Mbr([Option<Entry>; 4]),
    Gpt(Vec<Option<GptEntry>>),
}

#[derive(Debug, PartialEq, Eq)]
pub struct FormatStep {
    pub start_lba: u64,
    pub fs: Fs,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Translated {
    pub mbr: Vec<E>,
    pub gpt: Vec<GOp>,
    pub formats: Vec<FormatStep>,
    /// (início da partição, novo tamanho em setores): o Windows ajusta tabela e NTFS juntos.
    pub resizes: Vec<(u64, u64)>,
    /// (início da partição, rótulo já restrito a `[A-Za-z0-9 _.-]`).
    pub labels: Vec<(u64, String)>,
    pub summary: Vec<String>,
}

fn fs_of(fs: DFs) -> Result<Fs, String> {
    match fs {
        DFs::Fat32 => Ok(Fs::Fat32),
        DFs::Ntfs => Ok(Fs::Ntfs),
        other => Err(format!(
            "sistema de arquivos {other:?} não é suportado na aplicação real (só FAT32 e NTFS)"
        )),
    }
}

/// GUID v4 derivado de um hash. Determinístico de propósito: a prévia e a aplicação recalculam o mesmo plano
/// (e o mesmo hash de aprovação).
// ponytail: FNV de serial+início, não criptográfico; trocar por um gerador aleatório se GUIDs previsíveis importarem.
fn new_guid(seed: &str) -> [u8; 16] {
    let (a, b) = (
        plan_hash_str(&format!("{seed}a")),
        plan_hash_str(&format!("{seed}b")),
    );
    let mut g = [a.to_le_bytes(), b.to_le_bytes()].concat();
    g[7] = (g[7] & 0x0F) | 0x40;
    g[8] = (g[8] & 0x3F) | 0x80;
    g.try_into().unwrap()
}

fn gpt_entry(seed: &str, first: u64, sectors: u64, type_guid: [u8; 16]) -> GptEntry {
    GptEntry {
        type_guid,
        unique: new_guid(&format!("{seed}{first}")),
        first,
        last: first + sectors - 1,
        name: "Basic data partition".into(),
    }
}

enum Mode {
    Mbr([Option<Entry>; 4]),
    Gpt(Vec<Option<GptEntry>>),
}

pub fn translate(
    disk: &Disk,
    layout: &Layout,
    serial: &str,
    ops: &[Op],
) -> Result<Translated, String> {
    if disk.sector_size != 512 {
        return Err("a aplicação real só suporta discos com setor de 512 bytes".into());
    }
    let plan = disk_planner::simulate(std::slice::from_ref(disk), ops);
    if !plan.blocked_ops.is_empty() {
        return Err(format!(
            "o plano tem operações bloqueadas: {}",
            plan.blocked_ops.join(", ")
        ));
    }
    let after = &plan.disks[0];
    let mut mode =
        match (disk.table, layout) {
            (Table::Mbr, Layout::Mbr(s)) => Mode::Mbr(*s),
            (Table::Gpt, Layout::Gpt(e)) => Mode::Gpt(e.clone()),
            _ => return Err(
                "o tipo de tabela lido do disco não bate com o inventário; recarregue os discos"
                    .into(),
            ),
        };
    let mut t = Translated {
        mbr: vec![],
        gpt: vec![],
        formats: vec![],
        resizes: vec![],
        labels: vec![],
        summary: vec![],
    };
    let set_format = |t: &mut Translated, start_lba: u64, fs: Fs| {
        t.formats.retain(|f| f.start_lba != start_lba);
        t.formats.push(FormatStep { start_lba, fs });
    };
    for op in ops {
        match &op.kind {
            OpKind::Delete { part_id } => {
                let p = disk
                    .partitions
                    .iter()
                    .find(|p| &p.id == part_id)
                    .ok_or(format!("partição {part_id} não existe no disco real"))?;
                match &mut mode {
                    Mode::Mbr(slots) => {
                        let slot = slots
                            .iter()
                            .position(|e| e.is_some_and(|e| e.start as u64 == p.start_lba))
                            .ok_or("partição não encontrada no MBR")?;
                        slots[slot] = None;
                        t.mbr.push(E::Delete { slot });
                    }
                    Mode::Gpt(es) => {
                        let index = es
                            .iter()
                            .position(|e| e.as_ref().is_some_and(|e| e.first == p.start_lba))
                            .ok_or("partição não encontrada no GPT")?;
                        es[index] = None;
                        t.gpt.push(GOp::Remove { index });
                    }
                }
                t.summary.push(format!(
                    "EXCLUIR a partição {} (início no setor {})",
                    p.name, p.start_lba
                ));
            }
            OpKind::Create {
                disk_id,
                start_lba,
                length_sectors,
                fs,
                ..
            } => {
                if disk_id != &disk.id {
                    return Err("a operação é de outro disco".into());
                }
                let (kind, format) = match fs {
                    DFs::Fat32 => (0x0C, Some(Fs::Fat32)),
                    DFs::Ntfs => (0x07, Some(Fs::Ntfs)),
                    DFs::None => (0x07, None),
                    other => return Err(fs_of(*other).unwrap_err()),
                };
                match &mut mode {
                    Mode::Mbr(slots) => {
                        let slot = slots
                            .iter()
                            .position(Option::is_none)
                            .ok_or("o MBR não tem entrada livre (máximo 4 primárias)")?;
                        let (start, sectors) = (
                            u32::try_from(*start_lba).map_err(|_| "início acima de 2 TiB")?,
                            u32::try_from(*length_sectors).map_err(|_| "tamanho acima de 2 TiB")?,
                        );
                        slots[slot] = Some(Entry {
                            boot: 0,
                            kind,
                            start,
                            sectors,
                        });
                        t.mbr.push(E::Create {
                            slot,
                            start,
                            sectors,
                            kind,
                        });
                    }
                    Mode::Gpt(es) => {
                        let e = gpt_entry(serial, *start_lba, *length_sectors, BASIC_DATA);
                        *es.iter_mut()
                            .find(|s| s.is_none())
                            .ok_or("a tabela GPT está cheia")? = Some(e.clone());
                        t.gpt.push(GOp::Add(e));
                    }
                }
                t.summary.push(format!(
                    "CRIAR partição no setor {start_lba}, {} MiB",
                    length_sectors / 2048
                ));
                if let Some(f) = format {
                    set_format(&mut t, *start_lba, f);
                    t.summary
                        .push(format!("FORMATAR essa partição como {}", f.name()));
                }
            }
            OpKind::Format { part_id, fs } => {
                let p = after
                    .partitions
                    .iter()
                    .find(|p| &p.id == part_id)
                    .ok_or(format!("partição {part_id} não existe no plano"))?;
                let f = fs_of(*fs)?;
                set_format(&mut t, p.start_lba, f);
                t.summary.push(format!(
                    "FORMATAR {} como {} (APAGA o conteúdo)",
                    p.name,
                    f.name()
                ));
            }
            OpKind::Resize {
                part_id,
                new_length_sectors,
            } => {
                let p = disk
                    .partitions
                    .iter()
                    .find(|p| &p.id == part_id)
                    .ok_or(format!("partição {part_id} não existe no disco real"))?;
                if p.fs != DFs::Ntfs {
                    return Err("redimensionar na aplicação real só é suportado para NTFS".into());
                }
                t.resizes.retain(|r| r.0 != p.start_lba);
                t.resizes.push((p.start_lba, *new_length_sectors));
                t.summary.push(format!(
                    "REDIMENSIONAR {} para {} MiB (NTFS e tabela, feito pelo Windows)",
                    p.name,
                    new_length_sectors / 2048
                ));
            }
            OpKind::SetLabel { part_id, label } => {
                let p = after
                    .partitions
                    .iter()
                    .find(|p| &p.id == part_id)
                    .ok_or(format!("partição {part_id} não existe no plano"))?;
                if !label
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || " _.-".contains(c))
                {
                    return Err("rótulo: só letras, números, espaço, _ . -".into());
                }
                t.labels.retain(|l| l.0 != p.start_lba);
                t.labels.push((p.start_lba, label.clone()));
                t.summary.push(format!("RÓTULO de {}: \"{label}\"", p.name));
            }
            OpKind::ConvertTable {
                disk_id,
                to: Table::Gpt,
            } if disk_id == &disk.id => {
                let Mode::Mbr(slots) = &mode else {
                    return Err("o disco já é GPT".into());
                };
                if !t.mbr.is_empty() {
                    return Err(
                        "a conversão para GPT precisa ser a primeira operação da fila".into(),
                    );
                }
                t.gpt.push(GOp::Init {
                    disk_guid: new_guid(serial),
                });
                let mut es: Vec<Option<GptEntry>> = vec![None; 128];
                for (i, e) in slots.iter().flatten().enumerate() {
                    let ty = if e.kind == 0xEF { EFI } else { BASIC_DATA };
                    let g = gpt_entry(serial, e.start as u64, e.sectors as u64, ty);
                    es[i] = Some(g.clone());
                    t.gpt.push(GOp::Add(g));
                }
                t.summary.push("CONVERTER para GPT mantendo as partições existentes (o MBR é substituído por MBR protetor)".into());
                mode = Mode::Gpt(es);
            }
            other => {
                return Err(format!(
                    "operação não suportada na aplicação real: {other:?}"
                ))
            }
        }
    }
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use disk_domain::{Kind, Partition};

    fn disk() -> Disk {
        Disk {
            id: "disk1".into(),
            model: "USB".into(),
            bus: "USB".into(),
            table: Table::Mbr,
            sector_size: 512,
            total_sectors: 60_000_000,
            first_usable_lba: 2048,
            last_usable_excl_lba: 60_000_000,
            partitions: vec![Partition {
                id: "d1p1".into(),
                name: "Partição 1".into(),
                start_lba: 8064,
                length_sectors: 100_000,
                fs: DFs::Fat32,
                kind: Kind::Normal,
                used_sectors: None,
                r#virtual: false,
            }],
        }
    }
    fn table() -> [Option<Entry>; 4] {
        [
            Some(Entry {
                boot: 0,
                kind: 0x0C,
                start: 8064,
                sectors: 100_000,
            }),
            None,
            None,
            None,
        ]
    }
    fn op(id: &str, kind: OpKind) -> Op {
        Op {
            id: id.into(),
            kind,
        }
    }

    #[test]
    fn exclui_cria_e_formata() {
        let ops = [
            op(
                "a",
                OpKind::Delete {
                    part_id: "d1p1".into(),
                },
            ),
            op(
                "b",
                OpKind::Create {
                    disk_id: "disk1".into(),
                    start_lba: 2048,
                    length_sectors: 2_097_152,
                    fs: DFs::Ntfs,
                    name: "x".into(),
                },
            ),
        ];
        let t = translate(&disk(), &Layout::Mbr(table()), "SN", &ops).unwrap();
        assert_eq!(
            t.mbr,
            [
                E::Delete { slot: 0 },
                E::Create {
                    slot: 0,
                    start: 2048,
                    sectors: 2_097_152,
                    kind: 0x07
                }
            ]
        );
        assert_eq!(
            t.formats,
            [FormatStep {
                start_lba: 2048,
                fs: Fs::Ntfs
            }]
        );
    }

    #[test]
    fn recusa_o_que_nao_e_suportado() {
        let r = translate(
            &disk(),
            &Layout::Mbr(table()),
            "SN",
            &[op(
                "a",
                OpKind::Move {
                    part_id: "d1p1".into(),
                    new_start_lba: 20_480,
                },
            )],
        );
        assert!(r.unwrap_err().contains("não suportada"));
        let c = op(
            "c",
            OpKind::Create {
                disk_id: "disk1".into(),
                start_lba: 2048,
                length_sectors: 4096,
                fs: DFs::Ext4,
                name: "x".into(),
            },
        );
        assert!(translate(&disk(), &Layout::Mbr(table()), "SN", &[c]).is_err());
        let mut gpt = disk();
        gpt.table = Table::Gpt;
        assert!(translate(&gpt, &Layout::Mbr(table()), "SN", &[]).is_err());
    }

    #[test]
    fn plano_bloqueado_nao_e_traduzido() {
        // cria por cima da partição existente
        let c = op(
            "c",
            OpKind::Create {
                disk_id: "disk1".into(),
                start_lba: 8064,
                length_sectors: 4096,
                fs: DFs::Ntfs,
                name: "x".into(),
            },
        );
        assert!(translate(&disk(), &Layout::Mbr(table()), "SN", &[c])
            .unwrap_err()
            .contains("bloqueadas"));
    }

    #[test]
    fn converte_mbr_para_gpt_preservando_particoes() {
        let t = translate(
            &disk(),
            &Layout::Mbr(table()),
            "SN",
            &[op(
                "c",
                OpKind::ConvertTable {
                    disk_id: "disk1".into(),
                    to: Table::Gpt,
                },
            )],
        )
        .unwrap();
        assert!(matches!(t.gpt[0], GOp::Init { .. }));
        let GOp::Add(e) = &t.gpt[1] else { panic!() };
        assert_eq!((e.first, e.last), (8064, 8064 + 100_000 - 1));
        assert!(t.mbr.is_empty());
        // converter depois de outra operação não é aceito
        let ops = [
            op(
                "a",
                OpKind::Delete {
                    part_id: "d1p1".into(),
                },
            ),
            op(
                "c",
                OpKind::ConvertTable {
                    disk_id: "disk1".into(),
                    to: Table::Gpt,
                },
            ),
        ];
        assert!(translate(&disk(), &Layout::Mbr(table()), "SN", &ops).is_err());
    }

    #[test]
    fn gpt_exclui_e_cria() {
        let mut d = disk();
        d.table = Table::Gpt;
        let existing = gpt_entry("SN", 8064, 100_000, BASIC_DATA);
        let mut es = vec![None; 128];
        es[0] = Some(existing);
        let ops = [
            op(
                "a",
                OpKind::Delete {
                    part_id: "d1p1".into(),
                },
            ),
            op(
                "b",
                OpKind::Create {
                    disk_id: "disk1".into(),
                    start_lba: 2048,
                    length_sectors: 4096,
                    fs: DFs::Ntfs,
                    name: "x".into(),
                },
            ),
        ];
        let t = translate(&d, &Layout::Gpt(es), "SN", &ops).unwrap();
        assert_eq!(t.gpt[0], GOp::Remove { index: 0 });
        assert!(matches!(&t.gpt[1], GOp::Add(e) if e.first == 2048 && e.last == 6143));
        assert_eq!(
            t.formats,
            [FormatStep {
                start_lba: 2048,
                fs: Fs::Ntfs
            }]
        );
    }

    #[test]
    fn rotulo_e_redimensionar() {
        let mut d = disk();
        d.partitions[0].fs = DFs::Ntfs;
        let lay = Layout::Mbr(table());
        let ok = [
            op(
                "r",
                OpKind::Resize {
                    part_id: "d1p1".into(),
                    new_length_sectors: 50_000,
                },
            ),
            op(
                "l",
                OpKind::SetLabel {
                    part_id: "d1p1".into(),
                    label: "Dados 1".into(),
                },
            ),
        ];
        let t = translate(&d, &lay, "SN", &ok).unwrap();
        assert_eq!(
            (t.resizes, t.labels),
            (vec![(8064, 50_000)], vec![(8064, "Dados 1".to_string())])
        );
        // injeção via rótulo é recusada antes de chegar ao PowerShell
        let bad = [op(
            "l",
            OpKind::SetLabel {
                part_id: "d1p1".into(),
                label: "x'; calc".into(),
            },
        )];
        assert!(translate(&d, &lay, "SN", &bad).is_err());
        // FAT32 não redimensiona
        let fat = [op(
            "r",
            OpKind::Resize {
                part_id: "d1p1".into(),
                new_length_sectors: 50_000,
            },
        )];
        assert!(translate(&disk(), &lay, "SN", &fat).is_err());
    }
}
