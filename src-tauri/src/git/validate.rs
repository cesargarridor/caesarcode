//! Validación de argumentos que vienen del frontend antes de tocar `git`.
//! Regla: todo valor que no sea ruta va antes de `--`, nunca empieza por `-`; las rutas van después de `--`.

use std::path::{Component, Path, PathBuf};

use super::exec::{run_raw, run_str, Ctx, RunOpts};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::util::paths::{clean_canonicalize, ensure_in_roots};

fn bad(msg: impl Into<String>) -> AppError {
    AppError::InvalidArgument(msg.into())
}

/// Rechaza vacío, NUL/saltos de línea y cualquier valor que empiece por `-`.
pub fn reject_dash(what: &str, s: &str) -> AppResult<()> {
    if s.is_empty() {
        return Err(bad(format!("{what} vacío")));
    }
    if s.starts_with('-') {
        return Err(bad(format!("{what} no puede empezar por '-': {s}")));
    }
    if s.chars().any(|c| c == '\0' || c == '\n' || c == '\r') {
        return Err(bad(format!("{what} contiene caracteres de control")));
    }
    Ok(())
}

/// Refname conservador (equivale a la regex del plan, sin lookaround).
pub fn valid_refname(s: &str) -> bool {
    if s.is_empty() || s.starts_with('-') || s.starts_with('/') || s.ends_with('/') || s.ends_with('.') || s.ends_with(".lock") {
        return false;
    }
    if s.contains("..") || s.contains("//") || s.contains("@{") {
        return false;
    }
    if s == "@" {
        return false;
    }
    if !s.chars().all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '/' | '-' | '+')) {
        return false;
    }
    // Ningún componente puede empezar por '.' ni terminar en '.lock'.
    s.split('/').all(|p| !p.starts_with('.') && !p.ends_with(".lock"))
}

pub fn refname(what: &str, s: &str) -> AppResult<()> {
    reject_dash(what, s)?;
    if valid_refname(s) {
        Ok(())
    } else {
        Err(bad(format!("{what} inválido: {s}")))
    }
}

pub fn valid_sha(s: &str) -> bool {
    (4..=64).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn sha(s: &str) -> AppResult<()> {
    if valid_sha(s) {
        Ok(())
    } else {
        Err(bad(format!("sha inválido: {s}")))
    }
}

/// `HEAD` o `HEAD~n` / `HEAD^`.
fn valid_head_rev(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("HEAD") else { return false };
    if rest.is_empty() || rest == "^" {
        return true;
    }
    match rest.strip_prefix('~') {
        Some(n) => !n.is_empty() && n.len() <= 6 && n.chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

/// Revisión aceptada: sha, `HEAD`, `HEAD~n` o refname válido (rama/tag/remota).
pub fn rev(what: &str, s: &str) -> AppResult<()> {
    reject_dash(what, s)?;
    if valid_sha(s) || valid_head_rev(s) || valid_refname(s) {
        Ok(())
    } else {
        Err(bad(format!("{what} inválido: {s}")))
    }
}

/// `git check-ref-format --branch`: validación definitiva de git tras la manual.
pub async fn check_branch_name(state: &AppState, ctx: &Ctx, name: &str) -> AppResult<()> {
    refname("nombre de rama", name)?;
    let out = run_raw(state, &ctx.top, &["check-ref-format", "--branch", name], RunOpts::default()).await?;
    if out.ok() {
        Ok(())
    } else {
        Err(bad(format!("nombre de rama inválido: {name}")))
    }
}

pub async fn check_tag_name(state: &AppState, ctx: &Ctx, name: &str) -> AppResult<()> {
    refname("nombre de tag", name)?;
    let full = format!("refs/tags/{name}");
    let out = run_raw(state, &ctx.top, &["check-ref-format", full.as_str()], RunOpts::default()).await?;
    if out.ok() {
        Ok(())
    } else {
        Err(bad(format!("nombre de tag inválido: {name}")))
    }
}

pub async fn remote_names(state: &AppState, ctx: &Ctx) -> AppResult<Vec<String>> {
    let out = run_str(state, &ctx.top, &["remote"]).await?;
    Ok(out.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
}

/// El remoto debe existir en `git remote`.
pub async fn remote(state: &AppState, ctx: &Ctx, name: &str) -> AppResult<()> {
    reject_dash("remoto", name)?;
    if remote_names(state, ctx).await?.iter().any(|r| r == name) {
        Ok(())
    } else {
        Err(bad(format!("el remoto no existe: {name}")))
    }
}

/// Convierte las rutas del frontend a relativas al top del repo con `/`.
/// Acepta absolutas (dentro de alguna raíz del workspace Y del repo `ctx.top`) o relativas al top (sin `..`).
pub fn rel_paths(ctx: &Ctx, paths: &[String]) -> AppResult<Vec<String>> {
    if paths.is_empty() {
        return Err(bad("sin rutas"));
    }
    paths.iter().map(|p| rel_path(ctx, p)).collect()
}

pub fn rel_path(ctx: &Ctx, p: &str) -> AppResult<String> {
    if p.is_empty() || p.contains('\0') {
        return Err(bad("ruta inválida"));
    }
    let path = Path::new(p);
    let rel: PathBuf = if path.is_absolute() {
        // Debe estar dentro de alguna raíz del workspace (y, abajo, dentro del repo).
        let resolved = match ensure_in_roots(&ctx.roots, path) {
            Ok((r, _)) => r,
            // Archivo borrado cuyo padre ya no existe: comprobación léxica.
            Err(AppError::NotFound(_)) => {
                lexical_inside(&ctx.roots, path)?;
                path.to_path_buf()
            }
            Err(e) => return Err(e),
        };
        match resolved.strip_prefix(&ctx.top) {
            Ok(r) => r.to_path_buf(),
            Err(_) => {
                // El top puede diferir por symlinks: se reintenta con el top canonicalizado.
                let top = clean_canonicalize(&ctx.top).unwrap_or_else(|_| ctx.top.clone());
                resolved
                    .strip_prefix(&top)
                    .map_err(|_| AppError::OutsideWorkspace(p.to_string()))?
                    .to_path_buf()
            }
        }
    } else {
        if path.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_) | Component::RootDir)) {
            return Err(AppError::OutsideWorkspace(p.to_string()));
        }
        path.to_path_buf()
    };
    let mut parts: Vec<String> = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return Err(AppError::OutsideWorkspace(p.to_string())),
        }
    }
    if parts.is_empty() {
        // La raíz del repo ("." ) como pathspec.
        return Ok(".".into());
    }
    Ok(parts.join("/"))
}

