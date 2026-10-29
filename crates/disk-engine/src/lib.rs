//! Fase 0.2 (ADR 0003). Motor de escrita de tabela MBR sobre um `BlockDevice` abstrato.
//! Só existe `MemDevice`; não há backend de dispositivo real e este crate não é usado pelo app.
//! Toda execução exige: alvo idêntico (serial + tamanho), aprovação ligada ao hash do plano,
//! journal com pré-imagem, verificação por releitura e rollback automático.
use std::io;

pub const SECTOR: u64 = 512;
const ALIGN: u64 = 2048;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub serial: String,
    pub size_bytes: u64,
}

pub trait BlockDevice {
    fn identity(&self) -> &Identity;
    fn read_at(&self, off: u64, buf: &mut [u8]) -> io::Result<()>;
    fn write_at(&mut self, off: u64, data: &[u8]) -> io::Result<()>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    TargetMismatch,
    NotApproved,
    Invalid(String),
    Io(String),
    /// A escrita falhou ou a releitura divergiu; `rolled_back` diz se a pré-imagem foi restaurada e verificada.
    Aborted {
        reason: String,
        rolled_back: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub boot: u8,
    pub kind: u8,
    pub start: u32,
    pub sectors: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    Create {
        slot: usize,
        start: u32,
        sectors: u32,
        kind: u8,
    },
    Delete {
        slot: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Approval {
    pub serial: String,
    pub size_bytes: u64,
    pub plan_hash: u64,
}

/// FNV-1a do plano; a aprovação só vale para exatamente este plano.
pub fn plan_hash(ops: &[Op]) -> u64 {
    format!("{ops:?}").bytes().fold(0xcbf29ce484222325, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    })
}

fn sector0(dev: &dyn BlockDevice) -> Result<[u8; 512], Error> {
    let mut s = [0u8; 512];
    dev.read_at(0, &mut s)
        .map_err(|e| Error::Io(e.to_string()))?;
    Ok(s)
}

fn parse(s: &[u8; 512]) -> [Option<Entry>; 4] {
    std::array::from_fn(|i| {
        let e = &s[446 + 16 * i..462 + 16 * i];
        let start = u32::from_le_bytes(e[8..12].try_into().unwrap());
        let sectors = u32::from_le_bytes(e[12..16].try_into().unwrap());
        (e[4] != 0).then_some(Entry {
            boot: e[0],
            kind: e[4],
            start,
            sectors,
        })
    })
}

pub fn read_table(dev: &dyn BlockDevice) -> Result<[Option<Entry>; 4], Error> {
    let s = sector0(dev)?;
    if s[510..] != [0x55, 0xAA] {
        return Err(Error::Invalid("MBR sem assinatura 55AA".into()));
    }
    Ok(parse(&s))
}

fn validate(table: &[Option<Entry>; 4], op: &Op, disk_sectors: u64) -> Result<(), Error> {
    let bad = |m: &str| Err(Error::Invalid(m.into()));
    match *op {
        Op::Delete { slot } if slot >= 4 || table[slot].is_none() => bad("slot inexistente"),
        Op::Create {
            slot,
            start,
            sectors,
            kind,
        } => {
            if slot >= 4 || table[slot].is_some() {
                return bad("slot inválido ou ocupado");
            }
            if kind == 0
                || sectors == 0
                || !(start as u64).is_multiple_of(ALIGN)
                || (start as u64) < ALIGN
            {
                return bad(
                    "tipo/tamanho/alinhamento inválido (início ≥ 1 MiB, múltiplo de 1 MiB)",
                );
            }
            let end = start as u64 + sectors as u64;
            if end > disk_sectors {
                return bad("partição ultrapassa o disco");
            }
            if table.iter().flatten().any(|e| {
                (start as u64) < e.start as u64 + e.sectors as u64 && (e.start as u64) < end
            }) {
                return bad("sobrepõe outra partição");
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn apply_to(s: &mut [u8; 512], op: &Op) {
    let (slot, e) = match *op {
        Op::Create {
            slot,
            start,
            sectors,
            kind,
        } => {
            let mut e = [0u8; 16];
            e[4] = kind;
            e[8..12].copy_from_slice(&start.to_le_bytes());
            e[12..16].copy_from_slice(&sectors.to_le_bytes());
            (slot, e)
        }
        Op::Delete { slot } => (slot, [0u8; 16]),
    };
    s[446 + 16 * slot..462 + 16 * slot].copy_from_slice(&e);
}

/// Valida `ops` contra o MBR atual sem escrever; devolve (setor 0 atual, setor 0 resultante).
pub fn check(dev: &dyn BlockDevice, ops: &[Op]) -> Result<([u8; 512], [u8; 512]), Error> {
    let before = sector0(dev)?;
    let mut after = before;
    let mut table = read_table(dev)?;
    for op in ops {
        validate(&table, op, dev.identity().size_bytes / SECTOR)?;
        apply_to(&mut after, op);
        table = parse(&after);
    }
    Ok((before, after))
}

/// Aplica `ops` ao MBR de `dev`. Nada é escrito se qualquer validação falhar.
pub fn execute(
    dev: &mut dyn BlockDevice,
    target: &Identity,
    approval: &Approval,
    ops: &[Op],
) -> Result<(), Error> {
    if dev.identity() != target || target.serial.is_empty() {
        return Err(Error::TargetMismatch);
    }
    if approval.serial != target.serial
        || approval.size_bytes != target.size_bytes
        || approval.plan_hash != plan_hash(ops)
    {
        return Err(Error::NotApproved);
    }
    let (before, after) = check(dev, ops)?; // before = journal: pré-imagem
    let abort = |dev: &mut dyn BlockDevice, reason: String| {
        let rolled_back =
            dev.write_at(0, &before).is_ok() && sector0(dev).map(|s| s == before).unwrap_or(false);
        Err(Error::Aborted {
            reason,
            rolled_back,
        })
    };
    if let Err(e) = dev.write_at(0, &after) {
        return abort(dev, e.to_string());
    }
    match sector0(dev) {
        Ok(s) if s == after => Ok(()),
        _ => abort(dev, "releitura diverge do escrito".into()),
    }
}

pub struct MemDevice {
    id: Identity,
    data: Vec<u8>,
    /// Falha injetada na N-ésima escrita (testes de rollback).
    pub fail_write: Option<u32>,
    /// Corrompe tudo que for gravado (testes de verificação).
    pub corrupt: bool,
}

impl MemDevice {
    pub fn new(serial: &str, size_bytes: u64) -> Self {
        let mut data = vec![0u8; size_bytes as usize];
        data[510] = 0x55;
        data[511] = 0xAA;
        Self {
            id: Identity {
                serial: serial.into(),
                size_bytes,
            },
            data,
            fail_write: None,
            corrupt: false,
        }
    }
}

impl BlockDevice for MemDevice {
    fn identity(&self) -> &Identity {
        &self.id
    }
    fn read_at(&self, off: u64, buf: &mut [u8]) -> io::Result<()> {
        let o = off as usize;
        buf.copy_from_slice(
            self.data
                .get(o..o + buf.len())
                .ok_or_else(|| io::Error::other("fora do disco"))?,
        );
        Ok(())
    }
    fn write_at(&mut self, off: u64, d: &[u8]) -> io::Result<()> {
        if let Some(n) = self.fail_write {
            if n == 0 {
                self.fail_write = None;
                return Err(io::Error::other("falha injetada"));
            }
            self.fail_write = Some(n - 1);
        }
        let o = off as usize;
        self.data
            .get_mut(o..o + d.len())
            .ok_or_else(|| io::Error::other("fora do disco"))?
            .copy_from_slice(d);
        if self.corrupt {
            self.data[o + 446] ^= 0xFF;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1 << 30;
    fn setup() -> (MemDevice, Identity) {
        let d = MemDevice::new("SN-TESTE", GIB);
        let id = d.identity().clone();
        (d, id)
    }
    fn approve(id: &Identity, ops: &[Op]) -> Approval {
        Approval {
            serial: id.serial.clone(),
            size_bytes: id.size_bytes,
            plan_hash: plan_hash(ops),
        }
    }
    fn create(slot: usize, start: u32, sectors: u32) -> Op {
        Op::Create {
            slot,
            start,
            sectors,
            kind: 0x07,
        }
    }
    fn empty(d: &MemDevice) -> bool {
        read_table(d).unwrap().iter().all(Option::is_none)
    }

    #[test]
    fn cria_e_exclui() {
        let (mut d, id) = setup();
        let ops = [create(0, 2048, 100_000), create(1, 104_448, 50_000)];
        execute(&mut d, &id, &approve(&id, &ops), &ops).unwrap();
        let t = read_table(&d).unwrap();
        assert_eq!(t[0].unwrap().sectors, 100_000);
        assert!(t[2].is_none());
        let del = [Op::Delete { slot: 0 }];
        execute(&mut d, &id, &approve(&id, &del), &del).unwrap();
        assert!(read_table(&d).unwrap()[0].is_none());
    }

    #[test]
    fn recusa_alvo_diferente_e_aprovacao_de_outro_plano() {
        let (mut d, id) = setup();
        let ops = [create(0, 2048, 1000)];
        let outro = Identity {
            serial: "OUTRO".into(),
            size_bytes: id.size_bytes,
        };
        assert_eq!(
            execute(&mut d, &outro, &approve(&outro, &ops), &ops),
            Err(Error::TargetMismatch)
        );
        let ap = approve(&id, &[create(0, 2048, 2000)]);
        assert_eq!(execute(&mut d, &id, &ap, &ops), Err(Error::NotApproved));
        assert!(empty(&d));
    }

    #[test]
    fn validacao_nao_escreve_nada() {
        let (mut d, id) = setup();
        for ops in [
            vec![create(0, 2048, 1000), create(1, 2048, 1000)], // sobreposição
            vec![create(0, 2049, 1000)],                        // desalinhado
            vec![create(0, 2048, u32::MAX)],                    // além do disco
            vec![Op::Delete { slot: 3 }],                       // slot vazio
        ] {
            assert!(matches!(
                execute(&mut d, &id, &approve(&id, &ops), &ops),
                Err(Error::Invalid(_))
            ));
        }
        assert!(empty(&d));
    }

    #[test]
    fn falha_de_escrita_faz_rollback() {
        let (mut d, id) = setup();
        d.fail_write = Some(0);
        let ops = [create(0, 2048, 1000)];
        // a 1ª escrita falha; o rollback (2ª escrita) funciona e a tabela segue vazia
        assert!(matches!(
            execute(&mut d, &id, &approve(&id, &ops), &ops),
            Err(Error::Aborted {
                rolled_back: true,
                ..
            })
        ));
        assert!(empty(&d));
    }

    #[test]
    fn releitura_divergente_e_detectada() {
        let (mut d, id) = setup();
        d.corrupt = true;
        let ops = [create(0, 2048, 1000)];
        // a corrupção também atinge o rollback, que então é reportado como não verificado
        assert!(matches!(
            execute(&mut d, &id, &approve(&id, &ops), &ops),
            Err(Error::Aborted {
                rolled_back: false,
                ..
            })
        ));
    }
}

#[cfg(windows)]
pub mod win;
