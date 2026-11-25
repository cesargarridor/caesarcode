use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

/// Quita el prefijo `\\?\` que añade `canonicalize` en Windows. No-op en el resto.
pub fn strip_verbatim(p: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let s = p.to_string_lossy().into_owned();
        if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(rest.to_string());
        }
        p
    }
    #[cfg(not(windows))]
    {
        p
    }
}

/// `canonicalize` sin prefijo `\\?\` en Windows.
pub fn clean_canonicalize(p: &Path) -> std::io::Result<PathBuf> {
    p.canonicalize().map(strip_verbatim)
}

fn check_inside(root: &Path, resolved: PathBuf, original: &Path) -> AppResult<PathBuf> {
    if resolved.starts_with(root) {
        Ok(resolved)
    } else {
        Err(AppError::OutsideWorkspace(original.display().to_string()))
    }
}

/// Canonicaliza el padre de `p` y vuelve a unir el nombre (sin resolver el último componente).
fn canonical_parent_join(p: &Path) -> AppResult<PathBuf> {
    let name = p
        .file_name()
        .ok_or_else(|| AppError::InvalidArgument(p.display().to_string()))?;
    let parent = p
        .parent()
        .ok_or_else(|| AppError::InvalidArgument(p.display().to_string()))?;
    if !parent.exists() {
        return Err(AppError::NotFound(parent.display().to_string()));
    }
    Ok(clean_canonicalize(parent)?.join(name))
}

/// Valida que `p` está dentro de `root` (canonicalizado). Si `p` no existe aún,
/// se canonicaliza su padre y se vuelve a unir el nombre.
pub fn ensure_in_workspace(root: &Path, p: &Path) -> AppResult<PathBuf> {
    let resolved = if p.exists() {
        clean_canonicalize(p)?
    } else {
        canonical_parent_join(p)?
    };
    check_inside(root, resolved, p)
}

/// Elige la raíz (prefijo más largo) que contiene `resolved`.
fn pick_root(roots: &[PathBuf], resolved: PathBuf, original: &Path) -> AppResult<(PathBuf, PathBuf)> {
    roots
        .iter()
        .filter(|r| resolved.starts_with(r))
        .max_by_key(|r| r.components().count())
        .cloned()
        .map(|r| (resolved, r))
        .ok_or_else(|| AppError::OutsideWorkspace(original.display().to_string()))
}

/// Como `ensure_in_workspace` pero contra varias raíces. Devuelve `(ruta resuelta, raíz dueña)`.
pub fn ensure_in_roots(roots: &[PathBuf], p: &Path) -> AppResult<(PathBuf, PathBuf)> {
    let resolved = if p.exists() { clean_canonicalize(p)? } else { canonical_parent_join(p)? };
    pick_root(roots, resolved, p)
}

/// Como `ensure_in_roots`, pero admite también archivos concedidos por Rust (`granted`, rutas
/// canónicas, solo archivos existentes). Devuelve la ruta resuelta.
pub fn ensure_in_roots_or_granted(
    roots: &[PathBuf],
    granted: &std::collections::HashSet<PathBuf>,
    p: &Path,
) -> AppResult<PathBuf> {
    match ensure_in_roots(roots, p) {
        Ok((r, _)) => Ok(r),
        Err(e) => {
            if p.exists() {
                if let Ok(c) = clean_canonicalize(p) {
                    if granted.contains(&c) {
                        return Ok(c);
                    }
                }
            }
            Err(e)
        }
    }
}

/// Como `ensure_in_roots` pero NO sigue un symlink en el último componente.
pub fn ensure_in_roots_nofollow(roots: &[PathBuf], p: &Path) -> AppResult<(PathBuf, PathBuf)> {
    let resolved = canonical_parent_join(p)?;
    pick_root(roots, resolved, p)
}

/// Longitud en unidades UTF-16 (columnas LSP / CodeMirror).
pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_in_roots_picks_owner_and_rejects_outside() {
        let d = tempfile::tempdir().unwrap();
        let base = clean_canonicalize(d.path()).unwrap();
        let (a, b, c) = (base.join("a"), base.join("b"), base.join("c"));
        for x in [&a, &b, &c] {
            std::fs::create_dir_all(x.join("src")).unwrap();
        }
        std::fs::write(b.join("src/f.txt"), "x").unwrap();
        let roots = vec![a.clone(), b.clone()];
        let (p, r) = ensure_in_roots(&roots, &b.join("src/f.txt")).unwrap();
        assert_eq!((p, r), (b.join("src/f.txt"), b.clone()));
        // no existe aún: se valida el padre
        let (_, r) = ensure_in_roots(&roots, &a.join("src/new.txt")).unwrap();
        assert_eq!(r, a);
        assert!(ensure_in_roots(&roots, &c.join("src")).is_err());
        // `..` que se escapa de la raíz
        assert!(ensure_in_roots(&roots, &a.join("src/../../c/src")).is_err());
        assert!(ensure_in_roots_nofollow(&roots, &c.join("x")).is_err());
        assert_eq!(ensure_in_roots_nofollow(&roots, &b.join("src/f.txt")).unwrap().1, b);
    }

    #[test]
    fn granted_files_allowed_outside_roots_only_if_granted() {
        let d = tempfile::tempdir().unwrap();
        let base = clean_canonicalize(d.path()).unwrap();
        let (a, out) = (base.join("a"), base.join("out"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        let f = out.join("x.txt");
        let g = out.join("y.txt");
        std::fs::write(&f, "x").unwrap();
        std::fs::write(&g, "y").unwrap();
        let roots = vec![a.clone()];
        let mut granted = std::collections::HashSet::new();
        assert!(ensure_in_roots_or_granted(&roots, &granted, &f).is_err());
        granted.insert(f.clone());
        assert_eq!(ensure_in_roots_or_granted(&roots, &granted, &f).unwrap(), f);
        assert!(ensure_in_roots_or_granted(&roots, &granted, &g).is_err());
        // `..` que apunta a un archivo concedido se resuelve a su ruta canónica
        assert_eq!(ensure_in_roots_or_granted(&roots, &granted, &a.join("../out/x.txt")).unwrap(), f);
        // sin raíces (sin workspace) también funciona
        assert_eq!(ensure_in_roots_or_granted(&[], &granted, &f).unwrap(), f);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escaping_roots_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        let base = clean_canonicalize(d.path()).unwrap();
        let (a, out) = (base.join("a"), base.join("out"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        std::os::unix::fs::symlink(&out, a.join("link")).unwrap();
        assert!(ensure_in_roots(&[a], &base.join("a/link")).is_err());
    }
}
