//! Ejecución de `git` con bytes, stdin, timeout, env extra y streaming de progreso.
//! Todas las variantes comparten `base_command` (misma configuración que `run_git`) y nunca usan shell.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{oneshot, OwnedMutexGuard};

use super::base_command;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::util::paths::{clean_canonicalize, strip_verbatim};

/// Tiempo máximo por defecto de una invocación local.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
/// Operaciones locales potencialmente lentas (commit con hooks, rebase, checkout grande).
pub const MUT_TIMEOUT: Duration = Duration::from_secs(300);
/// Fetch/pull/push.
pub const REMOTE_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Default)]
pub struct RunOpts {
    pub stdin: Option<Vec<u8>>,
    pub env: Vec<(String, String)>,
    pub timeout: Option<Duration>,
}

impl RunOpts {
    pub fn stdin(data: impl Into<Vec<u8>>) -> Self {
        Self { stdin: Some(data.into()), ..Default::default() }
    }
    pub fn env(mut self, k: &str, v: &str) -> Self {
        self.env.push((k.to_string(), v.to_string()));
        self
    }
    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = Some(t);
        self
    }
    /// Editores no interactivos para merge/rebase/cherry-pick/revert.
    pub fn no_editor(self) -> Self {
        self.env("GIT_EDITOR", "true").env("GIT_SEQUENCE_EDITOR", "true")
    }
}

pub struct RawOut {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl RawOut {
    pub fn ok(&self) -> bool {
        self.code == 0
    }
    pub fn out_str(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
    /// stderr si hay; si no stdout (git a veces explica el fallo en stdout, p. ej. "nothing to commit").
    pub fn message(&self) -> String {
        let e = String::from_utf8_lossy(&self.stderr).trim().to_string();
        if !e.is_empty() {
            return e;
        }
        String::from_utf8_lossy(&self.stdout).trim().to_string()
    }
}

fn mark_run(state: &AppState, cwd: &Path) {
    state.git_mark_run(cwd);
}

/// Las rutas del usuario siempre van tras `--`: ahí los pathspec son literales (sin `:(magic)` ni globs).
/// No se activa globalmente: `stash -u` y otros usan pathspec internos que lo necesitan desactivado.
fn literal_if_paths(cmd: &mut tokio::process::Command, argv: &[&str]) {
    if argv.contains(&"--") {
        cmd.env("GIT_LITERAL_PATHSPECS", "1");
    }
}

fn spawn_err(e: std::io::Error) -> AppError {
    AppError::Git(format!("no se pudo ejecutar git: {e}"))
}

/// Ejecuta git y devuelve el resultado SIN convertir un código != 0 en error.
pub async fn run_raw<S: AsRef<str>>(state: &AppState, cwd: &Path, args: &[S], opts: RunOpts) -> AppResult<RawOut> {
    let argv: Vec<&str> = args.iter().map(|a| a.as_ref()).collect();
    let mut cmd = base_command(cwd, &argv);
    literal_if_paths(&mut cmd, &argv);
    for (k, v) in &opts.env {
        cmd.env(k, v);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if opts.stdin.is_some() {
        cmd.stdin(Stdio::piped());
    }
    let mut child = cmd.spawn().map_err(spawn_err)?;
    if let Some(data) = opts.stdin {
        if let Some(mut si) = child.stdin.take() {
            // En una tarea aparte: un parche grande no debe bloquear la lectura de la salida.
            tokio::spawn(async move {
                let _ = si.write_all(&data).await;
                let _ = si.shutdown().await;
            });
        }
    }
    let timeout = opts.timeout.unwrap_or(DEFAULT_TIMEOUT);
    let res = tokio::time::timeout(timeout, child.wait_with_output()).await;
    mark_run(state, cwd);
    match res {
        Err(_) => Err(AppError::Git(format!(
            "git {} excedió el tiempo máximo ({} s)",
            argv.first().copied().unwrap_or(""),
            timeout.as_secs()
        ))),
        Ok(Err(e)) => Err(spawn_err(e)),
        Ok(Ok(o)) => Ok(RawOut { code: o.status.code().unwrap_or(-1), stdout: o.stdout, stderr: o.stderr }),
    }
}

/// Como `run_raw` pero un código != 0 es `Err(Git(mensaje))`. Devuelve stdout en bytes.
pub async fn run_ok<S: AsRef<str>>(state: &AppState, cwd: &Path, args: &[S], opts: RunOpts) -> AppResult<Vec<u8>> {
    let out = run_raw(state, cwd, args, opts).await?;
    if out.ok() {
        Ok(out.stdout)
    } else {
        let msg = out.message();
        let first = args.first().map(|a| a.as_ref()).unwrap_or("");
        Err(AppError::Git(if msg.is_empty() { format!("git {first} falló") } else { sanitize_text(&msg) }))
    }
}

/// `run_ok` → String (lossy).
pub async fn run_str<S: AsRef<str>>(state: &AppState, cwd: &Path, args: &[S]) -> AppResult<String> {
    let b = run_ok(state, cwd, args, RunOpts::default()).await?;
    Ok(String::from_utf8_lossy(&b).into_owned())
}

/// Quita `user:pass@` (y `token@`) de cualquier URL con esquema dentro de un texto.
pub fn sanitize_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("://") {
        out.push_str(&rest[..i + 3]);
        rest = &rest[i + 3..];
        let end = rest.find(|c: char| c.is_whitespace() || c == '/' || c == '\'' || c == '"').unwrap_or(rest.len());
        let authority = &rest[..end];
        match authority.rfind('@') {
            Some(at) => out.push_str(&authority[at + 1..]),
            None => out.push_str(authority),
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------------------
// Guard de operaciones mutadoras
// ---------------------------------------------------------------------------

/// Incrementa el contador `git_busy` del repo mientras vive (RAII: también en error o panic).
pub struct BusyGuard<'a> {
    state: &'a AppState,
    top: PathBuf,
}

impl<'a> BusyGuard<'a> {
    pub fn new(state: &'a AppState, top: &Path) -> Self {
        if let Ok(mut m) = state.git_busy.lock() {
            *m.entry(top.to_path_buf()).or_insert(0) += 1;
        }
        Self { state, top: top.to_path_buf() }
    }
}

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut m) = self.state.git_busy.lock() {
            if let Some(n) = m.get_mut(&self.top) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    m.remove(&self.top);
                }
            }
        }
        // La ventana anti-bucle del watcher arranca al soltar.
        mark_run(self.state, &self.top);
    }
}

