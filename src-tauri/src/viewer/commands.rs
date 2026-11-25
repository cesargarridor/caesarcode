use tauri::{AppHandle, State};

use super::tabular::{self, TabularData};
use crate::error::AppResult;
use crate::state::AppState;
use crate::util::run_blocking;

/// Lee un archivo tabular (csv/tsv/xlsx/xls/ods). Misma regla de acceso que `fs_read_file`
/// (raíces del workspace o archivo externo concedido por Rust).
#[tauri::command]
pub async fn read_tabular_file(
    state: State<'_, AppState>,
    path: String,
    sheet: Option<String>,
    limit: Option<usize>,
) -> AppResult<TabularData> {
    let p = state.resolve_file(&path)?;
    run_blocking(move || tabular::read(&p, sheet.as_deref(), limit.unwrap_or(tabular::DEFAULT_LIMIT))).await
}

/// Añade un archivo multimedia al scope del asset protocol (`convertFileSrc`). Solo
/// rutas dentro de las raíces o concedidas; solo extensiones multimedia.
#[tauri::command]
pub async fn media_allow(app: AppHandle, state: State<'_, AppState>, path: String) -> AppResult<String> {
    let p = state.resolve_file(&path)?;
    super::media::allow(&app, &p)?;
    Ok(p.to_string_lossy().into_owned())
}
