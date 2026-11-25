//! Detección de repos git del workspace (multi-raíz) y resumen de estado por repo.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::Serialize;

use super::exec::{ctx_for, run_raw, RunOpts};
use super::status2::{detect_repo_state, git_dir, parse_status_v2, RepoState};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::util::paths::{clean_canonicalize, strip_verbatim};
use crate::util::run_blocking;

/// Profundidad máxima de subcarpetas (relativa a cada raíz) donde se busca `.git`.
pub const REPO_SCAN_DEPTH: usize = 3;
pub const MAX_REPOS: usize = 200;
/// Resúmenes en paralelo.
pub const SUMMARY_PARALLELISM: usize = 4;

const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", ".venv", "venv", "dist", "build", "out", ".next", ".idea", "coverage", "__pycache__", ".gradle",
];

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoEntry {
    /// Toplevel canónico del repo (identidad).
    pub top: PathBuf,
    pub name: String,
    /// Raíz del workspace en la que se encontró.
    pub root: PathBuf,
    /// Subcarpetas por debajo de la raíz (0 = la propia raíz o un ancestro).
    pub depth: usize,
    /// Su top está dentro de otro repo detectado.
    pub nested: bool,
    /// `.git` es un archivo con `gitdir: .../modules/...`.
    pub submodule: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReposResult {
    pub repos: Vec<RepoEntry>,
    pub truncated: bool,
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

/// `.git` de `dir`: `None` si no hay; `Some(es_submódulo)` si lo hay (directorio o archivo).
fn dot_git(dir: &Path) -> Option<bool> {
    let g = dir.join(".git");
    let meta = std::fs::symlink_metadata(&g).ok()?;
    if meta.is_dir() {
        return Some(false);
    }
    if meta.is_file() {
        let text = std::fs::read_to_string(&g).unwrap_or_default();
        let submodule = text.trim().strip_prefix("gitdir:").map(|p| p.replace('\\', "/").contains("/modules/")).unwrap_or(false);
        return Some(submodule);
    }
    None
}

/// Recorre `root` buscando `.git` hasta `REPO_SCAN_DEPTH`, sin seguir symlinks. Devuelve `(dir, depth, submodule)`.
fn scan_root(root: &Path, limit: usize) -> Vec<(PathBuf, usize, bool)> {
    let mut found = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if let Some(sub) = dot_git(&dir) {
            found.push((dir.clone(), depth, sub));
            if found.len() >= limit {
                break;
            }
        }
        if depth >= REPO_SCAN_DEPTH {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            // `file_type` de DirEntry no sigue symlinks: un enlace a carpeta no es `is_dir`.
            let Ok(ft) = e.file_type() else { continue };
            if !ft.is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            stack.push((e.path(), depth + 1));
        }
    }
    found
}

/// Detecta los repos de todas las raíces. Por raíz: toplevel de `rev-parse` (cubre raíz = subcarpeta de un
/// repo) + recorrido de subcarpetas. Dedupe por top canónico; orden: raíz, profundidad, nombre.
pub async fn detect(roots: &[PathBuf]) -> AppResult<ReposResult> {
    let mut entries: Vec<RepoEntry> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut truncated = false;
    for root in roots {
        let mut batch: Vec<RepoEntry> = Vec::new();
        // (1) El repo que contiene a la raíz (puede ser un ancestro).
        if let Ok(out) = super::run_git_in(root, &["rev-parse", "--show-toplevel"]).await {
            let t = out.trim();
            if !t.is_empty() {
                let top = clean_canonicalize(&strip_verbatim(PathBuf::from(t))).unwrap_or_else(|_| PathBuf::from(t));
                if !seen.contains(&top) {
                    let submodule = dot_git(&top).unwrap_or(false);
                    batch.push(RepoEntry { name: name_of(&top), top, root: root.clone(), depth: 0, nested: false, submodule });
                }
            }
        }
        // (2) Recorrido de subcarpetas (bloqueante).
        let r = root.clone();
        let scanned = run_blocking(move || Ok(scan_root(&r, MAX_REPOS + 1))).await?;
        for (dir, depth, submodule) in scanned {
            if batch.iter().any(|e| e.top == dir) {
                continue;
            }
            batch.push(RepoEntry { name: name_of(&dir), top: dir, root: root.clone(), depth, nested: false, submodule });
        }
        batch.sort_by(|a, b| a.depth.cmp(&b.depth).then_with(|| a.name.cmp(&b.name)).then_with(|| a.top.cmp(&b.top)));
        for e in batch {
            if seen.insert(e.top.clone()) {
                entries.push(e);
            }
        }
        if entries.len() > MAX_REPOS {
            truncated = true;
            entries.truncate(MAX_REPOS);
            break;
        }
    }
    let tops: Vec<PathBuf> = entries.iter().map(|e| e.top.clone()).collect();
    for e in &mut entries {
        e.nested = tops.iter().any(|t| *t != e.top && e.top.starts_with(t));
    }
    Ok(ReposResult { repos: entries, truncated })
}

/// Repo "dueño" de un archivo: el top más largo que lo contiene.
#[cfg(test)]
pub fn owner_of<'a>(repos: &'a [RepoEntry], path: &Path) -> Option<&'a RepoEntry> {
    repos.iter().filter(|r| path.starts_with(&r.top)).max_by_key(|r| r.top.components().count())
}

