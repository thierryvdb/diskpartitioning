import type { Disk, Partition } from "./types";

/** 1 MiB com setor de 512 (mesmo valor de disk-planner). */
export const ALIGN_SECTORS = 2048;
export const alignUp = (n: number) => Math.ceil(n / ALIGN_SECTORS) * ALIGN_SECTORS;
export const end = (p: Partition) => p.startLba + p.lengthSectors;

/** Uso dentro do FS; `null` quando desconhecido (nunca inventar valores). */
export function usage(p: Partition): { used: number; free: number; pct: number } | null {
  if (p.usedSectors == null || p.lengthSectors <= 0) return null;
  const used = Math.min(p.usedSectors, p.lengthSectors);
  return { used, free: p.lengthSectors - used, pct: (used / p.lengthSectors) * 100 };
}

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
