//! Archivos externos al workspace: SOLO Rust concede el acceso (diálogo nativo, drag & drop,
//! "Abrir con", args CLI). Cada concesión emite `external-open { path }` al frontend.
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::error::AppResult;
use crate::state::AppState;
use crate::util::run_blocking;

#[derive(Serialize, Clone)]
struct ExternalOpen {
    path: String,
}

/// Concede cada archivo (ignora directorios y rutas inexistentes) y emite `external-open`.
/// Devuelve las rutas canónicas concedidas.
pub fn grant_and_emit(app: &AppHandle, paths: impl IntoIterator<Item = PathBuf>) -> Vec<String> {
    let state = app.state::<AppState>();
    let mut out = Vec::new();
    for p in paths {
        if let Ok(c) = state.grant_external(&p) {
            let s = c.to_string_lossy().into_owned();
            let _ = app.emit("external-open", ExternalOpen { path: s.clone() });
            out.push(s);
        }
    }
    out
}

/// Argumentos de línea de comandos que son archivos (`CaesarCode archivo.txt`).
pub fn cli_paths() -> Vec<PathBuf> {
    std::env::args_os().skip(1).map(PathBuf::from).filter(|p| !p.to_string_lossy().starts_with('-') && Path::new(p).is_file()).collect()
}

/// Diálogo nativo (selección múltiple). Concede y emite `external-open` por archivo; también
/// devuelve las rutas (el frontend debe deduplicar por ruta si escucha ambos).
#[tauri::command]
pub async fn external_open_dialog(app: AppHandle, _state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let a = app.clone();
    let picked = run_blocking(move || {
        Ok(a.dialog().file().blocking_pick_files().unwrap_or_default().into_iter().filter_map(|f| f.into_path().ok()).collect::<Vec<_>>())
    })
    .await?;
    if picked.is_empty() {
        return Ok(Vec::new());
    }
    Ok(grant_and_emit(&app, picked))
}
