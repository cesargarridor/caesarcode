//! `git log` paginado, detalle de commit e historial de archivo. Parseo por NUL (`%x00`) y RS (`%x1e`).

use serde::{Deserialize, Serialize};

use super::exec::{ctx_for, run_ok, run_raw, RunOpts};
use super::validate;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RefKind {
    Head,
    Local,
    Remote,
    Tag,
    Stash,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RefLabel {
    pub name: String,
    pub kind: RefKind,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CommitRow {
    pub sha: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    pub time: i64,
    pub subject: String,
    pub refs: Vec<RefLabel>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LogPage {
    pub commits: Vec<CommitRow>,
    pub has_more: bool,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct LogQuery {
    pub skip: u32,
    pub limit: u32,
    #[serde(default)]
    pub all: bool,
    pub path: Option<String>,
    pub search: Option<String>,
    pub author: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    pub orig_path: Option<String>,
    pub status: char,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    pub row: CommitRow,
    pub body: String,
    pub committer: String,
    pub commit_time: i64,
    pub files: Vec<ChangedFile>,
}

pub const LOG_FORMAT: &str = "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%D%x00%s%x1e";
pub const MAX_PAGE: u32 = 2000;

/// `%D` → etiquetas. `remotes` desambigua `origin/x` (remota) de `feature/x` (local).
pub fn parse_decorations(d: &str, remotes: &[String]) -> Vec<RefLabel> {
    let mut out = Vec::new();
    for part in d.split(", ").map(str::trim).filter(|p| !p.is_empty()) {
        if let Some(b) = part.strip_prefix("HEAD -> ") {
            out.push(RefLabel { name: b.to_string(), kind: RefKind::Head });
        } else if part == "HEAD" {
            out.push(RefLabel { name: "HEAD".into(), kind: RefKind::Head });
        } else if let Some(t) = part.strip_prefix("tag: ") {
            out.push(RefLabel { name: t.to_string(), kind: RefKind::Tag });
        } else if part == "refs/stash" {
            out.push(RefLabel { name: "stash".into(), kind: RefKind::Stash });
        } else if remotes.iter().any(|r| part.strip_prefix(r.as_str()).is_some_and(|rest| rest.starts_with('/'))) {
            out.push(RefLabel { name: part.to_string(), kind: RefKind::Remote });
        } else {
            out.push(RefLabel { name: part.to_string(), kind: RefKind::Local });
        }
    }
    out
}

/// Parsea la salida de `LOG_FORMAT`.
pub fn parse_log(raw: &[u8], remotes: &[String]) -> Vec<CommitRow> {
    let text = String::from_utf8_lossy(raw);
    let mut rows = Vec::new();
    for rec in text.split('\x1e') {
        let rec = rec.trim_start_matches(['\n', '\r']);
        if rec.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = rec.split('\0').collect();
        if f.len() < 7 {
            continue;
        }
        rows.push(CommitRow {
            sha: f[0].to_string(),
            parents: f[1].split_whitespace().map(str::to_string).collect(),
            author: f[2].to_string(),
            email: f[3].to_string(),
            time: f[4].parse().unwrap_or(0),
            refs: parse_decorations(f[5], remotes),
            subject: f[6].to_string(),
        });
    }
    rows
}

fn no_commits_yet(msg: &str) -> bool {
    msg.contains("does not have any commits yet") || msg.contains("bad default revision") || msg.contains("unknown revision")
}

pub async fn log(state: &AppState, repo: Option<&str>, q: LogQuery) -> AppResult<LogPage> {
    let c = ctx_for(state, repo).await?;
    let limit = q.limit.clamp(1, MAX_PAGE);
    let remotes = validate::remote_names(state, &c).await.unwrap_or_default();
    let mut args: Vec<String> = vec![
        "log".into(),
        "--topo-order".into(),
        LOG_FORMAT.into(),
        format!("--skip={}", q.skip),
        format!("--max-count={}", limit + 1),
    ];
    if q.all {
        args.push("--exclude=refs/stash".into());
        args.push("--all".into());
    }
    if let Some(s) = q.search.as_deref().filter(|s| !s.is_empty()) {
        validate_value(s)?;
        args.push("--fixed-strings".into());
        args.push("--regexp-ignore-case".into());
        args.push(format!("--grep={s}"));
    }
    if let Some(a) = q.author.as_deref().filter(|s| !s.is_empty()) {
        validate_value(a)?;
        args.push("--regexp-ignore-case".into());
        args.push(format!("--author={a}"));
    }
    if let Some(p) = q.path.as_deref().filter(|s| !s.is_empty()) {
        let rel = validate::rel_path(&c, p)?;
        args.push("--".into());
        args.push(rel);
    }
    let out = run_raw(state, &c.top, &args, RunOpts::default()).await?;
    if !out.ok() {
        let msg = out.message();
        if no_commits_yet(&msg) {
            return Ok(LogPage { commits: vec![], has_more: false });
        }
        return Err(AppError::Git(msg));
    }
    let mut commits = parse_log(&out.stdout, &remotes);
    let has_more = commits.len() > limit as usize;
    commits.truncate(limit as usize);
    Ok(LogPage { commits, has_more })
}

fn validate_value(s: &str) -> AppResult<()> {
    if s.contains('\0') {
        return Err(AppError::InvalidArgument("valor con NUL".into()));
    }
    Ok(())
}

pub async fn file_history(state: &AppState, repo: Option<&str>, path: String, skip: u32, limit: u32) -> AppResult<LogPage> {
    let c = ctx_for(state, repo).await?;
    let rel = validate::rel_path(&c, &path)?;
    let limit = limit.clamp(1, MAX_PAGE);
    let remotes = validate::remote_names(state, &c).await.unwrap_or_default();
    let args: Vec<String> = vec![
        "log".into(),
        "--follow".into(),
        LOG_FORMAT.into(),
        format!("--skip={skip}"),
        format!("--max-count={}", limit + 1),
        "--".into(),
        rel,
    ];
    let out = run_raw(state, &c.top, &args, RunOpts::default()).await?;
    if !out.ok() {
        let msg = out.message();
        if no_commits_yet(&msg) {
            return Ok(LogPage { commits: vec![], has_more: false });
        }
        return Err(AppError::Git(msg));
    }
    let mut commits = parse_log(&out.stdout, &remotes);
    let has_more = commits.len() > limit as usize;
    commits.truncate(limit as usize);
    Ok(LogPage { commits, has_more })
}

// ---------------------------------------------------------------------------
// Detalle de commit
// ---------------------------------------------------------------------------

/// `--name-status -z`: `M\0path\0`, `R100\0old\0new\0`.
pub fn parse_name_status(raw: &[u8]) -> Vec<(char, Option<String>, String)> {
    let text = String::from_utf8_lossy(raw);
    let mut toks = text.split('\0').filter(|t| !t.is_empty());
    let mut out = Vec::new();
    while let Some(st) = toks.next() {
        let c = st.chars().next().unwrap_or('M');
        if matches!(c, 'R' | 'C') {
            let old = toks.next().unwrap_or("").to_string();
            let new = toks.next().unwrap_or("").to_string();
            out.push((c, Some(old), new));
        } else if let Some(p) = toks.next() {
            out.push((c, None, p.to_string()));
        }
    }
    out
}

/// `--numstat -z`: `add\tdel\tpath\0` o, en renombres, `add\tdel\t\0old\0new\0`. Devuelve por ruta nueva.
pub fn parse_numstat(raw: &[u8]) -> Vec<(String, Option<u32>, Option<u32>)> {
    let text = String::from_utf8_lossy(raw);
    let mut toks = text.split('\0');
    let mut out = Vec::new();
    while let Some(t) = toks.next() {
        let t = t.trim_start_matches('\n');
        if t.is_empty() {
            continue;
        }
        let mut p = t.splitn(3, '\t');
        let (Some(a), Some(d), Some(path)) = (p.next(), p.next(), p.next()) else { continue };
        let a = a.parse::<u32>().ok();
        let d = d.parse::<u32>().ok();
        let path = if path.is_empty() {
            let _old = toks.next();
            toks.next().unwrap_or("").to_string()
        } else {
            path.to_string()
        };
        out.push((path, a, d));
    }
    out
}

pub async fn commit_detail(state: &AppState, repo: Option<&str>, sha: String) -> AppResult<CommitDetail> {
    validate::rev("sha", &sha)?;
    let c = ctx_for(state, repo).await?;
    let remotes = validate::remote_names(state, &c).await.unwrap_or_default();
    let fmt = "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%D%x00%s%x00%b%x00%cn%x00%ct";
    let meta = run_ok(state, &c.top, &["show", "-s", "--no-color", fmt, sha.as_str()], RunOpts::default()).await?;
    let text = String::from_utf8_lossy(&meta).into_owned();
    let f: Vec<&str> = text.trim_end_matches(['\n', '\r']).split('\0').collect();
    if f.len() < 10 {
        return Err(AppError::Git("salida de git show inesperada".into()));
    }
    let row = CommitRow {
        sha: f[0].to_string(),
        parents: f[1].split_whitespace().map(str::to_string).collect(),
        author: f[2].to_string(),
        email: f[3].to_string(),
        time: f[4].parse().unwrap_or(0),
        refs: parse_decorations(f[5], &remotes),
        subject: f[6].to_string(),
    };
    let merge = row.parents.len() > 1;
    let mk = |extra: &str| -> Vec<String> {
        let mut a: Vec<String> = vec!["show".into(), "--format=".into(), "--no-color".into(), "-M".into(), "-z".into(), extra.into()];
        if merge {
            a.push("-m".into());
            a.push("--first-parent".into());
        }
        a.push(sha.clone());
        a
    };
    let (a_ns, a_num) = (mk("--name-status"), mk("--numstat"));
    let (ns, num) = tokio::join!(
        run_ok(state, &c.top, &a_ns, RunOpts::default()),
        run_ok(state, &c.top, &a_num, RunOpts::default()),
    );
    let names = parse_name_status(&ns?);
    let nums = parse_numstat(&num?);
    let files = names
        .into_iter()
        .map(|(status, orig, path)| {
            let n = nums.iter().find(|(p, _, _)| *p == path);
            ChangedFile {
                additions: n.and_then(|x| x.1),
                deletions: n.and_then(|x| x.2),
                path,
                orig_path: orig,
                status,
            }
        })
        .collect();
    Ok(CommitDetail {
        body: f[7].trim().to_string(),
        committer: f[8].to_string(),
        commit_time: f[9].parse().unwrap_or(0),
        row,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_log_fixture() {
        let remotes = vec!["origin".to_string()];
        let raw = "aaa\0bbb ccc\0Ana\0a@x.io\x00100\0HEAD -> main, origin/main, tag: v1, feature/x\0Merge: \"x\"\x1e\nbbb\0\0Bob\0b@x.io\x0050\0\0root\x1e";
        let rows = parse_log(raw.as_bytes(), &remotes);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].parents, vec!["bbb", "ccc"]);
        assert_eq!(rows[0].subject, "Merge: \"x\"");
        let kinds: Vec<_> = rows[0].refs.iter().map(|r| (r.name.as_str(), r.kind)).collect();
        assert_eq!(
            kinds,
            vec![("main", RefKind::Head), ("origin/main", RefKind::Remote), ("v1", RefKind::Tag), ("feature/x", RefKind::Local)]
        );
        assert!(rows[1].parents.is_empty());
        assert_eq!(rows[1].sha, "bbb");
    }

    #[test]
    fn parses_name_status_and_numstat() {
        let ns = parse_name_status(b"M\0a.rs\0R100\0old name.rs\0new name.rs\0A\0z\0");
        assert_eq!(ns.len(), 3);
        assert_eq!(ns[1], ('R', Some("old name.rs".into()), "new name.rs".into()));
        let n = parse_numstat(b"3\t1\ta.rs\0-\t-\tbin.png\0" .as_slice());
        assert_eq!(n[0], ("a.rs".into(), Some(3), Some(1)));
        assert_eq!(n[1], ("bin.png".into(), None, None));
        let n = parse_numstat(b"2\t0\t\0old.rs\0new.rs\0");
        assert_eq!(n[0], ("new.rs".into(), Some(2), Some(0)));
    }
}
