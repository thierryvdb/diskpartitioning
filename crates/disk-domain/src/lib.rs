//! Entidades do domínio. Sem dependência de SO, Tauri ou UI.
//! Tamanhos/offsets em setores (`u64`); intervalo de partição: `[start_lba, start_lba + length_sectors)`.
//! Obs.: IPC serializa u64 como número JSON (exato até 2^53 setores, > 4 PiB com setor de 512).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fs {
    Ntfs,
    Fat32,
    Exfat,
    Ext4,
    Swap,
    Unknown,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Normal,
    Efi,
    Msr,
    Recovery,
}

impl Kind {
    pub fn is_protected(self) -> bool {
        self != Kind::Normal
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Partition {
    pub id: String,
    pub name: String,
    pub start_lba: u64,
    pub length_sectors: u64,
    pub fs: Fs,
    pub kind: Kind,
    /// Uso DENTRO do FS; `None` = desconhecido.
    pub used_sectors: Option<u64>,
    /// Existe só no plano simulado.
    #[serde(default)]
    pub r#virtual: bool,
}

impl Partition {
    /// Fim exclusivo; `None` em overflow.
    pub fn end(&self) -> Option<u64> {
        self.start_lba.checked_add(self.length_sectors)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Table {
    #[serde(rename = "GPT")]
    Gpt,
    #[serde(rename = "MBR")]
    Mbr,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Disk {
    pub id: String,
    pub model: String,
    pub bus: String,
    pub table: Table,
    pub sector_size: u32,
    pub total_sectors: u64,
    pub first_usable_lba: u64,
    pub last_usable_excl_lba: u64,
    pub partitions: Vec<Partition>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum OpKind {
    #[serde(rename_all = "camelCase")]
    Create {
        disk_id: String,
        start_lba: u64,
        length_sectors: u64,
        fs: Fs,
        name: String,
    },
    #[serde(rename_all = "camelCase")]
    Delete { part_id: String },
    /// Clona o layout completo do disco origem sobre o destino (destino é sobrescrito).
    #[serde(rename_all = "camelCase")]
    CloneDisk {
        source_disk_id: String,
        target_disk_id: String,
    },
    /// Copia uma partição para um espaço do disco destino.
    #[serde(rename_all = "camelCase")]
    ClonePartition {
        part_id: String,
        target_disk_id: String,
        start_lba: u64,
    },
    #[serde(rename_all = "camelCase")]
    Resize {
        part_id: String,
        new_length_sectors: u64,
    },
    #[serde(rename_all = "camelCase")]
    Move { part_id: String, new_start_lba: u64 },
    #[serde(rename_all = "camelCase")]
    Format { part_id: String, fs: Fs },
    /// Mescla duas partições adjacentes na primeira (por posição).
    #[serde(rename_all = "camelCase")]
    Merge { part_id: String, other_id: String },
    /// Divide: a original fica com `first_length_sectors`; o resto vira nova partição.
    #[serde(rename_all = "camelCase")]
    Split {
        part_id: String,
        first_length_sectors: u64,
        new_name: String,
        fs: Fs,
    },
    #[serde(rename_all = "camelCase")]
    SetLabel { part_id: String, label: String },
    #[serde(rename_all = "camelCase")]
    SetKind { part_id: String, kind: Kind },
    #[serde(rename_all = "camelCase")]
    ConvertTable { disk_id: String, to: Table },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Op {
    pub id: String,
    #[serde(flatten)]
    pub kind: OpKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub op_id: String,
    pub severity: Severity,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Resize,
    Move,
    Format,
    Delete,
    Clone,
    Merge,
    Split,
    Label,
    Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    /// Pode ser simulada; execução real indisponível no MVP 0.1.
    SimulationOnly,
    Unsupported,
}

/// Capacidade de uma ação sobre um alvo, sempre com justificativa (nada de promessa implícita).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub action: Action,
    pub support: Support,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub disks: Vec<Disk>,
    pub issues: Vec<Issue>,
    pub blocked_ops: Vec<String>,
    /// Capacidades por ID de partição do layout previsto.
    pub capabilities: std::collections::BTreeMap<String, Vec<Capability>>,
}

/// Resultado de uma descoberta: dados parciais + erros contextuais por dispositivo.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Discovery {
    pub disks: Vec<Disk>,
    pub errors: Vec<String>,
}

/// Contrato de descoberta (somente leitura). Adaptadores reais só após autorização do proprietário.
pub trait StorageProvider {
    fn discover(&self) -> Vec<Disk>;
}