/// Serializa las operaciones mutadoras DEL REPO (`git_ops[top]`) y marca `git_busy` mientras esperan y ejecutan.
/// Un repo ocupado no bloquea a los demás.
pub struct OpGuard<'a> {
    _lock: OwnedMutexGuard<()>,
    _busy: BusyGuard<'a>,
}

impl<'a> OpGuard<'a> {
    pub async fn acquire(state: &'a AppState, top: &Path) -> OpGuard<'a> {
        let busy = BusyGuard::new(state, top);
        let lock = state.git_op_lock(top).lock_owned().await;
        OpGuard { _lock: lock, _busy: busy }
    }
}

// ---------------------------------------------------------------------------
// Contexto: raíces + toplevel del repo
// ---------------------------------------------------------------------------

/// Raíces del workspace y toplevel del repo validado. Los comandos corren con `cwd = top` para que los pathspec
/// (relativos al top) y `git apply` se comporten igual aunque la raíz sea una subcarpeta del repo.
#[derive(Clone, Debug)]
pub struct Ctx {
    /// Raíces del workspace: las rutas absolutas del frontend deben caer en alguna.
    pub roots: Vec<PathBuf>,
    pub top: PathBuf,
}

/// El repo está relacionado con alguna raíz: su top está dentro de una raíz o una raíz dentro del top.
pub fn related_to_roots(roots: &[PathBuf], top: &Path) -> bool {
    roots.iter().any(|r| top.starts_with(r) || r.starts_with(top))
}

/// Resuelve y valida el repo sobre el que operar. `repo = None` → repo de la raíz primaria.
/// `Some(top)` debe ser el toplevel de un repo relacionado con alguna raíz (caché de `git_repos` o `rev-parse`).
pub async fn ctx_for(state: &AppState, repo: Option<&str>) -> AppResult<Ctx> {
    let roots = state.roots()?;
    let outside = || AppError::InvalidArgument("repo fuera del workspace".into());
    let top = match repo.filter(|s| !s.is_empty()) {
        None => {
            let primary = state.primary()?;
            let out = super::run_git_in(&primary, &["rev-parse", "--show-toplevel"]).await;
            let top = out.map(|o| o.trim().to_string()).unwrap_or_default();
            if top.is_empty() {
                return Err(AppError::Git("no es un repositorio git".into()));
            }
            let top = strip_verbatim(PathBuf::from(top));
            clean_canonicalize(&top).unwrap_or(top)
        }
        Some(r) => {
            let canon = clean_canonicalize(Path::new(r)).map_err(|_| outside())?;
            if !related_to_roots(&roots, &canon) {
                return Err(outside());
            }
            let cached = state.git_repos.lock().map(|g| g.iter().any(|e| e.top == canon)).unwrap_or(false);
            if !cached {
                let out = super::run_git_in(&canon, &["rev-parse", "--show-toplevel"]).await.map_err(|_| outside())?;
                let found = clean_canonicalize(Path::new(out.trim())).map_err(|_| outside())?;
                if found != canon {
                    return Err(outside());
                }
            }
            canon
        }
    };
    Ok(Ctx { roots, top })
}

// ---------------------------------------------------------------------------
// Streaming (fetch/pull/push)
// ---------------------------------------------------------------------------

/// Parte el progreso de stderr (`\r` y `\n` son separadores) y lo entrega línea a línea.
pub struct LineSplitter {
    buf: Vec<u8>,
}

impl LineSplitter {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }
    pub fn feed(&mut self, chunk: &[u8], mut on_line: impl FnMut(String)) {
        for &b in chunk {
            if b == b'\r' || b == b'\n' {
                self.flush(&mut on_line);
            } else {
                self.buf.push(b);
            }
        }
    }
    pub fn flush(&mut self, on_line: &mut impl FnMut(String)) {
        if !self.buf.is_empty() {
            let line = String::from_utf8_lossy(&self.buf).trim().to_string();
            self.buf.clear();
            if !line.is_empty() {
                on_line(line);
            }
        }
    }
}

