use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use super::FsEvent;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Ventana anti-bucle: no emitir `gitChanged` justo después de ejecutar git nosotros.
const GIT_QUIET_MS: u128 = 700;

/// Mientras exista, se vigilan las raíces del workspace; al hacer drop se deja de vigilar.
pub struct WatcherHandle {
    debouncer: Debouncer<RecommendedWatcher>,
}

impl WatcherHandle {
    /// Empieza a vigilar una raíz más (mismo debouncer).
    pub fn watch_root(&mut self, root: &Path) -> AppResult<()> {
        self.debouncer
            .watcher()
            .watch(root, RecursiveMode::Recursive)
            .map_err(|e| AppError::Watch(e.to_string()))
    }

    pub fn unwatch_root(&mut self, root: &Path) {
        let _ = self.debouncer.watcher().unwatch(root);
    }
}

#[derive(Debug, PartialEq)]
enum Kind {
    Ignore,
    /// Cambio en `.git` de un repo; lleva la ruta del repo relativa a la raíz (padre de `.git`).
    Git(PathBuf),
    Path,
}

fn classify(rel: &Path) -> Kind {
    let comps: Vec<Component> = rel.components().collect();
    if let Some(i) = comps.iter().position(|c| c.as_os_str() == ".git") {
        let first = comps.get(i + 1).map(|c| c.as_os_str().to_string_lossy().into_owned());
        return match first.as_deref() {
            Some("HEAD") | Some("index") | Some("refs") => Kind::Git(comps[..i].iter().collect()),
            _ => Kind::Ignore,
        };
    }
    let tmp = rel
        .file_name()
        .map(|n| n.to_string_lossy().contains(".cc-tmp-"))
        .unwrap_or(false);
    if tmp {
        Kind::Ignore
    } else {
        Kind::Path
    }
}

/// ¿Se debe emitir `gitChanged` de este repo? No si acabamos de ejecutar git en él o hay una operación en curso.
/// Es por repo: un pull lento en A no silencia los eventos de B.
pub fn git_event_allowed(state: &AppState, repo: &Path) -> bool {
    !state.git_ran_within(repo, GIT_QUIET_MS) && !state.git_busy_for(repo)
}

fn send(slot: &Mutex<Option<Channel<FsEvent>>>, ev: FsEvent) {
    if let Ok(mut guard) = slot.lock() {
        if let Some(ch) = guard.as_ref() {
            if ch.send(ev).is_err() {
                // Webview recargado / canal cerrado: se descarta la suscripción.
                *guard = None;
            }
        }
    }
}

fn handle(app: &AppHandle, slot: &Mutex<Option<Channel<FsEvent>>>, res: DebounceEventResult) {
    let events = match res {
        Ok(e) => e,
        Err(e) => {
            send(slot, FsEvent::Error { message: e.to_string() });
            return;
        }
    };
    let state = app.state::<AppState>();
    let roots = state.roots().unwrap_or_default();
    let mut paths: BTreeSet<String> = BTreeSet::new();
    let mut repos: BTreeSet<PathBuf> = BTreeSet::new();
    let mut dirty_roots: BTreeSet<PathBuf> = BTreeSet::new();
    let mut roots_changed = false;
    for ev in &events {
        let Some(root) = roots.iter().filter(|r| ev.path.starts_with(r)).max_by_key(|r| r.components().count()) else {
            continue;
        };
        let rel = ev.path.strip_prefix(root).unwrap_or(&ev.path);
        if rel.as_os_str().is_empty() {
            // Evento sobre la propia raíz (borrada o renombrada).
            roots_changed = true;
            continue;
        }
        match classify(rel) {
            Kind::Ignore => {}
            Kind::Git(repo_rel) => {
                repos.insert(root.join(repo_rel));
            }
            Kind::Path => {
                dirty_roots.insert(root.clone());
                paths.insert(ev.path.to_string_lossy().into_owned());
            }
        }
    }
    if !paths.is_empty() {
        if let Ok(mut idx) = state.file_index.lock() {
            for r in &dirty_roots {
                idx.mark_root_dirty(r);
            }
        }
        send(slot, FsEvent::Changed { paths: paths.into_iter().collect() });
    }
    for repo in repos {
        if git_event_allowed(&state, &repo) {
            send(slot, FsEvent::GitChanged { repo: Some(repo.to_string_lossy().into_owned()) });
        }
    }
    if roots_changed {
        send(slot, FsEvent::RootsChanged);
    }
}

/// Un único debouncer con `watch()` por raíz. Una raíz que no se puede vigilar (límite de inotify...)
/// se registra en el log y se avisa por el canal, sin abortar.
pub fn start(app: AppHandle, roots: Vec<PathBuf>, channel: Channel<FsEvent>) -> AppResult<WatcherHandle> {
    let slot = Arc::new(Mutex::new(Some(channel)));
    let slot_cb = slot.clone();
    let debouncer = new_debouncer(Duration::from_millis(200), move |res: DebounceEventResult| {
        handle(&app, &slot_cb, res);
    })
    .map_err(|e| AppError::Watch(e.to_string()))?;
    let mut handle = WatcherHandle { debouncer };
    for root in &roots {
        if let Err(e) = handle.watch_root(root) {
            log::warn!("no se pudo vigilar {}: {e}", root.display());
            send(&slot, FsEvent::Error { message: format!("no se pudo vigilar {}: {e}", root.display()) });
        }
    }
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_git_events_per_repo() {
        assert_eq!(classify(Path::new(".git/HEAD")), Kind::Git(PathBuf::new()));
        assert_eq!(classify(Path::new("svc/vendor/lib/.git/refs/heads/x")), Kind::Git(PathBuf::from("svc/vendor/lib")));
        assert_eq!(classify(Path::new("a/.git/objects/ab")), Kind::Ignore);
        assert_eq!(classify(Path::new("a/b.rs")), Kind::Path);
        assert_eq!(classify(Path::new("a/b.rs.cc-tmp-1")), Kind::Ignore);
    }

    #[test]
    fn busy_repo_does_not_silence_other_repo() {
        use crate::git::exec::BusyGuard;
        let st = AppState::new();
        let (a, b) = (PathBuf::from("/w/a"), PathBuf::from("/w/b"));
        assert!(git_event_allowed(&st, &a) && git_event_allowed(&st, &b));
        {
            let _g = BusyGuard::new(&st, &a);
            assert!(!git_event_allowed(&st, &a));
            assert!(git_event_allowed(&st, &b));
        }
        // al soltar, A queda en ventana anti-bucle; B sigue libre
        assert!(!git_event_allowed(&st, &a));
        assert!(git_event_allowed(&st, &b));
    }
}
