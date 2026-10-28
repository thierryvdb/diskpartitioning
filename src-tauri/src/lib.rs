//! Bootstrap Tauri. IPC restrito a comandos nomeados e tipados, somente leitura/simulação.
//! Proibido: `execute_command` genérico, escrita em dispositivos, elevação (AGENTS.md).
//! Exportações vão só para a pasta de dados do app, com nomes fixos (nunca caminho vindo da UI).
//! Fonte de dados: Mock por padrão. A leitura REAL (somente leitura) só ocorre quando o usuário
//! confirma na interface (`load_inventory(real = true)`); nenhum agente a executa sem autorização.

use disk_domain::{Discovery, Disk, Op, Plan, StorageProvider};
use serde::Serialize;
use std::sync::Mutex;
use storage_mock::MockProvider;
use storage_persistence::{fingerprint, ImportedPlan, Snapshot};
use tauri::{Manager, State};
use tracing::{info, warn};

/// Snapshot atual (imutável até o usuário atualizar o inventário).
struct Inventory {
    source: &'static str,
    disks: Vec<Disk>,
    errors: Vec<String>,
}

type Shared<'a> = State<'a, Mutex<Inventory>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InventoryView {
    source: &'static str,
    disks: Vec<Disk>,
    errors: Vec<String>,
    fingerprint: String,
}

fn view(inv: &Inventory) -> InventoryView {
    InventoryView {
        source: inv.source,
        disks: inv.disks.clone(),
        errors: inv.errors.clone(),
        fingerprint: fingerprint(&inv.disks),
    }
}

fn mock() -> Inventory {
    Inventory {
        source: "mock",
        disks: MockProvider.discover(),
        errors: vec![],
    }
}

/// Descoberta real SOMENTE LEITURA (PowerShell `Get-*` / `lsblk`).
fn discover_real() -> Result<Discovery, String> {
    #[cfg(windows)]
    return storage_windows::WindowsProvider.discover_with_errors();
    #[cfg(target_os = "linux")]
    return storage_linux::LinuxProvider.discover_with_errors();
    #[cfg(not(any(windows, target_os = "linux")))]
    Err("Plataforma sem adaptador de inventário.".into())
}

#[tauri::command]
fn get_inventory(inv: Shared) -> InventoryView {
    view(&inv.lock().unwrap())
}

/// Troca/atualiza o snapshot. `real = true` lê os discos da máquina (somente leitura).
#[tauri::command]
fn load_inventory(inv: Shared, real: bool) -> Result<InventoryView, String> {
    let next = if real {
        info!("inventário real solicitado pelo usuário (somente leitura)");
        let d = discover_real().map_err(|e| {
            warn!(error = %e, "falha no inventário real");
            e
        })?;
        Inventory {
            source: "real",
            disks: d.disks,
            errors: d.errors,
        }
    } else {
        mock()
    };
    info!(
        source = next.source,
        disks = next.disks.len(),
        errors = next.errors.len(),
        "inventário carregado"
    );
    let mut guard = inv.lock().unwrap();
    *guard = next;
    Ok(view(&guard))
}

/// Simula a fila sobre o snapshot atual; nada é gravado.
#[tauri::command]
fn simulate_plan(inv: Shared, ops: Vec<Op>) -> Plan {
    let plan = disk_planner::simulate(&inv.lock().unwrap().disks, &ops);
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
fn export_snapshot(app: tauri::AppHandle, inv: Shared) -> Result<String, String> {
    let g = inv.lock().unwrap();
    export(
        &app,
        "snapshot.json",
        Snapshot::new(g.source, &g.disks).to_json(),
    )
}

#[tauri::command]
fn export_plan(app: tauri::AppHandle, inv: Shared, ops: Vec<Op>) -> Result<String, String> {
    let json = storage_persistence::plan_to_json(&inv.lock().unwrap().disks, &ops);
    export(&app, "plano-simulado.json", json)
}

/// Importa um plano exportado; `stale` indica layout diferente do atual.
#[tauri::command]
fn import_plan(inv: Shared, json: String) -> Result<ImportedPlan, String> {
    storage_persistence::plan_from_json(&json, &inv.lock().unwrap().disks).map_err(|e| {
        warn!(error = %e, "importação de plano recusada");
        e.to_string()
    })
}

pub fn run() {
    tracing_subscriber::fmt().with_target(false).init();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(mock()))
        .invoke_handler(tauri::generate_handler![
            get_inventory,
            load_inventory,
            simulate_plan,
            export_snapshot,
            export_plan,
            import_plan
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
}
