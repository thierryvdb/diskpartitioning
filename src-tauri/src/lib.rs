//! Bootstrap Tauri. IPC restrito a comandos nomeados e tipados, simulação e, só com confirmação explícita (serial + hash), aplicação via `disk-engine`.
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyPreview {
    serial: String,
    size_bytes: u64,
    steps: Vec<String>,
    /// String: u64 não cabe com exatidão em número JSON.
    hash: String,
}

/// Só discos do inventário REAL; o disco é clonado para não segurar o lock durante a E/S.
fn real_disk(inv: &Shared, disk_id: &str) -> Result<Disk, String> {
    let g = inv.lock().unwrap();
    if g.source != "real" {
        return Err("Aplicar só existe com o inventário REAL; o mock é apenas simulação.".into());
    }
    g.disks
        .iter()
        .find(|d| d.id == disk_id)
        .cloned()
        .ok_or_else(|| "Disco não encontrado no inventário.".into())
}

/// Prévia exata do que será gravado (abre o disco só para leitura; exige app elevado). Não grava nada.
#[tauri::command]
async fn preview_apply(
    inv: Shared<'_>,
    disk_id: String,
    ops: Vec<Op>,
) -> Result<ApplyPreview, String> {
    let disk = real_disk(&inv, &disk_id)?;
    #[cfg(windows)]
    {
        let p =
            tauri::async_runtime::spawn_blocking(move || disk_engine::win::preview(&disk, &ops))
                .await
                .map_err(|e| e.to_string())??;
        Ok(ApplyPreview {
            serial: p.identity.serial,
            size_bytes: p.identity.size_bytes,
            steps: p.steps,
            hash: p.hash.to_string(),
        })
    }
    #[cfg(not(windows))]
    {
        let _ = (disk, ops);
        Err("A aplicação real só está implementada no Windows.".into())
    }
}

/// Grava. Exige o serial digitado pelo usuário e o hash da prévia; ambos são rechecados no backend.
#[tauri::command]
async fn apply_plan(
    inv: Shared<'_>,
    disk_id: String,
    ops: Vec<Op>,
    hash: String,
    serial: String,
) -> Result<Vec<String>, String> {
    let disk = real_disk(&inv, &disk_id)?;
    warn!(disk = %disk_id, ops = ops.len(), "aplicação real solicitada pelo usuário");
    #[cfg(windows)]
    {
        let hash: u64 = hash.parse().map_err(|_| "hash inválido")?;
        let r = tauri::async_runtime::spawn_blocking(move || {
            disk_engine::win::apply(&disk, &ops, &serial, hash)
        })
        .await
        .map_err(|e| e.to_string())?;
        match &r {
            Ok(log) => warn!(steps = log.len(), "aplicação concluída"),
            Err(e) => warn!(error = %e, "aplicação falhou"),
        }
        r
    }
    #[cfg(not(windows))]
    {
        let _ = (disk, ops, hash, serial);
        Err("A aplicação real só está implementada no Windows.".into())
    }
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
            import_plan,
            preview_apply,
            apply_plan
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
}