fn lexical_inside(roots: &[PathBuf], p: &Path) -> AppResult<()> {
    if p.components().any(|c| matches!(c, Component::ParentDir)) || !roots.iter().any(|r| p.starts_with(r)) {
        return Err(AppError::OutsideWorkspace(p.display().to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refnames() {
        for ok in ["main", "feature/x-1", "v1.2.3", "a_b", "release/1.0", "ñandú"] {
            assert!(valid_refname(ok), "{ok}");
        }
        for bad in ["-x", "a..b", "x@{1}", "foo.lock", "a//b", "/a", "a/", "HEAD:evil", "--upload-pack=x", "a b", "a.", ".hidden", "a/.b", "", "@", "a~1", "a^"] {
            assert!(!valid_refname(bad), "{bad}");
        }
    }

    #[test]
    fn dash_and_revs() {
        assert!(reject_dash("x", "-x").is_err());
        assert!(reject_dash("x", "--upload-pack=evil").is_err());
        assert!(reject_dash("x", "").is_err());
        assert!(reject_dash("x", "a\nb").is_err());
        assert!(reject_dash("x", "ok").is_ok());
        assert!(rev("r", "HEAD~3").is_ok());
        assert!(rev("r", "HEAD").is_ok());
        assert!(rev("r", "abcd1234").is_ok());
        assert!(rev("r", "origin/main").is_ok());
        assert!(rev("r", "HEAD:evil").is_err());
        assert!(rev("r", "HEAD~x").is_err());
        assert!(sha("abc").is_err());
        assert!(sha("abcd").is_ok());
        assert!(sha("zzzz").is_err());
    }

    #[test]
    fn paths_relative_and_outside() {
        let d = tempfile::tempdir().unwrap();
        let ws = clean_canonicalize(d.path()).unwrap();
        std::fs::create_dir_all(ws.join("src")).unwrap();
        std::fs::write(ws.join("src/a.rs"), "x").unwrap();
        let ctx = Ctx { roots: vec![ws.clone()], top: ws.clone() };
        assert_eq!(rel_path(&ctx, ws.join("src/a.rs").to_str().unwrap()).unwrap(), "src/a.rs");
        assert_eq!(rel_path(&ctx, "src/a.rs").unwrap(), "src/a.rs");
        assert_eq!(rel_path(&ctx, "-weird.txt").unwrap(), "-weird.txt");
        assert!(rel_path(&ctx, "../etc/passwd").is_err());
        assert!(rel_path(&ctx, "/etc/passwd").is_err());
        assert!(rel_path(&ctx, ws.join("src/../../x").to_str().unwrap()).is_err());
        // Borrado en una carpeta que ya no existe.
        assert_eq!(rel_path(&ctx, ws.join("gone/dir/f.txt").to_str().unwrap()).unwrap(), "gone/dir/f.txt");
        assert!(rel_paths(&ctx, &[]).is_err());
    }
}
