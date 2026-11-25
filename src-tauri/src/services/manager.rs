use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use super::logs::LogStore;
use super::metrics::Sampler;
use super::ports::is_port_open;
use super::process_tree::{kill_group_best_effort, kill_tree};
use super::{config, now_ms, LogLine, ServiceConfig, ServiceState, ServiceStatus};
use crate::error::{AppError, AppResult};
use crate::util::paths::ensure_in_workspace;
use crate::workspace::display_names;

/// Envía un lote; devuelve `false` si el receptor desapareció (se descarta la suscripción).
pub type LogSink = Box<dyn Fn(Vec<LogLine>) -> bool + Send + Sync>;
pub type StatusSink = Box<dyn Fn(Vec<ServiceStatus>) -> bool + Send + Sync>;

const KILL_GRACE: Duration = Duration::from_secs(3);
const LOG_BATCH: usize = 2000;

struct Runtime {
    run_id: u64,
    state: ServiceState,
    pid: Option<u32>,
    child: Option<Box<dyn Child + Send + Sync>>,
    master: Option<Box<dyn MasterPty + Send>>,
    started_at: Option<u64>,
    exit_code: Option<i32>,
    user_stopped: bool,
    message: Option<String>,
    cpu: f32,
    memory_mb: f64,
}

impl Runtime {
    fn busy(&self) -> bool {
        matches!(self.state, ServiceState::Starting | ServiceState::Running | ServiceState::Stopping)
    }
}

/// Servicio con la raíz a la que pertenece. `cfg.id` es el id VISIBLE (único entre raíces);
/// `raw_id` es el que se persiste en `services.json`.
#[derive(Clone)]
struct Loaded {
    root: PathBuf,
    raw_id: String,
    cfg: ServiceConfig,
}

/// Id visible: el crudo en la raíz primaria (`roots[0]`); `"<nombreRaíz>/<id>"` en las demás.
fn visible_id(roots: &[PathBuf], root: &Path, raw: &str) -> String {
    match roots.iter().position(|r| r == root) {
        Some(0) | None => raw.to_string(),
        Some(i) => format!("{}/{raw}", display_names(roots)[i]),
    }
}

/// Id visible de un servicio efímero: conserva el prefijo (`file:`, `run:`, `auto:`) para no romper al frontend.
fn visible_ephemeral_id(roots: &[PathBuf], root: &Path, raw: &str) -> String {
    match roots.iter().position(|r| r == root) {
        Some(0) | None => raw.to_string(),
        Some(i) => {
            let name = &display_names(roots)[i];
            match raw.split_once(':') {
                Some((prefix, rest)) => format!("{prefix}:{name}/{rest}"),
                None => format!("{name}/{raw}"),
            }
        }
    }
}

pub struct ServiceManager {
    /// `roots[0]` = primaria.
    roots: RwLock<Vec<PathBuf>>,
    configs: Mutex<Vec<Loaded>>,
    ephemeral: Mutex<Vec<Loaded>>,
    runtimes: Mutex<HashMap<String, Runtime>>,
    port_open: Mutex<HashMap<String, bool>>,
    logs: Mutex<LogStore>,
    log_sink: Mutex<Option<LogSink>>,
    status_sink: Mutex<Option<StatusSink>>,
    loops_started: AtomicBool,
    shutting_down: AtomicBool,
    next_run: AtomicU64,
}

fn poisoned<T>(_: T) -> AppError {
    AppError::Service("lock envenenado".into())
}

impl ServiceManager {
    pub fn new() -> Self {
        Self {
            roots: RwLock::new(Vec::new()),
            configs: Mutex::new(Vec::new()),
            ephemeral: Mutex::new(Vec::new()),
            runtimes: Mutex::new(HashMap::new()),
            port_open: Mutex::new(HashMap::new()),
            logs: Mutex::new(LogStore::new()),
            log_sink: Mutex::new(None),
            status_sink: Mutex::new(None),
            loops_started: AtomicBool::new(false),
            shutting_down: AtomicBool::new(false),
            next_run: AtomicU64::new(1),
        }
    }

    // ---------- workspace / configuración ----------

    fn roots(&self) -> AppResult<Vec<PathBuf>> {
        let r = self.roots.read().map_err(poisoned)?.clone();
        if r.is_empty() {
            Err(AppError::NoWorkspace)
        } else {
            Ok(r)
        }
    }

    #[cfg(test)]
    fn primary(&self) -> AppResult<PathBuf> {
        Ok(self.roots()?.remove(0))
    }

    /// Una raíz del workspace (None = primaria) o `OutsideWorkspace`.
    fn check_root(&self, root: &Path) -> AppResult<Vec<PathBuf>> {
        let roots = self.roots()?;
        if roots.iter().any(|r| r == root) {
            Ok(roots)
        } else {
            Err(AppError::OutsideWorkspace(root.display().to_string()))
        }
    }

    fn load_root(roots: &[PathBuf], root: &Path) -> AppResult<Vec<Loaded>> {
        Ok(config::load(root)?
            .into_iter()
            .map(|mut c| {
                let raw_id = c.id.clone();
                c.id = visible_id(roots, root, &raw_id);
                Loaded { root: root.to_path_buf(), raw_id, cfg: c }
            })
            .collect())
    }