impl Default for LineSplitter {
    fn default() -> Self {
        Self::new()
    }
}

pub enum StreamEnd {
    Finished(RawOut),
    Cancelled,
    TimedOut,
}

/// Ejecuta git leyendo stderr en tiempo real. `cancel` mata el proceso. No convierte el código en error.
pub async fn run_streaming<S: AsRef<str>>(
    state: &AppState,
    cwd: &Path,
    args: &[S],
    env: &[(String, String)],
    timeout: Duration,
    mut cancel: oneshot::Receiver<()>,
    mut on_line: impl FnMut(String),
) -> AppResult<StreamEnd> {
    let argv: Vec<&str> = args.iter().map(|a| a.as_ref()).collect();
    let mut cmd = base_command(cwd, &argv);
    literal_if_paths(&mut cmd, &argv);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(spawn_err)?;
    let mut stdout = child.stdout.take().ok_or_else(|| AppError::Git("sin stdout".into()))?;
    let mut stderr = child.stderr.take().ok_or_else(|| AppError::Git("sin stderr".into()))?;

    let out_task = tokio::spawn(async move {
        let mut v = Vec::new();
        let _ = stdout.read_to_end(&mut v).await;
        v
    });

    let mut all_err: Vec<u8> = Vec::new();
    let mut splitter = LineSplitter::new();
    let mut chunk = [0u8; 4096];
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);
    let mut cancel_open = true;

    let outcome: Option<StreamEnd> = loop {
        tokio::select! {
            n = stderr.read(&mut chunk) => {
                match n {
                    Ok(0) | Err(_) => break None,
                    Ok(n) => {
                        if all_err.len() < 64 * 1024 {
                            all_err.extend_from_slice(&chunk[..n]);
                        }
                        splitter.feed(&chunk[..n], &mut on_line);
                    }
                }
            }
            r = &mut cancel, if cancel_open => {
                if r.is_ok() {
                    break Some(StreamEnd::Cancelled);
                }
                // Emisor soltado sin enviar: ya no se puede cancelar, se sigue.
                cancel_open = false;
            }
            _ = &mut deadline => break Some(StreamEnd::TimedOut),
        }
    };
    splitter.flush(&mut on_line);
    if let Some(end) = outcome {
        let _ = child.kill().await;
        let _ = child.wait().await;
        out_task.abort();
        mark_run(state, cwd);
        return Ok(end);
    }
    let status = child.wait().await.map_err(spawn_err)?;
    let stdout = out_task.await.unwrap_or_default();
    mark_run(state, cwd);
    Ok(StreamEnd::Finished(RawOut { code: status.code().unwrap_or(-1), stdout, stderr: all_err }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_credentials() {
        assert_eq!(sanitize_text("fatal: https://user:pw@github.com/a/b.git no"), "fatal: https://github.com/a/b.git no");
        assert_eq!(sanitize_text("https://tok@host/x"), "https://host/x");
        assert_eq!(sanitize_text("git@github.com:a/b.git"), "git@github.com:a/b.git");
        assert_eq!(sanitize_text("https://host/a@b"), "https://host/a@b");
    }

    #[test]
    fn splits_progress_on_cr_and_lf() {
        let mut sp = LineSplitter::new();
        let mut got = Vec::new();
        sp.feed(b"Receiving objects:  10%\rReceiving objects:  20%\nDone", |l| got.push(l));
        sp.flush(&mut |l| got.push(l));
        assert_eq!(got, vec!["Receiving objects:  10%", "Receiving objects:  20%", "Done"]);
    }

    #[test]
    fn busy_guard_balances_even_on_panic() {
        let st = AppState::new();
        let a = PathBuf::from("/w/a");
        {
            let _g = BusyGuard::new(&st, &a);
            assert_eq!(st.git_busy_total(), 1);
            assert!(st.git_busy_for(&a));
        }
        assert_eq!(st.git_busy_total(), 0);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = BusyGuard::new(&st, &a);
            panic!("boom");
        }));
        assert!(r.is_err());
        assert_eq!(st.git_busy_total(), 0);
        assert!(st.git_last_run.lock().unwrap().contains_key(&a));
    }

    #[tokio::test]
    async fn op_guard_of_one_repo_does_not_block_another() {
        let st = AppState::new();
        let (a, b) = (PathBuf::from("/w/a"), PathBuf::from("/w/b"));
        let _ga = OpGuard::acquire(&st, &a).await;
        // B se adquiere sin esperar; A no (try_lock falla mientras A está tomado).
        assert!(st.git_op_lock(&b).try_lock_owned().is_ok());
        assert!(st.git_op_lock(&a).try_lock_owned().is_err());
        assert!(st.git_busy_for(&a) && !st.git_busy_for(&b));
    }
}
