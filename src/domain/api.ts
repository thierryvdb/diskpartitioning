import { invoke } from "@tauri-apps/api/core";
import type { Disk, Op, Plan } from "./types";

// Únicos comandos IPC do app: leitura (mock) e simulação. Nada é gravado.
export const listDisks = () => invoke<Disk[]>("list_disks");
export const simulatePlan = (ops: Op[]) => invoke<Plan>("simulate_plan", { ops });

// Exportações vão para a pasta de dados do app; retorna o caminho gravado.
export const exportSnapshot = () => invoke<string>("export_snapshot");
export const exportPlan = (ops: Op[]) => invoke<string>("export_plan", { ops });
export const importPlan = (json: string) => invoke<{ ops: Op[]; stale: boolean }>("import_plan", { json });
