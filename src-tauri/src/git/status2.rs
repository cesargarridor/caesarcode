//! `git status --porcelain=v2 --branch -z` → `StatusV2` / `RepoInfo`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;

use super::exec::{ctx_for, run_raw, RunOpts};
use crate::error::AppResult;
use crate::state::AppState;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RepoState {
    Clean,
    Merging,
    Rebasing,
    CherryPicking,
    Reverting,
    Bisecting,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub is_repo: bool,
    pub top: Option<String>,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub state: RepoState,
    pub git_version: Option<String>,
    /// `true` si el status usa `--untracked-files=normal` por lentitud del repo.
    pub untracked_limited: bool,
}

impl RepoInfo {
    pub fn not_repo() -> Self {
        Self {
            is_repo: false,
            top: None,
            head: None,
            branch: None,
            detached: false,
            upstream: None,
            ahead: 0,
            behind: 0,
            state: RepoState::Clean,
            git_version: None,
            untracked_limited: false,
        }
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EntryKind {
    Ordinary,
    Renamed,
    Copied,
    Unmerged,
    Untracked,
    #[allow(dead_code)] // parte del contrato TS; no se piden ignorados
    Ignored,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatusEntry {
    pub path: String,
    pub orig_path: Option<String>,
    pub index: char,
    pub worktree: char,
    pub kind: EntryKind,
    pub conflict: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StatusV2 {
    pub info: RepoInfo,
    pub staged: Vec<StatusEntry>,
    pub unstaged: Vec<StatusEntry>,
    pub untracked: Vec<StatusEntry>,
    pub conflicted: Vec<StatusEntry>,
}

impl StatusV2 {
    pub fn not_repo() -> Self {
        Self { info: RepoInfo::not_repo(), staged: vec![], unstaged: vec![], untracked: vec![], conflicted: vec![] }
    }
}

fn norm(c: char) -> char {
    if c == '.' {
        ' '
    } else {
        c
    }
}

/// Parser puro de bytes porcelain v2 (`-z`). `info` se rellena con las cabeceras `# branch.*`.
pub fn parse_status_v2(raw: &[u8]) -> StatusV2 {
    let text = String::from_utf8_lossy(raw);
    let mut st = StatusV2::not_repo();
    st.info.is_repo = true;
    let mut toks = text.split('\0');
    while let Some(tok) = toks.next() {
        if tok.is_empty() {
            continue;
        }
        if let Some(h) = tok.strip_prefix("# ") {
            parse_header(h, &mut st.info);
            continue;
        }
        let mut chars = tok.chars();
        let kind = chars.next().unwrap_or(' ');
        match kind {
            '1' => {
                // 1 XY sub mH mI mW hH hI path
                let f: Vec<&str> = tok.splitn(9, ' ').collect();
                if f.len() < 9 {
                    continue;
                }
                push_entry(&mut st, f[1], f[8], None, EntryKind::Ordinary);
            }
            '2' => {
                // 2 XY sub mH mI mW hH hI Xscore path \0 orig
                let f: Vec<&str> = tok.splitn(10, ' ').collect();
                let orig = toks.next().unwrap_or("").to_string();
                if f.len() < 10 {
                    continue;
                }
                let k = if f[8].starts_with('C') { EntryKind::Copied } else { EntryKind::Renamed };
                push_entry(&mut st, f[1], f[9], Some(orig), k);
            }
            'u' => {
                // u XY sub m1 m2 m3 mW h1 h2 h3 path
                let f: Vec<&str> = tok.splitn(11, ' ').collect();
                if f.len() < 11 {
                    continue;
                }
                let xy = f[1].to_string();
                let mut c = xy.chars();
                st.conflicted.push(StatusEntry {
                    path: f[10].to_string(),
                    orig_path: None,
                    index: c.next().unwrap_or('U'),
                    worktree: c.next().unwrap_or('U'),
                    kind: EntryKind::Unmerged,
                    conflict: Some(xy),
                });
            }
            '?' => {
                if let Some(p) = tok.strip_prefix("? ") {
                    st.untracked.push(StatusEntry {
                        path: p.to_string(),
                        orig_path: None,
                        index: '?',
                        worktree: '?',
                        kind: EntryKind::Untracked,
                        conflict: None,
                    });
                }
            }
            // '!' (ignorados) no se piden; si llegan, se descartan.
            _ => {}
        }
    }
    st
}

fn push_entry(st: &mut StatusV2, xy: &str, path: &str, orig: Option<String>, kind: EntryKind) {
    let mut c = xy.chars();
    let (x, y) = (norm(c.next().unwrap_or('.')), norm(c.next().unwrap_or('.')));
    let e = StatusEntry { path: path.to_string(), orig_path: orig, index: x, worktree: y, kind, conflict: None };
    if x != ' ' {
        st.staged.push(e.clone());
    }
    if y != ' ' {
        st.unstaged.push(e);
    }
}

fn parse_header(h: &str, info: &mut RepoInfo) {
    if let Some(v) = h.strip_prefix("branch.oid ") {
        info.head = if v == "(initial)" { None } else { Some(v.to_string()) };
    } else if let Some(v) = h.strip_prefix("branch.head ") {
        if v == "(detached)" {
            info.detached = true;
            info.branch = None;
        } else {
            info.branch = Some(v.to_string());
        }
    } else if let Some(v) = h.strip_prefix("branch.upstream ") {
        info.upstream = Some(v.to_string());
    } else if let Some(v) = h.strip_prefix("branch.ab ") {
        for part in v.split_whitespace() {
            if let Some(n) = part.strip_prefix('+') {
                info.ahead = n.parse().unwrap_or(0);
            } else if let Some(n) = part.strip_prefix('-') {
                info.behind = n.parse().unwrap_or(0);
            }
        }
    }
}

/// Estado del repo según los marcadores de `.git`.
pub fn detect_repo_state(git_dir: &Path) -> RepoState {
    if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
        RepoState::Rebasing
    } else if git_dir.join("MERGE_HEAD").exists() {
        RepoState::Merging
    } else if git_dir.join("CHERRY_PICK_HEAD").exists() {
        RepoState::CherryPicking
    } else if git_dir.join("REVERT_HEAD").exists() {
        RepoState::Reverting
    } else if git_dir.join("BISECT_LOG").exists() {
        RepoState::Bisecting
    } else {
        RepoState::Clean
    }
}

async fn git_version(state: &AppState, cwd: &Path) -> Option<String> {
    if let Ok(g) = state.git_version.lock() {
        if g.is_some() {
            return g.clone();
        }
    }
    let out = run_raw(state, cwd, &["--version"], RunOpts::default()).await.ok()?;
    let v = out.out_str().trim().trim_start_matches("git version ").to_string();
    let v = if v.is_empty() { None } else { Some(v) };
    if let Ok(mut g) = state.git_version.lock() {
        *g = v.clone();
    }
    v
}

pub(super) async fn git_dir(state: &AppState, top: &Path) -> Option<PathBuf> {
    let out = run_raw(state, top, &["rev-parse", "--absolute-git-dir"], RunOpts::default()).await.ok()?;
    let s = out.out_str().trim().to_string();
    if out.ok() && !s.is_empty() {
        Some(PathBuf::from(s))
    } else {
        None
    }
}

async fn fill_info(state: &AppState, top: &Path, info: &mut RepoInfo) {
    info.top = Some(top.to_string_lossy().into_owned());
    info.git_version = git_version(state, top).await;
    if let Some(gd) = git_dir(state, top).await {
        info.state = detect_repo_state(&gd);
    }
    info.untracked_limited = state.git_untracked_limited(top);
}

/// `git_status_v2`. Fuera de un repo (o sin git) devuelve `not_repo` sin error.
pub async fn status_v2(state: &AppState, repo: Option<&str>) -> AppResult<StatusV2> {
    let Ok(c) = ctx_for(state, repo).await else { return Ok(StatusV2::not_repo()) };
    let limited = state.git_untracked_limited(&c.top);
    let mode = if limited { "--untracked-files=normal" } else { "--untracked-files=all" };
    let t0 = Instant::now();
    let out = run_raw(
        state,
        &c.top,
        &["status", "--porcelain=v2", "--branch", "-z", mode],
        RunOpts::default().timeout(std::time::Duration::from_secs(120)),
    )
    .await?;
    if !out.ok() {
        return Ok(StatusV2::not_repo());
    }
    if !limited && t0.elapsed().as_secs() >= 3 {
        if let Ok(mut set) = state.git_untracked_normal.lock() {
            set.insert(c.top.clone());
        }
    }
    let mut st = parse_status_v2(&out.stdout);
    fill_info(state, &c.top, &mut st.info).await;
    Ok(st)
}

/// `git_repo_info`: solo cabeceras (sin listar archivos no rastreados).
pub async fn repo_info(state: &AppState, repo: Option<&str>) -> AppResult<RepoInfo> {
    let Ok(c) = ctx_for(state, repo).await else { return Ok(RepoInfo::not_repo()) };
    let out = run_raw(state, &c.top, &["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=no"], RunOpts::default()).await?;
    if !out.ok() {
        return Ok(RepoInfo::not_repo());
    }
    let mut info = parse_status_v2(&out.stdout).info;
    fill_info(state, &c.top, &mut info).await;
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixture() {
        let raw = concat!(
            "# branch.oid 0123456789abcdef0123456789abcdef01234567\0",
            "# branch.head feature/ñ\0",
            "# branch.upstream origin/feature/ñ\0",
            "# branch.ab +2 -1\0",
            "1 .M N... 100644 100644 100644 aaa bbb src/a b.rs\0",
            "1 M. N... 100644 100644 100644 aaa bbb staged.rs\0",
            "1 MM N... 100644 100644 100644 aaa bbb both.rs\0",
            "2 R. N... 100644 100644 100644 aaa bbb R100 nuevo ñandú.rs\0viejo nombre.rs\0",
            "u UU N... 100644 100644 100644 100644 a b c conflict file.txt\0",
            "1 A. S... 160000 160000 160000 aaa bbb sub/mod\0",
            "? untracked dir/new.txt\0",
        );
        let st = parse_status_v2(raw.as_bytes());
        assert!(st.info.is_repo);
        assert_eq!(st.info.branch.as_deref(), Some("feature/ñ"));
        assert_eq!(st.info.upstream.as_deref(), Some("origin/feature/ñ"));
        assert_eq!((st.info.ahead, st.info.behind), (2, 1));
        assert!(!st.info.detached);
        // staged: staged.rs, both.rs, rename, submódulo
        let staged: Vec<_> = st.staged.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(staged, vec!["staged.rs", "both.rs", "nuevo ñandú.rs", "sub/mod"]);
        let unstaged: Vec<_> = st.unstaged.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(unstaged, vec!["src/a b.rs", "both.rs"]);
        let r = st.staged.iter().find(|e| e.kind == EntryKind::Renamed).unwrap();
        assert_eq!(r.orig_path.as_deref(), Some("viejo nombre.rs"));
        assert_eq!(r.index, 'R');
        assert_eq!(r.worktree, ' ');
        assert_eq!(st.conflicted.len(), 1);
        assert_eq!(st.conflicted[0].path, "conflict file.txt");
        assert_eq!(st.conflicted[0].conflict.as_deref(), Some("UU"));
        assert_eq!(st.untracked[0].path, "untracked dir/new.txt");
    }

    #[test]
    fn parses_detached_and_initial() {
        let st = parse_status_v2(b"# branch.oid (initial)\0# branch.head (detached)\0");
        assert!(st.info.detached);
        assert!(st.info.head.is_none());
        assert!(st.info.branch.is_none());
    }
}
