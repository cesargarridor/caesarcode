use std::path::{Path, PathBuf};

use tauri::{AppHandle, State};

use super::{store, validate, RootInfo, Workspace, WorkspaceInfo};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::util::paths::clean_canonicalize;
use crate::util::run_blocking;

fn lock_err<T>(_: T) -> AppError {
    AppError::Other("lock envenenado".into())
}

fn canon_dir(path: &str) -> AppResult<PathBuf> {
    let p = PathBuf::from(path);
    if !p.exists() {
        return Err(AppError::NotFound(path.to_string()));
    }
    if !p.is_dir() {
        return Err(AppError::InvalidArgument(format!("no es una carpeta: {path}")));
    }
    Ok(clean_canonicalize(&p)?)
}

// ---------------------------------------------------------------------------
// Lógica de estado (síncrona, sin AppHandle: testeable en headless)
// ---------------------------------------------------------------------------

/// Abre `primary` restaurando las raíces adjuntas persistidas. Una raíz persistida que ya no existe se
/// conserva con `exists: false` (nunca se borra sola: puede ser un disco desmontado). Los solapes con
/// otras raíces se descartan en silencio.
pub fn open_state(state: &AppState, data_dir: &Path, primary: PathBuf) -> AppResult<Workspace> {
    let mut paths = vec![primary.clone()];
    for p in store::load(data_dir, &primary) {
        let cand = if p.is_dir() { clean_canonicalize(&p).unwrap_or(p) } else { p };
        if cand.exists() {
            match validate::check_new_root(&paths, &cand) {
                Ok(c) => paths.push(c),
                Err(e) => log::warn!("raíz persistida descartada ({}): {e}", cand.display()),
            }
        } else if !paths.contains(&cand) {
            paths.push(cand);
        }
    }
    let ws = Workspace::new(paths);
    state.file_index.lock().map_err(lock_err)?.reset();
    state.symbols.lock().map_err(lock_err)?.clear();
    state.git_repos.lock().map_err(lock_err)?.clear();
    *state.workspace.write().map_err(lock_err)? = Some(ws.clone());
    Ok(ws)
}

/// Añade una raíz (reglas 0.4), persiste y devuelve `(raíz canónica, workspace nuevo)`.
pub fn add_root_state(state: &AppState, data_dir: &Path, path: &Path) -> AppResult<(PathBuf, Workspace)> {
    let mut guard = state.workspace.write().map_err(lock_err)?;
    let ws = guard.as_mut().ok_or(AppError::NoWorkspace)?;
    let mut paths = ws.all_paths();
    let canon = validate::check_new_root(&paths, path)?;
    paths.push(canon.clone());
    let primary = paths[0].clone();
    *ws = Workspace::new(paths.clone());
    let snapshot = ws.clone();
    drop(guard);
    if let Err(e) = store::save(data_dir, &primary, &paths[1..]) {
        log::warn!("no se pudo persistir el workspace: {e}");
    }
    state.git_repos.lock().map_err(lock_err)?.clear();
    Ok((canon, snapshot))
}

/// Quita una raíz adjunta (nunca la primaria) y persiste. Devuelve `(raíz quitada, workspace nuevo)`.
pub fn remove_root_state(state: &AppState, data_dir: &Path, path: &str) -> AppResult<(PathBuf, Workspace)> {
    let mut guard = state.workspace.write().map_err(lock_err)?;
    let ws = guard.as_mut().ok_or(AppError::NoWorkspace)?;
    let wanted = clean_canonicalize(Path::new(path)).unwrap_or_else(|_| PathBuf::from(path));
    let idx = ws
        .roots
        .iter()
        .position(|r| r.path == wanted)
        .ok_or_else(|| AppError::NotFound(format!("raíz {path}")))?;
    if idx == 0 {
        return Err(AppError::InvalidArgument("la raíz principal no se puede quitar; abre otra carpeta".into()));
    }
    let removed = ws.roots[idx].path.clone();
    let mut paths = ws.all_paths();
    paths.remove(idx);
    let primary = paths[0].clone();
    *ws = Workspace::new(paths.clone());
    let snapshot = ws.clone();
    drop(guard);
    if let Err(e) = store::save(data_dir, &primary, &paths[1..]) {
        log::warn!("no se pudo persistir el workspace: {e}");
    }
    if let Ok(mut idx) = state.file_index.lock() {
        idx.remove_root(&removed);
    }
    if let Ok(mut s) = state.symbols.lock() {
        s.clear_under(&removed);
    }
    if let Ok(mut g) = state.git_repos.lock() {
        g.retain(|e| !e.top.starts_with(&removed) && e.root != removed);
    }
    Ok((removed, snapshot))
}

