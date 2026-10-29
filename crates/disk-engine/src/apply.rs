//! Tradução pura (sem E/S) do plano da UI para passos do motor. Só MBR de 512 B, criar/excluir/formatar
//! FAT32 e NTFS; qualquer outra operação é recusada para a aplicação real.
use crate::{Entry, Fs, Op as E};
use disk_domain::{Disk, Fs as DFs, Op, OpKind, Table};

#[derive(Debug, PartialEq, Eq)]
pub struct FormatStep {
    pub start_lba: u64,
    pub fs: Fs,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Translated {
    pub mbr: Vec<E>,
    pub formats: Vec<FormatStep>,
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

pub fn translate(
    disk: &Disk,
    table: &[Option<Entry>; 4],
    ops: &[Op],
) -> Result<Translated, String> {
    if disk.table != Table::Mbr || disk.sector_size != 512 {
        return Err("a aplicação real só suporta discos MBR com setor de 512 bytes".into());
    }
    let plan = disk_planner::simulate(std::slice::from_ref(disk), ops);
    if !plan.blocked_ops.is_empty() {
        return Err(format!(
            "o plano tem operações bloqueadas: {}",
            plan.blocked_ops.join(", ")
        ));
    }
    let after = &plan.disks[0];
    let mut slots = *table;
    let mut t = Translated {
        mbr: vec![],
        formats: vec![],
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
                let slot = slots
                    .iter()
                    .position(|e| e.is_some_and(|e| e.start as u64 == p.start_lba))
                    .ok_or("partição não encontrada no MBR")?;
                slots[slot] = None;
                t.mbr.push(E::Delete { slot });
                t.summary.push(format!(
                    "EXCLUIR a partição {} (entrada {slot} do MBR, início no setor {})",
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
                t.summary.push(format!(
                    "CRIAR partição no setor {start_lba}, {} MiB (entrada {slot})",
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
        let t = translate(&disk(), &table(), &ops).unwrap();
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
            &table(),
            &[op(
                "a",
                OpKind::Resize {
                    part_id: "d1p1".into(),
                    new_length_sectors: 5000,
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
        assert!(translate(&disk(), &table(), &[c]).is_err());
        let mut gpt = disk();
        gpt.table = Table::Gpt;
        assert!(translate(&gpt, &table(), &[]).is_err());
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
        assert!(translate(&disk(), &table(), &[c])
            .unwrap_err()
            .contains("bloqueadas"));
    }
}
