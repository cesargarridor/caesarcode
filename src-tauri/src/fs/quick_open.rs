use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ignore::WalkBuilder;

use super::QuickOpenItem;
use crate::state::AppState;

const MAX_FILES: usize = 200_000;

pub type IndexedFile = (PathBuf, String);

struct RootIndex {
    files: Arc<Vec<IndexedFile>>,
    dirty: bool,
}

/// Caché de archivos del workspace para Quick Open: un índice por raíz, reconstruido de forma perezosa.
#[derive(Default)]
pub struct FileIndex {
    roots: HashMap<PathBuf, RootIndex>,
}

impl FileIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Marca sucio solo el índice de la raíz.
    pub fn mark_root_dirty(&mut self, root: &Path) {
        if let Some(r) = self.roots.get_mut(root) {
            r.dirty = true;
        }
    }

    pub fn remove_root(&mut self, root: &Path) {
        self.roots.remove(root);
    }

    pub fn reset(&mut self) {
        self.roots.clear();
    }
}

/// Walker común (Quick Open y búsqueda global): respeta .gitignore (también fuera de repos),
/// incluye ocultos y excluye `.git`.
pub fn build_walker(root: &Path) -> WalkBuilder {
    let mut b = WalkBuilder::new(root);
    b.hidden(false)
        .require_git(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|e| e.file_name() != ".git" && e.file_name() != ".DS_Store")
        .sort_by_file_path(|a, b| a.cmp(b));
    b
}

pub fn rel_unix(root: &Path, p: &Path) -> String {
    let rel = p.strip_prefix(root).unwrap_or(p);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn build_index(root: &Path) -> Vec<IndexedFile> {
    let mut files = Vec::new();
    for entry in build_walker(root).build().flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let rel = rel_unix(root, entry.path());
        files.push((entry.into_path(), rel));
        if files.len() >= MAX_FILES {
            break;
        }
    }
    files
}

/// Devuelve el índice vigente de la raíz; lo reconstruye (fuera del lock) si está sucio o no existe.
fn current_index(state: &AppState, root: &Path) -> Arc<Vec<IndexedFile>> {
    {
        let Ok(mut idx) = state.file_index.lock() else { return Arc::new(build_index(root)) };
        if let Some(r) = idx.roots.get_mut(root) {
            if !r.dirty {
                return r.files.clone();
            }
            // Se limpia antes de construir: un cambio durante el walk lo vuelve a ensuciar.
            r.dirty = false;
        } else {
            idx.roots.insert(root.to_path_buf(), RootIndex { files: Arc::new(Vec::new()), dirty: false });
        }
    }
    let files = Arc::new(build_index(root));
    if let Ok(mut idx) = state.file_index.lock() {
        if let Some(r) = idx.roots.get_mut(root) {
            r.files = files.clone();
        }
    }
    files
}

/// Quick Open sobre todas las raíces `(ruta, nombre)`. Con más de una raíz, `rel_path` se prefija con `"<nombre>/"`.
pub fn query(state: &AppState, roots: &[(PathBuf, String)], query: &str, limit: usize) -> Vec<QuickOpenItem> {
    let multi = roots.len() > 1;
    let indexes: Vec<(Arc<Vec<IndexedFile>>, &str)> =
        roots.iter().map(|(p, n)| (current_index(state, p), n.as_str())).collect();
    let to_item = |f: &IndexedFile, root: &str, score: i64, mut indices: Vec<usize>| {
        let rel_path = if multi {
            let shift = root.chars().count() + 1;
            for i in &mut indices {
                *i += shift;
            }
            format!("{root}/{}", f.1)
        } else {
            f.1.clone()
        };
        QuickOpenItem { path: f.0.to_string_lossy().into_owned(), rel_path, root: root.to_string(), score, indices }
    };
    let q: String = query.split_whitespace().collect();
    if q.is_empty() {
        return indexes
            .iter()
            .flat_map(|(files, name)| files.iter().map(move |f| (f, *name)))
            .take(limit)
            .map(|(f, name)| to_item(f, name, 0, Vec::new()))
            .collect();
    }
    let matcher = SkimMatcherV2::default();
    let ql = q.to_lowercase();
    let mut scored: Vec<(i64, &IndexedFile, &str, Vec<usize>)> = indexes
        .iter()
        .flat_map(|(files, name)| files.iter().map(move |f| (f, *name)))
        .filter_map(|(f, name)| {
            let (mut score, indices) = matcher.fuzzy_indices(&f.1, &q)?;
            let base = f.1.rsplit('/').next().unwrap_or(&f.1).to_lowercase();
            if base.starts_with(&ql) {
                score += 300;
            } else if base.contains(&ql) {
                score += 200;
            }
            Some((score, f, name, indices))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1 .1.len().cmp(&b.1 .1.len())));
    scored.into_iter().take(limit).map(|(s, f, name, i)| to_item(f, name, s, i)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_per_root_and_prefix_when_multi() {
        let d = tempfile::tempdir().unwrap();
        let base = crate::util::paths::clean_canonicalize(d.path()).unwrap();
        let (a, b) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("alpha.txt"), "").unwrap();
        std::fs::write(b.join("beta.txt"), "").unwrap();
        let st = AppState::new();
        let one = query(&st, &[(a.clone(), "a".into())], "", 10);
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].rel_path, "alpha.txt");
        let both = query(&st, &[(a.clone(), "a".into()), (b.clone(), "b".into())], "beta", 10);
        assert_eq!(both.len(), 1);
        assert_eq!(both[0].rel_path, "b/beta.txt");
        assert_eq!(both[0].root, "b");
        // los índices apuntan al texto prefijado
        let chars: Vec<char> = both[0].rel_path.chars().collect();
        assert!(both[0].indices.iter().all(|i| *i < chars.len()));
        // índice sucio de una raíz solo la reconstruye a ella
        std::fs::write(b.join("gamma.txt"), "").unwrap();
        st.file_index.lock().unwrap().mark_root_dirty(&b);
        let all = query(&st, &[(a, "a".into()), (b, "b".into())], "", 10);
        assert_eq!(all.len(), 3);
    }
}
