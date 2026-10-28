//! Planejador determinístico: sem E/S de armazenamento; o snapshot base nunca é mutado.

use disk_domain::*;

/// 1 MiB com setor de 512.
pub const ALIGN_SECTORS: u64 = 2048;
const MAX_MBR_PRIMARY: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gap {
    pub start_lba: u64,
    pub length_sectors: u64,
}

/// Espaço NÃO ALOCADO (derivado da tabela, não do espaço livre do FS).
pub fn gaps(d: &Disk) -> Vec<Gap> {
    let mut parts: Vec<&Partition> = d.partitions.iter().collect();
    parts.sort_by_key(|p| p.start_lba);
    let mut out = Vec::new();
    let mut cur = d.first_usable_lba;
    for p in parts {
        if p.start_lba > cur {
            out.push(Gap {
                start_lba: cur,
                length_sectors: p.start_lba - cur,
            });
        }
        if let Some(e) = p.end() {
            cur = cur.max(e);
        }
    }
    if d.last_usable_excl_lba > cur {
        out.push(Gap {
            start_lba: cur,
            length_sectors: d.last_usable_excl_lba - cur,
        });
    }
    out
}

/// Valida o layout; vazio = ok.
pub fn check_layout(d: &Disk) -> Vec<String> {
    let mut errs = Vec::new();
    let mut parts: Vec<&Partition> = d.partitions.iter().collect();
    parts.sort_by_key(|p| p.start_lba);
    for (i, p) in parts.iter().enumerate() {
        if p.length_sectors == 0 {
            errs.push(format!("{}: tamanho inválido.", p.name));
        }
        match p.end() {
            Some(e) if p.start_lba >= d.first_usable_lba && e <= d.last_usable_excl_lba => {}
            _ => errs.push("A operação excede o espaço disponível no disco.".into()),
        }
        if i > 0 {
            if let Some(e) = parts[i - 1].end() {
                if e > p.start_lba {
                    errs.push(format!("{} e {} se sobrepõem.", parts[i - 1].name, p.name));
                }
            }
        }
    }
    if d.table == Table::Mbr && d.partitions.len() > MAX_MBR_PRIMARY {
        errs.push("MBR suporta no máximo 4 partições primárias.".into());
    }
    errs
}

fn find_mut<'a>(disks: &'a mut [Disk], part_id: &str) -> Option<(&'a mut Disk, usize)> {
    disks.iter_mut().find_map(|d| {
        let i = d.partitions.iter().position(|p| p.id == part_id)?;
        Some((d, i))
    })
}

