import { invoke } from "@tauri-apps/api/core";
import type { Inventory, Op, Plan } from "./types";

// Comandos IPC do app: leitura, simulação e, só com confirmação explícita (serial digitado + hash da prévia), aplicação.
export const getInventory = () => invoke<Inventory>("get_inventory");
// Lê os discos da máquina (somente leitura); só chamar após confirmação explícita do usuário.
export const loadInventory = () => invoke<Inventory>("load_inventory");
export const simulatePlan = (ops: Op[]) => invoke<Plan>("simulate_plan", { ops });

// Exportações vão para a pasta de dados do app; retorna o caminho gravado.
export const exportSnapshot = () => invoke<string>("export_snapshot");
export const exportPlan = (ops: Op[]) => invoke<string>("export_plan", { ops });
export const importPlan = (json: string) => invoke<{ ops: Op[]; stale: boolean }>("import_plan", { json });

// Aplicação real (somente inventário REAL, Windows, app elevado). `previewApply` não grava nada.
export type ApplyPreview = { serial: string; sizeBytes: number; steps: string[]; hash: string };
export const previewApply = (diskId: string, ops: Op[]) => invoke<ApplyPreview>("preview_apply", { diskId, ops });
export const applyPlan = (diskId: string, ops: Op[], hash: string, serial: string) =>
  invoke<string[]>("apply_plan", { diskId, ops, hash, serial });
