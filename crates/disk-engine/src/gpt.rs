//! GPT (ADR 0003): inicializar, adicionar e remover entradas, com cabeçalho primário + backup, CRC32,
//! MBR protetor, pré-imagem das duas regiões, verificação por releitura e rollback.
//! Somente sobre `BlockDevice` (hoje só `MemDevice` nos testes); ainda NÃO está ligado ao app nem a nenhuma ferramenta.
use crate::{plan_hash_str, Approval, BlockDevice, Error, Identity, SECTOR};

const S: usize = SECTOR as usize;
const N_ENTRIES: usize = 128;
const ENTRY: usize = 128;
const ARRAY_SECTORS: u64 = (N_ENTRIES * ENTRY / S) as u64; // 32
const FIRST_USABLE: u64 = 2 + ARRAY_SECTORS; // 34
const ALIGN: u64 = 2048;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GptEntry {
    pub type_guid: [u8; 16],
    pub unique: [u8; 16],
    pub first: u64,
    pub last: u64,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GOp {
    /// Substitui o MBR por GPT vazio (DESTRÓI a tabela atual).
    Init {
        disk_guid: [u8; 16],
    },
    Add(GptEntry),
    Remove {
        index: usize,
    },
}

pub fn gpt_plan_hash(ops: &[GOp]) -> u64 {
    plan_hash_str(&format!("gpt {ops:?}"))
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 == 1 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

type Table = (/* disk_guid */ [u8; 16], Vec<Option<GptEntry>>);

fn serialize_array(entries: &[Option<GptEntry>]) -> Vec<u8> {
    let mut a = vec![0u8; N_ENTRIES * ENTRY];
    for (i, e) in entries.iter().enumerate() {
        if let Some(e) = e {
            let o = &mut a[i * ENTRY..(i + 1) * ENTRY];
            o[..16].copy_from_slice(&e.type_guid);
            o[16..32].copy_from_slice(&e.unique);
            o[32..40].copy_from_slice(&e.first.to_le_bytes());
            o[40..48].copy_from_slice(&e.last.to_le_bytes());
            for (j, u) in e.name.encode_utf16().take(36).enumerate() {
                o[56 + 2 * j..58 + 2 * j].copy_from_slice(&u.to_le_bytes());
            }
        }
    }
    a
}

fn header(guid: &[u8; 16], total: u64, backup: bool, array_crc: u32) -> Vec<u8> {
    let (me, alt, arr) = if backup {
        (total - 1, 1, total - 1 - ARRAY_SECTORS)
    } else {
        (1, total - 1, 2)
    };
    let mut h = vec![0u8; S];
    h[..8].copy_from_slice(b"EFI PART");
    h[8..12].copy_from_slice(&0x0001_0000u32.to_le_bytes());
    h[12..16].copy_from_slice(&92u32.to_le_bytes());
    h[24..32].copy_from_slice(&me.to_le_bytes());
    h[32..40].copy_from_slice(&alt.to_le_bytes());
    h[40..48].copy_from_slice(&FIRST_USABLE.to_le_bytes());
    h[48..56].copy_from_slice(&(total - 1 - ARRAY_SECTORS - 1).to_le_bytes());
    h[56..72].copy_from_slice(guid);
    h[72..80].copy_from_slice(&arr.to_le_bytes());
    h[80..84].copy_from_slice(&(N_ENTRIES as u32).to_le_bytes());
    h[84..88].copy_from_slice(&(ENTRY as u32).to_le_bytes());
    h[88..92].copy_from_slice(&array_crc.to_le_bytes());
    let crc = crc32(&h[..92]);
    h[16..20].copy_from_slice(&crc.to_le_bytes());
    h
}

/// Região primária (LBA 0..34): MBR protetor, cabeçalho, entradas.
fn primary(t: &Table, total: u64) -> Vec<u8> {
    let array = serialize_array(&t.1);
    let mut r = vec![0u8; S];
    r[446 + 4] = 0xEE;
    r[446 + 8..446 + 12].copy_from_slice(&1u32.to_le_bytes());
    r[446 + 12..446 + 16]
        .copy_from_slice(&u32::try_from(total - 1).unwrap_or(u32::MAX).to_le_bytes());
    r[510] = 0x55;
    r[511] = 0xAA;
    r.extend(header(&t.0, total, false, crc32(&array)));
    r.extend(array);
    r
}

/// Região de backup (entradas + cabeçalho no último setor).
fn backup(t: &Table, total: u64) -> Vec<u8> {
    let mut r = serialize_array(&t.1);
    let crc = crc32(&r);
    r.extend(header(&t.0, total, true, crc));
    r
}

fn read(dev: &dyn BlockDevice, lba: u64, sectors: u64) -> Result<Vec<u8>, Error> {
    let mut b = vec![0u8; sectors as usize * S];
    dev.read_at(lba * SECTOR, &mut b)
        .map_err(|e| Error::Io(e.to_string()))?;
    Ok(b)
}

fn parse_array(a: &[u8]) -> Vec<Option<GptEntry>> {
    (0..N_ENTRIES)
        .map(|i| {
            let o = &a[i * ENTRY..(i + 1) * ENTRY];
            (o[..16] != [0; 16]).then(|| GptEntry {
                type_guid: o[..16].try_into().unwrap(),
                unique: o[16..32].try_into().unwrap(),
                first: u64::from_le_bytes(o[32..40].try_into().unwrap()),
                last: u64::from_le_bytes(o[40..48].try_into().unwrap()),
                name: String::from_utf16_lossy(
                    &o[56..128]
                        .chunks(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .take_while(|&u| u != 0)
                        .collect::<Vec<_>>(),
                ),
            })
        })
        .collect()
}

/// Lê e valida (assinatura, CRC do cabeçalho e das entradas) o GPT primário.
pub fn read_gpt(dev: &dyn BlockDevice) -> Result<Table, Error> {
    let h = read(dev, 1, 1)?;
    let bad = |m: &str| Err(Error::Invalid(format!("GPT inválido: {m}")));
    if &h[..8] != b"EFI PART" {
        return bad("sem assinatura");
    }
    let mut hc = h[..92].to_vec();
    hc[16..20].fill(0);
    if crc32(&hc) != u32::from_le_bytes(h[16..20].try_into().unwrap()) {
        return bad("CRC do cabeçalho");
    }
    let array = read(dev, 2, ARRAY_SECTORS)?;
    if crc32(&array) != u32::from_le_bytes(h[88..92].try_into().unwrap()) {
        return bad("CRC das entradas");
    }
    Ok((h[56..72].try_into().unwrap(), parse_array(&array)))
}

fn apply_op(t: &mut Option<Table>, op: &GOp, total: u64) -> Result<(), Error> {
    let bad = |m: &str| Err(Error::Invalid(m.into()));
    match op {
        GOp::Init { disk_guid } => {
            if t.is_some() {
                return bad("o disco já tem GPT válido");
            }
            *t = Some((*disk_guid, vec![None; N_ENTRIES]));
        }
        GOp::Add(e) => {
            let Some((_, entries)) = t else {
                return bad("sem GPT: faça Init antes");
            };
            if e.type_guid == [0; 16]
                || e.first % ALIGN != 0
                || e.first < FIRST_USABLE
                || e.last < e.first
                || e.last > total - 2 - ARRAY_SECTORS
            {
                return bad("entrada fora dos limites, desalinhada ou com tipo vazio");
            }
            if entries
                .iter()
                .flatten()
                .any(|x| e.first <= x.last && x.first <= e.last)
            {
                return bad("sobrepõe outra partição");
            }
            *entries
                .iter_mut()
                .find(|s| s.is_none())
                .ok_or(Error::Invalid("tabela cheia".into()))? = Some(e.clone());
        }
        GOp::Remove { index } => match t.as_mut().and_then(|(_, e)| e.get_mut(*index)) {
            Some(s @ Some(_)) => *s = None,
            _ => return bad("entrada inexistente"),
        },
    }
    Ok(())
}

pub fn execute_gpt(
    dev: &mut dyn BlockDevice,
    target: &Identity,
    approval: &Approval,
    ops: &[GOp],
) -> Result<(), Error> {
    if dev.identity() != target || target.serial.is_empty() {
        return Err(Error::TargetMismatch);
    }
    if approval.serial != target.serial
        || approval.size_bytes != target.size_bytes
        || approval.plan_hash != gpt_plan_hash(ops)
    {
        return Err(Error::NotApproved);
    }
    let total = target.size_bytes / SECTOR;
    if total < 2 * FIRST_USABLE + 2048 {
        return Err(Error::Invalid("disco pequeno demais".into()));
    }
    let mut t = read_gpt(dev).ok();
    for op in ops {
        apply_op(&mut t, op, total)?;
    }
    let t = t.ok_or(Error::Invalid("plano vazio".into()))?;
    let (new_p, new_b) = (primary(&t, total), backup(&t, total));
    let (p_lba, b_lba) = (0, total - 1 - ARRAY_SECTORS);
    let (old_p, old_b) = (
        read(dev, p_lba, FIRST_USABLE)?,
        read(dev, b_lba, ARRAY_SECTORS + 1)?,
    ); // journal
    let abort = |dev: &mut dyn BlockDevice, reason: String| {
        let ok = dev.write_at(p_lba * SECTOR, &old_p).is_ok()
            && dev.write_at(b_lba * SECTOR, &old_b).is_ok()
            && read(dev, p_lba, FIRST_USABLE).is_ok_and(|x| x == old_p)
            && read(dev, b_lba, ARRAY_SECTORS + 1).is_ok_and(|x| x == old_b);
        Err(Error::Aborted {
            reason,
            rolled_back: ok,
        })
    };
    // backup primeiro: se o primário falhar, o rollback restaura os dois.
    if let Err(e) = dev
        .write_at(b_lba * SECTOR, &new_b)
        .and_then(|_| dev.write_at(p_lba * SECTOR, &new_p))
    {
        return abort(dev, e.to_string());
    }
    let same = read(dev, p_lba, FIRST_USABLE).is_ok_and(|x| x == new_p)
        && read(dev, b_lba, ARRAY_SECTORS + 1).is_ok_and(|x| x == new_b);
    if !same || read_gpt(dev).is_err() {
        return abort(dev, "releitura diverge do escrito".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemDevice;

    const GIB: u64 = 1 << 30;
    const EFI: [u8; 16] = [
        0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e, 0xc9,
        0x3b,
    ];
    fn setup() -> (MemDevice, Identity) {
        let d = MemDevice::new("SN-GPT", GIB);
        let id = d.identity().clone();
        (d, id)
    }
    fn run(d: &mut MemDevice, id: &Identity, ops: &[GOp]) -> Result<(), Error> {
        let ap = Approval {
            serial: id.serial.clone(),
            size_bytes: id.size_bytes,
            plan_hash: gpt_plan_hash(ops),
        };
        execute_gpt(d, id, &ap, ops)
    }
    fn add(first: u64, last: u64) -> GOp {
        GOp::Add(GptEntry {
            type_guid: EFI,
            unique: [7; 16],
            first,
            last,
            name: "dados".into(),
        })
    }

    #[test]
    fn crc32_conhecido() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn inicializa_adiciona_e_remove() {
        let (mut d, id) = setup();
        run(
            &mut d,
            &id,
            &[
                GOp::Init { disk_guid: [1; 16] },
                add(2048, 4095),
                add(4096, 8191),
            ],
        )
        .unwrap();
        let (guid, e) = read_gpt(&d).unwrap();
        assert_eq!(guid, [1; 16]);
        assert_eq!(e[0].as_ref().unwrap().name, "dados");
        assert_eq!(e[1].as_ref().unwrap().first, 4096);
        // MBR protetor
        let mut s0 = [0u8; 512];
        d.read_at(0, &mut s0).unwrap();
        assert_eq!((s0[450], s0[510], s0[511]), (0xEE, 0x55, 0xAA));
        run(&mut d, &id, &[GOp::Remove { index: 0 }]).unwrap();
        assert!(read_gpt(&d).unwrap().1[0].is_none());
    }

    #[test]
    fn backup_no_fim_do_disco_e_valido() {
        let (mut d, id) = setup();
        run(
            &mut d,
            &id,
            &[GOp::Init { disk_guid: [2; 16] }, add(2048, 4095)],
        )
        .unwrap();
        let total = id.size_bytes / SECTOR;
        let mut h = vec![0u8; 512];
        d.read_at((total - 1) * SECTOR, &mut h).unwrap();
        assert_eq!(&h[..8], b"EFI PART");
        assert_eq!(u64::from_le_bytes(h[24..32].try_into().unwrap()), total - 1);
        let mut hc = h[..92].to_vec();
        hc[16..20].fill(0);
        assert_eq!(
            crc32(&hc),
            u32::from_le_bytes(h[16..20].try_into().unwrap())
        );
    }

    #[test]
    fn validacoes_nao_escrevem_nada() {
        let (mut d, id) = setup();
        let init = GOp::Init { disk_guid: [1; 16] };
        for ops in [
            vec![add(2048, 4095)],                                // sem Init
            vec![init.clone(), add(2049, 4095)],                  // desalinhado
            vec![init.clone(), add(2048, 4095), add(4000, 9000)], // sobreposição
            vec![init.clone(), add(2048, u64::MAX)],              // além do disco
            vec![init.clone(), GOp::Remove { index: 3 }],         // inexistente
        ] {
            assert!(matches!(run(&mut d, &id, &ops), Err(Error::Invalid(_))));
        }
        assert!(read_gpt(&d).is_err());
        run(&mut d, &id, std::slice::from_ref(&init)).unwrap();
        assert!(matches!(run(&mut d, &id, &[init]), Err(Error::Invalid(_)))); // já existe
    }

    #[test]
    fn recusa_alvo_e_aprovacao_erradas() {
        let (mut d, id) = setup();
        let ops = [GOp::Init { disk_guid: [1; 16] }];
        let outro = Identity {
            serial: "X".into(),
            size_bytes: id.size_bytes,
        };
        let ap = Approval {
            serial: "X".into(),
            size_bytes: id.size_bytes,
            plan_hash: gpt_plan_hash(&ops),
        };
        assert_eq!(
            execute_gpt(&mut d, &outro, &ap, &ops),
            Err(Error::TargetMismatch)
        );
        let ap = Approval {
            serial: id.serial.clone(),
            size_bytes: id.size_bytes,
            plan_hash: 1,
        };
        assert_eq!(execute_gpt(&mut d, &id, &ap, &ops), Err(Error::NotApproved));
    }

    #[test]
    fn falha_no_meio_restaura_as_duas_regioes() {
        let (mut d, id) = setup();
        run(
            &mut d,
            &id,
            &[GOp::Init { disk_guid: [1; 16] }, add(2048, 4095)],
        )
        .unwrap();
        d.fail_write = Some(1); // o backup grava, o primário falha
        let r = run(&mut d, &id, &[add(4096, 8191)]);
        assert!(matches!(
            r,
            Err(Error::Aborted {
                rolled_back: true,
                ..
            })
        ));
        let (_, e) = read_gpt(&d).unwrap();
        assert!(e[1].is_none() && e[0].is_some());
    }
}
