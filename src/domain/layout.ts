import type { Disk, Partition } from "./types";

/** 1 MiB com setor de 512 (mesmo valor de disk-planner). */
export const ALIGN_SECTORS = 2048;
export const alignUp = (n: number) => Math.ceil(n / ALIGN_SECTORS) * ALIGN_SECTORS;
export const end = (p: Partition) => p.startLba + p.lengthSectors;

/** Só exibição do espaço não alocado; validação/simulação são do Rust. */
export function gaps(d: Disk): { startLba: number; lengthSectors: number }[] {
  const out = [];
  let cur = d.firstUsableLba;
  for (const p of [...d.partitions].sort((a, b) => a.startLba - b.startLba)) {
    if (p.startLba > cur) out.push({ startLba: cur, lengthSectors: p.startLba - cur });
    cur = Math.max(cur, end(p));
  }
  if (d.lastUsableExclLba > cur) out.push({ startLba: cur, lengthSectors: d.lastUsableExclLba - cur });
  return out;
}
