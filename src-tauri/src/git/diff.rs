//! Diff unificado → hunks, generación de parches por hunk y lectura de archivos por revisión.

use serde::{Deserialize, Serialize};

use super::exec::{ctx_for, run_raw, Ctx, RunOpts};
use super::validate;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub const MAX_DIFF_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_FILE_BYTES: usize = 5 * 1024 * 1024;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: char,
    pub text: String,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub binary: bool,
    pub hunks: Vec<Hunk>,
    /// `diff --git…` hasta el primer `@@` (para reconstruir parches).
    pub header: String,
    pub too_large: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DiffTarget {
    Worktree { path: String },
    Index { path: String },
    Commit { sha: String, path: String },
    Range { from: String, to: String, path: String },
}

fn parse_range(s: &str) -> (u32, u32) {
    // "a,b" o "a" (b = 1)
    let mut p = s.splitn(2, ',');
    let a = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let b = p.next().map(|x| x.parse().unwrap_or(0)).unwrap_or(1);
    (a, b)
}

/// `@@ -a,b +c,d @@ ctx` → (a, b, c, d)
fn parse_hunk_header(h: &str) -> Option<(u32, u32, u32, u32)> {
    let rest = h.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(" +")?;
    let (new, _) = rest.split_once(" @@")?;
    let (os, ol) = parse_range(old);
    let (ns, nl) = parse_range(new);
    Some((os, ol, ns, nl))
}

fn unquote(p: &str) -> String {
    p.trim_matches('"').to_string()
}

/// Parser de diff unificado de UN archivo (el primero que aparezca). `fallback_path` si no se deduce.
pub fn parse_unified(raw: &[u8], fallback_path: &str) -> FileDiff {
    let text = String::from_utf8_lossy(raw);
    let mut fd = FileDiff {
        path: fallback_path.to_string(),
        old_path: None,
        binary: false,
        hunks: vec![],
        header: String::new(),
        too_large: false,
    };
    let mut header: Vec<&str> = Vec::new();
    let (mut old_no, mut new_no) = (0u32, 0u32);
    let mut seen_files = 0;
    let mut cur: Option<Hunk> = None;
    let mut rename_to: Option<String> = None;
    let mut rename_from: Option<String> = None;
    let mut minus: Option<String> = None;
    let mut plus: Option<String> = None;

    for line in text.split('\n') {
        if line.starts_with("diff --git ") || line.starts_with("diff --cc ") {
            seen_files += 1;
            if seen_files > 1 {
                break;
            }
            header.push(line);
            continue;
        }
        if cur.is_none() && !line.starts_with("@@") {
            // Cabecera extendida.
            if let Some(r) = line.strip_prefix("rename from ") {
                rename_from = Some(unquote(r));
            } else if let Some(r) = line.strip_prefix("rename to ") {
                rename_to = Some(unquote(r));
            } else if let Some(r) = line.strip_prefix("--- ") {
                minus = Some(unquote(r));
            } else if let Some(r) = line.strip_prefix("+++ ") {
                plus = Some(unquote(r));
            } else if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
                fd.binary = true;
            }
            if !line.is_empty() || !header.is_empty() {
                header.push(line);
            }
            continue;
        }
        if line.starts_with("@@") {
            if let Some(h) = cur.take() {
                fd.hunks.push(h);
            }
            let Some((os, ol, ns, nl)) = parse_hunk_header(line) else { continue };
            old_no = os;
            new_no = ns;
            cur = Some(Hunk { header: line.to_string(), old_start: os, old_lines: ol, new_start: ns, new_lines: nl, lines: vec![] });
            continue;
        }
        let Some(h) = cur.as_mut() else { continue };
        match line.chars().next() {
            Some('+') => {
                h.lines.push(DiffLine { kind: '+', text: line[1..].to_string(), old_no: None, new_no: Some(new_no) });
                new_no += 1;
            }
            Some('-') => {
                h.lines.push(DiffLine { kind: '-', text: line[1..].to_string(), old_no: Some(old_no), new_no: None });
                old_no += 1;
            }
            Some(' ') => {
                h.lines.push(DiffLine { kind: ' ', text: line[1..].to_string(), old_no: Some(old_no), new_no: Some(new_no) });
                old_no += 1;
                new_no += 1;
            }
            Some('\\') => h.lines.push(DiffLine { kind: '\\', text: line[1..].trim_start().to_string(), old_no: None, new_no: None }),
            // Línea vacía final del split (o basura): se ignora.
            _ => {}
        }
    }
    if let Some(h) = cur.take() {
        fd.hunks.push(h);
    }
    // Sin cola vacía: `header` termina antes del primer `@@` y conserva el salto final.
    while header.last().is_some_and(|l| l.is_empty()) {
        header.pop();
    }
    fd.header = if header.is_empty() { String::new() } else { format!("{}\n", header.join("\n")) };

    let strip = |s: &str, pre: &str| s.strip_prefix(pre).map(str::to_string);
    if let Some(to) = rename_to {
        fd.path = to;
        fd.old_path = rename_from;
    } else {
        if let Some(p) = plus.as_deref().filter(|p| *p != "/dev/null").and_then(|p| strip(p, "b/")) {
            fd.path = p;
        } else if let Some(p) = minus.as_deref().filter(|p| *p != "/dev/null").and_then(|p| strip(p, "a/")) {
            fd.path = p;
        }
    }
    fd
}