/// Recalcula `exists` de cada raíz y devuelve la lista.
pub fn roots_state(state: &AppState) -> AppResult<Vec<RootInfo>> {
    let mut guard = state.workspace.write().map_err(lock_err)?;
    let ws = guard.as_mut().ok_or(AppError::NoWorkspace)?;
    ws.refresh();
    Ok(ws.roots.iter().map(RootInfo::from).collect())
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn workspace_open(app: AppHandle, state: State<'_, AppState>, path: String) -> AppResult<WorkspaceInfo> {
    let root: PathBuf = run_blocking(move || canon_dir(&path)).await?;
    let data = crate::projects::commands::data_dir(&app);

    // Servidores LSP del workspace anterior: se olvidan ya y se apagan en segundo plano.
    let old = state.lsp.take_all().await;
    if !old.is_empty() {
        tokio::spawn(crate::lsp::LspManager::shutdown_clients(old, std::time::Duration::from_secs(1)));
    }
    // Soltar el watcher previo (drop = deja de vigilar).
    state.watcher.lock().map_err(lock_err)?.take();

    let ws = open_state(&state, &data, root.clone())?;
    // Servicios: se paran los de un workspace anterior; si es la misma primaria se conservan.
    let services = state.services.clone();
    let paths = ws.all_paths();
    run_blocking(move || services.set_roots(paths)).await?;

    // Recientes: un fallo al persistir nunca rompe la apertura.
    let recent_root = root.clone();
    let _ = run_blocking(move || {
        if let Err(e) = crate::projects::recent::touch(&data, &recent_root) {
            log::warn!("no se pudo actualizar proyectos recientes: {e}");
        }
        Ok(())
    })
    .await;
    Ok(WorkspaceInfo::from_workspace(&ws))
}

#[tauri::command]
pub async fn workspace_current(state: State<'_, AppState>) -> AppResult<Option<WorkspaceInfo>> {
    match state.with_ws(WorkspaceInfo::from_workspace) {
        Ok(info) => Ok(Some(info)),
        Err(AppError::NoWorkspace) => Ok(None),
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn workspace_roots(state: State<'_, AppState>) -> AppResult<Vec<RootInfo>> {
    // Solo comprueba `is_dir` de unas pocas raíces: sin I/O pesado.
    roots_state(&state)
}

#[tauri::command]
pub async fn workspace_add_root(app: AppHandle, state: State<'_, AppState>, path: String) -> AppResult<WorkspaceInfo> {
    let data = crate::projects::commands::data_dir(&app);
    let (canon, ws) = add_root_state(&state, &data, Path::new(&path))?;
    // Un único debouncer: se añade un `watch` más si el watcher está vivo.
    if let Some(w) = state.watcher.lock().map_err(lock_err)?.as_mut() {
        if let Err(e) = w.watch_root(&canon) {
            log::warn!("no se pudo vigilar {}: {e}", canon.display());
        }
    }
    let services = state.services.clone();
    let paths = ws.all_paths();
    run_blocking(move || services.set_roots(paths)).await?;
    Ok(WorkspaceInfo::from_workspace(&ws))
}

#[tauri::command]
pub async fn workspace_remove_root(app: AppHandle, state: State<'_, AppState>, path: String) -> AppResult<WorkspaceInfo> {
    let data = crate::projects::commands::data_dir(&app);
    let (removed, ws) = remove_root_state(&state, &data, &path)?;
    if let Some(w) = state.watcher.lock().map_err(lock_err)?.as_mut() {
        w.unwatch_root(&removed);
    }
    // Servicios (y logs/efímeros) solo de la raíz quitada; las demás siguen vivas.
    let services = state.services.clone();
    let paths = ws.all_paths();
    run_blocking(move || services.set_roots(paths)).await?;
    // Clientes LSP de esa raíz.
    let old = state.lsp.take_root(&removed).await;
    if !old.is_empty() {
        tokio::spawn(crate::lsp::LspManager::shutdown_clients(old, std::time::Duration::from_secs(1)));
    }
    Ok(WorkspaceInfo::from_workspace(&ws))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fx {
        _d: tempfile::TempDir,
        base: PathBuf,
        data: PathBuf,
        state: AppState,
    }

    fn fx() -> Fx {
        let d = tempfile::tempdir().unwrap();
        let base = clean_canonicalize(d.path()).unwrap();
        for n in ["p", "a", "b"] {
            std::fs::create_dir_all(base.join(n)).unwrap();
        }
        std::fs::create_dir_all(base.join("a/inner")).unwrap();
        Fx { data: base.join("data"), base, state: AppState::new(), _d: d }
    }

    #[test]
    fn add_remove_rules_and_primary_protected() {
        let f = fx();
        let p = f.base.join("p");
        open_state(&f.state, &f.data, p.clone()).unwrap();
        let (_, ws) = add_root_state(&f.state, &f.data, &f.base.join("a")).unwrap();
        assert_eq!(ws.roots.len(), 2);
        add_root_state(&f.state, &f.data, &f.base.join("b")).unwrap();
        // duplicado, dentro de otra raíz, contiene a otra
        assert!(add_root_state(&f.state, &f.data, &f.base.join("a")).is_err());
        assert!(add_root_state(&f.state, &f.data, &f.base.join("a/inner")).is_err());
        assert!(add_root_state(&f.state, &f.data, &f.base).is_err());
        // la primaria no se quita
        let e = remove_root_state(&f.state, &f.data, p.to_str().unwrap()).unwrap_err().to_string();
        assert!(e.contains("principal"), "{e}");
        let (removed, ws) = remove_root_state(&f.state, &f.data, f.base.join("a").to_str().unwrap()).unwrap();
        assert_eq!(removed, f.base.join("a"));
        assert_eq!(ws.roots.len(), 2);
        assert!(remove_root_state(&f.state, &f.data, f.base.join("a").to_str().unwrap()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_to_existing_root_rejected() {
        let f = fx();
        open_state(&f.state, &f.data, f.base.join("p")).unwrap();
        add_root_state(&f.state, &f.data, &f.base.join("a")).unwrap();
        let link = f.base.join("lnk");
        std::os::unix::fs::symlink(f.base.join("a"), &link).unwrap();
        assert!(add_root_state(&f.state, &f.data, &link).is_err());
    }

    #[test]
    fn persistence_restores_roots_and_marks_missing() {
        let f = fx();
        let p = f.base.join("p");
        open_state(&f.state, &f.data, p.clone()).unwrap();
        add_root_state(&f.state, &f.data, &f.base.join("a")).unwrap();
        add_root_state(&f.state, &f.data, &f.base.join("b")).unwrap();
        // otra "sesión": estado nuevo, mismo data dir
        std::fs::remove_dir_all(f.base.join("b")).unwrap();
        let st2 = AppState::new();
        let ws = open_state(&st2, &f.data, p.clone()).unwrap();
        assert_eq!(ws.roots.len(), 3);
        assert_eq!(ws.roots[0].path, p);
        assert!(ws.roots[1].exists && !ws.roots[2].exists);
        // no se borra del JSON aunque no exista
        assert_eq!(store::load(&f.data, &p).len(), 2);
        // roots() excluye las inexistentes
        assert_eq!(st2.roots().unwrap(), vec![p.clone(), f.base.join("a")]);
        // la lista se recalcula al pedirla
        std::fs::create_dir_all(f.base.join("b")).unwrap();
        let infos = roots_state(&st2).unwrap();
        assert!(infos.iter().all(|r| r.exists));
        // abrir una carpeta sin entrada = comportamiento actual (solo primaria)
        let st3 = AppState::new();
        assert_eq!(open_state(&st3, &f.data, f.base.join("a")).unwrap().roots.len(), 1);
    }

    #[test]
    fn root_for_uses_longest_prefix_and_rejects_outside() {
        let f = fx();
        open_state(&f.state, &f.data, f.base.join("p")).unwrap();
        add_root_state(&f.state, &f.data, &f.base.join("a")).unwrap();
        assert_eq!(f.state.root_for(&f.base.join("a/x/y.txt")).unwrap(), f.base.join("a"));
        assert_eq!(f.state.root_for(&f.base.join("p")).unwrap(), f.base.join("p"));
        assert!(f.state.root_for(&f.base.join("b/z")).is_err());
        assert_eq!(f.state.resolve_root(None).unwrap(), f.base.join("p"));
        assert_eq!(f.state.resolve_root(Some(f.base.join("a").to_str().unwrap())).unwrap(), f.base.join("a"));
        assert!(f.state.resolve_root(Some(f.base.join("b").to_str().unwrap())).is_err());
    }
}
