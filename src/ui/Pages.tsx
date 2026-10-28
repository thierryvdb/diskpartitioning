import { useRef, useState } from "react";
import { exportPlan, exportSnapshot, importPlan } from "../domain/api";
import { fmtBytes } from "../domain/format";
import { gaps } from "../domain/layout";
import type { Disk, Op, Plan } from "../domain/types";

export type Prefs = { theme: "auto" | "light" | "dark"; density: "comfortable" | "compact" };

export function Overview({ base, plan, ops }: { base: Disk[]; plan: Plan; ops: Op[] }) {
  const sum = (f: (d: Disk) => number) => base.reduce((a, d) => a + f(d) * d.sectorSize, 0);
  const alloc = sum((d) => d.partitions.reduce((a, p) => a + p.lengthSectors, 0));
  const free = sum((d) => gaps(d).reduce((a, g) => a + g.lengthSectors, 0));
  const warns = plan.issues.filter((i) => i.severity === "warning").length;
  const cards: [string, string][] = [
    ["Discos", String(base.length)],
    ["Partições", String(base.reduce((a, d) => a + d.partitions.length, 0))],
    ["Alocado em partições", fmtBytes(alloc)],
    ["Não alocado", fmtBytes(free)],
    ["Operações simuladas", String(ops.length)],
    ["Bloqueadas / avisos", `${plan.blockedOps.length} / ${warns}`],
  ];
  return (
    <>
      <h2>Visão geral</h2>
      <p className="muted">
        Inventário: mock (nenhum disco real é lido nesta versão). "Não alocado" vem da tabela de partições, não do espaço
        livre dos sistemas de arquivos.
      </p>
      <dl className="cards">
        {cards.map(([k, v]) => (
          <div key={k} className="card"><dt>{k}</dt><dd>{v}</dd></div>
        ))}
      </dl>
    </>
  );
}

export function Reports({ ops, plan, onImport }: { ops: Op[]; plan: Plan; onImport: (ops: Op[]) => void }) {
  const [msg, setMsg] = useState("");
  const file = useRef<HTMLInputElement>(null);
  const run = (p: Promise<string>) =>
    p.then((path) => setMsg(`Arquivo gravado em: ${path}`), (e) => setMsg(`Erro: ${e}`));
  const load = async (f: File) => {
    try {
      const r = await importPlan(await f.text());
      onImport(r.ops);
      setMsg(r.stale ? "O layout mudou. Atualize o inventário para recalcular a simulação. (plano importado mesmo assim)" : "Plano importado.");
    } catch (e) {
      setMsg(`Erro: ${e}`);
    }
  };
  return (
    <>
      <h2>Relatórios</h2>
      <p className="muted">
        Exportações são JSON com schema versionado, sem serial/WWN, gravadas na pasta de dados do aplicativo.
      </p>
      <div>
        <button className="btn" onClick={() => run(exportSnapshot())}>Exportar diagnóstico (snapshot)</button>
        <button className="btn" onClick={() => run(exportPlan(ops))}>Exportar plano simulado</button>
        <button className="btn" onClick={() => file.current?.click()}>Importar plano…</button>
        <input
          ref={file} type="file" accept="application/json,.json" hidden aria-label="Arquivo de plano"
          onChange={(e) => { const f = e.target.files?.[0]; if (f) load(f); e.target.value = ""; }}
        />
      </div>
      {msg && <p role="status" className="muted">{msg}</p>}
      <h3>Avisos e bloqueios da simulação atual</h3>
      {plan.issues.length === 0 ? (
        <p className="muted">Nenhum.</p>
      ) : (
        <ul>
          {plan.issues.map((i, k) => (
            <li key={k} className={i.severity}>
              Operação {i.opId} — {i.severity === "error" ? "Erro" : "Aviso"}: {i.message}
            </li>
          ))}
        </ul>
      )}
    </>
  );
}

export function Settings({ prefs, onChange }: { prefs: Prefs; onChange: (p: Prefs) => void }) {
  return (
    <>
      <h2>Configurações</h2>
      <label>Tema
        <select value={prefs.theme} onChange={(e) => onChange({ ...prefs, theme: e.target.value as Prefs["theme"] })}>
          <option value="auto">Automático (sistema)</option>
          <option value="light">Claro</option>
          <option value="dark">Escuro</option>
        </select>
      </label>
      <label>Densidade das tabelas
        <select value={prefs.density} onChange={(e) => onChange({ ...prefs, density: e.target.value as Prefs["density"] })}>
          <option value="comfortable">Confortável</option>
          <option value="compact">Compacta</option>
        </select>
      </label>
      <p className="muted">Idioma: Português (Brasil). Unidades: KiB/MiB/GiB/TiB (base 1024).</p>
    </>
  );
}
