use tauri::ipc::Channel;
use tauri::State;

use super::{detect, locate, AgentId, AgentInfo};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::terminal::TerminalEvent;
use crate::util::run_blocking;

#[tauri::command]
pub async fn agents_detect() -> AppResult<Vec<AgentInfo>> {
    run_blocking(|| Ok(detect())).await
}

/// Abre el agente en un PTY con cwd = raíz primaria y argv vacío. Devuelve el id de terminal
/// (usar `terminal_write/resize/close`).
#[tauri::command]
pub async fn agent_open(
    state: State<'_, AppState>,
    agent: AgentId,
    cols: u16,
    rows: u16,
    on_event: Channel<TerminalEvent>,
) -> AppResult<String> {
    let primary = state.primary()?;
    let terms = state.terminals.clone();
    run_blocking(move || {
        let bin = locate(agent).ok_or_else(|| AppError::NotFound(format!("{} no está en el PATH", agent.bin())))?;
        terms.open_program(&primary, &bin, agent.argv(), cols, rows, Box::new(move |ev| on_event.send(ev).is_ok()))
    })
    .await
}
