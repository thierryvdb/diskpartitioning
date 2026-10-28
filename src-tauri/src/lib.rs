//! Bootstrap Tauri. IPC restrito a comandos nomeados e tipados, somente leitura/simulação.
//! Proibido: `execute_command` genérico, escrita em dispositivos, elevação (AGENTS.md).
//! Exportações vão só para a pasta de dados do app, com nomes fixos (nunca caminho vindo da UI).

use disk_domain::{Disk, Op, Plan, StorageProvider};
use storage_mock::MockProvider;
use storage_persistence::{ImportedPlan, Snapshot};
use tauri::Manager;
use tracing::{info, warn};

/// Fonte atual de dados: Mock. Discovery real só após autorização do proprietário.
fn inventory() -> Vec<Disk> {
    MockProvider.discover()
}

#[tauri::command]
fn list_disks() -> Vec<Disk> {
    let disks = inventory();
    info!(disks = disks.len(), "inventário (mock) listado");
    disks
}

/// Simula a fila sobre o snapshot mock; nada é gravado.
#[tauri::command]
fn simulate_plan(ops: Vec<Op>) -> Plan {
    let plan = disk_planner::simulate(&inventory(), &ops);
    info!(
        ops = ops.len(),
        blocked = plan.blocked_ops.len(),
        "plano simulado"
    );
    plan
}

fn export(app: &tauri::AppHandle, name: &str, content: String) -> Result<String, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("exports");
    let path =
        storage_persistence::write_export(&dir, name, &content).map_err(|e| e.to_string())?;
    info!(file = name, "exportação gravada");
    Ok(path.display().to_string())
}

#[tauri::command]
fn export_snapshot(app: tauri::AppHandle) -> Result<String, String> {
    export(
        &app,
        "snapshot.json",
        Snapshot::new("mock", &inventory()).to_json(),
    )
}

#[tauri::command]
fn export_plan(app: tauri::AppHandle, ops: Vec<Op>) -> Result<String, String> {
    export(
        &app,
        "plano-simulado.json",
        storage_persistence::plan_to_json(&inventory(), &ops),
    )
}

/// Importa um plano exportado; `stale` indica layout diferente do atual.
#[tauri::command]
fn import_plan(json: String) -> Result<ImportedPlan, String> {
    storage_persistence::plan_from_json(&json, &inventory()).map_err(|e| {
        warn!(error = %e, "importação de plano recusada");
        e.to_string()
    })
}

pub fn run() {
    tracing_subscriber::fmt().with_target(false).init();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_disks,
            simulate_plan,
            export_snapshot,
            export_plan,
            import_plan
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
}