    /// Cambia de workspace (una sola raíz): para todo lo del anterior y recarga la configuración.
    /// Reabrir el MISMO root (p. ej. recarga del webview) conserva servicios y logs en marcha.
    #[cfg(test)]
    pub fn set_workspace(&self, root: PathBuf) -> AppResult<()> {
        self.set_roots(vec![root])
    }

    /// Fija las raíces del workspace (`roots[0]` = primaria) y recarga la configuración de todas.
    /// - Otra primaria (o primer uso): se para y limpia todo lo anterior.
    /// - Misma primaria: solo se paran servicios/logs/efímeros de las raíces que desaparecen
    ///   (o cuyo nombre visible cambia, porque sus ids cambiarían); las demás siguen en marcha.
    pub fn set_roots(&self, new: Vec<PathBuf>) -> AppResult<()> {
        if new.is_empty() {
            return Err(AppError::NoWorkspace);
        }
        let old = self.roots.read().map_err(poisoned)?.clone();
        if old.first() != new.first() {
            self.stop_all_blocking();
            if let Ok(mut rt) = self.runtimes.lock() {
                rt.clear();
            }
            if let Ok(mut p) = self.port_open.lock() {
                p.clear();
            }
            if let Ok(mut l) = self.logs.lock() {
                l.clear(None);
            }
            if let Ok(mut e) = self.ephemeral.lock() {
                e.clear();
            }
        } else {
            let (old_names, new_names) = (display_names(&old), display_names(&new));
            let gone: Vec<PathBuf> = old
                .iter()
                .enumerate()
                .filter(|(i, r)| match new.iter().position(|n| n == *r) {
                    None => true,
                    Some(j) => *i != 0 && old_names[*i] != new_names[j],
                })
                .map(|(_, r)| r.clone())
                .collect();
            self.drop_roots(&gone);
        }
        let mut loaded = Vec::new();
        for r in &new {
            loaded.extend(Self::load_root(&new, r).unwrap_or_default());
        }
        *self.roots.write().map_err(poisoned)? = new;
        *self.configs.lock().map_err(poisoned)? = loaded;
        self.emit_status();
        Ok(())
    }

    /// Para y olvida (runtimes, logs, puertos, efímeros) todo servicio de las raíces dadas.
    fn drop_roots(&self, gone: &[PathBuf]) {
        if gone.is_empty() {
            return;
        }
        let ids: Vec<String> = self
            .all_loaded()
            .into_iter()
            .filter(|(l, _)| gone.contains(&l.root))
            .map(|(l, _)| l.cfg.id)
            .collect();
        std::thread::scope(|s| {
            for id in &ids {
                s.spawn(move || {
                    let _ = self.stop_blocking(id);
                });
            }
        });
        for id in &ids {
            if let Ok(mut rt) = self.runtimes.lock() {
                rt.remove(id);
            }
            if let Ok(mut p) = self.port_open.lock() {
                p.remove(id);
            }
            if let Ok(mut l) = self.logs.lock() {
                l.clear(Some(id));
            }
        }
        if let Ok(mut e) = self.ephemeral.lock() {
            e.retain(|l| !gone.contains(&l.root));
        }
        if let Ok(mut c) = self.configs.lock() {
            c.retain(|l| !gone.contains(&l.root));
        }
    }

    /// Lee la configuración de una raíz del workspace y la deja activa (ids visibles).
    pub fn load_configs_for(&self, root: &Path) -> AppResult<Vec<ServiceConfig>> {
        let roots = self.check_root(root)?;
        let loaded = Self::load_root(&roots, root)?;
        let out = loaded.iter().map(|l| l.cfg.clone()).collect();
        {
            let mut cfgs = self.configs.lock().map_err(poisoned)?;
            cfgs.retain(|l| l.root != root);
            cfgs.extend(loaded);
        }
        self.emit_status();
        Ok(out)
    }

    /// Guarda en la raíz primaria (ver `save_configs_for`).
    #[cfg(test)]
    pub fn save_configs(&self, services: Vec<ServiceConfig>) -> AppResult<()> {
        let root = self.primary()?;
        self.save_configs_for(&root, services)
    }