pub async fn detect_and_cache(state: &AppState) -> AppResult<ReposResult> {
    let roots = state.roots()?;
    let res = detect(&roots).await?;
    if let Ok(mut g) = state.git_repos.lock() {
        *g = res.repos.clone();
    }
    Ok(res)
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoSummary {
    pub top: String,
    pub name: String,
    pub branch: Option<String>,
    pub detached: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// Archivos con cambios en total (staged, unstaged, en conflicto y sin seguimiento; sin repetir).
    pub changed: u32,
    pub staged: u32,
    pub untracked: u32,
    pub conflicted: u32,
    /// `merging | rebasing | cherry-picking | reverting | bisecting`; `None` si está limpio.
    pub state: Option<String>,
    /// Si el resumen de ESTE repo falló (el resto no se aborta).
    pub error: Option<String>,
}

fn state_name(s: RepoState) -> Option<String> {
    match s {
        RepoState::Clean => None,
        RepoState::Merging => Some("merging"),
        RepoState::Rebasing => Some("rebasing"),
        RepoState::CherryPicking => Some("cherry-picking"),
        RepoState::Reverting => Some("reverting"),
        RepoState::Bisecting => Some("bisecting"),
    }
    .map(String::from)
}

/// Resumen de un repo (`status --porcelain=v2 --branch -z`; `--untracked-files=normal` si está marcado lento).
pub async fn summary(state: &AppState, repo: &str) -> AppResult<RepoSummary> {
    let c = ctx_for(state, Some(repo)).await?;
    let mode = if state.git_untracked_limited(&c.top) { "--untracked-files=normal" } else { "--untracked-files=all" };
    let out = run_raw(
        state,
        &c.top,
        &["status", "--porcelain=v2", "--branch", "-z", mode],
        RunOpts::default().timeout(std::time::Duration::from_secs(120)),
    )
    .await?;
    if !out.ok() {
        return Err(AppError::Git(out.message()));
    }
    let st = parse_status_v2(&out.stdout);
    let mut dirty: HashSet<&str> = HashSet::new();
    for e in st.staged.iter().chain(&st.unstaged).chain(&st.conflicted).chain(&st.untracked) {
        dirty.insert(e.path.as_str());
    }
    let repo_state = match git_dir(state, &c.top).await {
        Some(gd) => detect_repo_state(&gd),
        None => RepoState::Clean,
    };
    Ok(RepoSummary {
        top: c.top.to_string_lossy().into_owned(),
        name: name_of(&c.top),
        branch: st.info.branch.clone(),
        detached: st.info.detached,
        upstream: st.info.upstream.clone(),
        ahead: st.info.ahead,
        behind: st.info.behind,
        changed: dirty.len() as u32,
        staged: st.staged.len() as u32,
        untracked: st.untracked.len() as u32,
        conflicted: st.conflicted.len() as u32,
        state: state_name(repo_state),
        error: None,
    })
}

/// Resumen de todos los repos (detecta si la caché está vacía). Semáforo de 4; un fallo → entrada con `error`.
pub async fn summary_all(state: &AppState) -> AppResult<Vec<RepoSummary>> {
    let mut repos = state.git_repos.lock().map(|g| g.clone()).unwrap_or_default();
    if repos.is_empty() {
        repos = detect_and_cache(state).await?.repos;
    }
    let out = futures_util::stream::iter(repos)
        .map(|r| async move {
            let top = r.top.to_string_lossy().into_owned();
            match summary(state, &top).await {
                Ok(s) => s,
                Err(e) => RepoSummary { top, name: r.name, error: Some(e.to_string()), ..Default::default() },
            }
        })
        .buffered(SUMMARY_PARALLELISM)
        .collect::<Vec<_>>()
        .await;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn have_git() -> bool {
        let ok = Command::new("git").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
        if !ok {
            eprintln!("git no está en PATH: test omitido");
        }
        ok
    }

    pub(crate) fn g(dir: &Path, args: &[&str]) -> String {
        let o = Command::new("git").args(args).current_dir(dir).output().expect("git");
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    }

    pub(crate) fn init_repo(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        g(dir, &["init", "-q", "-b", "main"]);
        g(dir, &["config", "user.name", "T"]);
        g(dir, &["config", "user.email", "t@e.com"]);
        g(dir, &["config", "commit.gpgsign", "false"]);
    }

    pub(crate) fn commit(dir: &Path, file: &str, content: &str) {
        std::fs::write(dir.join(file), content).unwrap();
        g(dir, &["add", "--", file]);
        g(dir, &["commit", "-q", "-m", file]);
    }

    fn base() -> (tempfile::TempDir, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let b = clean_canonicalize(d.path()).unwrap();
        (d, b)
    }

    fn names(r: &ReposResult) -> Vec<String> {
        r.repos.iter().map(|e| e.top.strip_prefix(&e.root).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()).collect()
    }

    #[tokio::test]
    async fn detect_depth_ignored_nested_worktree_symlink() {
        if !have_git() {
            return;
        }
        let (_d, root) = base();
        init_repo(&root.join("a"));
        commit(&root.join("a"), "f", "1");
        std::fs::create_dir_all(root.join("b")).unwrap(); // b sin repo
        init_repo(&root.join("b/svc1"));
        init_repo(&root.join("b/deep/x/y")); // depth 4: no detectado
        init_repo(&root.join("node_modules/pkg")); // ignorado
        init_repo(&root.join("a/vendor/lib")); // anidado, depth 3
        std::fs::create_dir_all(root.join(".hidden")).unwrap();
        init_repo(&root.join(".hidden/h")); // oculto: ignorado
        // worktree: `.git` es un archivo
        g(&root.join("a"), &["worktree", "add", "-q", root.join("b/wt").to_str().unwrap(), "-b", "wtb"]);
        // symlink a a/ dentro de b/: no duplica
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("a"), root.join("b/link")).unwrap();

        let res = detect(std::slice::from_ref(&root)).await.unwrap();
        let mut n = names(&res);
        assert!(!res.truncated);
        n.sort();
        assert_eq!(n, vec!["a", "a/vendor/lib", "b/svc1", "b/wt"], "{n:?}");
        let by = |p: &str| res.repos.iter().find(|e| e.top == root.join(p)).unwrap().clone();
        assert!(!by("a").nested);
        assert!(by("a/vendor/lib").nested);
        assert_eq!(by("a/vendor/lib").depth, 3);
        assert!(!by("b/wt").submodule, "un worktree no es submódulo");
        // dueño del archivo = el top más largo
        let owner = owner_of(&res.repos, &root.join("a/vendor/lib/src/x.rs")).unwrap();
        assert_eq!(owner.top, root.join("a/vendor/lib"));
        assert_eq!(owner_of(&res.repos, &root.join("a/f")).unwrap().top, root.join("a"));
        assert!(owner_of(&res.repos, &root.join("b/other.txt")).is_none());
    }

    #[tokio::test]
    async fn detect_root_inside_repo_and_multi_root_dedupe() {
        if !have_git() {
            return;
        }
        let (_d, base) = base();
        init_repo(&base.join("mono"));
        std::fs::create_dir_all(base.join("mono/svc/api")).unwrap();
        init_repo(&base.join("other"));
        // raíz = subcarpeta de un repo: el repo contenedor aparece con depth 0
        let res = detect(&[base.join("mono/svc/api"), base.join("other")]).await.unwrap();
        assert_eq!(res.repos.len(), 2);
        assert_eq!(res.repos[0].top, base.join("mono"));
        assert_eq!(res.repos[0].root, base.join("mono/svc/api"));
        assert_eq!(res.repos[0].depth, 0);
        assert_eq!(res.repos[1].top, base.join("other"));
        // dos raíces dentro del mismo repo: un solo repo
        std::fs::create_dir_all(base.join("mono/svc/web")).unwrap();
        let res = detect(&[base.join("mono/svc/api"), base.join("mono/svc/web")]).await.unwrap();
        assert_eq!(res.repos.len(), 1);
    }

    #[tokio::test]
    async fn summary_ahead_behind_and_counts() {
        if !have_git() {
            return;
        }
        let (_d, base) = base();
        let remote = base.join("remote.git");
        std::fs::create_dir_all(&remote).unwrap();
        g(&remote, &["init", "-q", "--bare", "-b", "main"]);
        let a = base.join("a");
        init_repo(&a);
        commit(&a, "one.txt", "1\n");
        g(&a, &["remote", "add", "origin", remote.to_str().unwrap()]);
        g(&a, &["push", "-q", "-u", "origin", "main"]);

        let state = AppState::new();
        state.workspace.write().unwrap().replace(crate::workspace::Workspace::single(a.clone()));
        let top = a.to_str().unwrap();
        let s = summary(&state, top).await.unwrap();
        assert_eq!((s.ahead, s.behind, s.changed), (0, 0, 0));
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.upstream.as_deref(), Some("origin/main"));
        assert!(s.state.is_none());

        // ahead = 1
        commit(&a, "two.txt", "2\n");
        let s = summary(&state, top).await.unwrap();
        assert_eq!((s.ahead, s.behind), (1, 0));
        g(&a, &["push", "-q"]);

        // behind = 1: otro clon sube un commit y A solo hace fetch
        let b = base.join("b");
        g(&base, &["clone", "-q", remote.to_str().unwrap(), "b"]);
        g(&b, &["config", "user.name", "T"]);
        g(&b, &["config", "user.email", "t@e.com"]);
        g(&b, &["config", "commit.gpgsign", "false"]);
        commit(&b, "three.txt", "3\n");
        g(&b, &["push", "-q"]);
        g(&a, &["fetch", "-q"]);
        let s = summary(&state, top).await.unwrap();
        assert_eq!((s.ahead, s.behind), (0, 1));

        // modificado + staged + untracked
        std::fs::write(a.join("one.txt"), "changed\n").unwrap();
        std::fs::write(a.join("new.txt"), "n\n").unwrap();
        g(&a, &["add", "new.txt"]);
        std::fs::write(a.join("loose.txt"), "l\n").unwrap();
        let s = summary(&state, top).await.unwrap();
        assert_eq!((s.staged, s.untracked, s.conflicted), (1, 1, 0));
        assert_eq!(s.changed, 3);
    }

    #[tokio::test]
    async fn summary_conflict_and_state() {
        if !have_git() {
            return;
        }
        let (_d, base) = base();
        let a = base.join("a");
        init_repo(&a);
        commit(&a, "f.txt", "base\n");
        g(&a, &["checkout", "-q", "-b", "side"]);
        commit(&a, "f.txt", "side\n");
        g(&a, &["checkout", "-q", "main"]);
        commit(&a, "f.txt", "main\n");
        let o = Command::new("git").args(["merge", "side"]).current_dir(&a).output().unwrap();
        assert!(!o.status.success());
        let state = AppState::new();
        state.workspace.write().unwrap().replace(crate::workspace::Workspace::single(a.clone()));
        let s = summary(&state, a.to_str().unwrap()).await.unwrap();
        assert_eq!(s.conflicted, 1);
        assert_eq!(s.state.as_deref(), Some("merging"));
    }

    #[tokio::test]
    async fn summary_all_isolates_failures_and_repo_param_is_validated() {
        if !have_git() {
            return;
        }
        let (_d, base) = base();
        let (a, b) = (base.join("ws/a"), base.join("ws/b"));
        init_repo(&a);
        commit(&a, "x", "1");
        init_repo(&b);
        commit(&b, "y", "1");
        let outside = base.join("outside");
        init_repo(&outside);
        let state = AppState::new();
        state.workspace.write().unwrap().replace(crate::workspace::Workspace::single(base.join("ws")));
        let all = summary_all(&state).await.unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().all(|s| s.error.is_none()));
        // un repo cacheado que desaparece no aborta el resto
        std::fs::remove_dir_all(&b).unwrap();
        let all = summary_all(&state).await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all.iter().filter(|s| s.error.is_some()).count(), 1);
        // repo fuera del workspace
        assert!(summary(&state, outside.to_str().unwrap()).await.is_err());
        // subcarpeta de un repo (no es top): rechazada
        std::fs::create_dir_all(a.join("sub")).unwrap();
        assert!(summary(&state, a.join("sub").to_str().unwrap()).await.is_err());
    }

    #[tokio::test]
    async fn git_ops_are_isolated_between_repos_in_different_roots() {
        if !have_git() {
            return;
        }
        let (_d, base) = base();
        let (a, b) = (base.join("a"), base.join("b"));
        init_repo(&a);
        commit(&a, "fa.txt", "1\n");
        init_repo(&b);
        commit(&b, "fb.txt", "1\n");
        let state = AppState::new();
        state.workspace.write().unwrap().replace(crate::workspace::Workspace::new(vec![a.clone(), b.clone()]));
        std::fs::write(a.join("fa.txt"), "changed\n").unwrap();
        std::fs::write(b.join("fb.txt"), "changed\n").unwrap();
        let (sa, sb) = (a.to_str().unwrap(), b.to_str().unwrap());

        // Un archivo de B no se puede operar con repo = A.
        let fb = b.join("fb.txt").to_string_lossy().into_owned();
        assert!(crate::git::ops::stage(&state, Some(sa), vec![fb.clone()]).await.is_err());
        // Con repo = B sí (archivo de la raíz 2).
        crate::git::ops::stage(&state, Some(sb), vec![fb]).await.unwrap();
        // None = repo de la primaria (A): su estado no se contagia del de B.
        let st_a = crate::git::status2::status_v2(&state, None).await.unwrap();
        assert_eq!((st_a.staged.len(), st_a.unstaged.len()), (0, 1));
        let st_b = crate::git::status2::status_v2(&state, Some(sb)).await.unwrap();
        assert_eq!((st_b.staged.len(), st_b.unstaged.len()), (1, 0));
        // Marca anti-bucle y busy por repo: operar en B no marca A.
        assert!(state.git_ran_within(&b, 5_000));
        // repo fuera del workspace / no top: rechazado.
        let outside = base.join("outside");
        init_repo(&outside);
        assert!(crate::git::status2::repo_info(&state, Some(outside.to_str().unwrap())).await.unwrap().top.is_none());
        assert!(ctx_for(&state, Some(outside.to_str().unwrap())).await.is_err());
        assert!(ctx_for(&state, Some(sb)).await.is_ok());
    }
}
