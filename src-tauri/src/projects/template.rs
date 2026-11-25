//! Motor de plantillas mínimo: sustituye `{{var}}`. Variable desconocida = error; sin dependencias.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use crate::error::{AppError, AppResult};

pub type Vars = HashMap<&'static str, String>;

/// Sustituye `{{nombre}}` (espacios internos tolerados). Error si falta la variable o queda `{{` sin cerrar.
pub fn render(tpl: &str, vars: &Vars) -> AppResult<String> {
    let mut out = String::with_capacity(tpl.len() + 64);
    let mut rest = tpl;
    while let Some(i) = rest.find("{{") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        let Some(j) = after.find("}}") else {
            return Err(AppError::Other("plantilla con '{{' sin cerrar".into()));
        };
        let key = after[..j].trim();
        let val = vars
            .get(key)
            .ok_or_else(|| AppError::Other(format!("variable de plantilla desconocida: {key}")))?;
        out.push_str(val);
        rest = &after[j + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Ruta relativa segura (sin `..`, sin absolutas) para escribir bajo `root`.
pub fn safe_join(root: &Path, rel: &str) -> AppResult<PathBuf> {
    let p = Path::new(rel);
    if p.components().any(|c| !matches!(c, Component::Normal(_) | Component::CurDir)) {
        return Err(AppError::InvalidArgument(format!("ruta de plantilla insegura: {rel}")));
    }
    Ok(root.join(p))
}

/// Escribe un conjunto de `(ruta con vars, contenido con vars)` bajo `root`. Devuelve cuántos archivos.
pub fn write_all(root: &Path, files: &[(&str, &str)], vars: &Vars) -> AppResult<usize> {
    for (path, body) in files {
        let rel = render(path, vars)?;
        let dest = safe_join(root, &rel)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(dest, render(body, vars)?)?;
    }
    Ok(files.len())
}

/// Escribe un archivo ya renderizado creando los directorios intermedios.
pub fn write_text(root: &Path, rel: &str, text: &str) -> AppResult<()> {
    let dest = safe_join(root, rel)?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vars {
        let mut v = Vars::new();
        v.insert("name", "demo".into());
        v.insert("package_path", "com/example".into());
        v
    }

    #[test]
    fn substitutes() {
        assert_eq!(render("a {{name}} b {{ name }}", &vars()).unwrap(), "a demo b demo");
        assert_eq!(render("sin vars", &vars()).unwrap(), "sin vars");
        assert_eq!(render("{{name}}{{name}}", &vars()).unwrap(), "demodemo");
    }

    #[test]
    fn unknown_or_unclosed_is_error() {
        assert!(render("{{nope}}", &vars()).is_err());
        assert!(render("{{name", &vars()).is_err());
    }

    #[test]
    fn writes_and_rejects_traversal() {
        let d = tempfile::tempdir().unwrap();
        write_all(d.path(), &[("src/{{package_path}}/A.java", "class {{name}}")], &vars()).unwrap();
        assert_eq!(std::fs::read_to_string(d.path().join("src/com/example/A.java")).unwrap(), "class demo");
        assert!(write_all(d.path(), &[("../evil", "x")], &vars()).is_err());
        assert!(write_all(d.path(), &[("/abs", "x")], &vars()).is_err());
    }
}
