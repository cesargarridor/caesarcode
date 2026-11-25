//! Generadores por CLI (mvn, npm, ng/npx, cargo, go, python, gradle) con streaming de salida.
//! Siempre `tokio::process::Command` con vector de argumentos: nada pasa por un shell.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use super::Reporter;
use crate::error::{AppError, AppResult};
use crate::services::process_tree;
use crate::util::proc::no_window_tokio;

/// Tiempo total máximo de una herramienta.
pub const TOTAL_TIMEOUT: Duration = Duration::from_secs(600);
/// Sin salida durante este tiempo se asume que espera entrada (prompt) y se cancela.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_LINE: usize = 4000;

#[derive(Clone, Debug)]
pub struct CliSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    /// Cambia el tiempo de inactividad (tests).
    pub idle: Option<Duration>,
}

impl CliSpec {
    pub fn new(program: impl Into<PathBuf>, args: &[&str], cwd: &Path) -> Self {
        Self {
            program: program.into(),
            args: args.iter().map(|s| (*s).to_string()).collect(),
            cwd: cwd.to_path_buf(),
            env: Vec::new(),
            idle: None,
        }
    }
    pub fn env(mut self, k: &str, v: &str) -> Self {
        self.env.push((k.into(), v.into()));
        self
    }
}

/// Resuelve un binario del PATH o devuelve un error claro "falta X".
pub fn require(bin: &str, hint: &str) -> AppResult<PathBuf> {
    which::which(bin).map_err(|_| {
        AppError::Other(format!("Falta la herramienta '{bin}' en el PATH. {hint}"))
    })
}

fn clean_line(raw: &str) -> String {
    // Las barras de progreso usan '\r': nos quedamos con el último tramo no vacío.
    let last = raw.rsplit('\r').find(|p| !p.trim().is_empty()).unwrap_or("");
    let stripped = strip_ansi_escapes::strip(last.as_bytes());
    let s = String::from_utf8_lossy(&stripped);
    let mut out: String = s.trim_end().chars().take(MAX_LINE).collect();
    if out.chars().count() == MAX_LINE {
        out.push('…');
    }
    out
}

/// Ejecuta la herramienta emitiendo cada línea como `GenEvent::Output`. `Ok` solo con código 0.
/// Cancelación (árbol de procesos completo), timeout total e inactividad.
pub async fn run_cli(spec: CliSpec, rep: &Reporter) -> AppResult<()> {
    rep.alive()?;
    let shown = format!(
        "$ {} {}",
        spec.program.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        spec.args.join(" ")
    );
    rep.output(shown, "stdout");

    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .current_dir(&spec.cwd)
        .env("CI", "1")
        .env("npm_config_yes", "true")
        .env("npm_config_update_notifier", "false")
        .env("NG_CLI_ANALYTICS", "false")
        .env("NO_COLOR", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
    no_window_tokio(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::Other(format!("no se pudo ejecutar {}: {e}", spec.program.display())))?;
    let pid = child.id();
    let mut out = BufReader::new(child.stdout.take().ok_or_else(|| AppError::Other("sin stdout".into()))?).lines();
    let mut err = BufReader::new(child.stderr.take().ok_or_else(|| AppError::Other("sin stderr".into()))?).lines();
    let (mut out_done, mut err_done) = (false, false);
    let mut cancel = rep.cancel_rx();
    let deadline = tokio::time::Instant::now() + TOTAL_TIMEOUT;
    let idle = spec.idle.unwrap_or(IDLE_TIMEOUT);
    let mut failure: Option<String> = None;

    while !(out_done && err_done) {
        tokio::select! {
            l = out.next_line(), if !out_done => match l {
                Ok(Some(line)) => rep.output(clean_line(&line), "stdout"),
                _ => out_done = true,
            },
            l = err.next_line(), if !err_done => match l {
                Ok(Some(line)) => rep.output(clean_line(&line), "stderr"),
                _ => err_done = true,
            },
            _ = super::wait_cancel(&mut cancel) => {
                failure = Some("Cancelado".into());
                break;
            },
            _ = tokio::time::sleep_until(deadline) => {
                failure = Some("La herramienta superó el tiempo máximo (10 min)".into());
                break;
            },
            _ = tokio::time::sleep(idle) => {
                failure = Some(format!(
                    "La herramienta no produjo salida en {} s: probablemente espera entrada interactiva. Cancelada.",
                    idle.as_secs()
                ));
                break;
            },
        }
        if rep.is_cancelled() {
            failure = Some("Cancelado".into());
            break;
        }
    }

    if let Some(msg) = failure {
        if let Some(pid) = pid {
            let _ = tokio::task::spawn_blocking(move || process_tree::kill_tree(pid, Duration::from_millis(800), false)).await;
        }
        let _ = child.kill().await;
        let _ = child.wait().await;
        return Err(AppError::Other(msg));
    }
    let status = child.wait().await.map_err(|e| AppError::Other(format!("error esperando al proceso: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::Other(format!(
            "{} terminó con código {}",
            spec.program.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            status.code().map(|c| c.to_string()).unwrap_or_else(|| "desconocido".into())
        )))
    }
}

// ---------- Constructores de comandos (todos validados antes por `validate::request`) ----------

pub fn maven_archetype(mvn: PathBuf, location: &Path, a: &super::ArchetypeRef, group: &str, artifact: &str, pkg: &str, jdk: Option<&str>) -> CliSpec {
    let mut spec = CliSpec::new(
        mvn,
        &[
            "-B",
            "archetype:generate",
            &format!("-DarchetypeGroupId={}", a.group),
            &format!("-DarchetypeArtifactId={}", a.artifact),
            &format!("-DarchetypeVersion={}", a.version),
            &format!("-DgroupId={group}"),
            &format!("-DartifactId={artifact}"),
            &format!("-Dpackage={pkg}"),
            "-DinteractiveMode=false",
        ],
        location,
    );
    if let Some(h) = jdk {
        spec = spec.env("JAVA_HOME", h);
    }
    spec
}

