use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use crate::error::{AppError, AppResult};
use crate::fs::quick_open::FileIndex;
use crate::fs::symbols::SymbolCache;
use crate::fs::watcher::WatcherHandle;
use crate::git::repos::RepoEntry;
use crate::lsp::LspManager;
use crate::projects::{CancelHandle, ToolsReport};
use crate::services::manager::ServiceManager;
use crate::terminal::TerminalManager;
use crate::workspace::Workspace;

/// Estado global de la app. Cada bloque añade sus campos.
pub struct AppState {
    /// Workspace multi-raíz (`roots[0]` = primaria), rutas canonicalizadas.
    pub workspace: RwLock<Option<Workspace>>,
    /// Watcher de disco; drop = deja de vigilar.
    pub watcher: Mutex<Option<WatcherHandle>>,
    /// Caché de archivos para Quick Open (un índice por raíz).
    pub file_index: Mutex<FileIndex>,
    /// Generación de búsqueda global (cancelación cooperativa).
    pub search_gen: AtomicU64,
    /// Última vez que ejecutamos git en cada repo (anti-bucle con el watcher).
    pub git_last_run: Mutex<HashMap<PathBuf, Instant>>,
    /// Serializa las operaciones git mutadoras (index.lock, rebase...) POR repo.
    pub git_ops: Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>,
    /// >0 mientras hay una operación git mutadora en curso o en cola en ese repo: el watcher no emite `gitChanged` de él.
    pub git_busy: Mutex<HashMap<PathBuf, u32>>,
    /// Remotos en curso (fetch/pull/push) cancelables por `job_id`.
    pub git_jobs: Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>,
    /// Caché de la última detección de repos (`git_repos`): valida el parámetro `repo` sin ejecutar git.
    pub git_repos: Mutex<Vec<RepoEntry>>,
    /// Versión de git cacheada.
    pub git_version: Mutex<Option<String>>,
    /// Repos cuyo último `status` con `--untracked-files=all` tardó > 3 s: usar `normal`.
    pub git_untracked_normal: Mutex<HashSet<PathBuf>>,
    /// Caché del clasificador de símbolos Java/Kotlin (iconos del árbol).
    pub symbols: Mutex<SymbolCache>,
    /// Motor de microservicios (Bloque 3).
    pub services: Arc<ServiceManager>,
    /// Terminales PTY interactivas (Bloque 3).
    pub terminals: Arc<TerminalManager>,
    /// Servidores LSP (Bloque 4).
    pub lsp: Arc<LspManager>,
    /// Trabajos de creación de proyecto en curso (cancelables por `job_id`).
    pub project_jobs: Mutex<HashMap<String, CancelHandle>>,
    /// Última detección de herramientas (para `project_generators`).
    pub tools: Mutex<Option<ToolsReport>>,
    /// Archivos externos al workspace concedidos (rutas canónicas). SOLO los concede Rust
    /// (diálogo nativo, drag & drop, "abrir con", args CLI), nunca el frontend.
    pub external_allow: Mutex<HashSet<PathBuf>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            workspace: RwLock::new(None),
            watcher: Mutex::new(None),
            file_index: Mutex::new(FileIndex::new()),
            search_gen: AtomicU64::new(0),
            git_last_run: Mutex::new(HashMap::new()),
            git_ops: Mutex::new(HashMap::new()),
            git_busy: Mutex::new(HashMap::new()),
            git_jobs: Mutex::new(HashMap::new()),
            git_repos: Mutex::new(Vec::new()),
            git_version: Mutex::new(None),
            git_untracked_normal: Mutex::new(HashSet::new()),
            symbols: Mutex::new(SymbolCache::new()),
            services: Arc::new(ServiceManager::new()),
            terminals: Arc::new(TerminalManager::new()),
            lsp: Arc::new(LspManager::new()),
            project_jobs: Mutex::new(HashMap::new()),
            tools: Mutex::new(None),
            external_allow: Mutex::new(HashSet::new()),
        }
    }

    pub fn with_ws<T>(&self, f: impl FnOnce(&Workspace) -> T) -> AppResult<T> {
        let g = self.workspace.read().map_err(|_| AppError::Other("workspace lock envenenado".into()))?;
        g.as_ref().map(f).ok_or(AppError::NoWorkspace)
    }

    /// Raíz PRIMARIA del workspace actual o `NoWorkspace`.
    pub fn primary(&self) -> AppResult<PathBuf> {
        self.with_ws(|w| w.primary().map(Path::to_path_buf))?.ok_or(AppError::NoWorkspace)
    }

    /// Alias de `primary()` (módulos que aún no son multi-raíz).
    pub fn root(&self) -> AppResult<PathBuf> {
        self.primary()
    }

    /// Raíces que existen en disco (primaria primero).
    pub fn roots(&self) -> AppResult<Vec<PathBuf>> {
        self.with_ws(|w| w.existing())
    }

    /// Raíz existente que contiene `path` (prefijo más largo).
    #[cfg(test)]
    pub fn root_for(&self, path: &Path) -> AppResult<PathBuf> {
        self.with_ws(|w| w.root_for(path).map(|r| r.path.clone()))?
            .ok_or_else(|| AppError::OutsideWorkspace(path.display().to_string()))
    }

    /// Resuelve un parámetro `root: Option<String>` (None = primaria) a una raíz existente del workspace.
    pub fn resolve_root(&self, root: Option<&str>) -> AppResult<PathBuf> {
        match root.filter(|s| !s.is_empty()) {
            None => self.primary(),
            Some(r) => {
                let p = crate::util::paths::clean_canonicalize(Path::new(r)).unwrap_or_else(|_| PathBuf::from(r));
                self.with_ws(|w| w.roots.iter().find(|x| x.path == p).map(|x| x.path.clone()))?
                    .ok_or_else(|| AppError::OutsideWorkspace(r.to_string()))
            }
        }
    }

    /// Concede el acceso a un archivo externo (ruta canónica). Devuelve la ruta canónica.
    pub fn grant_external(&self, p: &Path) -> AppResult<PathBuf> {
        let c = crate::util::paths::clean_canonicalize(p)?;
        if !c.is_file() {
            return Err(AppError::InvalidArgument(format!("no es un archivo: {}", c.display())));
        }
        self.external_allow
            .lock()
            .map_err(|_| AppError::Other("external_allow envenenado".into()))?
            .insert(c.clone());
        Ok(c)
    }

    /// Valida `path` contra las raíces del workspace O las concesiones externas.
    /// Sin workspace abierto, solo valen las concesiones.
    pub fn resolve_file(&self, path: &str) -> AppResult<PathBuf> {
        let roots = self.roots().unwrap_or_default();
        let granted = self
            .external_allow
            .lock()
            .map_err(|_| AppError::Other("external_allow envenenado".into()))?;
        crate::util::paths::ensure_in_roots_or_granted(&roots, &granted, Path::new(path))
    }

    // ---- git por repo ----

    /// Anota que hemos ejecutado git en `top` (los eventos del watcher llegan ~200 ms después).
    pub fn git_mark_run(&self, top: &Path) {
        if let Ok(mut m) = self.git_last_run.lock() {
            m.insert(top.to_path_buf(), Instant::now());
        }
    }

    pub fn git_ran_within(&self, top: &Path, ms: u128) -> bool {
        self.git_last_run.lock().ok().and_then(|m| m.get(top).map(|t| t.elapsed().as_millis() < ms)).unwrap_or(false)
    }

    pub fn git_busy_for(&self, top: &Path) -> bool {
        self.git_busy.lock().ok().and_then(|m| m.get(top).copied()).unwrap_or(0) > 0
    }

    /// Suma de operaciones git en curso en todos los repos.
    #[cfg(test)]
    pub fn git_busy_total(&self) -> u32 {
        self.git_busy.lock().map(|m| m.values().sum()).unwrap_or(0)
    }

    /// Mutex de operaciones mutadoras del repo `top` (se crea bajo demanda).
    pub fn git_op_lock(&self, top: &Path) -> Arc<tokio::sync::Mutex<()>> {
        match self.git_ops.lock() {
            Ok(mut m) => m.entry(top.to_path_buf()).or_default().clone(),
            Err(_) => Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub fn git_untracked_limited(&self, top: &Path) -> bool {
        self.git_untracked_normal.lock().map(|s| s.contains(top)).unwrap_or(false)
    }

    /// Apagado síncrono (hilo principal): servicios, terminales, servidores LSP, watcher.
    pub fn shutdown(&self) {
        if let Ok(jobs) = self.project_jobs.lock() {
            for h in jobs.values() {
                let _ = h.send(true);
            }
        }
        self.services.shutdown();
        self.terminals.close_all();
        self.lsp.shutdown_blocking();
        if let Ok(mut w) = self.watcher.lock() {
            w.take();
        }
        if let Ok(mut ws) = self.workspace.write() {
            ws.take();
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
