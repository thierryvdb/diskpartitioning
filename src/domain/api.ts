import { invoke } from "@tauri-apps/api/core";
import type { Inventory, Op, Plan } from "./types";

// Únicos comandos IPC do app: leitura (mock) e simulação. Nada é gravado.
export const getInventory = () => invoke<Inventory>("get_inventory");
// real = true lê os discos da máquina (somente leitura); só chamar após confirmação explícita do usuário.
export const loadInventory = (real: boolean) => invoke<Inventory>("load_inventory", { real });
export const simulatePlan = (ops: Op[]) => invoke<Plan>("simulate_plan", { ops });

// Exportações vão para a pasta de dados do app; retorna o caminho gravado.
export const exportSnapshot = () => invoke<string>("export_snapshot");
export const exportPlan = (ops: Op[]) => invoke<string>("export_plan", { ops });
export const importPlan = (json: string) => invoke<{ ops: Op[]; stale: boolean }>("import_plan", { json });
