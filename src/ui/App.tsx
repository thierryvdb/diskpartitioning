import { useEffect, useRef, useState } from "react";
import { APP_NAME, fmtSize, gibToSectors } from "../domain/format";
import { type ApplyPreview, applyPlan, getInventory, loadInventory, previewApply, simulatePlan } from "../domain/api";
import { alignUp, end, gaps, usage } from "../domain/layout";
import type { Capability, Disk, Fs, Inventory, Kind, Op, Plan } from "../domain/types";
import About from "./About";
import DiskMap, { type Sel } from "./DiskMap";
import { Overview, type Prefs, Reports, Settings } from "./Pages";

const PAGES = [["visao", "Visão geral"], ["discos", "Discos"], ["relatorios", "Relatórios"], ["config", "Configurações"]] as const;
type Page = (typeof PAGES)[number][0];

const NO_DISK: Disk = {
  id: "-", model: "Nenhum disco encontrado", bus: "", table: "GPT", sectorSize: 512,
  totalSectors: 0, firstUsableLba: 0, lastUsableExclLba: 0, partitions: [],
};

const DEFAULT_PREFS: Prefs = { theme: "auto", density: "comfortable" };
function loadPrefs(): Prefs {
  try { return { ...DEFAULT_PREFS, ...JSON.parse(localStorage.getItem("prefs") ?? "{}") }; } catch { return DEFAULT_PREFS; }
}

const FS_OPTIONS: Fs[] = ["ntfs", "fat32", "exfat", "ext4"];

function describe(op: Op): string {
  switch (op.type) {
    case "create": return `Criar "${op.name}" (${op.fs}) em LBA ${op.startLba}`;
    case "delete": return `Excluir ${op.partId}`;
    case "merge": return `Mesclar ${op.partId} com ${op.otherId}`;
    case "split": return `Dividir ${op.partId}: manter ${op.firstLengthSectors} setores e criar "${op.newName}"`;
    case "setLabel": return `Rótulo de ${op.partId}: "${op.label}"`;
    case "setKind": return `Tipo de ${op.partId}: ${op.kind.toUpperCase()}`;
    case "convertTable": return `Converter ${op.diskId} para ${op.to}`;
    case "cloneDisk": return `Clonar disco ${op.sourceDiskId} → ${op.targetDiskId}`;
    case "clonePartition": return `Clonar ${op.partId} → ${op.targetDiskId} em LBA ${op.startLba}`;
    case "resize": return `Redimensionar ${op.partId} para ${op.newLengthSectors} setores`;
    case "move": return `Mover ${op.partId} para LBA ${op.newStartLba}`;
    case "format": return `Formatar ${op.partId} como ${op.fs}`;
  }
}

function PartUsage({ p, d }: { p: Disk["partitions"][number]; d: Disk }) {
  const u = usage(p);
  if (!u) return <p className="muted">Uso do sistema de arquivos: desconhecido.</p>;
  return (
    <div className="usage">
      <progress value={u.used} max={p.lengthSectors} aria-label={`Espaço ocupado em ${p.name}`} />
      <dl>
        <dt>Tamanho</dt><dd>{fmtSize(p.lengthSectors, d)}</dd>
        <dt>Ocupado</dt><dd>{fmtSize(u.used, d)} ({u.pct.toFixed(1)}%)</dd>
        <dt>Livre (no FS)</dt><dd>{fmtSize(u.free, d)}</dd>
      </dl>
    </div>
  );
}

