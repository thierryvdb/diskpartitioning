import { useEffect, useRef, useState } from "react";
import { APP_NAME, exportJson, fmtSize, gibToSectors } from "../domain/format";
import { listDisks, simulatePlan } from "../domain/api";
import { alignUp, end, gaps } from "../domain/layout";
import type { Disk, Fs, Op, Plan } from "../domain/types";
import About from "./About";
import DiskMap, { type Sel } from "./DiskMap";

const FS_OPTIONS: Fs[] = ["ntfs", "fat32", "exfat", "ext4"];

function describe(op: Op): string {
  switch (op.type) {
    case "create": return `Criar "${op.name}" (${op.fs}) em LBA ${op.startLba}`;
    case "delete": return `Excluir ${op.partId}`;
    case "cloneDisk": return `Clonar disco ${op.sourceDiskId} → ${op.targetDiskId}`;
    case "clonePartition": return `Clonar ${op.partId} → ${op.targetDiskId} em LBA ${op.startLba}`;
    case "resize": return `Redimensionar ${op.partId} para ${op.newLengthSectors} setores`;
    case "move": return `Mover ${op.partId} para LBA ${op.newStartLba}`;
    case "format": return `Formatar ${op.partId} como ${op.fs}`;
  }
}

export default function App() {
  const [base, setBase] = useState<Disk[] | null>(null); // snapshot imutável (fonte: Mock via IPC)
  const [plan, setPlan] = useState<Plan | null>(null);
  const [error, setError] = useState("");
  const [ops, setOps] = useState<Op[]>([]);
  const [redo, setRedo] = useState<Op[]>([]);
  const [diskIdx, setDiskIdx] = useState(0);
  const [sel, setSel] = useState<Sel>(null);
  const [about, setAbout] = useState(false);
  const [help, setHelp] = useState(false);
  const [gib, setGib] = useState("1");
  const [fs, setFs] = useState<Fs>("ext4");
  const [name, setName] = useState("nova");
  const [target, setTarget] = useState("");
  const seq = useRef(0);

  useEffect(() => { listDisks().then(setBase, (e) => setError(String(e))); }, []);
  useEffect(() => { simulatePlan(ops).then(setPlan, (e) => setError(String(e))); }, [ops]);

  if (error) return <p role="alert" className="error">Falha de IPC: {error}</p>;
  if (!base || !plan) return <p role="status">Carregando inventário…</p>;

  const disk = plan.disks[diskIdx];
  const original = base[diskIdx];
  const part = disk.partitions.find((p) => p.id === sel?.partId);
  const gap = gaps(disk).find((g) => g.startLba === sel?.gapStart);
  const sectors = gibToSectors(Number(gib) || 0, disk);
  const targets = plan.disks.filter((d) => d.id !== disk.id);
  const tgt = targets.find((d) => d.id === target) ?? targets[0];
  // primeiro espaço não alocado do destino onde a partição selecionada cabe (alinhado a 1 MiB)
  const fitStart = part && tgt
    ? gaps(tgt).map((g) => alignUp(g.startLba)).find((s, i) => s + part.lengthSectors <= gaps(tgt)[i].startLba + gaps(tgt)[i].lengthSectors)
    : undefined;
  const nextId = () => String(++seq.current);

  const add = (op: Op) => { setOps([...ops, op]); setRedo([]); };
  const undo = () => { setRedo([ops.at(-1)!, ...redo]); setOps(ops.slice(0, -1)); };
  const redoOp = () => { setOps([...ops, redo[0]]); setRedo(redo.slice(1)); };
  const move = (i: number, d: number) => {
    const next = [...ops]; [next[i], next[i + d]] = [next[i + d], next[i]]; setOps(next);
  };
  const download = () => {
    const url = URL.createObjectURL(new Blob([exportJson({ schema: 1, source: "mock", ops, plan })], { type: "application/json" }));
    Object.assign(document.createElement("a"), { href: url, download: "plano-simulado.json" }).click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="app">
      <header className="top">
        <img src="/icon.png" alt="" width={32} height={32} />
        <h1>{APP_NAME}</h1>
        <span className="banner" role="status">Modo simulação — nenhuma alteração será gravada em discos.</span>
        <nav className="menu">
          <button className="btn" aria-expanded={help} onClick={() => setHelp(!help)}>Ajuda</button>
          {help && (
            <div className="dropdown" role="menu">
              <button role="menuitem" onClick={() => { setAbout(true); setHelp(false); }}>Sobre</button>
            </div>
          )}
        </nav>
      </header>

      <aside className="side" aria-label="Discos">
        <h2>Discos</h2>
        {base.map((d, i) => (
          <button key={d.id} className={i === diskIdx ? "disk active" : "disk"} onClick={() => { setDiskIdx(i); setSel(null); }}>
            <strong>{d.model}</strong>
            <small>{fmtSize(d.totalSectors, d)} · {d.bus} · {d.table} · setor {d.sectorSize}</small>
          </button>
        ))}
      </aside>

      <main className="main">
        <h2>{disk.model}</h2>
        <p className="muted">Espaço não alocado é derivado da tabela de partições, não do espaço livre dentro do sistema de arquivos.</p>
        {ops.length > 0 && <h3>Atual</h3>}
        {ops.length > 0 && <DiskMap disk={original} sel={null} onSel={() => {}} label="Layout atual" />}
        {ops.length > 0 && <h3>Previsto (simulado)</h3>}
        <DiskMap disk={disk} sel={sel} onSel={setSel} label="Layout previsto" />

        <table>
          <caption className="sr">Partições e espaço não alocado</caption>
          <thead><tr><th>Nome</th><th>FS</th><th>Tamanho</th><th>Início (LBA)</th><th>Fim (excl.)</th><th>Uso no FS</th></tr></thead>
          <tbody>
            {[...disk.partitions.map((p) => ({ p, start: p.startLba, len: p.lengthSectors })),
              ...gaps(disk).map((g) => ({ p: undefined, start: g.startLba, len: g.lengthSectors }))]
              .sort((a, b) => (a.start < b.start ? -1 : 1))
              .map((r) => (
                <tr key={r.p?.id ?? `g${r.start}`} className={(r.p ? sel?.partId === r.p.id : sel?.gapStart === r.start) ? "selected" : ""}
                  onClick={() => setSel(r.p ? { partId: r.p.id } : { gapStart: r.start })}>
                  <td>{r.p ? r.p.name + (r.p.virtual ? " (PREVISTO)" : "") : "Não alocado"}</td>
                  <td>{r.p?.fs ?? "—"}</td>
                  <td>{fmtSize(r.len, disk)}</td>
                  <td>{String(r.start)}</td>
                  <td>{String(r.p ? end(r.p) : r.start + r.len)}</td>
                  <td>{r.p?.usedSectors != null ? fmtSize(r.p.usedSectors, disk) : "desconhecido"}</td>
                </tr>
              ))}
          </tbody>
        </table>
      </main>

      <aside className="details" aria-label="Detalhes e operações">
        <h2>Detalhes</h2>
        {!part && !gap && <p className="muted">Selecione uma partição ou espaço não alocado.</p>}
        {(part || gap) && (
          <label>Tamanho (GiB)
            <input type="number" min="0" step="0.5" value={gib} onChange={(e) => setGib(e.target.value)} />
          </label>
        )}
        <div className="actions clone">
          <h3>Clonar (simulação)</h3>
          {tgt ? (
            <label>Disco destino
              <select value={tgt.id} onChange={(e) => setTarget(e.target.value)}>
                {targets.map((d) => <option key={d.id} value={d.id}>{d.model}</option>)}
              </select>
            </label>
          ) : <p className="muted">Nenhum outro disco disponível.</p>}
          <button className="btn" disabled={!tgt}
            onClick={() => add({ id: nextId(), type: "cloneDisk", sourceDiskId: disk.id, targetDiskId: tgt.id })}>
            Clonar disco inteiro → destino
          </button>
          {part && (
            <button className="btn" disabled={!tgt || fitStart === undefined}
              title={fitStart === undefined ? "Não há espaço não alocado suficiente no destino." : undefined}
              onClick={() => add({ id: nextId(), type: "clonePartition", partId: part.id, targetDiskId: tgt.id, startLba: fitStart! })}>
              Clonar partição selecionada → destino
            </button>
          )}
          {part && tgt && fitStart === undefined && <p className="muted">Sem espaço não alocado suficiente no destino.</p>}
        </div>
        {part && (
          <div className="actions">
            <p><strong>{part.name}</strong> · {part.kind} · {part.fs}</p>
            <button className="btn" onClick={() => add({ id: nextId(), type: "resize", partId: part.id, newLengthSectors: sectors })}>Redimensionar (simular)</button>
            <button className="btn" onClick={() => add({ id: nextId(), type: "move", partId: part.id, newStartLba: alignUp(part.startLba + sectors) })}>Mover +tamanho (simular)</button>
            <label>Formatar como
              <select value={fs} onChange={(e) => setFs(e.target.value as Fs)}>{FS_OPTIONS.map((f) => <option key={f}>{f}</option>)}</select>
            </label>
            <button className="btn" onClick={() => add({ id: nextId(), type: "format", partId: part.id, fs })}>Formatar (simular)</button>
            <button className="btn danger" onClick={() => add({ id: nextId(), type: "delete", partId: part.id })}>Excluir (simular)</button>
          </div>
        )}
        {gap && (
          <div className="actions">
            <p><strong>Não alocado</strong> · {fmtSize(gap.lengthSectors, disk)}</p>
            <label>Nome <input value={name} onChange={(e) => setName(e.target.value)} /></label>
            <label>FS <select value={fs} onChange={(e) => setFs(e.target.value as Fs)}>{FS_OPTIONS.map((f) => <option key={f}>{f}</option>)}</select></label>
            <button className="btn" onClick={() => add({ id: nextId(), type: "create", diskId: disk.id, startLba: alignUp(gap.startLba), lengthSectors: sectors, fs, name })}>
              Adicionar à fila simulada
            </button>
          </div>
        )}
      </aside>

      <section className="queue" aria-label="Fila de operações simuladas">
        <div className="qhead">
          <h2>Fila pendente: {ops.length} simulação(ões)</h2>
          <span>
            <button className="btn" disabled={!ops.length} onClick={undo}>Desfazer</button>
            <button className="btn" disabled={!redo.length} onClick={redoOp}>Refazer</button>
            <button className="btn" disabled={!ops.length} onClick={() => { setOps([]); setRedo([]); }}>Limpar</button>
            <button className="btn" onClick={download}>Exportar JSON</button>
          </span>
        </div>
        <ol>
          {ops.map((op, i) => {
            const blocked = plan.blockedOps.includes(op.id);
            return (
              <li key={op.id} className={blocked ? "blocked" : ""}>
                {describe(op)} {blocked ? "— BLOQUEADA" : ""}
                <button aria-label="Subir" disabled={i === 0} onClick={() => move(i, -1)}>↑</button>
                <button aria-label="Descer" disabled={i === ops.length - 1} onClick={() => move(i, 1)}>↓</button>
                <button aria-label="Remover" onClick={() => setOps(ops.filter((o) => o !== op))}>✕</button>
                <ul>{plan.issues.filter((s) => s.opId === op.id).map((s, j) => (
                  <li key={j} className={s.severity}>{s.severity === "error" ? "Erro" : "Aviso"}: {s.message}</li>
                ))}</ul>
              </li>
            );
          })}
        </ol>
        {ops.length > 0 && <p className={plan.blockedOps.length ? "error" : "ok"}>{plan.blockedOps.length ? "Plano inválido: há operações bloqueadas." : "Simulação válida."}</p>}
        <button className="btn" disabled title="Escrita real indisponível no MVP 0.1">Aplicar alterações (indisponível nesta versão)</button>
      </section>

      <footer className="status">Inventário: mock · Nenhuma operação será gravada em discos</footer>
      {about && <About onClose={() => setAbout(false)} />}
    </div>
  );
}
