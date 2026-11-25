use std::path::Path;

use super::roles::{compact_chain, DirCtx};
use super::FsEntry;
use crate::error::AppResult;

const HIDDEN: &[&str] = &[".git", ".DS_Store"];
/// Directorios donde no se busca marcador de módulo (evita miles de stats).
const NO_MODULE: &[&str] = &[
    "node_modules", "target", "dist", "build", "out", ".venv", "venv", "__pycache__", ".next", ".idea", "coverage",
];
const MARKERS: &[(&str, &str)] = &[
    ("pom.xml", "maven"),
    ("build.gradle", "gradle"),
    ("build.gradle.kts", "gradle"),
    ("Cargo.toml", "cargo"),
    ("go.mod", "go"),
    ("package.json", "node"),
    ("pyproject.toml", "python"),
];

pub(crate) fn module_of(dir: &Path) -> Option<String> {
    MARKERS
        .iter()
        .find(|(file, _)| dir.join(file).is_file())
        .map(|(_, m)| (*m).to_string())
}

/// Lista un directorio: carpetas primero, luego alfabético sin distinguir mayúsculas.
/// Omite `.git` y `.DS_Store`. No aplica .gitignore.
pub fn read_dir(dir: &Path) -> AppResult<Vec<FsEntry>> {
    let mut out = Vec::new();
    let ctx = DirCtx::of(dir);
    let compactable = ctx.compactable();
    for entry in std::fs::read_dir(dir)? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        if HIDDEN.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        let is_symlink = entry.file_type().map(|t| t.is_symlink()).unwrap_or(false);
        // metadata sigue enlaces: un symlink a carpeta se comporta como carpeta.
        let (is_dir, size) = match std::fs::metadata(&path) {
            Ok(m) => (m.is_dir(), if m.is_dir() { 0 } else { m.len() }),
            Err(_) => (false, 0), // enlace roto
        };
        let ext = if is_dir {
            None
        } else {
            path.extension().map(|e| e.to_string_lossy().to_lowercase())
        };
        let module = if is_dir && !is_symlink && !NO_MODULE.contains(&name.as_str()) {
            module_of(&path)
        } else {
            None
        };
        let role = if is_dir { ctx.child_role(&name, module.is_some()) } else { None };
        let compact = if is_dir && !is_symlink && compactable { compact_chain(&path, &name) } else { None };
        out.push(FsEntry {
            name,
            path: path.to_string_lossy().into_owned(),
            is_dir,
            is_symlink,
            size,
            ext,
            module,
            role,
            compact,
        });
    }
    out.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_module_dirs() {
        let d = std::env::temp_dir().join(format!("cc-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("crate1")).unwrap();
        std::fs::write(d.join("crate1/Cargo.toml"), "[package]").unwrap();
        std::fs::create_dir_all(d.join("plain")).unwrap();
        std::fs::create_dir_all(d.join("node_modules")).unwrap();
        std::fs::write(d.join("node_modules/package.json"), "{}").unwrap();
        std::fs::write(d.join("package.json"), "{}").unwrap();
        let es = read_dir(&d).unwrap();
        let m = |n: &str| es.iter().find(|e| e.name == n).unwrap().module.clone();
        assert_eq!(m("crate1").as_deref(), Some("cargo"));
        assert_eq!(m("plain"), None);
        assert_eq!(m("node_modules"), None);
        assert_eq!(m("package.json"), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
