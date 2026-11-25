//! Ramas, tags, stashes y remotos.

use serde::Serialize;

use super::exec::{ctx_for, run_ok, run_raw, sanitize_text, RunOpts};
use crate::error::AppResult;
use crate::state::AppState;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BranchRef {
    pub name: String,
    pub full_name: String,
    pub sha: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub is_head: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TagRef {
    pub name: String,
    pub sha: String,
    pub annotated: bool,
    pub message: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StashRef {
    pub index: u32,
    pub sha: String,
    pub message: String,
    pub time: i64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub name: String,
    pub fetch_url: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Refs {
    pub local: Vec<BranchRef>,
    pub remote: Vec<BranchRef>,
    pub tags: Vec<TagRef>,
    pub stashes: Vec<StashRef>,
    pub remotes: Vec<RemoteInfo>,
}

pub const REFS_FORMAT: &str = "--format=%(refname)%00%(objectname)%00%(upstream:short)%00%(upstream:track)%00%(HEAD)%00%(objecttype)%00%(*objectname)%00%(contents:subject)%1e";

fn parse_track(t: &str) -> (u32, u32) {
    let (mut a, mut b) = (0, 0);
    for part in t.trim_matches(|c| c == '[' || c == ']').split(',') {
        let part = part.trim();
        if let Some(n) = part.strip_prefix("ahead ") {
            a = n.trim().parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            b = n.trim().parse().unwrap_or(0);
        }
    }
    (a, b)
}

/// Parser de `for-each-ref` con `REFS_FORMAT` sobre `refs/heads refs/remotes refs/tags`.
pub fn parse_for_each_ref(raw: &[u8]) -> (Vec<BranchRef>, Vec<BranchRef>, Vec<TagRef>) {
    let text = String::from_utf8_lossy(raw);
    let (mut local, mut remote, mut tags) = (Vec::new(), Vec::new(), Vec::new());
    for rec in text.split('\x1e') {
        let rec = rec.trim_start_matches(['\n', '\r']);
        if rec.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = rec.split('\0').collect();
        if f.len() < 8 {
            continue;
        }
        let full = f[0];
        if let Some(n) = full.strip_prefix("refs/heads/") {
            let (ahead, behind) = parse_track(f[3]);
            local.push(BranchRef {
                name: n.to_string(),
                full_name: full.to_string(),
                sha: f[1].to_string(),
                upstream: Some(f[2].to_string()).filter(|s| !s.is_empty()),
                ahead,
                behind,
                is_head: f[4] == "*",
            });
        } else if let Some(n) = full.strip_prefix("refs/remotes/") {
            if n.ends_with("/HEAD") {
                continue;
            }
            remote.push(BranchRef {
                name: n.to_string(),
                full_name: full.to_string(),
                sha: f[1].to_string(),
                upstream: None,
                ahead: 0,
                behind: 0,
                is_head: false,
            });
        } else if let Some(n) = full.strip_prefix("refs/tags/") {
            let annotated = f[5] == "tag";
            let peeled = if annotated && !f[6].is_empty() { f[6] } else { f[1] };
            tags.push(TagRef {
                name: n.to_string(),
                sha: peeled.to_string(),
                annotated,
                message: if annotated { Some(f[7].to_string()) } else { None },
            });
        }
    }
    (local, remote, tags)
}

pub fn parse_stashes(raw: &[u8]) -> Vec<StashRef> {
    let text = String::from_utf8_lossy(raw);
    text.split('\x1e')
        .filter_map(|rec| {
            let rec = rec.trim_start_matches(['\n', '\r']);
            let f: Vec<&str> = rec.split('\0').collect();
            if f.len() < 4 {
                return None;
            }
            let index = f[0].strip_prefix("stash@{")?.strip_suffix('}')?.parse().ok()?;
            Some(StashRef { index, sha: f[1].to_string(), message: f[2].to_string(), time: f[3].trim().parse().unwrap_or(0) })
        })
        .collect()
}

/// Quita `user[:pass]@` de la URL.
pub fn sanitize_url(url: &str) -> String {
    sanitize_text(url)
}

pub fn parse_remotes(raw: &str) -> Vec<RemoteInfo> {
    let mut out: Vec<RemoteInfo> = Vec::new();
    for line in raw.lines() {
        // origin\thttps://host/x.git (fetch)
        let mut p = line.split_whitespace();
        let (Some(name), Some(url), Some(kind)) = (p.next(), p.next(), p.next()) else { continue };
        if kind == "(fetch)" && !out.iter().any(|r| r.name == name) {
            out.push(RemoteInfo { name: name.to_string(), fetch_url: sanitize_url(url) });
        }
    }
    out
}

pub async fn refs(state: &AppState, repo: Option<&str>) -> AppResult<Refs> {
    let c = ctx_for(state, repo).await?;
    let (fe, st, rm) = tokio::join!(
        run_ok(state, &c.top, &["for-each-ref", REFS_FORMAT, "refs/heads", "refs/remotes", "refs/tags"], RunOpts::default()),
        run_raw(state, &c.top, &["stash", "list", "--format=%gd%x00%H%x00%gs%x00%ct%x1e"], RunOpts::default()),
        run_ok(state, &c.top, &["remote", "-v"], RunOpts::default()),
    );
    let (local, remote, tags) = parse_for_each_ref(&fe?);
    let stashes = match st {
        Ok(o) if o.ok() => parse_stashes(&o.stdout),
        _ => vec![],
    };
    let remotes = parse_remotes(&String::from_utf8_lossy(&rm?));
    Ok(Refs { local, remote, tags, stashes, remotes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_refs_fixture() {
        let raw = "refs/heads/main\0aaa\0origin/main\0[ahead 2, behind 1]\0*\0commit\0\0subj\x1e\n\
                   refs/heads/dev\0bbb\0\0\0 \0commit\0\0s\x1e\n\
                   refs/remotes/origin/main\0aaa\0\0\0 \0commit\0\0s\x1e\n\
                   refs/remotes/origin/HEAD\0aaa\0\0\0 \0commit\0\0s\x1e\n\
                   refs/tags/v1\0ccc\0\0\0 \0commit\0\0s\x1e\n\
                   refs/tags/v2\0ddd\0\0\0 \0tag\0eee\0Release 2\x1e\n";
        let (l, r, t) = parse_for_each_ref(raw.as_bytes());
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].ahead, l[0].behind, l[0].is_head), (2, 1, true));
        assert_eq!(l[0].upstream.as_deref(), Some("origin/main"));
        assert!(l[1].upstream.is_none());
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "origin/main");
        assert_eq!(t.len(), 2);
        assert!(!t[0].annotated);
        assert!(t[1].annotated);
        assert_eq!(t[1].sha, "eee");
        assert_eq!(t[1].message.as_deref(), Some("Release 2"));
    }

    #[test]
    fn parses_stash_and_remotes() {
        let s = parse_stashes(b"stash@{0}\0abc\0On main: wip\x00170\x1e\nstash@{1}\0def\0WIP\x00160\x1e");
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].index, 1);
        assert_eq!(s[0].message, "On main: wip");
        let r = parse_remotes("origin\thttps://u:pw@github.com/a/b.git (fetch)\norigin\thttps://u:pw@github.com/a/b.git (push)\nup\tgit@github.com:a/b.git (fetch)\n");
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].fetch_url, "https://github.com/a/b.git");
        assert_eq!(r[1].fetch_url, "git@github.com:a/b.git");
        assert_eq!(parse_track("[gone]"), (0, 0));
    }
}
