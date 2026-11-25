//! Detección de lenguaje y localización de servidores LSP en el PATH.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub struct ServerSpec {
    /// Nombre mostrado (`pyright`, `rust-analyzer`...).
    pub name: String,
    pub program: PathBuf,
    pub args: Vec<String>,
}

fn ext_lower(path: &Path) -> Option<String> {
    path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase())
}

/// Lenguaje LSP del archivo (un servidor por lenguaje).
pub fn language_for(path: &Path) -> Option<&'static str> {
    match ext_lower(path)?.as_str() {
        "py" | "pyi" => Some("python"),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => Some("typescript"),
        "rs" => Some("rust"),
        "java" => Some("java"),
        _ => None,
    }
}

/// `languageId` del `didOpen`.
pub fn language_id_for(path: &Path) -> Option<&'static str> {
    Some(match ext_lower(path)?.as_str() {
        "py" | "pyi" => "python",
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascriptreact",
        "rs" => "rust",
        "java" => "java",
        _ => return None,
    })
}

/// Nombre del ejecutable esperado, para el aviso "LSP no instalado".
pub fn expected_server(lang: &str) -> &'static str {
    match lang {
        "python" => "pyright-langserver",
        "typescript" => "typescript-language-server",
        "rust" => "rust-analyzer",
        "java" => "jdtls",
        _ => "language-server",
    }
}

fn root_hash(root: &Path) -> String {
    let mut h = DefaultHasher::new();
    root.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Comando para arrancar el servidor del lenguaje, o `None` si no está instalado.
pub fn server_command(lang: &str, root: &Path, app_data_dir: &Path) -> Option<ServerSpec> {
    let stdio = || vec!["--stdio".to_string()];
    let found = |name: &str, args: Vec<String>| {
        which::which(name).ok().map(|program| ServerSpec {
            name: name.trim_end_matches("-langserver").to_string(),
            program,
            args,
        })
    };
    match lang {
        "python" => found("pyright-langserver", stdio()).or_else(|| found("basedpyright-langserver", stdio())),
        "typescript" => found("typescript-language-server", stdio()),
        "rust" => found("rust-analyzer", vec![]),
        "java" => {
            let data = app_data_dir.join("jdtls").join(root_hash(root));
            std::fs::create_dir_all(&data).ok()?;
            found("jdtls", vec!["-data".to_string(), data.to_string_lossy().into_owned()])
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_languages() {
        let l = |p: &str| language_for(Path::new(p));
        assert_eq!(l("a/b.py"), Some("python"));
        assert_eq!(l("a/b.PYI"), Some("python"));
        for e in ["ts", "tsx", "js", "jsx", "mjs", "cjs"] {
            assert_eq!(l(&format!("x.{e}")), Some("typescript"));
        }
        assert_eq!(l("lib.rs"), Some("rust"));
        assert_eq!(l("A.java"), Some("java"));
        assert_eq!(l("README.md"), None);
        assert_eq!(l("Makefile"), None);
    }

    #[test]
    fn language_ids() {
        let id = |p: &str| language_id_for(Path::new(p));
        assert_eq!(id("a.tsx"), Some("typescriptreact"));
        assert_eq!(id("a.jsx"), Some("javascriptreact"));
        assert_eq!(id("a.mjs"), Some("javascript"));
        assert_eq!(id("a.ts"), Some("typescript"));
        assert_eq!(id("a.txt"), None);
    }
}