/// Aplica a operação num layout já clonado; devolve avisos ou erro.
fn apply(disks: &mut [Disk], op: &Op) -> Result<Vec<String>, String> {
    let mut warn = Vec::new();
    if let OpKind::Create {
        disk_id,
        start_lba,
        length_sectors,
        fs,
        name,
    } = &op.kind
    {
        let d = disks
            .iter_mut()
            .find(|d| &d.id == disk_id)
            .ok_or("Disco não encontrado.")?;
        if !start_lba.is_multiple_of(ALIGN_SECTORS) {
            warn.push("Início não alinhado a 1 MiB.".to_string());
        }
        d.partitions.push(Partition {
            id: format!("new-{}", op.id),
            name: name.clone(),
            start_lba: *start_lba,
            length_sectors: *length_sectors,
            fs: *fs,
            kind: Kind::Normal,
            used_sectors: None,
            r#virtual: true,
        });
        return Ok(warn);
    }
    if let OpKind::CloneDisk {
        source_disk_id,
        target_disk_id,
    } = &op.kind
    {
        return clone_disk(disks, &op.id, source_disk_id, target_disk_id);
    }
    if let OpKind::ClonePartition {
        part_id,
        target_disk_id,
        start_lba,
    } = &op.kind
    {
        return clone_partition(disks, &op.id, part_id, target_disk_id, *start_lba);
    }
    let part_id = match &op.kind {
        OpKind::Delete { part_id }
        | OpKind::Resize { part_id, .. }
        | OpKind::Move { part_id, .. }
        | OpKind::Format { part_id, .. } => part_id,
        OpKind::Create { .. } | OpKind::CloneDisk { .. } | OpKind::ClonePartition { .. } => {
            unreachable!()
        }
    };
    let (d, i) = find_mut(disks, part_id)
        .ok_or("Esta tarefa depende de uma operação anterior (partição alvo inexistente).")?;
    if d.partitions[i].kind.is_protected() {
        return Err(format!("Partição {:?} é protegida.", d.partitions[i].kind).to_uppercase());
    }
    match &op.kind {
        OpKind::Delete { .. } => {
            d.partitions.remove(i);
        }
        OpKind::Resize {
            new_length_sectors, ..
        } => {
            let p = &mut d.partitions[i];
            if p.fs == Fs::Unknown {
                return Err("Sistema de arquivos desconhecido: resize bloqueado.".into());
            }
            if p.used_sectors.is_some_and(|u| *new_length_sectors < u) {
                return Err("Novo tamanho menor que o espaço usado no sistema de arquivos.".into());
            }
            p.length_sectors = *new_length_sectors;
        }
        OpKind::Move { new_start_lba, .. } => {
            let p = &mut d.partitions[i];
            if p.fs == Fs::Unknown {
                return Err("Sistema de arquivos desconhecido: mover bloqueado.".into());
            }
            if !new_start_lba.is_multiple_of(ALIGN_SECTORS) {
                warn.push("Início não alinhado a 1 MiB.".to_string());
            }
            p.start_lba = *new_start_lba;
        }
        OpKind::Format { fs, .. } => {
            let p = &mut d.partitions[i];
            p.fs = *fs;
            p.used_sectors = Some(0);
            warn.push("Formatar apagaria todos os dados (apenas simulação).".to_string());
        }
        OpKind::Create { .. } | OpKind::CloneDisk { .. } | OpKind::ClonePartition { .. } => {
            unreachable!()
        }
    }
    Ok(warn)
}

fn disk_idx(disks: &[Disk], id: &str) -> Result<usize, String> {
    disks
        .iter()
        .position(|d| d.id == id)
        .ok_or_else(|| "Disco não encontrado.".to_string())
}

/// Clone de disco (simulado): destino recebe tabela e partições da origem.
/// Exige mesmo tamanho de setor (LBAs não são remapeados); capacidade/MBR são validados por `check_layout`.
fn clone_disk(
    disks: &mut [Disk],
    op_id: &str,
    src: &str,
    dst: &str,
) -> Result<Vec<String>, String> {
    if src == dst {
        return Err("Origem e destino do clone devem ser discos diferentes.".into());
    }
    let (si, di) = (disk_idx(disks, src)?, disk_idx(disks, dst)?);
    if disks[si].sector_size != disks[di].sector_size {
        return Err("Clone entre discos com tamanhos de setor diferentes não é suportado.".into());
    }
    let mut warn = vec![format!(
        "Todos os dados de {} seriam apagados (apenas simulação).",
        disks[di].model
    )];
    if disks[si].table != disks[di].table {
        warn.push("A tabela de partições do destino seria convertida (GPT/MBR).".into());
    }
    if disks[di].total_sectors < disks[si].total_sectors {
        warn.push(
            "Destino menor que a origem: só cabe se as partições terminarem dentro do destino."
                .into(),
        );
    }
    warn.push(
        "Identificadores (GUID/assinatura) duplicados: evite manter os dois discos conectados."
            .into(),
    );
    let (table, parts) = (disks[si].table, disks[si].partitions.clone());
    disks[di].table = table;
    disks[di].partitions = parts
        .into_iter()
        .map(|p| Partition {
            id: format!("{op_id}-{}", p.id),
            r#virtual: true,
            ..p
        })
        .collect();
    Ok(warn)
}

