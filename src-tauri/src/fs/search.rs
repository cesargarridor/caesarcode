use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::Lossy;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use ignore::overrides::OverrideBuilder;

use super::quick_open::{build_walker, rel_unix};
use super::{SearchMatch, SearchOptions, SearchResult};
use crate::error::{AppError, AppResult};
use crate::util::paths::utf16_len;

const DEFAULT_MAX_RESULTS: usize = 2000;
const MAX_PREVIEW_CHARS: usize = 300;
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;

/// Recorta la línea a ~300 chars alrededor del match y devuelve offsets UTF-16 sobre el preview.
fn make_preview(line: &str, ms: usize, me: usize) -> (String, usize, usize) {
    let total = line.chars().count();
    if total <= MAX_PREVIEW_CHARS {
        return (line.to_string(), utf16_len(&line[..ms]), utf16_len(&line[..me]));
    }
    let match_char = line[..ms].chars().count();
    let start_c = match_char.saturating_sub(80).min(total - MAX_PREVIEW_CHARS);
    let end_c = start_c + MAX_PREVIEW_CHARS;
    let bs = line.char_indices().nth(start_c).map(|(i, _)| i).unwrap_or(0);
    let be = line.char_indices().nth(end_c).map(|(i, _)| i).unwrap_or(line.len());
    let ms2 = ms.clamp(bs, be);
    let me2 = me.clamp(bs, be);
    (
        line[bs..be].to_string(),
        utf16_len(&line[bs..ms2]),
        utf16_len(&line[bs..me2]),
    )
}

/// Búsqueda global sobre todas las raíces `(ruta, nombre)` con presupuesto `max_results` compartido
/// y la misma generación de cancelación.
pub fn search(
    roots: &[(PathBuf, String)],
    query: &str,
    opts: &SearchOptions,
    gen: u64,
    search_gen: &AtomicU64,
) -> AppResult<SearchResult> {
    let empty = SearchResult { matches: Vec::new(), truncated: false, files_searched: 0, cancelled: false };
    if query.is_empty() {
        return Ok(empty);
    }
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(!opts.case_sensitive)
        .word(opts.whole_word)
        .fixed_strings(!opts.regex)
        .build(query)
        .map_err(|e| AppError::InvalidArgument(format!("expresión no válida: {e}")))?;

    let max = opts.max_results.map(|n| n as usize).filter(|n| *n > 0).unwrap_or(DEFAULT_MAX_RESULTS);
    let include: Vec<String> = opts
        .include
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|inc| inc.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .build();

    let mut result = empty;
    'roots: for (root, root_name) in roots {
        let mut walker = build_walker(root);
        walker.max_filesize(Some(MAX_FILE_BYTES));
        if !include.is_empty() {
            let mut ob = OverrideBuilder::new(root);
            for g in &include {
                ob.add(g).map_err(|e| AppError::InvalidArgument(format!("glob no válido {g:?}: {e}")))?;
            }
            let ov = ob.build().map_err(|e| AppError::InvalidArgument(format!("glob no válido: {e}")))?;
            walker.overrides(ov);
        }
        for entry in walker.build().flatten() {
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            if search_gen.load(Ordering::SeqCst) != gen {
                result.cancelled = true;
                break 'roots;
            }
            let path = entry.path();
            let rel = rel_unix(root, path);
            let path_str = path.to_string_lossy().into_owned();
            result.files_searched += 1;

            let mut hit_limit = false;
            let mut cancelled = false;
            let found = searcher.search_path(
                &matcher,
                path,
                Lossy(|lnum, line| {
                    if search_gen.load(Ordering::SeqCst) != gen {
                        cancelled = true;
                        return Ok(false);
                    }
                    let line = line.trim_end_matches(['\n', '\r']);
                    if let Ok(Some(m)) = matcher.find(line.as_bytes()) {
                        let column = utf16_len(&line[..m.start()]);
                        let (preview, ps, pe) = make_preview(line, m.start(), m.end());
                        result.matches.push(SearchMatch {
                            path: path_str.clone(),
                            rel_path: rel.clone(),
                            root: root_name.clone(),
                            line: lnum,
                            column,
                            preview,
                            match_start: ps,
                            match_end: pe,
                        });
                        if result.matches.len() >= max {
                            hit_limit = true;
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }),
            );
            // Archivos ilegibles (permisos, borrados durante el walk) se saltan.
            let _ = found;
            if cancelled {
                result.cancelled = true;
                break 'roots;
            }
            if hit_limit {
                result.truncated = true;
                break 'roots;
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_offsets_are_utf16() {
        let line = "áé😀 foo bar";
        let ms = line.find("foo").unwrap();
        let (p, s, e) = make_preview(line, ms, ms + 3);
        assert_eq!(p, line);
        let units: Vec<u16> = p.encode_utf16().collect();
        assert_eq!(String::from_utf16(&units[s..e]).unwrap(), "foo");
    }

    #[test]
    fn long_lines_are_trimmed_around_match() {
        let line = format!("{}needle{}", "x".repeat(500), "y".repeat(500));
        let ms = 500;
        let (p, s, e) = make_preview(&line, ms, ms + 6);
        assert_eq!(p.chars().count(), MAX_PREVIEW_CHARS);
        assert_eq!(&p[s..e], "needle");
    }

    #[test]
    fn searches_all_roots_with_shared_budget() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a"), d.path().join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("x.txt"), "needle one\nneedle two\n").unwrap();
        std::fs::write(b.join("y.txt"), "needle three\n").unwrap();
        let roots = vec![(a, "a".to_string()), (b, "b".to_string())];
        let opts = SearchOptions { case_sensitive: false, regex: false, whole_word: false, include: None, max_results: None };
        let gen = AtomicU64::new(1);
        let r = search(&roots, "needle", &opts, 1, &gen).unwrap();
        assert_eq!(r.matches.len(), 3);
        assert!(r.matches.iter().any(|m| m.root == "b" && m.rel_path == "y.txt"));
        let limited = SearchOptions { max_results: Some(2), ..opts };
        let r = search(&roots, "needle", &limited, 1, &gen).unwrap();
        assert_eq!(r.matches.len(), 2);
        assert!(r.truncated);
        // cancelada
        let r = search(&roots, "needle", &limited, 0, &gen).unwrap();
        assert!(r.cancelled);
    }
}
