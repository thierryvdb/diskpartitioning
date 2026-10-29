// Tamanhos/offsets em setores como number: nunca float para aritmética de disco.
// Intervalo de partição: [startLba, startLba + lengthSectors).
export type Fs = "ntfs" | "fat32" | "exfat" | "ext4" | "swap" | "unknown" | "none";
export type Kind = "normal" | "efi" | "msr" | "recovery";

export interface Partition {
  id: string;
  name: string;
  startLba: number;
  lengthSectors: number;
  fs: Fs;
  kind: Kind;
  usedSectors?: number | null; // uso DENTRO do FS; desconhecido = undefined
  virtual?: boolean; // existe só no plano simulado
}

export interface Disk {
  id: string;
  model: string;
  bus: string;
  table: "GPT" | "MBR";
  sectorSize: number;
  totalSectors: number;
  firstUsableLba: number;
  lastUsableExclLba: number;
  partitions: Partition[];
}

export type Op =
  | { id: string; type: "create"; diskId: string; startLba: number; lengthSectors: number; fs: Fs; name: string }
  | { id: string; type: "delete"; partId: string }
  | { id: string; type: "cloneDisk"; sourceDiskId: string; targetDiskId: string }
  | { id: string; type: "clonePartition"; partId: string; targetDiskId: string; startLba: number }
  | { id: string; type: "resize"; partId: string; newLengthSectors: number }
  | { id: string; type: "move"; partId: string; newStartLba: number }
  | { id: string; type: "format"; partId: string; fs: Fs }
  | { id: string; type: "merge"; partId: string; otherId: string }
  | { id: string; type: "split"; partId: string; firstLengthSectors: number; newName: string; fs: Fs }
  | { id: string; type: "setLabel"; partId: string; label: string }
  | { id: string; type: "setKind"; partId: string; kind: Kind }
  | { id: string; type: "convertTable"; diskId: string; to: "GPT" | "MBR" };

export interface Issue {
  opId: string;
  severity: "error" | "warning";
  message: string;
}

export interface Inventory {
  source: "none" | "real";
  disks: Disk[];
  errors: string[];
  fingerprint: string;
}

export interface Capability {
  action: "resize" | "move" | "format" | "delete" | "clone" | "merge" | "split" | "label" | "kind";
  support: "simulation_only" | "unsupported";
  reason: string;
}

export interface Plan {
  disks: Disk[]; // layout previsto; o snapshot base nunca é alterado
  issues: Issue[];
  blockedOps: string[];
  capabilities: Record<string, Capability[]>;
}