/// Clone de partição (simulado) para um intervalo do disco destino.
fn clone_partition(
    disks: &mut [Disk],
    op_id: &str,
    part_id: &str,
    dst: &str,
    start: u64,
) -> Result<Vec<String>, String> {
    let src = disks
        .iter()
        .find_map(|d| {
            d.partitions
                .iter()
                .find(|p| p.id == part_id)
                .map(|p| (d.sector_size, p.clone()))
        })
        .ok_or("Esta tarefa depende de uma operação anterior (partição alvo inexistente).")?;
    let di = disk_idx(disks, dst)?;
    if disks[di].sector_size != src.0 {
        return Err("Clone entre discos com tamanhos de setor diferentes não é suportado.".into());
    }
    let mut warn = vec!["Identificadores duplicados (UUID/GUID) após a cópia.".to_string()];
    if !start.is_multiple_of(ALIGN_SECTORS) {
        warn.push("Início não alinhado a 1 MiB.".into());
    }
    disks[di].partitions.push(Partition {
        id: format!("new-{op_id}"),
        start_lba: start,
        r#virtual: true,
        ..src.1
    });
    Ok(warn)
}

/// Matriz de capacidades de uma partição (mesmas regras que `apply` bloqueia, mas com motivo antecipado).
pub fn capabilities(p: &Partition) -> Vec<Capability> {
    const SIM: &str = "Somente simulação: escrita real indisponível no MVP 0.1.";
    let cap = |action, blocked: Option<String>| match blocked {
        Some(reason) => Capability {
            action,
            support: Support::Unsupported,
            reason,
        },
        None => Capability {
            action,
            support: Support::SimulationOnly,
            reason: SIM.into(),
        },
    };
    let protected = p
        .kind
        .is_protected()
        .then(|| format!("Partição {:?} é protegida.", p.kind).to_uppercase());
    let unknown = (p.fs == Fs::Unknown).then(|| {
        "Sistema de arquivos desconhecido ou não suportado (ex.: criptografado, LVM, RAID)."
            .to_string()
    });
    vec![
        cap(Action::Resize, protected.clone().or(unknown.clone())),
        cap(Action::Move, protected.clone().or(unknown)),
        cap(Action::Format, protected.clone()),
        cap(Action::Delete, protected),
        cap(Action::Clone, None),
    ]
}

