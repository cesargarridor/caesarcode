//! fetch / pull / push con progreso, timeouts y cancelación.

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use super::exec::{ctx_for, run_streaming, sanitize_text, OpGuard, StreamEnd, REMOTE_TIMEOUT};
use super::ops::OpOutcome;
use super::validate;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum GitProgress {
    Line { text: String },
    Percent { phase: String, pct: u8 },
    Done,
    Failed { message: String },
}

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RemoteOp {
    Fetch { remote: Option<String>, prune: bool },
    /// `ff_only`: solo avance rápido ("Pull all"); gana sobre `rebase`.
    Pull {
        rebase: bool,
        #[serde(default)]
        ff_only: bool,
    },
    Push { remote: Option<String>, branch: Option<String>, force_with_lease: bool, set_upstream: bool, tags: bool },
}

/// Entorno de red: no bloquear nunca esperando contraseña/confirmación; conexión rápida.
pub fn remote_env() -> Vec<(String, String)> {
    let mut env = Vec::new();
    if std::env::var_os("GIT_SSH_COMMAND").is_none() {
        env.push(("GIT_SSH_COMMAND".to_string(), "ssh -o BatchMode=yes -o ConnectTimeout=15".to_string()));
    }
    env
}

/// `Receiving objects:  45% (9/20)` → (`Receiving objects`, 45).
pub fn parse_percent(line: &str) -> Option<(String, u8)> {
    let line = line.strip_prefix("remote: ").unwrap_or(line);
    let pct_pos = line.find('%')?;
    let before = &line[..pct_pos];
    let digits_start = before.rfind(|c: char| !c.is_ascii_digit()).map(|i| i + 1).unwrap_or(0);
    let pct: u8 = before[digits_start..].parse().ok().filter(|p| *p <= 100)?;
    let phase = before[..digits_start].trim().trim_end_matches(':').trim().to_string();
    if phase.is_empty() {
        return None;
    }
    Some((phase, pct))
}

fn auth_hint(msg: &str) -> Option<&'static str> {
    let m = msg.to_lowercase();
    if m.contains("terminal prompts disabled")
        || m.contains("could not read username")
        || m.contains("could not read password")
        || m.contains("authentication failed")
        || m.contains("permission denied (publickey")
        || m.contains("host key verification failed")
    {
        Some("Autenticación fallida: configura un credential helper o SSH.")
    } else {
        None
    }
}

/// Registro de cancelación: se quita siempre al salir.
struct JobSlot<'a> {
    state: &'a AppState,
    id: String,
}

impl<'a> JobSlot<'a> {
    fn register(state: &'a AppState, id: &str) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        if let Ok(mut g) = state.git_jobs.lock() {
            g.insert(id.to_string(), tx);
        }
        rx
    }
}

impl Drop for JobSlot<'_> {
    fn drop(&mut self) {
        if let Ok(mut g) = self.state.git_jobs.lock() {
            g.remove(&self.id);
        }
    }
}

pub fn cancel(state: &AppState, job_id: &str) -> bool {
    let tx = state.git_jobs.lock().ok().and_then(|mut g| g.remove(job_id));
    match tx {
        Some(tx) => tx.send(()).is_ok(),
        None => false,
    }
}

/// Ejecuta la operación remota emitiendo `GitProgress`. Siempre termina con `Done` o `Failed`.
/// `on_progress` devuelve `false` si el receptor desapareció (se aborta limpiamente).
pub async fn run_remote(
    state: &AppState,
    repo: Option<&str>,
    op: RemoteOp,
    job_id: String,
    mut on_progress: impl FnMut(GitProgress) -> bool,
) -> AppResult<OpOutcome> {
    let res = run_remote_inner(state, repo, op, &job_id, &mut on_progress).await;
    match &res {
        Ok(_) => {
            on_progress(GitProgress::Done);
        }
        Err(e) => {
            on_progress(GitProgress::Failed { message: e.to_string() });
        }
    }
    res
}

