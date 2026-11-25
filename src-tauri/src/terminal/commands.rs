use std::path::PathBuf;

use tauri::ipc::Channel;
use tauri::State;

use super::TerminalEvent;
use crate::error::AppResult;
use crate::state::AppState;
use crate::util::paths::ensure_in_roots;
use crate::util::run_blocking;

#[tauri::command]
pub async fn terminal_open(
    state: State<'_, AppState>,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
    on_event: Channel<TerminalEvent>,
) -> AppResult<String> {
    let roots = state.roots()?;
    let primary = state.primary()?;
    let terms = state.terminals.clone();
    run_blocking(move || {
        let dir: PathBuf = match cwd.as_deref().filter(|s| !s.is_empty()) {
            Some(c) => ensure_in_roots(&roots, std::path::Path::new(c))?.0,
            None => primary,
        };
        terms.open(&dir, cols, rows, Box::new(move |ev| on_event.send(ev).is_ok()))
    })
    .await
}

#[tauri::command]
pub async fn terminal_write(state: State<'_, AppState>, id: String, data: String) -> AppResult<()> {
    state.terminals.write(&id, &data)
}

#[tauri::command]
pub async fn terminal_resize(state: State<'_, AppState>, id: String, cols: u16, rows: u16) -> AppResult<()> {
    state.terminals.resize(&id, cols, rows)
}

#[tauri::command]
pub async fn terminal_close(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let terms = state.terminals.clone();
    run_blocking(move || terms.close(&id)).await
}
