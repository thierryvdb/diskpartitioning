import { end, gaps, usage } from "../domain/layout";
import { fmtSize } from "../domain/format";
import type { Disk } from "../domain/types";

export type Sel = { partId?: string; gapStart?: number } | null;

/** Largura proporcional; mínimo visual para regiões pequenas (distorção só na renderização). */
const pct = (len: number, total: number) => Math.max((len / total) * 100, 1.2);

export default function DiskMap({ disk, sel, onSel, label }: {
  disk: Disk; sel: Sel; onSel: (s: Sel) => void; label: string;
}) {
  const total = disk.lastUsableExclLba - disk.firstUsableLba;
  const items = [
    ...disk.partitions.map((p) => ({ k: "p" as const, start: p.startLba, len: p.lengthSectors, p })),
    ...gaps(disk).map((g) => ({ k: "g" as const, start: g.startLba, len: g.lengthSectors, p: undefined })),
  ].sort((a, b) => (a.start < b.start ? -1 : 1));

  return (
    <div role="group" aria-label={label} className="map">
      {items.map((it) => {
        const size = fmtSize(it.len, disk);
        const u = it.p && usage(it.p);
        const useTxt = u ? `\nUsado ${fmtSize(u.used, disk)} · Livre ${fmtSize(u.free, disk)}` : it.p ? "\nUso: desconhecido" : "";
        const range = `LBA ${it.start}–${it.p ? end(it.p) : it.start + it.len}`;
        const selected = it.p ? sel?.partId === it.p.id : sel?.gapStart === it.start;
        const cls = it.p ? `seg fs-${it.p.fs} ${it.p.kind} ${it.p.virtual ? "virtual" : ""}` : "seg gap";
        const name = it.p ? `${it.p.name} · ${it.p.fs.toUpperCase()}` : "Não alocado";
        return (
          <button
            key={it.p?.id ?? `gap-${it.start}`}
            className={`${cls} ${selected ? "selected" : ""}`}
            style={{ flexGrow: pct(it.len, total) }}
            title={`${name}\n${size}${useTxt}\n${range}${it.p?.virtual ? "\nPREVISTO" : ""}`}
            aria-label={`${name}, ${size}${u ? `, usado ${fmtSize(u.used, disk)}, livre ${fmtSize(u.free, disk)}` : ""}${it.p?.virtual ? ", previsto" : ""}`}
            aria-pressed={selected}
            onClick={() => onSel(it.p ? { partId: it.p.id } : { gapStart: it.start })}
          >
            <span>{name}</span>
            <small>{size}</small>
            {u && <i className="use" style={{ width: `${u.pct}%` }} aria-hidden="true" />}
          </button>
        );
      })}
    </div>
  );
}