    /// Guarda en disco la lista de UNA raíz, para los servicios eliminados y activa la nueva lista.
    /// Los ids pueden llegar crudos o visibles (`"<raíz>/<id>"`): se guardan siempre crudos.
    pub fn save_configs_for(&self, root: &Path, services: Vec<ServiceConfig>) -> AppResult<()> {
        let roots = self.check_root(root)?;
        let prefix = match roots.iter().position(|r| r == root) {
            Some(i) if i > 0 => Some(format!("{}/", display_names(&roots)[i])),
            _ => None,
        };
        let raw: Vec<ServiceConfig> = services
            .into_iter()
            .map(|mut c| {
                if let Some(rest) = prefix.as_deref().and_then(|p| c.id.strip_prefix(p)) {
                    c.id = rest.to_string();
                }
                c
            })
            .collect();
        config::save(root, &raw)?;
        let removed: Vec<String> = self
            .configs
            .lock()
            .map_err(poisoned)?
            .iter()
            .filter(|l| l.root == root && !raw.iter().any(|s| s.id == l.raw_id))
            .map(|l| l.cfg.id.clone())
            .collect();
        for id in &removed {
            let _ = self.stop_blocking(id);
            if let Ok(mut rt) = self.runtimes.lock() {
                rt.remove(id);
            }
            if let Ok(mut l) = self.logs.lock() {
                l.clear(Some(id));
            }
        }
        let new: Vec<Loaded> = raw
            .into_iter()
            .map(|mut c| {
                let raw_id = c.id.clone();
                c.id = visible_id(&roots, root, &raw_id);
                Loaded { root: root.to_path_buf(), raw_id, cfg: c }
            })
            .collect();
        {
            let mut cfgs = self.configs.lock().map_err(poisoned)?;
            cfgs.retain(|l| l.root != root);
            cfgs.extend(new);
        }
        self.emit_status();
        Ok(())
    }

    /// Servicios persistidos de todas las raíces (ids visibles).
    pub fn configs(&self) -> Vec<ServiceConfig> {
        self.configs.lock().map(|c| c.iter().map(|l| l.cfg.clone()).collect()).unwrap_or_default()
    }

    /// Persistidos + efímeros (estado, métricas y puertos cubren ambos).
    fn all_loaded(&self) -> Vec<(Loaded, bool)> {
        let mut out: Vec<(Loaded, bool)> =
            self.configs.lock().map(|c| c.iter().cloned().map(|l| (l, false)).collect()).unwrap_or_default();
        if let Ok(e) = self.ephemeral.lock() {
            out.extend(e.iter().cloned().map(|c| (c, true)));
        }
        out
    }

    /// Registra (o reemplaza por id visible) un servicio efímero de Run de la raíz.
    fn upsert_ephemeral(&self, root: &Path, cfg: ServiceConfig) -> AppResult<String> {
        let roots = self.check_root(root)?;
        let raw_id = cfg.id.clone();
        let mut cfg = cfg;
        cfg.id = visible_ephemeral_id(&roots, root, &raw_id);
        let id = cfg.id.clone();
        let loaded = Loaded { root: root.to_path_buf(), raw_id, cfg };
        if let Ok(mut e) = self.ephemeral.lock() {
            match e.iter_mut().find(|l| l.cfg.id == id) {
                Some(slot) => *slot = loaded,
                None => e.push(loaded),
            }
        }
        Ok(id)
    }

    /// Run en la raíz primaria (ver `run_ephemeral_in`).
    #[cfg(test)]
    pub fn run_ephemeral(self: &Arc<Self>, cfg: ServiceConfig) -> AppResult<String> {
        let root = self.primary()?;
        self.run_ephemeral_in(&root, cfg)
    }

    /// Upsert + (stop si corría) + start, con logs limpios y cabecera. Devuelve el id visible.
    /// Bloqueante: usar `run_blocking`.
    pub fn run_ephemeral_in(self: &Arc<Self>, root: &Path, cfg: ServiceConfig) -> AppResult<String> {
        let header = format!(
            "[CaesarCode] $ {}  ({})",
            cfg.command,
            if cfg.cwd.is_empty() { "." } else { cfg.cwd.as_str() }
        );
        let id = self.upsert_ephemeral(root, cfg)?;
        self.stop_blocking(&id)?;
        self.clear_logs(Some(&id));
        self.push_log(&id, header);
        self.start(&id)?;
        Ok(id)
    }

    // ---------- suscripciones y bucles ----------

    pub fn subscribe(self: &Arc<Self>, log_sink: LogSink, status_sink: StatusSink) {
        if let Ok(mut s) = self.log_sink.lock() {
            *s = Some(log_sink);
        }
        if let Ok(mut s) = self.status_sink.lock() {
            *s = Some(status_sink);
        }
        self.start_loops();
        self.emit_status();
    }