/// Simulação determinística; operações inválidas são bloqueadas e puladas.
pub fn simulate(base: &[Disk], ops: &[Op]) -> Plan {
    let mut disks = base.to_vec();
    let (mut issues, mut blocked_ops) = (Vec::new(), Vec::new());
    for op in ops {
        let mut next = disks.clone();
        let res = apply(&mut next, op);
        let error = match &res {
            Err(e) => Some(e.clone()),
            Ok(_) => next.iter().flat_map(check_layout).next(),
        };
        if let Ok(ws) = res {
            issues.extend(ws.into_iter().map(|message| Issue {
                op_id: op.id.clone(),
                severity: Severity::Warning,
                message,
            }));
        }
        match error {
            Some(message) => {
                issues.push(Issue {
                    op_id: op.id.clone(),
                    severity: Severity::Error,
                    message,
                });
                blocked_ops.push(op.id.clone());
            }
            None => disks = next,
        }
    }
    let capabilities = disks
        .iter()
        .flat_map(|d| &d.partitions)
        .map(|p| (p.id.clone(), capabilities(p)))
        .collect();
    Plan {
        disks,
        issues,
        blocked_ops,
        capabilities,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage_mock::MockProvider;

    const GIB: u64 = 1 << 21; // setores de 512
    fn base() -> Vec<Disk> {
        MockProvider.discover()
    }
    fn op(id: &str, kind: OpKind) -> Op {
        Op {
            id: id.into(),
            kind,
        }
    }
    fn del(id: &str, part: &str) -> Op {
        op(
            id,
            OpKind::Delete {
                part_id: part.into(),
            },
        )
    }

    #[test]
    fn nao_muta_base_e_e_deterministica() {
        let b = base();
        let before = b.clone();
        let ops = [del("1", "d1p2")];
        assert_eq!(simulate(&b, &ops), simulate(&b, &ops));
        assert_eq!(b, before);
    }

    #[test]
    fn delete_cria_espaco_nao_alocado() {
        let p = simulate(&base(), &[del("1", "d1p2")]);
        assert!(p.blocked_ops.is_empty());
        assert!(gaps(&p.disks[1])
            .iter()
            .any(|g| g.length_sectors == 100 * GIB));
    }

    #[test]
    fn bloqueia_protegidas() {
        assert_eq!(simulate(&base(), &[del("1", "d0p1")]).blocked_ops, ["1"]);
    }

    #[test]
    fn bloqueia_resize_abaixo_do_uso_e_fs_desconhecido() {
        let p = simulate(
            &base(),
            &[
                op(
                    "1",
                    OpKind::Resize {
                        part_id: "d0p3".into(),
                        new_length_sectors: 100 * GIB,
                    },
                ),
                op(
                    "2",
                    OpKind::Resize {
                        part_id: "d1p3".into(),
                        new_length_sectors: 10 * GIB,
                    },
                ),
            ],
        );
        assert_eq!(p.blocked_ops, ["1", "2"]);
    }

    #[test]
    fn bloqueia_sobreposicao_excesso_e_overflow() {
        let p = simulate(
            &base(),
            &[
                op(
                    "1",
                    OpKind::Resize {
                        part_id: "d1p1".into(),
                        new_length_sectors: 250 * GIB,
                    },
                ),
                op(
                    "2",
                    OpKind::Create {
                        disk_id: "disk2".into(),
                        start_lba: 256,
                        length_sectors: 1 << 40,
                        fs: Fs::Ext4,
                        name: "x".into(),
                    },
                ),
                op(
                    "3",
                    OpKind::Create {
                        disk_id: "disk2".into(),
                        start_lba: u64::MAX,
                        length_sectors: 2,
                        fs: Fs::Ext4,
                        name: "y".into(),
                    },
                ),
            ],
        );
        assert_eq!(p.blocked_ops, ["1", "2", "3"]);
    }

    #[test]
    fn dependencia_usa_id_virtual() {
        let p = simulate(
            &base(),
            &[
                op(
                    "a",
                    OpKind::Create {
                        disk_id: "disk1".into(),
                        start_lba: 2048 + 700 * GIB,
                        length_sectors: 10 * GIB,
                        fs: Fs::Ext4,
                        name: "n".into(),
                    },
                ),
                op(
                    "b",
                    OpKind::Resize {
                        part_id: "new-a".into(),
                        new_length_sectors: 20 * GIB,
                    },
                ),
            ],
        );
        assert!(p.blocked_ops.is_empty());
        let np = p.disks[1]
            .partitions
            .iter()
            .find(|x| x.id == "new-a")
            .unwrap();
        assert_eq!(np.length_sectors, 20 * GIB);
    }

    fn clone_disk(id: &str, s: &str, t: &str) -> Op {
        op(
            id,
            OpKind::CloneDisk {
                source_disk_id: s.into(),
                target_disk_id: t.into(),
            },
        )
    }

    #[test]
    fn clone_de_disco_copia_layout_sem_mutar_origem() {
        let b = base();
        let p = simulate(&b, &[clone_disk("k", "disk0", "disk1")]);
        assert!(p.blocked_ops.is_empty(), "{:?}", p.issues);
        let n = |d: &Disk| {
            d.partitions
                .iter()
                .map(|x| (x.start_lba, x.length_sectors, x.fs))
                .collect::<Vec<_>>()
        };
        assert_eq!(n(&p.disks[1]), n(&b[0]));
        assert!(p.disks[1]
            .partitions
            .iter()
            .all(|x| x.r#virtual && x.id.starts_with("k-")));
        assert_eq!(p.disks[0], b[0]);
        assert!(p
            .issues
            .iter()
            .any(|i| i.severity == Severity::Warning && i.message.contains("apagados")));
    }

    #[test]
    fn clone_de_disco_bloqueios() {
        let p = simulate(
            &base(),
            &[
                clone_disk("1", "disk0", "disk0"),  // mesmo disco
                clone_disk("2", "disk0", "disk2"),  // setor 512 -> 4096
                clone_disk("3", "disk1", "disk0"),  // origem ocupa 700 GiB > destino 512 GiB
                clone_disk("4", "disk0", "nenhum"), // destino inexistente
            ],
        );
        assert_eq!(p.blocked_ops, ["1", "2", "3", "4"]);
    }

    #[test]
    fn clone_de_particao() {
        let b = base();
        let clone = |id: &str, part: &str, start| {
            op(
                id,
                OpKind::ClonePartition {
                    part_id: part.into(),
                    target_disk_id: "disk1".into(),
                    start_lba: start,
                },
            )
        };
        let p = simulate(
            &b,
            &[
                clone("a", "d0p3", 2048 + 720 * GIB), // 300 GiB não cabe (disco tem 1024 GiB: 720+300 = ok)
                clone("b", "d0p3", 2048 + 100 * GIB), // sobrepõe "/"
            ],
        );
        assert_eq!(p.blocked_ops, ["b"]);
        assert!(p.disks[1]
            .partitions
            .iter()
            .any(|x| x.id == "new-a" && x.r#virtual));
    }

    #[test]
    fn matriz_de_capacidades_tem_motivo() {
        let p = simulate(&base(), &[]);
        let get = |id: &str, a: Action| {
            p.capabilities[id]
                .iter()
                .find(|c| c.action == a)
                .unwrap()
                .clone()
        };
        assert_eq!(get("d0p1", Action::Delete).support, Support::Unsupported); // EFI
        assert!(get("d0p1", Action::Delete).reason.contains("EFI"));
        assert_eq!(get("d0p1", Action::Clone).support, Support::SimulationOnly);
        assert_eq!(get("d1p3", Action::Resize).support, Support::Unsupported); // FS desconhecido
        assert_eq!(get("d1p3", Action::Delete).support, Support::SimulationOnly);
        assert_eq!(get("d0p3", Action::Resize).support, Support::SimulationOnly);
        // coerência: o que a matriz marca como não suportado, o planejador bloqueia
        let blocked = simulate(
            &base(),
            &[op(
                "1",
                OpKind::Resize {
                    part_id: "d1p3".into(),
                    new_length_sectors: 10 * GIB,
                },
            )],
        );
        assert_eq!(blocked.blocked_ops, ["1"]);
    }

    #[test]
    fn mbr_limita_a_quatro() {
        let ops: Vec<Op> = [3u64, 4, 5]
            .iter()
            .map(|i| {
                op(
                    &format!("c{i}"),
                    OpKind::Create {
                        disk_id: "disk2".into(),
                        start_lba: 256 + i * (1 << 20),
                        length_sectors: 1000,
                        fs: Fs::Fat32,
                        name: format!("p{i}"),
                    },
                )
            })
            .collect();
        assert_eq!(simulate(&base(), &ops).blocked_ops, ["c5"]);
    }
}

/// MVP 0.1: nenhum código de produção pode conter chamadas de escrita/elevação em dispositivos,
/// e processos só podem ser criados em `storage-common` (que aplica uma allowlist de programas de leitura).
#[cfg(test)]
mod no_write_paths {
    const CRATES: [&str; 8] = [
        "disk-domain",
        "disk-planner",
        "storage-mock",
        "storage-persistence",
        "storage-common",
        "storage-windows",
        "storage-linux",
        "../src-tauri",
    ];
    const MUTANTES: [&str; 14] = [
        "diskpart",
        "DeviceIoControl",
        "mkfs",
        "parted",
        "CreateFileW",
        "/dev/",
        "wipefs",
        "sfdisk",
        "sgdisk",
        "Set-Partition",
        "Resize-Partition",
        "New-Partition",
        "Format-Volume",
        "Clear-Disk",
    ];

    fn producao(c: &str) -> String {
        let f = format!("{}/../{c}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let src = std::fs::read_to_string(&f).unwrap_or_else(|e| panic!("{f}: {e}"));
        src.split("#[cfg(test)]").next().unwrap().to_string()
    }

    #[test]
    fn producao_sem_apis_de_escrita() {
        for c in CRATES {
            let src = producao(c);
            for p in MUTANTES {
                assert!(!src.contains(p), "{c} contém `{p}`");
            }
        }
    }

    #[test]
    fn processos_so_em_storage_common() {
        for c in CRATES {
            let n = producao(c).matches("Command::new").count();
            assert_eq!(
                n,
                usize::from(c == "storage-common"),
                "{c}: Command::new x{n}"
            );
        }
    }
}
