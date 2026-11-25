//! Workspace multi-raíz: una raíz primaria (la carpeta abierta) y raíces adjuntas.

pub mod commands;
pub mod store;
pub mod validate;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Una raíz del workspace. `path` está canonicalizada y es su identidad.
#[derive(Clone, Debug)]
pub struct Root {
    pub path: PathBuf,
    /// Basename; si se repite entre raíces: `"api (services)"` (basename del padre).
    pub name: String,
    /// `false` si la carpeta persistida ya no existe (no se vigila ni indexa).
    pub exists: bool,
}

/// `roots[0]` = raíz PRIMARIA (identidad del workspace).
#[derive(Clone, Debug, Default)]
pub struct Workspace {
    pub roots: Vec<Root>,
}

fn base_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

/// Nombres visibles de las raíces (mismo orden), desambiguando basenames repetidos con el del padre.
pub fn display_names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| {
            let base = base_name(p);
            let dup = paths.iter().filter(|q| base_name(q) == base).count() > 1;
            if !dup {
                return base;
            }
            match p.parent().map(base_name) {
                Some(parent) if !parent.is_empty() => format!("{base} ({parent})"),
                _ => base,
            }
        })
        .collect()
}

impl Workspace {
    pub fn new(paths: Vec<PathBuf>) -> Self {
        let names = display_names(&paths);
        let roots = paths
            .into_iter()
            .zip(names)
            .map(|(path, name)| {
                let exists = path.is_dir();
                Root { path, name, exists }
            })
            .collect();
        Self { roots }
    }

    #[cfg(test)]
    pub fn single(path: PathBuf) -> Self {
        Self::new(vec![path])
    }

    pub fn primary(&self) -> Option<&Path> {
        self.roots.first().map(|r| r.path.as_path())
    }

    /// Rutas de las raíces que existen en disco.
    pub fn existing(&self) -> Vec<PathBuf> {
        self.roots.iter().filter(|r| r.exists).map(|r| r.path.clone()).collect()
    }

    pub fn all_paths(&self) -> Vec<PathBuf> {
        self.roots.iter().map(|r| r.path.clone()).collect()
    }

    /// Recalcula `exists` y los nombres.
    pub fn refresh(&mut self) {
        let paths = self.all_paths();
        *self = Self::new(paths);
    }

    /// Raíz existente que contiene `p` (prefijo más largo).
    #[cfg(test)]
    pub fn root_for(&self, p: &Path) -> Option<&Root> {
        self.roots.iter().filter(|r| r.exists && p.starts_with(&r.path)).max_by_key(|r| r.path.components().count())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RootInfo {
    pub path: String,
    pub name: String,
    pub exists: bool,
}

impl From<&Root> for RootInfo {
    fn from(r: &Root) -> Self {
        Self { path: r.path.to_string_lossy().into_owned(), name: r.name.clone(), exists: r.exists }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    /// Raíz primaria (compatibilidad con el frontend actual).
    pub root: String,
    pub name: String,
    pub roots: Vec<RootInfo>,
}

impl WorkspaceInfo {
    pub fn from_workspace(ws: &Workspace) -> Self {
        let roots: Vec<RootInfo> = ws.roots.iter().map(RootInfo::from).collect();
        let (root, name) = roots.first().map(|r| (r.path.clone(), r.name.clone())).unwrap_or_default();
        Self { root, name, roots }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_disambiguate_duplicates() {
        let n = display_names(&[PathBuf::from("/a/api"), PathBuf::from("/b/api"), PathBuf::from("/c/web")]);
        assert_eq!(n, vec!["api (a)", "api (b)", "web"]);
    }

    #[test]
    fn root_for_picks_longest_existing() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a");
        std::fs::create_dir_all(&a).unwrap();
        let ws = Workspace::new(vec![a.clone(), d.path().join("gone")]);
        assert!(!ws.roots[1].exists);
        assert_eq!(ws.root_for(&a.join("x/y")).map(|r| r.path.clone()), Some(a));
        assert!(ws.root_for(&d.path().join("gone/f")).is_none());
    }
}
