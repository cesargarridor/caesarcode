use std::path::{Path, PathBuf};

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use super::manager::Ensured;
use super::servers::{language_for, language_id_for};
use super::{CompletionDto, HoverDto, LocationDto, LspDocStatus, LspEvent, LspServerStatus};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::util::paths::ensure_in_roots;
use crate::util::run_blocking;

/// Valida la ruta (dentro de alguna raíz) y devuelve la raíz dueña, la ruta tal cual la conoce el
/// frontend y su lenguaje LSP (`None` si el archivo no tiene lenguaje LSP).
/// La clave `(raíz, lenguaje)` del manager da un servidor por raíz y lenguaje.
async fn resolve(state: &AppState, path: &str) -> AppResult<(PathBuf, PathBuf, Option<&'static str>)> {
    let roots = state.roots()?;
    let p = PathBuf::from(path);
    let pp = p.clone();
    let (_, root) = run_blocking(move || ensure_in_roots(&roots, &pp)).await?;
    let lang = language_for(&p);
    Ok((root, p, lang))
}

/// Cliente ya arrancado del archivo, si existe.
async fn existing(state: &AppState, path: &str) -> AppResult<Option<(std::sync::Arc<super::client::LspClient>, PathBuf)>> {
    let (root, p, lang) = resolve(state, path).await?;
    let Some(lang) = lang else { return Ok(None) };
    Ok(state.lsp.client(&root, lang).await.map(|c| (c, p)))
}

fn lsp_err(e: String) -> AppError {
    AppError::Lsp(e)
}

#[tauri::command]
pub async fn lsp_subscribe(state: State<'_, AppState>, on_event: Channel<LspEvent>) -> AppResult<()> {
    state.lsp.set_sink(Box::new(move |ev| on_event.send(ev).is_ok()));
    Ok(())
}

#[tauri::command]
pub async fn lsp_did_open(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    text: String,
) -> AppResult<LspDocStatus> {
    let (root, p, lang) = resolve(&state, &path).await?;
    let (Some(lang), Some(language_id)) = (lang, language_id_for(&p)) else {
        return Ok(LspDocStatus { language: None, server: None, available: false });
    };
    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir());
    match state.lsp.ensure_client(&root, lang, &data_dir).await {
        Ensured::Client(c) => {
            // No espera a `ready`: la apertura real se encola en el cliente.
            c.did_open(&p, language_id, text);
            Ok(LspDocStatus { language: Some(lang.into()), server: Some(c.server_name.clone()), available: true })
        }
        Ensured::Unavailable(st) => {
            Ok(LspDocStatus { language: Some(lang.into()), server: Some(st.server), available: false })
        }
    }
}

#[tauri::command]
pub async fn lsp_did_change(state: State<'_, AppState>, path: String, version: i32, text: String) -> AppResult<()> {
    if let Some((c, p)) = existing(&state, &path).await? {
        c.did_change(&p, version, text);
    }
    Ok(())
}

#[tauri::command]
pub async fn lsp_did_save(state: State<'_, AppState>, path: String, text: String) -> AppResult<()> {
    if let Some((c, p)) = existing(&state, &path).await? {
        c.did_save(&p, text);
    }
    Ok(())
}

#[tauri::command]
pub async fn lsp_did_close(state: State<'_, AppState>, path: String) -> AppResult<()> {
    if let Some((c, p)) = existing(&state, &path).await? {
        c.did_close(&p);
    }
    Ok(())
}

#[tauri::command]
pub async fn lsp_completion(
    state: State<'_, AppState>,
    path: String,
    line: u32,
    character: u32,
) -> AppResult<Vec<CompletionDto>> {
    match existing(&state, &path).await? {
        Some((c, p)) => c.completion(Path::new(&p), line, character).await.map_err(lsp_err),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub async fn lsp_hover(
    state: State<'_, AppState>,
    path: String,
    line: u32,
    character: u32,
) -> AppResult<Option<HoverDto>> {
    match existing(&state, &path).await? {
        Some((c, p)) => c.hover(&p, line, character).await.map_err(lsp_err),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn lsp_definition(
    state: State<'_, AppState>,
    path: String,
    line: u32,
    character: u32,
) -> AppResult<Vec<LocationDto>> {
    match existing(&state, &path).await? {
        Some((c, p)) => c.definition(&p, line, character).await.map_err(lsp_err),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub async fn lsp_status(state: State<'_, AppState>) -> AppResult<Vec<LspServerStatus>> {
    Ok(state.lsp.statuses().await)
}
