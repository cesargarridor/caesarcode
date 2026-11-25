//! Roles de carpeta estilo IntelliJ (source/test/resources roots, paquetes, excluidas) y
//! compactación de cadenas de paquetes (`com.example.app`).

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::tree::module_of;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FolderRole {
    SourceRoot,
    TestRoot,
    ResourcesRoot,
    TestResourcesRoot,
    GeneratedRoot,
    Package,
    Excluded,
    Module,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompactChain {
    /// Nombre punteado: `com.example.finance_tracker`.
    pub name: String,
    /// Carpeta hoja, ruta absoluta.
    pub path: String,
}

const JVM_LANGS: &[&str] = &["java", "kotlin", "groovy", "scala"];
const EXCLUDED: &[&str] = &[
    "target", "build", "out", "dist", "node_modules", ".gradle", ".idea", ".venv", "venv", "__pycache__", ".next", "coverage",
];
const HIDDEN: &[&str] = &[".git", ".DS_Store"];
const MAX_UP: usize = 24;
const MAX_CHAIN: usize = 16;

/// Contexto de un directorio: ruta relativa al módulo más cercano y tipo de módulo.
pub struct DirCtx {
    rel: Vec<String>,
    kind: Option<String>,
    has_module: bool,
}

impl DirCtx {
    pub fn of(dir: &Path) -> Self {
        let mut rel: Vec<String> = Vec::new();
        for anc in dir.ancestors().take(MAX_UP) {
            if let Some(kind) = module_of(anc) {
                rel.reverse();
                return Self { rel, kind: Some(kind), has_module: true };
            }
            match anc.file_name() {
                Some(n) => rel.push(n.to_string_lossy().into_owned()),
                None => break,
            }
        }
        Self { rel: Vec::new(), kind: None, has_module: false }
    }

    /// Rol de una ruta relativa al módulo (sin el caso `Module`, que depende del marcador).
    fn role_of(&self, rel: &[&str]) -> Option<FolderRole> {
        if !self.has_module || rel.is_empty() {
            return None;
        }
        if rel.len() == 1 && EXCLUDED.contains(&rel[0]) {
            return Some(FolderRole::Excluded);
        }
        if rel[0] == "src" && rel.len() >= 3 && (rel[1] == "main" || rel[1] == "test") {
            let main = rel[1] == "main";
            if JVM_LANGS.contains(&rel[2]) {
                return Some(match (rel.len(), main) {
                    (3, true) => FolderRole::SourceRoot,
                    (3, false) => FolderRole::TestRoot,
                    _ => FolderRole::Package,
                });
            }
            if rel[2] == "resources" && rel.len() == 3 {
                return Some(if main { FolderRole::ResourcesRoot } else { FolderRole::TestResourcesRoot });
            }
            return None;
        }
        if rel.len() == 3
            && ((rel[0] == "target" && rel[1] == "generated-sources") || (rel[0] == "build" && rel[1] == "generated"))
        {
            return Some(FolderRole::GeneratedRoot);
        }
        let non_jvm = matches!(self.kind.as_deref(), Some("node" | "cargo" | "go" | "python"));
        if rel.len() == 1 {
            if rel[0] == "src" && non_jvm {
                return Some(FolderRole::SourceRoot);
            }
            if matches!(rel[0], "test" | "tests" | "__tests__") {
                return Some(FolderRole::TestRoot);
            }
        }
        None
    }

    /// Rol del propio directorio que se está listando.
    pub fn own_role(&self) -> Option<FolderRole> {
        let rel: Vec<&str> = self.rel.iter().map(String::as_str).collect();
        self.role_of(&rel)
    }

    /// Rol de la subcarpeta `name`. `is_module`: la subcarpeta tiene su propio marcador.
    pub fn child_role(&self, name: &str, is_module: bool) -> Option<FolderRole> {
        if !self.has_module {
            return is_module.then_some(FolderRole::Module);
        }
        let mut rel: Vec<&str> = self.rel.iter().map(String::as_str).collect();
        rel.push(name);
        match self.role_of(&rel) {
            Some(FolderRole::Excluded) => Some(FolderRole::Excluded),
            _ if is_module => Some(FolderRole::Module),
            other => other,
        }
    }

    /// Las cadenas solo se compactan bajo source/test roots y paquetes.
    pub fn compactable(&self) -> bool {
        matches!(
            self.own_role(),
            Some(FolderRole::SourceRoot | FolderRole::TestRoot | FolderRole::Package)
        )
    }
}

fn is_hidden(name: &std::ffi::OsStr) -> bool {
    HIDDEN.contains(&name.to_string_lossy().as_ref())
}

/// Baja mientras la carpeta tenga exactamente una entrada visible y sea una carpeta (sin symlinks).
pub fn compact_chain(path: &Path, name: &str) -> Option<CompactChain> {
    let mut names = vec![name.to_string()];
    let mut cur = path.to_path_buf();
    for _ in 0..MAX_CHAIN {
        let Ok(rd) = std::fs::read_dir(&cur) else { break };
        let mut it = rd.flatten().filter(|e| !is_hidden(&e.file_name()));
        let (first, second) = (it.next(), it.next());
        match (first, second) {
            (Some(e), None) if e.file_type().is_ok_and(|t| t.is_dir()) => {
                names.push(e.file_name().to_string_lossy().into_owned());
                cur = e.path();
            }
            _ => break,
        }
    }
    (names.len() > 1).then(|| CompactChain { name: names.join("."), path: cur.to_string_lossy().into_owned() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, "x").unwrap();
    }

    fn maven_tree() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        touch(&r.join("pom.xml"));
        touch(&r.join("src/main/java/com/example/finance_tracker/App.java"));
        fs::create_dir_all(r.join("src/main/java/com/example/finance_tracker/web")).unwrap();
        touch(&r.join("src/test/java/com/example/finance_tracker/AppTest.java"));
        touch(&r.join("src/main/resources/application.yml"));
        touch(&r.join("src/test/resources/test.properties"));
        touch(&r.join("target/classes/A.class"));
        touch(&r.join("target/generated-sources/annotations/G.java"));
        d
    }

    fn role(parent: &Path, name: &str) -> Option<FolderRole> {
        DirCtx::of(parent).child_role(name, false)
    }

    #[test]
    fn maven_roles() {
        let d = maven_tree();
        let r = d.path();
        assert_eq!(role(r, "target"), Some(FolderRole::Excluded));
        assert_eq!(role(r, "src"), None);
        assert_eq!(role(&r.join("src/main"), "java"), Some(FolderRole::SourceRoot));
        assert_eq!(role(&r.join("src/test"), "java"), Some(FolderRole::TestRoot));
        assert_eq!(role(&r.join("src/main"), "resources"), Some(FolderRole::ResourcesRoot));
        assert_eq!(role(&r.join("src/test"), "resources"), Some(FolderRole::TestResourcesRoot));
        assert_eq!(role(&r.join("src/main/java"), "com"), Some(FolderRole::Package));
        assert_eq!(role(&r.join("src/main/java/com/example/finance_tracker"), "web"), Some(FolderRole::Package));
        assert_eq!(role(&r.join("target/generated-sources"), "annotations"), Some(FolderRole::GeneratedRoot));
        assert_eq!(role(&r.join("src/main/resources"), "static"), None);
    }

    #[test]
    fn nested_module_and_non_jvm_fallback() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        touch(&r.join("pom.xml"));
        touch(&r.join("svc/package.json"));
        fs::create_dir_all(r.join("svc/src")).unwrap();
        fs::create_dir_all(r.join("svc/tests")).unwrap();
        assert_eq!(DirCtx::of(r).child_role("svc", true), Some(FolderRole::Module));
        assert_eq!(role(&r.join("svc"), "src"), Some(FolderRole::SourceRoot));
        assert_eq!(role(&r.join("svc"), "tests"), Some(FolderRole::TestRoot));
        // `src` de un módulo Maven no es source root.
        assert_eq!(role(r, "src"), None);
        // Excluida gana a módulo.
        assert_eq!(DirCtx::of(r).child_role("node_modules", true), Some(FolderRole::Excluded));
    }

    #[test]
    fn no_marker_no_roles() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("src/main/java")).unwrap();
        assert_eq!(role(&d.path().join("src/main"), "java"), None);
    }

    #[test]
    fn compaction_stops_at_files_and_forks() {
        let d = maven_tree();
        let java = d.path().join("src/main/java");
        let ctx = DirCtx::of(&java);
        assert!(ctx.compactable());
        let ch = compact_chain(&java.join("com"), "com").unwrap();
        assert_eq!(ch.name, "com.example.finance_tracker");
        assert!(ch.path.ends_with("finance_tracker"));

        // Carpeta con archivo: no compacta.
        let d2 = tempfile::tempdir().unwrap();
        touch(&d2.path().join("a/b/F.java"));
        touch(&d2.path().join("a/f.txt"));
        assert!(compact_chain(&d2.path().join("a"), "a").is_none());
        // Dos hijos: para en el padre.
        fs::create_dir_all(d2.path().join("x/y/z1")).unwrap();
        fs::create_dir_all(d2.path().join("x/y/z2")).unwrap();
        let ch = compact_chain(&d2.path().join("x"), "x").unwrap();
        assert_eq!(ch.name, "x.y");

        // Raíz de recursos: no es compactable.
        assert!(!DirCtx::of(&d.path().join("src/main/resources")).compactable());
        assert!(!DirCtx::of(d.path()).compactable());
    }

    #[cfg(unix)]
    #[test]
    fn compaction_does_not_follow_symlinks() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("real/inner")).unwrap();
        fs::create_dir_all(d.path().join("a")).unwrap();
        std::os::unix::fs::symlink(d.path().join("real"), d.path().join("a/link")).unwrap();
        assert!(compact_chain(&d.path().join("a"), "a").is_none());
    }
}
