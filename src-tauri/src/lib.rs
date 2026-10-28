//! Bootstrap Tauri. IPC restrito a comandos nomeados e tipados, somente leitura/simulação.
//! Proibido: `execute_command` genérico, escrita em dispositivos, elevação (AGENTS.md).

use disk_domain::{Disk, Op, Plan, StorageProvider};
use storage_mock::MockProvider;

/// Fonte atual de dados: Mock. Discovery real só após autorização do proprietário.
#[tauri::command]
fn list_disks() -> Vec<Disk> {
    MockProvider.discover()
}

/// Simula a fila sobre o snapshot mock; nada é gravado.
#[tauri::command]
fn simulate_plan(ops: Vec<Op>) -> Plan {
    disk_planner::simulate(&MockProvider.discover(), &ops)
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![list_disks, simulate_plan])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
}