async fn run_remote_inner(
    state: &AppState,
    repo: Option<&str>,
    op: RemoteOp,
    job_id: &str,
    on_progress: &mut impl FnMut(GitProgress) -> bool,
) -> AppResult<OpOutcome> {
    validate::reject_dash("job_id", job_id)?;
    let c = ctx_for(state, repo).await?;
    let mut args: Vec<String> =
        ["-c", "http.lowSpeedLimit=1000", "-c", "http.lowSpeedTime=30"].iter().map(|s| s.to_string()).collect();
    match op {
        RemoteOp::Fetch { remote, prune } => {
            args.extend(["fetch".into(), "--progress".into()]);
            if prune {
                args.push("--prune".into());
            }
            match remote {
                Some(r) => {
                    validate::remote(state, &c, &r).await?;
                    args.push(r);
                }
                None => args.push("--all".into()),
            }
        }
        RemoteOp::Pull { rebase, ff_only } => {
            args.extend(["pull".into(), "--progress".into()]);
            args.push(if rebase && !ff_only { "--rebase" } else { "--ff-only" }.into());
        }
        RemoteOp::Push { remote, branch, force_with_lease, set_upstream, tags } => {
            args.extend(["push".into(), "--progress".into()]);
            if force_with_lease {
                args.push("--force-with-lease".into());
            }
            if set_upstream {
                args.push("--set-upstream".into());
            }
            if tags {
                args.push("--tags".into());
            }
            let names = validate::remote_names(state, &c).await?;
            let remote = match remote {
                Some(r) => {
                    validate::remote(state, &c, &r).await?;
                    Some(r)
                }
                None if branch.is_some() || set_upstream => {
                    let pick = if names.iter().any(|n| n == "origin") { names.iter().find(|n| *n == "origin") } else { names.first() };
                    Some(pick.cloned().ok_or_else(|| AppError::InvalidArgument("no hay remotos configurados".into()))?)
                }
                None => None,
            };
            if let Some(r) = remote {
                args.push(r);
                let branch = match branch {
                    Some(b) => {
                        validate::refname("rama", &b)?;
                        Some(b)
                    }
                    None if set_upstream => {
                        let o = super::exec::run_raw(state, &c.top, &["symbolic-ref", "--short", "-q", "HEAD"], Default::default()).await?;
                        let b = o.out_str().trim().to_string();
                        if b.is_empty() {
                            return Err(AppError::InvalidArgument("HEAD desacoplado: indica la rama".into()));
                        }
                        Some(b)
                    }
                    None => None,
                };
                if let Some(b) = branch {
                    args.push(b);
                }
            }
        }
    }

    // Registrar antes de esperar el lock: un `git_cancel` mientras está en cola también cuenta.
    let rx = JobSlot::register(state, job_id);
    let _slot = JobSlot { state, id: job_id.to_string() };
    let _g = OpGuard::acquire(state, &c.top).await;
    let mut alive = true;
    let mut last_text = String::new();
    let end = run_streaming(state, &c.top, &args, &remote_env(), REMOTE_TIMEOUT, rx, |line| {
        if !alive {
            return;
        }
        let line = sanitize_text(&line);
        let ev = match parse_percent(&line) {
            Some((phase, pct)) => GitProgress::Percent { phase, pct },
            None => GitProgress::Line { text: line.clone() },
        };
        last_text = line;
        if !on_progress(ev) {
            alive = false;
        }
    })
    .await?;
    if !alive {
        return Err(AppError::Git("canal de progreso cerrado: operación abandonada".into()));
    }
    match end {
        StreamEnd::Cancelled => Err(AppError::Git("operación cancelada".into())),
        StreamEnd::TimedOut => Err(AppError::Git(format!("tiempo máximo excedido ({} s)", REMOTE_TIMEOUT.as_secs()))),
        StreamEnd::Finished(out) => {
            let stderr = sanitize_text(String::from_utf8_lossy(&out.stderr).trim());
            if out.ok() {
                let mut message = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if message.is_empty() {
                    message = last_text;
                }
                Ok(OpOutcome { ok: true, conflicts: vec![], message: sanitize_text(&message) })
            } else {
                let mut msg = if stderr.is_empty() { sanitize_text(String::from_utf8_lossy(&out.stdout).trim()) } else { stderr };
                if let Some(h) = auth_hint(&msg) {
                    msg = format!("{h}\n{msg}");
                }
                // Pull con conflictos (merge/rebase): se informa como resultado, no como fallo.
                let conflicts = super::ops::list_conflicts(state, &c.top).await;
                if !conflicts.is_empty() {
                    return Ok(OpOutcome { ok: false, conflicts, message: msg });
                }
                Err(AppError::Git(if msg.is_empty() { "git falló".into() } else { msg }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_percent() {
        assert_eq!(parse_percent("Receiving objects:  45% (9/20)"), Some(("Receiving objects".into(), 45)));
        assert_eq!(parse_percent("remote: Compressing objects: 100% (5/5), done."), Some(("Compressing objects".into(), 100)));
        assert_eq!(parse_percent("Resolving deltas: 7% (1/14)"), Some(("Resolving deltas".into(), 7)));
        assert_eq!(parse_percent("From /tmp/x"), None);
        assert_eq!(parse_percent("100% done"), None);
    }

    #[test]
    fn auth_errors_get_hint() {
        assert!(auth_hint("fatal: could not read Username for 'https://x': terminal prompts disabled").is_some());
        assert!(auth_hint("fatal: something else").is_none());
    }
}