#[allow(dead_code)] // el frontend arma los parches; aquí se usa en tests y como referencia del contrato
/// Parche válido para `git apply` con los hunks indicados (índices en `fd.hunks`). Los hunks
/// descartados no alteran la validez gracias a `--recount`.
pub fn hunk_patch(fd: &FileDiff, indexes: &[usize]) -> String {
    let mut p = fd.header.clone();
    for &i in indexes {
        let Some(h) = fd.hunks.get(i) else { continue };
        p.push_str(&h.header);
        p.push('\n');
        for l in &h.lines {
            if l.kind == '\\' {
                p.push_str("\\ ");
            } else {
                p.push(l.kind);
            }
            p.push_str(&l.text);
            p.push('\n');
        }
    }
    p
}

fn null_device() -> &'static str {
    if cfg!(windows) {
        "NUL"
    } else {
        "/dev/null"
    }
}

async fn is_untracked(state: &AppState, c: &Ctx, rel: &str) -> bool {
    match run_raw(state, &c.top, &["ls-files", "-z", "--others", "--exclude-standard", "--", rel], RunOpts::default()).await {
        Ok(o) => o.ok() && !o.stdout.is_empty(),
        Err(_) => false,
    }
}

pub async fn diff(state: &AppState, repo: Option<&str>, target: DiffTarget, context: u32) -> AppResult<FileDiff> {
    let c = ctx_for(state, repo).await?;
    let u = format!("-U{}", context.min(10_000));
    let base = |extra: &[&str]| -> Vec<String> {
        let mut a: Vec<String> = vec!["diff".into(), "--no-color".into(), "--no-ext-diff".into(), "-M".into(), u.clone()];
        a.extend(extra.iter().map(|s| s.to_string()));
        a
    };
    let (args, rel, no_index): (Vec<String>, String, bool) = match target {
        DiffTarget::Worktree { path } => {
            let rel = validate::rel_path(&c, &path)?;
            if is_untracked(state, &c, &rel).await {
                // --no-index sale con 1 si hay diferencias.
                let mut a = base(&["--no-index", "--"]);
                a.push(null_device().into());
                a.push(rel.clone());
                (a, rel, true)
            } else {
                let mut a = base(&["--"]);
                a.push(rel.clone());
                (a, rel, false)
            }
        }
        DiffTarget::Index { path } => {
            let rel = validate::rel_path(&c, &path)?;
            let mut a = base(&["--cached", "--"]);
            a.push(rel.clone());
            (a, rel, false)
        }
        DiffTarget::Commit { sha, path } => {
            validate::rev("sha", &sha)?;
            let rel = validate::rel_path(&c, &path)?;
            // `show` gestiona el commit raíz y los merges (-m --first-parent).
            let a = vec![
                "show".into(),
                "--format=".into(),
                "--no-color".into(),
                "--no-ext-diff".into(),
                "-M".into(),
                u.clone(),
                "-m".into(),
                "--first-parent".into(),
                sha,
                "--".into(),
                rel.clone(),
            ];
            (a, rel, false)
        }
        DiffTarget::Range { from, to, path } => {
            validate::rev("from", &from)?;
            validate::rev("to", &to)?;
            let rel = validate::rel_path(&c, &path)?;
            let mut a = base(&[]);
            a.push(from);
            a.push(to);
            a.push("--".into());
            a.push(rel.clone());
            (a, rel, false)
        }
    };
    let out = run_raw(state, &c.top, &args, RunOpts::default()).await?;
    let ok = out.ok() || (no_index && out.code == 1);
    if !ok {
        return Err(AppError::Git(out.message()));
    }
    if out.stdout.len() > MAX_DIFF_BYTES {
        return Ok(FileDiff { path: rel, old_path: None, binary: false, hunks: vec![], header: String::new(), too_large: true });
    }
    Ok(parse_unified(&out.stdout, &rel))
}

