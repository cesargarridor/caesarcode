pub mod blame;
pub mod commands;
pub mod diff;
pub mod exec;
pub mod log;
pub mod ops;
pub mod refs;
pub mod remote;
pub mod repos;
pub mod status2;
pub mod validate;

#[cfg(test)]
mod tests_repo;

use std::path::Path;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::util::proc::no_window_tokio;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitFileStatus {
    #[serde(rename = "M")]
    M,
    #[serde(rename = "U")]
    U,
    #[serde(rename = "D")]
    D,
    #[serde(rename = "A")]
    A,
    #[serde(rename = "R")]
    R,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub files: std::collections::HashMap<String, GitFileStatus>,
}

impl GitStatus {
    pub fn not_repo() -> Self {
        Self { is_repo: false, branch: None, files: Default::default() }
    }
}

/// Comando `git` con la configuración común: sin locks opcionales (no toca `.git/index`),
/// sin prompts, sin consola en Windows y sin stdin.
pub(crate) fn base_command<I, S>(root: &Path, args: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut cmd = Command::new("git");
    cmd.arg("--no-optional-locks")
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    no_window_tokio(&mut cmd);
    cmd
}

async fn exec_git(root: &Path, args: &[&str]) -> AppResult<String> {
    let out = base_command(root, args).output().await;
    finish_git(out, args)
}

fn finish_git(out: std::io::Result<std::process::Output>, args: &[&str]) -> AppResult<String> {
    let out = out.map_err(|e| AppError::Git(format!("no se pudo ejecutar git: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(AppError::Git(if err.is_empty() { format!("git {} falló", args.join(" ")) } else { err }));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Ejecuta `git` en una ruta arbitraria, sin `AppState` (proyectos nuevos, antes de abrir el workspace).
pub async fn run_git_in(cwd: &Path, args: &[&str]) -> AppResult<String> {
    exec_git(cwd, args).await
}

/// Ejecuta `git` en `root`. `Err(Git)` si git no existe o sale con código != 0.
pub async fn run_git(state: &AppState, root: &Path, args: &[&str]) -> AppResult<String> {
    let out = base_command(root, args).output().await;
    // Se marca al terminar: los eventos del watcher llegan ~200 ms después.
    state.git_mark_run(root);
    finish_git(out, args)
}

fn map_status(x: char, y: char) -> GitFileStatus {
    if x == '?' && y == '?' {
        GitFileStatus::U
    } else if x == 'D' || y == 'D' {
        GitFileStatus::D
    } else if x == 'A' || y == 'A' {
        GitFileStatus::A
    } else if x == 'R' || y == 'R' || x == 'C' || y == 'C' {
        GitFileStatus::R
    } else {
        GitFileStatus::M
    }
}

/// Parsea `git status --porcelain=v1 -z`: entradas `XY path\0`; en renombres/copias
/// el siguiente token es la ruta original (se consume).
pub fn parse_porcelain(base: &Path, raw: &str) -> std::collections::HashMap<String, GitFileStatus> {
    let mut files = std::collections::HashMap::new();
    let mut tokens = raw.split('\0');
    while let Some(tok) = tokens.next() {
        let mut chars = tok.chars();
        let (Some(x), Some(y), Some(' ')) = (chars.next(), chars.next(), chars.next()) else {
            continue;
        };
        let rel = chars.as_str();
        if rel.is_empty() || (x == '!' && y == '!') {
            continue;
        }
        if matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C') {
            tokens.next();
        }
        let mut p = base.to_path_buf();
        for part in rel.split('/').filter(|s| !s.is_empty()) {
            p.push(part);
        }
        files.insert(p.to_string_lossy().into_owned(), map_status(x, y));
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain_z() {
        let base = Path::new("/r");
        let raw = " M src/a.rs\0?? new file.txt\0R  new.rs\0old.rs\0D  gone.rs\0A  add.rs\0";
        let f = parse_porcelain(base, raw);
        assert_eq!(f.len(), 5);
        assert_eq!(f[&base.join("src").join("a.rs").to_string_lossy().into_owned()], GitFileStatus::M);
        assert_eq!(f[&base.join("new file.txt").to_string_lossy().into_owned()], GitFileStatus::U);
        assert_eq!(f[&base.join("new.rs").to_string_lossy().into_owned()], GitFileStatus::R);
        assert!(!f.contains_key(&base.join("old.rs").to_string_lossy().into_owned()));
        assert_eq!(f[&base.join("gone.rs").to_string_lossy().into_owned()], GitFileStatus::D);
    }
}
