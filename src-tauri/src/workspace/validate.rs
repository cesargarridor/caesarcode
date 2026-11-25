//! Reglas de raíces (PLAN5 0.4): sin duplicados ni solapes, límite blando.

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::util::paths::clean_canonicalize;

pub const MAX_ROOTS: usize = 64;

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

/// Valida y canonicaliza `new` frente a las raíces ya presentes (rutas canónicas).
pub fn check_new_root(existing: &[PathBuf], new: &Path) -> AppResult<PathBuf> {
    if !new.exists() {
        return Err(AppError::NotFound(new.display().to_string()));
    }
    if !new.is_dir() {
        return Err(AppError::InvalidArgument(format!("no es una carpeta: {}", new.display())));
    }
    let canon = clean_canonicalize(new)?;
    if existing.len() >= MAX_ROOTS {
        return Err(AppError::InvalidArgument(format!("máximo {MAX_ROOTS} raíces en el workspace")));
    }
    for r in existing {
        if *r == canon {
            return Err(AppError::AlreadyExists("ya está en el workspace".into()));
        }
        if canon.starts_with(r) {
            return Err(AppError::InvalidArgument(format!("ya está dentro de {}", name_of(r))));
        }
        if r.starts_with(&canon) {
            return Err(AppError::InvalidArgument(format!(
                "contiene a {}: abre esa carpeta como principal",
                name_of(r)
            )));
        }
    }
    Ok(canon)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let base = clean_canonicalize(d.path()).unwrap();
        let a = base.join("a");
        let b = base.join("b");
        std::fs::create_dir_all(a.join("inner")).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        (d, a, b)
    }

    #[test]
    fn accepts_sibling_rejects_dup_inside_contains() {
        let (d, a, b) = dirs();
        let base = clean_canonicalize(d.path()).unwrap();
        assert_eq!(check_new_root(std::slice::from_ref(&a), &b).unwrap(), b);
        assert!(matches!(check_new_root(std::slice::from_ref(&a), &a), Err(AppError::AlreadyExists(_))));
        let e = check_new_root(std::slice::from_ref(&a), &a.join("inner")).unwrap_err().to_string();
        assert!(e.contains("ya está dentro de a"), "{e}");
        let e = check_new_root(std::slice::from_ref(&a), &base).unwrap_err().to_string();
        assert!(e.contains("contiene a a"), "{e}");
        assert!(check_new_root(&[a], &base.join("nope")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_to_existing_root_is_duplicate() {
        let (d, a, b) = dirs();
        let link = d.path().join("link");
        std::os::unix::fs::symlink(&a, &link).unwrap();
        assert!(matches!(check_new_root(&[a.clone(), b.clone()], &link), Err(AppError::AlreadyExists(_))));
        // symlink a un subdirectorio de una raíz: solape
        let link2 = d.path().join("link2");
        std::os::unix::fs::symlink(a.join("inner"), &link2).unwrap();
        assert!(check_new_root(&[a], &link2).is_err());
    }

    #[test]
    fn soft_limit() {
        let d = tempfile::tempdir().unwrap();
        let base = clean_canonicalize(d.path()).unwrap();
        let many: Vec<PathBuf> = (0..MAX_ROOTS).map(|i| base.join(format!("x{i}"))).collect();
        let new = base.join("new");
        std::fs::create_dir_all(&new).unwrap();
        assert!(check_new_root(&many, &new).is_err());
    }
}