/// Contenido de un archivo en `rev` (`sha`, `HEAD`, `INDEX`). Tope 5 MB; binario → error.
pub async fn file_at(state: &AppState, repo: Option<&str>, rev: String, path: String) -> AppResult<String> {
    let c = ctx_for(state, repo).await?;
    let rel = validate::rel_path(&c, &path)?;
    let spec = if rev == "INDEX" {
        format!(":{rel}")
    } else {
        validate::rev("rev", &rev)?;
        format!("{rev}:{rel}")
    };
    let out = run_raw(state, &c.top, &["show", spec.as_str()], RunOpts::default()).await?;
    if !out.ok() {
        return Err(AppError::Git(out.message()));
    }
    if out.stdout.len() > MAX_FILE_BYTES {
        return Err(AppError::InvalidArgument(format!("archivo demasiado grande (> {} MB)", MAX_FILE_BYTES / 1024 / 1024)));
    }
    if out.stdout.iter().take(8000).any(|&b| b == 0) {
        return Err(AppError::InvalidArgument("archivo binario".into()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "diff --git a/f.txt b/f.txt\nindex 111..222 100644\n--- a/f.txt\n+++ b/f.txt\n@@ -1,3 +1,3 @@ fn x\n a\n-b\n+B\n c\n@@ -10,2 +10,3 @@\n x\n+y\n z\n\\ No newline at end of file\n";

    #[test]
    fn parses_hunks_and_line_numbers() {
        let fd = parse_unified(SAMPLE.as_bytes(), "zz");
        assert_eq!(fd.path, "f.txt");
        assert_eq!(fd.hunks.len(), 2);
        assert!(fd.header.starts_with("diff --git a/f.txt b/f.txt\nindex"));
        assert!(fd.header.ends_with("+++ b/f.txt\n"));
        let h = &fd.hunks[0];
        assert_eq!((h.old_start, h.old_lines, h.new_start, h.new_lines), (1, 3, 1, 3));
        assert_eq!(h.lines[1], DiffLine { kind: '-', text: "b".into(), old_no: Some(2), new_no: None });
        assert_eq!(h.lines[2], DiffLine { kind: '+', text: "B".into(), old_no: None, new_no: Some(2) });
        let h2 = &fd.hunks[1];
        assert_eq!(h2.lines.last().unwrap().kind, '\\');
        assert_eq!(h2.lines[1].new_no, Some(11));
    }

    #[test]
    fn rebuilds_single_hunk_patch() {
        let fd = parse_unified(SAMPLE.as_bytes(), "zz");
        let p = hunk_patch(&fd, &[1]);
        assert!(p.contains("@@ -10,2 +10,3 @@"));
        assert!(!p.contains("@@ -1,3"));
        assert!(p.ends_with("\\ No newline at end of file\n"));
    }

    #[test]
    fn parses_rename_binary_and_new_file() {
        let r = "diff --git a/old.rs b/new.rs\nsimilarity index 90%\nrename from old.rs\nrename to new.rs\n";
        let fd = parse_unified(r.as_bytes(), "x");
        assert_eq!(fd.path, "new.rs");
        assert_eq!(fd.old_path.as_deref(), Some("old.rs"));
        assert!(fd.hunks.is_empty());
        let b = "diff --git a/i.png b/i.png\nindex 1..2 100644\nBinary files a/i.png and b/i.png differ\n";
        assert!(parse_unified(b.as_bytes(), "i.png").binary);
        let n = "diff --git a/n.txt b/n.txt\nnew file mode 100644\nindex 000..111\n--- /dev/null\n+++ b/n.txt\n@@ -0,0 +1,2 @@\n+a\n+b\n";
        let fd = parse_unified(n.as_bytes(), "x");
        assert_eq!(fd.path, "n.txt");
        assert_eq!(fd.hunks[0].new_lines, 2);
        assert_eq!(fd.hunks[0].old_start, 0);
    }
}
