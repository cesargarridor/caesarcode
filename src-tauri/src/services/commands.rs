use tauri::ipc::Channel;
use tauri::State;

use super::{detect, ports, LogLine, RootedService, ServiceConfig, ServiceStatus};
use crate::error::AppResult;
use crate::state::AppState;
use crate::util::run_blocking;

/// `root`: raíz del workspace (None = primaria).
#[tauri::command]
pub async fn services_load(state: State<'_, AppState>, root: Option<String>) -> AppResult<Vec<RootedService>> {
    let root = state.resolve_root(root.as_deref())?;
    let m = state.services.clone();
    run_blocking(move || {
        let cfgs = m.load_configs_for(&root)?;
        let root_s = root.to_string_lossy().into_owned();
        Ok(cfgs.into_iter().map(|cfg| RootedService { cfg, root: root_s.clone() }).collect())
    })
    .await
}

#[tauri::command]
pub async fn services_save(state: State<'_, AppState>, root: Option<String>, services: Vec<ServiceConfig>) -> AppResult<()> {
    let root = state.resolve_root(root.as_deref())?;
    let m = state.services.clone();
    run_blocking(move || m.save_configs_for(&root, services)).await
}

#[tauri::command]
pub async fn services_detect(state: State<'_, AppState>, root: Option<String>) -> AppResult<Vec<ServiceConfig>> {
    let root = state.resolve_root(root.as_deref())?;
    run_blocking(move || {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "root".into());
        Ok(detect::detect(&root, &name))
    })
    .await
}

#[tauri::command]
pub async fn services_subscribe(
    state: State<'_, AppState>,
    on_log: Channel<Vec<LogLine>>,
    on_status: Channel<Vec<ServiceStatus>>,
) -> AppResult<()> {
    state.root()?;
    state.services.subscribe(
        Box::new(move |batch| on_log.send(batch).is_ok()),
        Box::new(move |st| on_status.send(st).is_ok()),
    );
    Ok(())
}

#[tauri::command]
pub async fn service_start(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let m = state.services.clone();
    run_blocking(move || m.start(&id)).await
}

#[tauri::command]
pub async fn service_stop(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let m = state.services.clone();
    run_blocking(move || m.stop_blocking(&id)).await
}

#[tauri::command]
pub async fn service_restart(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let m = state.services.clone();
    run_blocking(move || m.restart(&id)).await
}

#[tauri::command]
pub async fn services_start_all(state: State<'_, AppState>) -> AppResult<()> {
    let m = state.services.clone();
    run_blocking(move || m.start_all()).await
}

#[tauri::command]
pub async fn services_stop_all(state: State<'_, AppState>) -> AppResult<()> {
    let m = state.services.clone();
    run_blocking(move || m.stop_all()).await
}

#[tauri::command]
pub async fn services_status(state: State<'_, AppState>) -> AppResult<Vec<ServiceStatus>> {
    Ok(state.services.statuses())
}

#[tauri::command]
pub async fn service_logs(state: State<'_, AppState>, id: Option<String>, limit: u32) -> AppResult<Vec<LogLine>> {
    Ok(state.services.logs(id.as_deref(), limit as usize))
}

#[tauri::command]
pub async fn service_clear_logs(state: State<'_, AppState>, id: Option<String>) -> AppResult<()> {
    state.services.clear_logs(id.as_deref());
    Ok(())
}

#[tauri::command]
pub async fn port_kill(state: State<'_, AppState>, port: u16) -> AppResult<Vec<u32>> {
    let pids = run_blocking(move || Ok(ports::port_kill(port))).await?;
    // Refresca el estado de puertos/servicios sin esperar al siguiente ciclo.
    state.services.emit_status();
    Ok(pids)
}
