//! Snapshots e planos exportáveis, com schema versionado e fingerprint de layout.
//! Sanitização: o domínio não carrega serial/WWN, então nada sensível é serializado.
//! Preferências ficam no webview (localStorage); SQLite fica para quando houver histórico/journal.

use disk_domain::{Disk, Op};
use serde::{Deserialize, Serialize};
use std::{fmt, fs, io, path::Path};

pub const SCHEMA: u32 = 1;
/// Limite de payload aceito na importação (1 MiB).
pub const MAX_IMPORT_BYTES: usize = 1 << 20;

#[derive(Debug)]
pub enum Error {
    TooLarge,
    Invalid(String),
    UnsupportedSchema(u32),
    Corrupted,
    BadName,
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::TooLarge => write!(f, "Arquivo grande demais."),
            Error::Invalid(e) => write!(f, "Arquivo inválido: {e}"),
            Error::UnsupportedSchema(v) => write!(f, "Versão de schema {v} não suportada."),
            Error::Corrupted => write!(
                f,
                "Fingerprint não confere: arquivo corrompido ou alterado."
            ),
            Error::BadName => write!(f, "Nome de arquivo não permitido."),
            Error::Io(e) => write!(f, "Erro de E/S: {e}"),
        }
    }
}

/// FNV-1a 64 bits sobre o JSON canônico dos discos (detecta mudança de layout; não é criptográfico).
pub fn fingerprint(disks: &[Disk]) -> String {
    let bytes = serde_json::to_vec(disks).unwrap_or_default();
    let h = bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{h:016x}")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub schema: u32,
    pub source: String,
    pub fingerprint: String,
    pub disks: Vec<Disk>,
}

impl Snapshot {
    pub fn new(source: &str, disks: &[Disk]) -> Self {
        Snapshot {
            schema: SCHEMA,
            source: source.into(),
            fingerprint: fingerprint(disks),
            disks: disks.to_vec(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_json(s: &str) -> Result<Self, Error> {
        if s.len() > MAX_IMPORT_BYTES {
            return Err(Error::TooLarge);
        }
        let snap: Snapshot = serde_json::from_str(s).map_err(|e| Error::Invalid(e.to_string()))?;
        if snap.schema != SCHEMA {
            return Err(Error::UnsupportedSchema(snap.schema));
        }
        if snap.fingerprint != fingerprint(&snap.disks) {
            return Err(Error::Corrupted);
        }
        Ok(snap)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanFile {
    pub schema: u32,
    pub base_fingerprint: String,
    pub ops: Vec<Op>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedPlan {
    pub ops: Vec<Op>,
    /// `true` se o layout atual difere do layout em que o plano foi criado.
    pub stale: bool,
}

pub fn plan_to_json(base: &[Disk], ops: &[Op]) -> String {
    let file = PlanFile {
        schema: SCHEMA,
        base_fingerprint: fingerprint(base),
        ops: ops.to_vec(),
    };
    serde_json::to_string_pretty(&file).unwrap_or_default()
}

pub fn plan_from_json(s: &str, current: &[Disk]) -> Result<ImportedPlan, Error> {
    if s.len() > MAX_IMPORT_BYTES {
        return Err(Error::TooLarge);
    }
    let f: PlanFile = serde_json::from_str(s).map_err(|e| Error::Invalid(e.to_string()))?;
    if f.schema != SCHEMA {
        return Err(Error::UnsupportedSchema(f.schema));
    }
    Ok(ImportedPlan {
        stale: f.base_fingerprint != fingerprint(current),
        ops: f.ops,
    })
}

/// Grava `content` em `dir/name`. Só nomes da allowlist; nunca caminho vindo da UI.
pub fn write_export(dir: &Path, name: &str, content: &str) -> Result<std::path::PathBuf, Error> {
    if !matches!(name, "snapshot.json" | "plano-simulado.json") {
        return Err(Error::BadName);
    }
    fs::create_dir_all(dir).map_err(Error::Io)?;
    let path = dir.join(name);
    fs::write(&path, content).map_err(Error::Io)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use disk_domain::{OpKind, StorageProvider};
    use storage_mock::MockProvider;

    #[test]
    fn snapshot_ida_e_volta_e_detecta_adulteracao() {
        let disks = MockProvider.discover();
        let json = Snapshot::new("mock", &disks).to_json();
        assert_eq!(Snapshot::from_json(&json).unwrap().disks, disks);
        let adulterado = json.replace("Mock NVMe 512 GB", "Outro");
        assert!(matches!(
            Snapshot::from_json(&adulterado),
            Err(Error::Corrupted)
        ));
        assert!(matches!(
            Snapshot::from_json(&json.replace("\"schema\": 1", "\"schema\": 9")),
            Err(Error::UnsupportedSchema(9))
        ));
        assert!(matches!(Snapshot::from_json("{"), Err(Error::Invalid(_))));
    }

    #[test]
    fn plano_obsoleto_quando_layout_muda() {
        let mut disks = MockProvider.discover();
        let ops = vec![Op {
            id: "1".into(),
            kind: OpKind::Delete {
                part_id: "d1p2".into(),
            },
        }];
        let json = plan_to_json(&disks, &ops);
        let ok = plan_from_json(&json, &disks).unwrap();
        assert!(!ok.stale && ok.ops == ops);
        disks[0].partitions.pop();
        assert!(plan_from_json(&json, &disks).unwrap().stale);
    }

    #[test]
    fn rejeita_payload_gigante_e_nomes_fora_da_allowlist() {
        assert!(matches!(
            Snapshot::from_json(&" ".repeat(MAX_IMPORT_BYTES + 1)),
            Err(Error::TooLarge)
        ));
        assert!(matches!(
            write_export(&std::env::temp_dir(), "../x.json", "{}"),
            Err(Error::BadName)
        ));
    }

    #[test]
    fn grava_export_na_pasta_dada() {
        let dir = std::env::temp_dir().join("gdp-test-export");
        let p = write_export(&dir, "snapshot.json", "{}").unwrap();
        assert_eq!(fs::read_to_string(p).unwrap(), "{}");
        let _ = fs::remove_dir_all(dir);
    }
}