pub fn npm_create(npm: PathBuf, location: &Path, initializer: &str, name: &str, extra: &[&str]) -> CliSpec {
    let mut args = vec!["create", initializer, name, "--"];
    args.extend_from_slice(extra);
    CliSpec::new(npm, &args, location)
}

/// `ng new` si hay `ng`; si no `npx -y @angular/cli@latest new`.
pub fn angular_new(tool: &AngularTool, location: &Path, name: &str, routing: bool, style: &str) -> CliSpec {
    let routing_arg = format!("--routing={routing}");
    let style_arg = format!("--style={style}");
    let tail = ["new", name, "--defaults", "--skip-git", routing_arg.as_str(), style_arg.as_str()];
    match tool {
        AngularTool::Ng(ng) => CliSpec::new(ng.clone(), &tail, location),
        AngularTool::Npx(npx) => {
            let mut args = vec!["-y", "@angular/cli@latest"];
            args.extend_from_slice(&tail);
            CliSpec::new(npx.clone(), &args, location)
        }
    }
}

pub enum AngularTool {
    Ng(PathBuf),
    Npx(PathBuf),
}

pub fn find_angular() -> Option<AngularTool> {
    which::which("ng").map(AngularTool::Ng).or_else(|_| which::which("npx").map(AngularTool::Npx)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::GenEvent;
    use std::sync::{Arc, Mutex};

    fn collector() -> (Reporter, Arc<Mutex<Vec<GenEvent>>>) {
        let ev = Arc::new(Mutex::new(Vec::new()));
        let e2 = ev.clone();
        (Reporter::detached(Arc::new(move |g| { e2.lock().unwrap().push(g); true })), ev)
    }

    #[test]
    fn builders_use_arg_vectors() {
        let a = crate::projects::ArchetypeRef { group: "org.apache.maven.archetypes".into(), artifact: "maven-archetype-quickstart".into(), version: "1.5".into() };
        let s = maven_archetype("/bin/mvn".into(), Path::new("/tmp"), &a, "com.example", "demo", "com.example.demo", None);
        assert!(s.args.contains(&"-DinteractiveMode=false".to_string()));
        assert!(s.args.contains(&"-DartifactId=demo".to_string()));
        let n = npm_create("/bin/npm".into(), Path::new("/tmp"), "vite@latest", "app", &["--template", "react-ts"]);
        assert_eq!(n.args, ["create", "vite@latest", "app", "--", "--template", "react-ts"]);
        let g = angular_new(&AngularTool::Npx("/bin/npx".into()), Path::new("/tmp"), "app", true, "scss");
        assert_eq!(g.args[..3], ["-y", "@angular/cli@latest", "new"]);
        assert!(g.args.contains(&"--routing=true".to_string()));
    }

    #[test]
    fn clean_line_strips_ansi_and_cr() {
        assert_eq!(clean_line("\u{1b}[32mok\u{1b}[0m"), "ok");
        assert_eq!(clean_line("10%\r50%\r100%"), "100%");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn streams_output_and_reports_exit_code() {
        let (rep, ev) = collector();
        let d = tempfile::tempdir().unwrap();
        run_cli(CliSpec::new("/bin/sh", &["-c", "echo hola; echo err >&2"], d.path()), &rep).await.unwrap();
        {
            let events = ev.lock().unwrap();
            assert!(events.iter().any(|e| matches!(e, GenEvent::Output { line, stream } if line == "hola" && stream == "stdout")));
            assert!(events.iter().any(|e| matches!(e, GenEvent::Output { line, stream } if line == "err" && stream == "stderr")));
        }
        let (rep, _) = collector();
        assert!(run_cli(CliSpec::new("/bin/sh", &["-c", "exit 3"], d.path()), &rep).await.is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancel_kills_process_tree() {
        use crate::projects::Reporter;
        let d = tempfile::tempdir().unwrap();
        let (tx, rx) = tokio::sync::watch::channel(false);
        let rep = Reporter::new(Arc::new(|_| true), rx);
        let spec = CliSpec::new("/bin/sh", &["-c", "sleep 305 & sleep 306; wait"], d.path());
        let h = tokio::spawn(async move { run_cli(spec, &rep).await });
        tokio::time::sleep(Duration::from_millis(400)).await;
        tx.send(true).unwrap();
        let r = tokio::time::timeout(Duration::from_secs(8), h).await.expect("termina").unwrap();
        assert!(r.is_err());
        tokio::time::sleep(Duration::from_millis(300)).await;
        let sys = process_tree::snapshot_system();
        let alive = sys.processes().values().any(|p| p.cmd().iter().any(|a| a == "305" || a == "306"));
        assert!(!alive, "sleep huérfano tras cancelar");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn idle_prompt_is_cancelled() {
        let (rep, _) = collector();
        let d = tempfile::tempdir().unwrap();
        let mut spec = CliSpec::new("/bin/sh", &["-c", "sleep 307"], d.path());
        spec.idle = Some(Duration::from_millis(300));
        let e = run_cli(spec, &rep).await.unwrap_err();
        assert!(e.to_string().contains("entrada interactiva"));
    }

    #[test]
    fn missing_tool_message_is_clear() {
        let e = require("herramienta-que-no-existe-xyz", "Instálala.").unwrap_err();
        assert!(e.to_string().contains("herramienta-que-no-existe-xyz") && e.to_string().contains("Instálala"));
    }
}
