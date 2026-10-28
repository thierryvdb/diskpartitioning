import type { Disk } from "./types";

export const APP_NAME = "Global Disk Partitioning";
export const REPO_URL = "https://github.com/thierryvdb/diskpartitioning";

/** Setores -> "12.3 GiB" (só exibição; cálculos ficam no Rust). */
export function fmtSize(sectors: number, d: Pick<Disk, "sectorSize">): string {
  let v = sectors * d.sectorSize;
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let i = 0;
  while (v >= 1024 && i < 4) { v /= 1024; i++; }
  return `${i ? v.toFixed(1) : v} ${units[i]}`;
}

export const gibToSectors = (gib: number, d: Pick<Disk, "sectorSize">) =>
  Math.floor((gib * 1024 ** 3) / d.sectorSize);

/** Exportação JSON sanitizada (sem serial/WWN). */
export const exportJson = (payload: unknown) =>
  JSON.stringify(payload, null, 2);