export default function App() {
  const [inv, setInv] = useState<Inventory | null>(null); // snapshot imutável (vazio ou Real via IPC)
  const [confirmReal, setConfirmReal] = useState(false);
  const [notice, setNotice] = useState("");
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
  const [label, setLabel] = useState("");
  const [newKind, setNewKind] = useState<Kind>("recovery");
  const [prev, setPrev] = useState<ApplyPreview | null>(null);
  const [typed, setTyped] = useState("");
  const [applyLog, setApplyLog] = useState<string[] | null>(null);
  const [busy, setBusy] = useState(false);
  const seq = useRef(0);
  const [page, setPage] = useState<Page>("discos");
  const [prefs, setPrefs] = useState<Prefs>(loadPrefs);

  useEffect(() => {
    document.documentElement.dataset.theme = prefs.theme;
    document.documentElement.dataset.density = prefs.density;
    try { localStorage.setItem("prefs", JSON.stringify(prefs)); } catch { /* sem storage: só não persiste */ }
  }, [prefs]);

  useEffect(() => { getInventory().then(setInv, (e) => setError(String(e))); }, []);

  // real = true só é chamado pelo botão de confirmação (leitura somente leitura dos discos da máquina)
  const reload = () => {
    setNotice("");
    loadInventory().then((next) => {
      if (inv && ops.length && next.fingerprint !== inv.fingerprint)
        setNotice("O layout mudou. Atualize o inventário para recalcular a simulação.");
      setInv(next); setDiskIdx(0); setSel(null); setConfirmReal(false);
    }, (e) => setNotice(`Falha ao ler inventário: ${e}`));
  };
  useEffect(() => { simulatePlan(ops).then(setPlan, (e) => setError(String(e))); }, [ops, inv?.fingerprint]);

  const startApply = () => {
    setApplyLog(null); setTyped("");
    previewApply(disk.id, ops).then(setPrev, (e) => setNotice(`Não foi possível preparar a aplicação: ${e}`));
  };
  const confirmApply = () => {
    if (!prev) return;
    setBusy(true);
    applyPlan(disk.id, ops, prev.hash, typed).then(
      (log) => { setApplyLog(log); setOps([]); setRedo([]); reload(); },
      (e) => setApplyLog([`ERRO: ${e}`]),
    ).finally(() => setBusy(false));
  };

  if (error) return <p role="alert" className="error">Falha de IPC: {error}</p>;
  if (!inv || !plan) return <p role="status">Carregando inventário…</p>;
  const base = inv.disks;

  const disk = plan.disks[diskIdx] ?? NO_DISK;
  const original = base[diskIdx] ?? NO_DISK;
  const part = disk.partitions.find((p) => p.id === sel?.partId);
  const gap = gaps(disk).find((g) => g.startLba === sel?.gapStart);
  const sectors = gibToSectors(Number(gib) || 0, disk);
  const targets = plan.disks.filter((d) => d.id !== disk.id);
  const tgt = targets.find((d) => d.id === target) ?? targets[0];
  // primeiro espaço não alocado do destino onde a partição selecionada cabe (alinhado a 1 MiB)
  const fitStart = part && tgt
    ? gaps(tgt).map((g) => alignUp(g.startLba)).find((s, i) => s + part.lengthSectors <= gaps(tgt)[i].startLba + gaps(tgt)[i].lengthSectors)
    : undefined;
  const unalloc = gaps(disk).reduce((a, g) => a + g.lengthSectors, 0);
  const known = disk.partitions.filter((p) => usage(p));
  const usedSum = known.reduce((a, p) => a + usage(p)!.used, 0);
  const knownSum = known.reduce((a, p) => a + p.lengthSectors, 0);
  const summary = `Capacidade ${fmtSize(disk.totalSectors, disk)} · Não alocado (tabela de partições) ${fmtSize(unalloc, disk)}` +
    (known.length ? ` · Dentro dos sistemas de arquivos com uso conhecido: usado ${fmtSize(usedSum, disk)}, livre ${fmtSize(knownSum - usedSum, disk)}` : "");
  const neighbor = part ? disk.partitions.find((p) => p.startLba === end(part)) : undefined;
  const caps = part ? plan.capabilities[part.id] ?? [] : [];
  const blockedReason = (a: Capability["action"]) => caps.find((c) => c.action === a && c.support === "unsupported")?.reason;
  const nextId = () => String(++seq.current);

  const add = (op: Op) => { setOps([...ops, op]); setRedo([]); };
  const undo = () => { setRedo([ops.at(-1)!, ...redo]); setOps(ops.slice(0, -1)); };
  const redoOp = () => { setOps([...ops, redo[0]]); setRedo(redo.slice(1)); };
  const move = (i: number, d: number) => {
    const next = [...ops]; [next[i], next[i + d]] = [next[i + d], next[i]]; setOps(next);
  };

  return (
    <div className="app">
      <header className="top">
        <img src="/icon.png" alt="" width={32} height={32} />
        <h1>{APP_NAME}</h1>
        <span className="banner" role="status">{inv?.source === "real" ? "Inventário REAL — a fila é simulação; só grava ao aplicar, com prévia e serial." : "Nenhum disco carregado — leia os discos reais para começar."}</span>
        <nav className="pages" aria-label="Páginas">
          {PAGES.map(([id, label]) => (
            <button key={id} className="btn" aria-current={page === id ? "page" : undefined} onClick={() => setPage(id)}>{label}</button>
          ))}
        </nav>
        <nav className="menu" aria-label="Ajuda">
          <button className="btn" aria-expanded={help} onClick={() => setHelp(!help)}>Ajuda</button>
          {help && (
            <div className="dropdown" role="menu">
              <button role="menuitem" onClick={() => { setAbout(true); setHelp(false); }}>Sobre</button>
            </div>
          )}
        </nav>
      </header>

      {page === "visao" && <main className="main wide"><Overview base={base} plan={plan} ops={ops} /></main>}
      {page === "relatorios" && (
        <main className="main wide">
          <Reports ops={ops} plan={plan} onImport={(o) => { setOps(o); setRedo([]); seq.current = o.length; }} />
        </main>
      )}
      {page === "config" && <main className="main wide"><Settings prefs={prefs} onChange={setPrefs} /></main>}
      {page === "discos" && <>
      <aside className="side" aria-label="Discos">
        <h2>Discos</h2>
        <div className="source">
          <p><strong>Fonte:</strong> {inv.source === "real" ? "Real" : "nenhum disco carregado"}</p>
          {inv.source === "real" ? (
            <>
              <button className="btn" onClick={() => reload()}>Atualizar inventário</button>
            </>
          ) : (
            <button className="btn" onClick={() => setConfirmReal(true)}>Ler discos reais…</button>
          )}
          {confirmReal && (
            <div role="alertdialog" aria-label="Confirmar leitura de discos reais" className="confirm">
              <p>
                Isto vai <strong>ler</strong> (somente leitura) os discos desta máquina via PowerShell/lsblk.
                Nada será gravado e nenhuma elevação será solicitada. Simulações continuam sem efeito em disco.
              </p>
              <button className="btn" onClick={() => reload()}>Confirmo: ler discos reais</button>
              <button className="btn" onClick={() => setConfirmReal(false)}>Cancelar</button>
            </div>
          )}
          {notice && <p role="status" className="warning">{notice}</p>}
          {inv.errors.map((e, i) => <p key={i} className="warning">{e}</p>)}
        </div>
        {base.map((d, i) => (
          <button key={d.id} className={i === diskIdx ? "disk active" : "disk"} onClick={() => { setDiskIdx(i); setSel(null); }}>
            <strong>{d.model}</strong>
            <small>{fmtSize(d.totalSectors, d)} · {d.bus} · {d.table} · setor {d.sectorSize}</small>
          </button>
        ))}
      </aside>

      <main className="main">
        <h2>{disk.model}</h2>
        <p className="muted">{summary}</p>
        {ops.length > 0 && <h3>Atual</h3>}
        {ops.length > 0 && <DiskMap disk={original} sel={null} onSel={() => {}} label="Layout atual" />}
        {ops.length > 0 && <h3>Previsto (simulado)</h3>}
        <DiskMap disk={disk} sel={sel} onSel={setSel} label="Layout previsto" />

        <table>
          <caption className="sr">Partições e espaço não alocado</caption>
          <thead><tr><th>Nome</th><th>FS</th><th>Tamanho</th><th>Início (LBA)</th><th>Fim (excl.)</th><th>Uso no FS (usado · livre)</th></tr></thead>
          <tbody>
            {[...disk.partitions.map((p) => ({ p, start: p.startLba, len: p.lengthSectors })),
              ...gaps(disk).map((g) => ({ p: undefined, start: g.startLba, len: g.lengthSectors }))]
              .sort((a, b) => (a.start < b.start ? -1 : 1))
              .map((r) => (
                <tr key={r.p?.id ?? `g${r.start}`} className={(r.p ? sel?.partId === r.p.id : sel?.gapStart === r.start) ? "selected" : ""}
                  tabIndex={0} aria-selected={r.p ? sel?.partId === r.p.id : sel?.gapStart === r.start}
                  onClick={() => setSel(r.p ? { partId: r.p.id } : { gapStart: r.start })}
                  onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setSel(r.p ? { partId: r.p.id } : { gapStart: r.start }); } }}>
                  <td>{r.p ? r.p.name + (r.p.virtual ? " (PREVISTO)" : "") : "Não alocado"}</td>
                  <td>{r.p?.fs ?? "—"}</td>
                  <td>{fmtSize(r.len, disk)}</td>
                  <td>{String(r.start)}</td>
                  <td>{String(r.p ? end(r.p) : r.start + r.len)}</td>
                  <td>{r.p ? (usage(r.p) ? `${fmtSize(usage(r.p)!.used, disk)} usado · ${fmtSize(usage(r.p)!.free, disk)} livre` : "desconhecido") : "—"}</td>
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
          <button className="btn" onClick={() => add({ id: nextId(), type: "convertTable", diskId: disk.id, to: disk.table === "GPT" ? "MBR" : "GPT" })}>
            Converter para {disk.table === "GPT" ? "MBR" : "GPT"} (simular)
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
            <PartUsage p={part} d={disk} />
            <button className="btn" disabled={!!blockedReason("resize")} title={blockedReason("resize")} onClick={() => add({ id: nextId(), type: "resize", partId: part.id, newLengthSectors: sectors })}>Redimensionar (simular)</button>
            <button className="btn" disabled={!!blockedReason("move")} title={blockedReason("move")} onClick={() => add({ id: nextId(), type: "move", partId: part.id, newStartLba: alignUp(part.startLba + sectors) })}>Mover +tamanho (simular)</button>
            <label>Formatar como
              <select value={fs} onChange={(e) => setFs(e.target.value as Fs)}>{FS_OPTIONS.map((f) => <option key={f}>{f}</option>)}</select>
            </label>
            <button className="btn" disabled={!!blockedReason("format")} title={blockedReason("format")} onClick={() => add({ id: nextId(), type: "format", partId: part.id, fs })}>Formatar (simular)</button>
            <button className="btn danger" disabled={!!blockedReason("delete")} title={blockedReason("delete")} onClick={() => add({ id: nextId(), type: "delete", partId: part.id })}>Excluir (simular)</button>
            <h3>Mesclar, dividir e editar (simulação)</h3>
            <button className="btn" disabled={!!blockedReason("merge") || !neighbor}
              title={blockedReason("merge") ?? (neighbor ? undefined : "Não há partição adjacente à direita.")}
              onClick={() => add({ id: nextId(), type: "merge", partId: part.id, otherId: neighbor!.id })}>
              Mesclar com a partição seguinte{neighbor ? ` (${neighbor.name})` : ""}
            </button>
            <button className="btn" disabled={!!blockedReason("split")} title={blockedReason("split")}
              onClick={() => add({ id: nextId(), type: "split", partId: part.id, firstLengthSectors: sectors, newName: name, fs })}>
              Dividir: manter o tamanho acima na original
            </button>
            <label>Novo rótulo <input value={label} maxLength={32} onChange={(e) => setLabel(e.target.value)} /></label>
            <button className="btn" disabled={!!blockedReason("label")} title={blockedReason("label")}
              onClick={() => add({ id: nextId(), type: "setLabel", partId: part.id, label })}>Alterar rótulo (simular)</button>
            <label>Tipo da partição
              <select value={newKind} onChange={(e) => setNewKind(e.target.value as Kind)}>
                <option value="recovery">Recovery</option><option value="efi">EFI (exige FAT32)</option>
              </select>
            </label>
            <button className="btn" disabled={!!blockedReason("kind")} title={blockedReason("kind")}
              onClick={() => add({ id: nextId(), type: "setKind", partId: part.id, kind: newKind })}>Alterar tipo (simular)</button>
            {[...new Set(caps.filter((c) => c.support === "unsupported").map((c) => c.reason))].map((r) => (
              <p key={r} className="muted" role="note">Indisponível: {r}</p>
            ))}
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

      </>}

      <section className="queue" aria-label="Fila de operações simuladas">
        <div className="qhead">
          <h2>Fila pendente: {ops.length} simulação(ões)</h2>
          <span>
            <button className="btn" disabled={!ops.length} onClick={undo}>Desfazer</button>
            <button className="btn" disabled={!redo.length} onClick={redoOp}>Refazer</button>
            <button className="btn" disabled={!ops.length} onClick={() => { setOps([]); setRedo([]); }}>Limpar</button>
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
        <button className="btn danger" disabled={inv.source !== "real" || !ops.length || plan.blockedOps.length > 0}
          title={inv.source !== "real" ? "Carregue primeiro os discos reais." : undefined}
          onClick={startApply}>Aplicar alterações ao disco {disk.id}…</button>
      </section>

      <footer className="status">Inventário: {inv.source === "real" ? "REAL" : "sem inventário"} · Só grava em disco após prévia e confirmação do serial</footer>
      {prev && (
        <div role="alertdialog" aria-label="Confirmar aplicação no disco" className="confirm">
          <h2>Aplicar no disco {disk.id}: {prev.serial}</h2>
          <p className="error">Isto altera o disco de verdade e não pode ser desfeito pelo app.</p>
          <ol>{prev.steps.map((t, i) => <li key={i}>{t}</li>)}</ol>
          {applyLog ? (
            <>
              <ul>{applyLog.map((t, i) => <li key={i}>{t}</li>)}</ul>
              <button className="btn" onClick={() => { setPrev(null); setApplyLog(null); }}>Fechar</button>
            </>
          ) : (
            <>
              <label>Digite o serial do disco ({prev.serial}) para confirmar
                <input value={typed} onChange={(e) => setTyped(e.target.value)} autoComplete="off" />
              </label>
              <button className="btn danger" disabled={busy || typed !== prev.serial} onClick={confirmApply}>
                {busy ? "Aplicando…" : "Aplicar agora"}
              </button>
              <button className="btn" disabled={busy} onClick={() => setPrev(null)}>Cancelar</button>
            </>
          )}
        </div>
      )}
      {about && <About onClose={() => setAbout(false)} />}
    </div>
  );
}
