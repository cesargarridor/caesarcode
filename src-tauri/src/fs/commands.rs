use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use super::symbols::{self, SymbolInfo};
use super::{ops, quick_open, search, tree, watcher};
use super::{FileContent, FsEntry, FsEvent, QuickOpenItem, SearchOptions, SearchResult};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::util::paths::{ensure_in_roots, ensure_in_roots_nofollow};
use crate::util::run_blocking;

/// Raíces existentes con su nombre visible (Quick Open y búsqueda global).
fn named_roots(state: &AppState) -> AppResult<Vec<(PathBuf, String)>> {
    state.with_ws(|w| w.roots.iter().filter(|r| r.exists).map(|r| (r.path.clone(), r.name.clone())).collect())
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[tauri::command]
pub async fn fs_read_dir(state: State<'_, AppState>, path: String) -> AppResult<Vec<FsEntry>> {
    let roots = state.roots()?;
    run_blocking(move || {
        let (p, _) = ensure_in_roots(&roots, Path::new(&path))?;
        tree::read_dir(&p)
    })
    .await
}

#[tauri::command]
pub async fn fs_read_file(state: State<'_, AppState>, path: String) -> AppResult<FileContent> {
    let p = state.resolve_file(&path)?;
    run_blocking(move || ops::read_file(&p))
    .await
}

#[tauri::command]
pub async fn fs_write_file(state: State<'_, AppState>, path: String, text: String) -> AppResult<()> {
    let p = state.resolve_file(&path)?;
    run_blocking(move || ops::write_file(&p, &text))
    .await
}

#[tauri::command]
pub async fn fs_create_file(state: State<'_, AppState>, parent: String, name: String) -> AppResult<String> {
    let roots = state.roots()?;
    run_blocking(move || {
        let (dir, _) = ensure_in_roots(&roots, Path::new(&parent))?;
        Ok(s(&ops::create_file(&dir, &name)?))
    })
    .await
}

#[tauri::command]
pub async fn fs_create_dir(state: State<'_, AppState>, parent: String, name: String) -> AppResult<String> {
    let roots = state.roots()?;
    run_blocking(move || {
        let (dir, _) = ensure_in_roots(&roots, Path::new(&parent))?;
        Ok(s(&ops::create_dir(&dir, &name)?))
    })
    .await
}

#[tauri::command]
pub async fn fs_rename(state: State<'_, AppState>, path: String, new_name: String) -> AppResult<String> {
    let roots = state.roots()?;
    run_blocking(move || {
        let (p, root) = ensure_in_roots_nofollow(&roots, Path::new(&path))?;
        if p == root {
            return Err(AppError::InvalidArgument("no se puede renombrar la raíz del workspace".into()));
        }
        Ok(s(&ops::rename(&p, &new_name)?))
    })
    .await
}

#[tauri::command]
pub async fn fs_delete(state: State<'_, AppState>, path: String) -> AppResult<()> {
    let roots = state.roots()?;
    run_blocking(move || {
        let (p, root) = ensure_in_roots_nofollow(&roots, Path::new(&path))?;
        if p == root {
            return Err(AppError::InvalidArgument("no se puede borrar la raíz del workspace".into()));
        }
        ops::delete(&p)
    })
    .await
}

#[tauri::command]
pub async fn fs_watch_subscribe(
    app: AppHandle,
    state: State<'_, AppState>,
    on_event: Channel<FsEvent>,
) -> AppResult<()> {
    let roots = state.roots()?;
    let handle = run_blocking(move || watcher::start(app, roots, on_event)).await?;
    // Reemplazo: el handle anterior se dropea aquí y deja de vigilar.
    *state
        .watcher
        .lock()
        .map_err(|_| AppError::Other("lock envenenado".into()))? = Some(handle);
    Ok(())
}

#[tauri::command]
pub async fn quick_open(
    app: AppHandle,
    state: State<'_, AppState>,
    query: String,
    limit: u32,
) -> AppResult<Vec<QuickOpenItem>> {
    let roots = named_roots(&state)?;
    run_blocking(move || {
        let state = app.state::<AppState>();
        Ok(quick_open::query(&state, &roots, &query, limit.clamp(1, 500) as usize))
    })
    .await
}

#[tauri::command]
pub async fn search_global(
    app: AppHandle,
    state: State<'_, AppState>,
    query: String,
    options: SearchOptions,
) -> AppResult<SearchResult> {
    let roots = named_roots(&state)?;
    // Invalida cualquier búsqueda en curso antes de lanzar la nueva.
    let gen = state.search_gen.fetch_add(1, Ordering::SeqCst) + 1;
    run_blocking(move || {
        let state = app.state::<AppState>();
        search::search(&roots, &query, &options, gen, &state.search_gen)
    })
    .await
}

/// Clasifica en lote (≤ 512 rutas) archivos Java/Kotlin visibles para los iconos del árbol.
/// Rutas fuera del workspace o no soportadas se omiten sin error.
#[tauri::command]
pub async fn fs_classify(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> AppResult<Vec<(String, SymbolInfo)>> {
    let roots = state.roots()?;
    run_blocking(move || {
        let state = app.state::<AppState>();
        Ok(symbols::classify_paths_in(&state.symbols, &roots, &paths))
    })
    .await
}
