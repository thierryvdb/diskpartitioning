import { invoke } from "@tauri-apps/api/core";
import type { Disk, Op, Plan } from "./types";

// Únicos comandos IPC do app: leitura (mock) e simulação. Nada é gravado.
export const listDisks = () => invoke<Disk[]>("list_disks");
export const simulatePlan = (ops: Op[]) => invoke<Plan>("simulate_plan", { ops });
