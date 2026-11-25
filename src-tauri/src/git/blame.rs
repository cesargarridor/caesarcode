//! `git blame --porcelain`.

use std::collections::HashMap;

use serde::Serialize;

use super::exec::{ctx_for, run_ok, RunOpts};
use super::validate;
use crate::error::AppResult;
use crate::state::AppState;

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct BlameCommit {
    pub author: String,
    pub time: i64,
    pub summary: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlameResult {
    pub commits: HashMap<String, BlameCommit>,
    /// `(línea final 1-based, sha)` en orden de archivo.
    pub lines: Vec<(u32, String)>,
}

fn is_header(line: &str) -> Option<(&str, u32)> {
    let mut p = line.split(' ');
    let sha = p.next()?;
    if sha.len() < 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let _orig: u32 = p.next()?.parse().ok()?;
    let fin: u32 = p.next()?.parse().ok()?;
    Some((sha, fin))
}

/// Los metadatos aparecen una sola vez por sha; cada línea del archivo lleva su cabecera + `\tcontenido`.
pub fn parse_porcelain(raw: &[u8]) -> BlameResult {
    let text = String::from_utf8_lossy(raw);
    let mut commits: HashMap<String, BlameCommit> = HashMap::new();
    let mut lines = Vec::new();
    let mut cur: Option<(String, u32)> = None;
    for line in text.split('\n') {
        if line.starts_with('\t') {
            if let Some((sha, n)) = cur.take() {
                lines.push((n, sha));
            }
            continue;
        }
        if let Some((sha, n)) = is_header(line) {
            commits.entry(sha.to_string()).or_default();
            cur = Some((sha.to_string(), n));
            continue;
        }
        if let Some((sha, _)) = &cur {
            let Some(c) = commits.get_mut(sha) else { continue };
            if let Some(v) = line.strip_prefix("author ") {
                c.author = v.to_string();
            } else if let Some(v) = line.strip_prefix("author-time ") {
                c.time = v.trim().parse().unwrap_or(0);
            } else if let Some(v) = line.strip_prefix("summary ") {
                c.summary = v.to_string();
            }
        }
    }
    BlameResult { commits, lines }
}

pub async fn blame(state: &AppState, repo: Option<&str>, path: String, rev: Option<String>) -> AppResult<BlameResult> {
    let c = ctx_for(state, repo).await?;
    let rel = validate::rel_path(&c, &path)?;
    let mut args: Vec<String> = vec!["blame".into(), "--porcelain".into()];
    if let Some(r) = rev.filter(|r| !r.is_empty()) {
        validate::rev("rev", &r)?;
        args.push(r);
    }
    args.push("--".into());
    args.push(rel);
    let out = run_ok(state, &c.top, &args, RunOpts::default()).await?;
    Ok(parse_porcelain(&out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blame_fixture() {
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        let raw = format!(
            "{a} 1 1 2\nauthor Ana\nauthor-mail <a@x>\nauthor-time 1700000000\nauthor-tz +0000\nsummary first\nfilename f.txt\n\tline one\n{a} 2 2\n\tline two\n{b} 3 3 1\nauthor Bob\nauthor-time 1700000100\nsummary second: \"x\"\nprevious {a} f.txt\nfilename f.txt\n\t\tindented\n"
        );
        let r = parse_porcelain(raw.as_bytes());
        assert_eq!(r.lines, vec![(1, a.clone()), (2, a.clone()), (3, b.clone())]);
        assert_eq!(r.commits.len(), 2);
        assert_eq!(r.commits[&a].author, "Ana");
        assert_eq!(r.commits[&a].time, 1_700_000_000);
        assert_eq!(r.commits[&b].summary, "second: \"x\"");
    }
}