    fn start_loops(self: &Arc<Self>) {
        if self.loops_started.swap(true, Ordering::SeqCst) {
            return;
        }
        // Hilo rápido: flusher de logs (50 ms) + detección de salidas (250 ms).
        let m = Arc::clone(self);
        std::thread::Builder::new()
            .name("services-fast".into())
            .spawn(move || {
                let mut tick = 0u64;
                while !m.shutting_down.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(50));
                    tick += 1;
                    m.flush_logs();
                    if tick % 5 == 0 {
                        m.check_exits();
                    }
                }
            })
            .ok();
        // Hilo de métricas (1 s).
        let m = Arc::clone(self);
        std::thread::Builder::new()
            .name("services-metrics".into())
            .spawn(move || {
                let mut sampler = Sampler::new();
                while !m.shutting_down.load(Ordering::SeqCst) {
                    for _ in 0..10 {
                        if m.shutting_down.load(Ordering::SeqCst) {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    m.sample_metrics(&mut sampler);
                }
            })
            .ok();
    }

    fn flush_logs(&self) {
        let batch = {
            let Ok(mut l) = self.logs.lock() else { return };
            l.flush_stale();
            l.take_pending(LOG_BATCH)
        };
        let Ok(mut sink) = self.log_sink.lock() else { return };
        match sink.as_ref() {
            Some(f) => {
                if !batch.is_empty() && !f(batch) {
                    *sink = None;
                }
            }
            None => {
                // Sin suscriptor no se acumulan lotes pendientes.
                if let Ok(mut l) = self.logs.lock() {
                    l.drop_pending();
                }
            }
        }
    }

    fn sample_metrics(&self, sampler: &mut Sampler) {
        sampler.refresh();
        let targets: Vec<(String, u32)> = match self.runtimes.lock() {
            Ok(rt) => rt
                .iter()
                .filter(|(_, r)| r.state == ServiceState::Running)
                .filter_map(|(id, r)| r.pid.map(|p| (id.clone(), p)))
                .collect(),
            Err(_) => return,
        };
        let usage: Vec<(String, (f32, f64))> = targets
            .into_iter()
            .map(|(id, pid)| (id, sampler.tree_usage(pid)))
            .collect();
        if let Ok(mut rt) = self.runtimes.lock() {
            for (id, (cpu, mem)) in usage {
                if let Some(r) = rt.get_mut(&id) {
                    r.cpu = cpu;
                    r.memory_mb = mem;
                }
            }
        }
        let ports: Vec<(String, u16)> = self
            .all_loaded()
            .iter()
            .filter_map(|(l, _)| l.cfg.port.map(|p| (l.cfg.id.clone(), p)))
            .collect();
        let open: HashMap<String, bool> = ports.into_iter().map(|(id, p)| (id, is_port_open(p))).collect();
        if let Ok(mut po) = self.port_open.lock() {
            *po = open;
        }
        self.emit_status();
    }

    fn check_exits(&self) {
        let mut changed = false;
        if let Ok(mut rt) = self.runtimes.lock() {
            for (id, r) in rt.iter_mut() {
                if r.child.is_some() && self.reap_locked(id, r) {
                    changed = true;
                }
            }
        }
        if changed {
            self.emit_status();
        }
    }

    /// Si el proceso ya terminó, cierra el runtime. Idempotente. Requiere el lock de `runtimes`.
    fn reap_locked(&self, id: &str, r: &mut Runtime) -> bool {
        let Some(child) = r.child.as_mut() else { return false };
        let code = match child.try_wait() {
            Ok(None) => return false,
            Ok(Some(status)) => status.exit_code() as i32,
            Err(_) => -1,
        };
        let pid = r.pid;
        r.child = None;
        r.master = None; // cierra el PTY (en ConPTY es lo que da EOF al lector)
        r.exit_code = Some(code);
        r.state = if r.user_stopped || code == 0 { ServiceState::Stopped } else { ServiceState::Error };
        r.message = if r.state == ServiceState::Error {
            Some(format!("terminó con código {code}"))
        } else {
            None
        };
        r.cpu = 0.0;
        r.memory_mb = 0.0;
        if !r.user_stopped {
            // Salida natural: que no queden nietos vivos reteniendo puertos.
            if let Some(p) = pid {
                kill_group_best_effort(p);
            }
        }
        let _ = id;
        true
    }

    // ---------- estado ----------

    pub fn statuses(&self) -> Vec<ServiceStatus> {
        let configs = self.all_loaded();
        let ports = self.port_open.lock().map(|p| p.clone()).unwrap_or_default();
        let rt = match self.runtimes.lock() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        configs
            .iter()
            .map(|(l, ephemeral)| {
                let c = &l.cfg;
                let root = l.root.to_string_lossy().into_owned();
                let port_open = c.port.is_some() && ports.get(&c.id).copied().unwrap_or(false);
                match rt.get(&c.id) {
                    Some(r) => ServiceStatus {
                        id: c.id.clone(),
                        name: c.name.clone(),
                        root: root.clone(),
                        ephemeral: *ephemeral,
                        state: r.state,
                        pid: r.pid.filter(|_| r.child.is_some()),
                        exit_code: r.exit_code,
                        cpu: r.cpu,
                        memory_mb: r.memory_mb,
                        port_open,
                        port: c.port,
                        started_at: r.started_at,
                        message: r.message.clone(),
                    },
                    None => ServiceStatus {
                        id: c.id.clone(),
                        name: c.name.clone(),
                        root: root.clone(),
                        ephemeral: *ephemeral,
                        state: ServiceState::Stopped,
                        pid: None,
                        exit_code: None,
                        cpu: 0.0,
                        memory_mb: 0.0,
                        port_open,
                        port: c.port,
                        started_at: None,
                        message: None,
                    },
                }
            })
            .collect()
    }

    /// Envía el estado actual de todos los servicios (NO llamar con `runtimes` bloqueado).
    pub fn emit_status(&self) {
        let st = self.statuses();
        let Ok(mut sink) = self.status_sink.lock() else { return };
        if let Some(f) = sink.as_ref() {
            if !f(st) {
                *sink = None;
            }
        }
    }

    pub fn logs(&self, id: Option<&str>, limit: usize) -> Vec<LogLine> {
        self.logs.lock().map(|l| l.backlog(id, limit)).unwrap_or_default()
    }

    pub fn clear_logs(&self, id: Option<&str>) {
        if let Ok(mut l) = self.logs.lock() {
            l.clear(id);
        }
    }

    fn push_log(&self, id: &str, text: String) {
        if let Ok(mut l) = self.logs.lock() {
            l.push_line(id, text);
        }
    }

    // ---------- start / stop ----------

    fn config_of(&self, id: &str) -> AppResult<Loaded> {
        self.all_loaded()
            .into_iter()
            .map(|(l, _)| l)
            .find(|l| l.cfg.id == id)
            .ok_or_else(|| AppError::NotFound(format!("servicio {id}")))
    }

    pub fn start(self: &Arc<Self>, id: &str) -> AppResult<()> {
        let Loaded { root, cfg, .. } = self.config_of(id)?;
        let run_id = self.next_run.fetch_add(1, Ordering::SeqCst);
        {
            let mut rt = self.runtimes.lock().map_err(poisoned)?;
            if let Some(r) = rt.get(id) {
                match r.state {
                    ServiceState::Starting | ServiceState::Running => return Ok(()),
                    ServiceState::Stopping => {
                        return Err(AppError::Service(format!("'{}' se está deteniendo", cfg.name)))
                    }
                    _ => {}
                }
            }
            rt.insert(
                id.to_string(),
                Runtime {
                    run_id,
                    state: ServiceState::Starting,
                    pid: None,
                    child: None,
                    master: None,
                    started_at: None,
                    exit_code: None,
                    user_stopped: false,
                    message: None,
                    cpu: 0.0,
                    memory_mb: 0.0,
                },
            );
        }
        self.emit_status();
        match self.spawn_process(&cfg, &root, run_id) {
            Ok(()) => {
                self.emit_status();
                Ok(())
            }
            Err(e) => {
                if let Ok(mut rt) = self.runtimes.lock() {
                    if let Some(r) = rt.get_mut(id) {
                        if r.run_id == run_id {
                            r.state = ServiceState::Error;
                            r.message = Some(e.to_string());
                        }
                    }
                }
                self.push_log(id, format!("[CaesarCode] no se pudo arrancar: {e}"));
                self.emit_status();
                Err(e)
            }
        }
    }

    fn spawn_process(self: &Arc<Self>, cfg: &ServiceConfig, root: &Path, run_id: u64) -> AppResult<()> {
        let cwd = if cfg.cwd.trim().is_empty() {
            root.to_path_buf()
        } else {
            ensure_in_workspace(root, &root.join(&cfg.cwd))?
        };
        if !cwd.is_dir() {
            return Err(AppError::NotFound(format!("cwd {}", cwd.display())));
        }
        let pair = native_pty_system()
            .openpty(PtySize { rows: 40, cols: 200, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| AppError::Pty(e.to_string()))?;

        #[cfg(unix)]
        let mut cmd = {
            let mut c = CommandBuilder::new("/bin/sh");
            c.args(["-c", cfg.command.as_str()]);
            c
        };
        #[cfg(windows)]
        let mut cmd = {
            let mut c = CommandBuilder::new("cmd.exe");
            c.args(["/C", cfg.command.as_str()]);
            c
        };
        cmd.cwd(&cwd);
        // CommandBuilder ya hereda el entorno del proceso (PATH corregido incluido).
        if let Some(ef) = cfg.env_file.as_deref().filter(|s| !s.trim().is_empty()) {
            let path = cwd.join(ef);
            match dotenvy::from_path_iter(&path) {
                Ok(iter) => {
                    for item in iter.flatten() {
                        cmd.env(item.0, item.1);
                    }
                }
                Err(e) => self.push_log(&cfg.id, format!("[CaesarCode] aviso: no se pudo leer {}: {e}", path.display())),
            }
        }
        for (k, v) in &cfg.env {
            cmd.env(k, v);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("FORCE_COLOR", "1");
        cmd.env("PYTHONUNBUFFERED", "1");

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| AppError::Pty(e.to_string()))?;
        // Imprescindible: sin esto el lector nunca recibe EOF.
        drop(pair.slave);
        let pid = child.process_id();
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| AppError::Pty(e.to_string()))?;

        {
            let mut rt = self.runtimes.lock().map_err(poisoned)?;
            match rt.get_mut(&cfg.id) {
                Some(r) if r.run_id == run_id => {
                    r.state = ServiceState::Running;
                    r.pid = pid;
                    r.child = Some(child);
                    r.master = Some(pair.master);
                    r.started_at = Some(now_ms());
                    r.message = None;
                }
                _ => {
                    // El runtime fue sustituido/eliminado mientras arrancábamos.
                    drop(rt);
                    if let Some(p) = pid {
                        kill_tree(p, Duration::from_millis(500), true);
                    }
                    return Err(AppError::Service("arranque cancelado".into()));
                }
            }
        }

        let m = Arc::clone(self);
        let id = cfg.id.clone();
        std::thread::Builder::new()
            .name(format!("svc-reader-{id}"))
            .spawn(move || {
                let mut buf = [0u8; 16 * 1024];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if let Ok(mut l) = m.logs.lock() {
                                l.feed(&id, &buf[..n]);
                            }
                        }
                    }
                }
                if let Ok(mut l) = m.logs.lock() {
                    l.finish(&id);
                }
                // Espera a que el bucle de salidas cierre el runtime para ordenar el mensaje final.
                let deadline = Instant::now() + Duration::from_secs(3);
                while Instant::now() < deadline {
                    let code = m.runtimes.lock().ok().and_then(|rt| {
                        rt.get(&id)
                            .filter(|r| r.run_id == run_id && !r.busy())
                            .map(|r| r.exit_code)
                    });
                    if let Some(code) = code {
                        let text = match code {
                            Some(c) => format!("[CaesarCode] proceso terminado con código {c}"),
                            None => "[CaesarCode] proceso terminado".to_string(),
                        };
                        m.push_log(&id, text);
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
            })
            .map_err(|e| AppError::Service(e.to_string()))?;
        Ok(())
    }

    /// Detiene un servicio y mata todo su árbol. Bloqueante (hasta ~6 s).
    pub fn stop_blocking(&self, id: &str) -> AppResult<()> {
        let pid = {
            let mut rt = self.runtimes.lock().map_err(poisoned)?;
            let Some(r) = rt.get_mut(id) else { return Ok(()) };
            if r.child.is_none() {
                if r.state == ServiceState::Starting {
                    // Arranque en curso sin proceso todavía: se marca para que spawn lo cancele.
                    r.user_stopped = true;
                }
                return Ok(());
            }
            r.user_stopped = true;
            r.state = ServiceState::Stopping;
            r.pid
        };
        self.emit_status();
        if let Some(pid) = pid {
            kill_tree(pid, KILL_GRACE, true);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        let hard = Instant::now() + Duration::from_secs(4);
        loop {
            {
                let mut rt = self.runtimes.lock().map_err(poisoned)?;
                let Some(r) = rt.get_mut(id) else { break };
                if r.child.is_none() || self.reap_locked(id, r) {
                    r.state = ServiceState::Stopped;
                    break;
                }
                let now = Instant::now();
                if now >= deadline {
                    if let Some(c) = r.child.as_mut() {
                        let _ = c.kill();
                    }
                }
                if now >= hard {
                    r.child = None;
                    r.master = None;
                    r.state = ServiceState::Stopped;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        self.emit_status();
        Ok(())
    }

    pub fn restart(self: &Arc<Self>, id: &str) -> AppResult<()> {
        self.stop_blocking(id)?;
        self.start(id)
    }

    fn for_all_parallel(self: &Arc<Self>, ids: Vec<String>, f: impl Fn(&Arc<Self>, &str) -> AppResult<()> + Sync) -> AppResult<()> {
        let results: Vec<AppResult<()>> = std::thread::scope(|s| {
            let handles: Vec<_> = ids
                .iter()
                .map(|id| {
                    let f = &f;
                    s.spawn(move || f(self, id))
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or_else(|_| Err(AppError::Service("hilo abortado".into()))))
                .collect()
        });
        results.into_iter().collect()
    }

    pub fn start_all(self: &Arc<Self>) -> AppResult<()> {
        let ids: Vec<String> = self.configs().into_iter().map(|c| c.id).collect();
        self.for_all_parallel(ids, |m, id| m.start(id))
    }

    pub fn stop_all(self: &Arc<Self>) -> AppResult<()> {
        let ids = self.active_ids();
        self.for_all_parallel(ids, |m, id| m.stop_blocking(id))
    }

    fn active_ids(&self) -> Vec<String> {
        self.runtimes
            .lock()
            .map(|rt| rt.iter().filter(|(_, r)| r.child.is_some()).map(|(id, _)| id.clone()).collect())
            .unwrap_or_default()
    }

    /// Para todos los servicios en paralelo (shutdown y cambio de workspace).
    pub fn stop_all_blocking(&self) {
        let ids = self.active_ids();
        std::thread::scope(|s| {
            for id in &ids {
                s.spawn(move || {
                    let _ = self.stop_blocking(id);
                });
            }
        });
    }

    /// Apagado de la app: mata servicios y detiene los bucles.
    pub fn shutdown(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        self.stop_all_blocking();
    }
}

impl Default for ServiceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::services::process_tree::{is_alive, snapshot_system, tree_pids};
    use crate::services::{LogLevel, ServiceKind};
    use std::collections::HashMap;

    fn svc(id: &str, command: &str, port: Option<u16>) -> ServiceConfig {
        ServiceConfig {
            id: id.into(),
            name: id.into(),
            command: command.into(),
            cwd: String::new(),
            env: HashMap::new(),
            env_file: None,
            port,
            kind: ServiceKind::Custom,
            tech: None,
            framework: None,
        }
    }

    fn setup(services: Vec<ServiceConfig>) -> (Arc<ServiceManager>, PathBuf, Arc<Mutex<Vec<LogLine>>>) {
        let dir = std::env::temp_dir().join(format!(
            "cc-svc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = std::fs::canonicalize(&dir).unwrap();
        config::save(&dir, &services).unwrap();
        let m = Arc::new(ServiceManager::new());
        m.set_workspace(dir.clone()).unwrap();
        let sink: Arc<Mutex<Vec<LogLine>>> = Arc::new(Mutex::new(Vec::new()));
        let s2 = Arc::clone(&sink);
        m.subscribe(
            Box::new(move |b| {
                s2.lock().unwrap().extend(b);
                true
            }),
            Box::new(|_| true),
        );
        (m, dir, sink)
    }

    fn state_of(m: &ServiceManager, id: &str) -> ServiceStatus {
        m.statuses().into_iter().find(|s| s.id == id).unwrap()
    }

    fn wait_for(mut f: impl FnMut() -> bool, ms: u64) -> bool {
        let end = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < end {
            if f() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        f()
    }

    #[test]
    fn start_logs_exit_codes_and_errors() {
        let (m, dir, sink) = setup(vec![
            svc("ok", "echo hello; echo 'ERROR boom'; echo 'warn x'", None),
            svc("bad", "exit 3", None),
        ]);
        m.start("ok").unwrap();
        m.start("bad").unwrap();
        assert!(wait_for(|| state_of(&m, "ok").state == ServiceState::Stopped, 4000));
        assert!(wait_for(|| state_of(&m, "bad").state == ServiceState::Error, 4000));
        assert_eq!(state_of(&m, "bad").exit_code, Some(3));
        assert_eq!(state_of(&m, "ok").exit_code, Some(0));
        assert!(wait_for(
            || sink.lock().unwrap().iter().any(|l| l.text.contains("terminado con código 3")),
            3000
        ));
        let got = sink.lock().unwrap().clone();
        let find = |t: &str| got.iter().find(|l| l.text == t).unwrap_or_else(|| panic!("falta {t}: {got:?}")).clone();
        assert_eq!(find("hello").level, LogLevel::Info);
        assert_eq!(find("ERROR boom").level, LogLevel::Error);
        assert_eq!(find("warn x").level, LogLevel::Warn);
        // El mensaje final va después de la salida del proceso.
        let last_out = got.iter().filter(|l| l.service_id == "ok" && l.text == "warn x").map(|l| l.seq).max().unwrap();
        let fin = got.iter().find(|l| l.service_id == "ok" && l.text.contains("terminado")).unwrap();
        assert!(fin.seq > last_out);
        assert_eq!(m.logs(Some("ok"), 100).len(), 4);
        m.clear_logs(Some("ok"));
        assert!(m.logs(Some("ok"), 100).is_empty());
        m.shutdown();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn stop_kills_entire_tree_and_restart_works() {
        let (m, dir, _sink) = setup(vec![svc(
            "tree",
            "sh -c 'sleep 311; true' & sleep 312 & wait",
            None,
        )]);
        m.start("tree").unwrap();
        let root = state_of(&m, "tree").pid.expect("pid");
        let mut tree = Vec::new();
        assert!(wait_for(
            || {
                tree = tree_pids(&snapshot_system(), root);
                tree.len() >= 4
            },
            4000
        ));
        assert_eq!(state_of(&m, "tree").state, ServiceState::Running);
        m.stop_blocking("tree").unwrap();
        let st = state_of(&m, "tree");
        assert_eq!(st.state, ServiceState::Stopped);
        for p in &tree {
            assert!(!is_alive(*p), "pid {p} huérfano tras stop");
        }
        // Reinicio
        m.restart("tree").unwrap();
        assert_eq!(state_of(&m, "tree").state, ServiceState::Running);
        let pid2 = state_of(&m, "tree").pid.unwrap();
        assert_ne!(pid2, root);
        m.stop_all().unwrap();
        assert_eq!(state_of(&m, "tree").state, ServiceState::Stopped);
        assert!(!is_alive(pid2));
        m.shutdown();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn workspace_switch_stops_services_but_same_root_keeps_them() {
        let (m, dir, _sink) = setup(vec![svc("keep", "sleep 331", None)]);
        m.start("keep").unwrap();
        let pid = state_of(&m, "keep").pid.unwrap();
        m.set_workspace(dir.clone()).unwrap(); // mismo root: sigue vivo
        assert_eq!(state_of(&m, "keep").state, ServiceState::Running);
        assert!(is_alive(pid));
        let other = std::fs::canonicalize(std::env::temp_dir()).unwrap().join(format!("cc-other-{}", std::process::id()));
        std::fs::create_dir_all(&other).unwrap();
        m.set_workspace(other.clone()).unwrap(); // otro root: se mata todo
        assert!(!is_alive(pid), "el servicio debía pararse al cambiar de workspace");
        assert!(m.statuses().is_empty());
        m.shutdown();
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(other);
    }

    #[test]
    fn metrics_and_port_kill() {
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let have_py = std::process::Command::new("python3").arg("--version").output().is_ok();
        if !have_py {
            eprintln!("python3 no disponible: se omite");
            return;
        }
        let (m, dir, _sink) = setup(vec![svc("web", &format!("python3 -m http.server {port} --bind 127.0.0.1"), Some(port))]);
        m.start("web").unwrap();
        assert!(wait_for(|| state_of(&m, "web").port_open, 8000), "puerto no abierto");
        assert!(wait_for(|| state_of(&m, "web").memory_mb > 0.0, 4000), "sin RAM");
        let pids = crate::services::ports::port_kill(port);
        assert!(!pids.is_empty());
        assert!(wait_for(|| !crate::services::ports::is_port_open(port), 3000), "puerto sigue abierto");
        // El servicio detecta que murió (señal -> código != 0 -> Error, sin user_stopped).
        assert!(wait_for(|| !matches!(state_of(&m, "web").state, ServiceState::Running), 4000));
        m.shutdown();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ephemeral_services_status_restart_and_workspace() {
        let (m, dir, _sink) = setup(vec![svc("persisted", "sleep 341", None)]);
        let mut e = svc("file:x", "sleep 342", None);
        e.name = "x".into();
        m.run_ephemeral(e.clone()).unwrap();
        let st = state_of(&m, "file:x");
        assert!(st.ephemeral && st.name == "x");
        assert_eq!(st.state, ServiceState::Running);
        assert!(!state_of(&m, "persisted").ephemeral);
        assert!(m.logs(Some("file:x"), 10).iter().any(|l| l.text.contains("[CaesarCode] $ sleep 342")));
        let pid1 = st.pid.unwrap();
        // segunda ejecución = restart (pid nuevo, anterior muerto)
        m.run_ephemeral(e.clone()).unwrap();
        let pid2 = state_of(&m, "file:x").pid.unwrap();
        assert_ne!(pid1, pid2);
        assert!(wait_for(|| !is_alive(pid1), 3000));
        // save_configs no borra efímeros
        m.save_configs(vec![svc("persisted", "sleep 341", None)]).unwrap();
        assert!(m.statuses().iter().any(|s| s.id == "file:x"));
        assert_eq!(state_of(&m, "file:x").state, ServiceState::Running);
        // stop mata el efímero con el comando normal
        m.stop_blocking("file:x").unwrap();
        assert_eq!(state_of(&m, "file:x").state, ServiceState::Stopped);
        assert!(!is_alive(pid2));
        // start_all solo arranca persistidos
        m.start_all().unwrap();
        assert_eq!(state_of(&m, "file:x").state, ServiceState::Stopped);
        // el mismo root conserva efímeros; otro root los limpia
        m.set_workspace(dir.clone()).unwrap();
        assert!(m.statuses().iter().any(|s| s.id == "file:x"));
        m.run_ephemeral(e).unwrap();
        let pid3 = state_of(&m, "file:x").pid.unwrap();
        let other = std::fs::canonicalize(std::env::temp_dir()).unwrap().join(format!("cc-other-eph-{}", std::process::id()));
        std::fs::create_dir_all(&other).unwrap();
        m.set_workspace(other.clone()).unwrap();
        assert!(!is_alive(pid3));
        assert!(m.statuses().is_empty());
        m.shutdown();
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(other);
    }

    #[test]
    fn same_id_in_two_roots_and_set_roots_only_stops_removed_root() {
        let (m, dir, _sink) = setup(vec![svc("api", "sleep 351", None)]);
        let other = std::fs::canonicalize(std::env::temp_dir()).unwrap().join(format!("cc-svc2-{}", std::process::id()));
        std::fs::create_dir_all(&other).unwrap();
        let other = std::fs::canonicalize(&other).unwrap();
        config::save(&other, &[svc("api", "sleep 352", None)]).unwrap();
        m.set_roots(vec![dir.clone(), other.clone()]).unwrap();
        let name = other.file_name().unwrap().to_string_lossy().into_owned();
        let vis = format!("{name}/api");
        let ids: Vec<String> = m.statuses().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, vec!["api".to_string(), vis.clone()]);
        assert_eq!(state_of(&m, &vis).root, other.to_string_lossy());
        // cada servicio arranca en su raíz y con su comando
        m.start("api").unwrap();
        m.start(&vis).unwrap();
        let (p1, p2) = (state_of(&m, "api").pid.unwrap(), state_of(&m, &vis).pid.unwrap());
        // guardar la raíz 2 con ids visibles o crudos guarda crudos
        m.save_configs_for(&other, vec![svc(&vis, "sleep 352", None)]).unwrap();
        assert_eq!(config::load(&other).unwrap()[0].id, "api");
        assert_eq!(state_of(&m, &vis).state, ServiceState::Running);
        // efímero en la raíz 2: prefijo conservado, nombre de raíz tras el prefijo
        let mut e = svc("file:x.js", "sleep 353", None);
        e.name = "x".into();
        let eid = m.run_ephemeral_in(&other, e).unwrap();
        assert_eq!(eid, format!("file:{name}/x.js"));
        let p3 = state_of(&m, &eid).pid.unwrap();
        // quitar la raíz 2: solo se paran sus servicios (persistidos y efímeros)
        m.set_roots(vec![dir.clone()]).unwrap();
        assert!(wait_for(|| !is_alive(p2) && !is_alive(p3), 4000), "los servicios de la raíz quitada debían pararse");
        assert!(is_alive(p1), "la raíz que sigue no debe tocarse");
        assert_eq!(state_of(&m, "api").state, ServiceState::Running);
        assert!(m.statuses().iter().all(|s| s.id == "api"));
        assert!(m.save_configs_for(&other, vec![]).is_err(), "raíz fuera del workspace");
        m.shutdown();
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(other);
    }
}
