//! Clasificador de símbolos Java/Kotlin por cabecera (≤ 8 KiB) para los iconos del árbol.
//! Sin parser real: pre-lexer que quita comentarios/strings y regex sobre el texto limpio.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use regex::Regex;
use serde::Serialize;

use crate::util::paths::ensure_in_roots;

const HEADER_BYTES: u64 = 8192;
const MAX_PATHS: usize = 512;
const MAX_CACHE: usize = 20_000;
const SYMBOL_EXTS: &[&str] = &["java", "kt", "kts", "groovy", "scala"];

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SymbolKind {
    Class,
    AbstractClass,
    Interface,
    Enum,
    Record,
    Annotation,
    Exception,
    Object,
    DataClass,
    SealedClass,
    KotlinFile,
    Script,
    Unknown,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Stereotype {
    SpringBootApp,
    Main,
    Controller,
    Service,
    Repository,
    Configuration,
    Entity,
    Component,
    Test,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInfo {
    pub kind: SymbolKind,
    pub stereotype: Option<Stereotype>,
    pub public: bool,
}

// ---------------------------------------------------------------------------------------------
// Caché
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
pub struct SymbolCache {
    map: HashMap<PathBuf, (SystemTime, u64, SymbolInfo)>,
    /// Lecturas reales de archivo (para verificar la caché en tests).
    pub reads: usize,
}

impl SymbolCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Olvida las rutas bajo `root` (al quitar una raíz del workspace).
    pub fn clear_under(&mut self, root: &Path) {
        self.map.retain(|p, _| !p.starts_with(root));
    }

    fn get(&self, p: &Path, mtime: SystemTime, len: u64) -> Option<SymbolInfo> {
        match self.map.get(p) {
            Some((m, l, info)) if *m == mtime && *l == len => Some(info.clone()),
            _ => None,
        }
    }

    fn insert(&mut self, p: PathBuf, mtime: SystemTime, len: u64, info: SymbolInfo) {
        if self.map.len() >= MAX_CACHE {
            self.map.clear();
        }
        self.map.insert(p, (mtime, len, info));
    }
}

fn ext_of(p: &Path) -> Option<String> {
    p.extension().map(|e| e.to_string_lossy().to_lowercase())
}

/// Clasifica en lote. Rutas fuera del workspace, no soportadas o ilegibles se omiten.
/// La clave devuelta es la ruta tal cual la pidió el frontend.
#[cfg(test)]
pub fn classify_paths(cache: &Mutex<SymbolCache>, root: &Path, paths: &[String]) -> Vec<(String, SymbolInfo)> {
    classify_paths_in(cache, &[root.to_path_buf()], paths)
}

/// Como `classify_paths` pero con varias raíces.
pub fn classify_paths_in(cache: &Mutex<SymbolCache>, roots: &[PathBuf], paths: &[String]) -> Vec<(String, SymbolInfo)> {
    let mut out = Vec::with_capacity(paths.len().min(MAX_PATHS));
    for raw in paths.iter().take(MAX_PATHS) {
        let Ok((p, _)) = ensure_in_roots(roots, Path::new(raw)) else { continue };
        let Some(ext) = ext_of(&p) else { continue };
        if !SYMBOL_EXTS.contains(&ext.as_str()) {
            continue;
        }
        let Ok(meta) = std::fs::metadata(&p) else { continue };
        if !meta.is_file() {
            continue;
        }
        let (Ok(mtime), len) = (meta.modified(), meta.len()) else { continue };
        let hit = cache.lock().ok().and_then(|c| c.get(&p, mtime, len));
        if let Some(info) = hit {
            out.push((raw.clone(), info));
            continue;
        }
        let Ok(file) = std::fs::File::open(&p) else { continue };
        let mut buf = Vec::new();
        if file.take(HEADER_BYTES).read_to_end(&mut buf).is_err() {
            continue;
        }
        let text = String::from_utf8_lossy(&buf);
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let info = classify_source(&name, &text);
        if let Ok(mut c) = cache.lock() {
            c.reads += 1;
            c.insert(p, mtime, len, info.clone());
        }
        out.push((raw.clone(), info));
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Pre-lexer
// ---------------------------------------------------------------------------------------------

/// Sustituye comentarios, strings, text blocks y chars por espacios (conserva saltos de línea).
fn strip(src: &str) -> String {
    let b = src.as_bytes();
    let n = b.len();
    let mut out = b.to_vec();
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for byte in out.iter_mut().take(to.min(n)).skip(from) {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    };
    let mut i = 0;
    while i < n {
        match b[i] {
            b'/' if i + 1 < n && b[i + 1] == b'/' => {
                let end = b[i..].iter().position(|&c| c == b'\n').map_or(n, |p| i + p);
                blank(&mut out, i, end);
                i = end;
            }
            b'/' if i + 1 < n && b[i + 1] == b'*' => {
                let end = find(b, i + 2, b"*/").map_or(n, |p| p + 2);
                blank(&mut out, i, end);
                i = end;
            }
            b'"' if b[i..].starts_with(b"\"\"\"") => {
                let end = find(b, i + 3, b"\"\"\"").map_or(n, |p| p + 3);
                blank(&mut out, i, end);
                i = end;
            }
            b'"' => {
                let mut j = i + 1;
                while j < n && b[j] != b'"' && b[j] != b'\n' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                let end = if j < n && b[j] == b'"' { j + 1 } else { j.min(n) };
                blank(&mut out, i, end);
                i = end.max(i + 1);
            }
            b'\'' => {
                // Char literal corto ('x', '\n', 'A'); si no cierra pronto no es un char.
                let limit = (i + 8).min(n);
                let mut j = i + 1;
                let mut closed = None;
                while j < limit && b[j] != b'\n' {
                    if b[j] == b'\\' {
                        j += 2;
                        continue;
                    }
                    if b[j] == b'\'' {
                        closed = Some(j);
                        break;
                    }
                    j += 1;
                }
                match closed {
                    Some(end) if end > i + 1 => {
                        blank(&mut out, i, end + 1);
                        i = end + 1;
                    }
                    _ => i += 1,
                }
            }
            _ => i += 1,
        }
    }
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn find(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

// ---------------------------------------------------------------------------------------------
// Clasificación
// ---------------------------------------------------------------------------------------------

static RE_ANNOT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"@([A-Za-z_][\w.]*)").unwrap());
static RE_DECL_JAVA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?P<mods>(?:\b(?:public|protected|private|abstract|final|sealed|non-sealed|static|strictfp)\s+)*)(?P<kw>@\s*interface|\bclass|\binterface|\benum|\brecord)\s+(?P<name>[A-Za-z_]\w*)",
    )
    .unwrap()
});
static RE_DECL_KT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?P<mods>(?:\b(?:public|private|internal|protected|open|abstract|final|sealed|data|enum|annotation|value|inline|inner|fun|expect|actual|external)\s+)*)(?P<kw>\bclass|\binterface|\bobject)\s+(?P<name>[A-Za-z_]\w*)",
    )
    .unwrap()
});
static RE_MAIN_JAVA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"static\s+void\s+main\s*\(").unwrap());
static RE_MAIN_KT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*(?:suspend\s+)?fun\s+main\s*\(").unwrap());
static RE_FUN_KT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*(?:[a-z]+\s+)*fun\s+[<\w]").unwrap());
static RE_REPO_SUPER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(JpaRepository|CrudRepository|PagingAndSortingRepository|MongoRepository|ReactiveCrudRepository)\b").unwrap()
});
static RE_EXC_SUPER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\w*(Exception|Error|Throwable)\b").unwrap());

struct Decl {
    start: usize,
    end: usize,
    mods: String,
    kw: String,
    name: String,
}

fn stereotype_of_annotation(name: &str) -> Option<Stereotype> {
    let last = name.rsplit('.').next().unwrap_or(name);
    Some(match last {
        "SpringBootApplication" => Stereotype::SpringBootApp,
        "RestController" | "Controller" => Stereotype::Controller,
        "Service" => Stereotype::Service,
        "Repository" => Stereotype::Repository,
        "Configuration" | "ConfigurationProperties" => Stereotype::Configuration,
        "Entity" | "Table" | "Document" | "MappedSuperclass" | "Embeddable" => Stereotype::Entity,
        "Component" => Stereotype::Component,
        "Test" | "ParameterizedTest" | "SpringBootTest" | "WebMvcTest" | "DataJpaTest" => Stereotype::Test,
        _ => return None,
    })
}

/// Prioridad entre estereotipos presentes a nivel de clase.
const PRIORITY: &[Stereotype] = &[
    Stereotype::SpringBootApp,
    Stereotype::Controller,
    Stereotype::Service,
    Stereotype::Repository,
    Stereotype::Configuration,
    Stereotype::Entity,
    Stereotype::Component,
    Stereotype::Test,
];

fn stereotype_from_suffix(stem: &str) -> Option<Stereotype> {
    let long = |suf: &str| stem.len() > suf.len() && stem.ends_with(suf);
    if long("Controller") {
        Some(Stereotype::Controller)
    } else if long("ServiceImpl") || long("Service") {
        Some(Stereotype::Service)
    } else if long("Repository") || long("Repo") {
        Some(Stereotype::Repository)
    } else if long("Configuration") || long("Config") {
        Some(Stereotype::Configuration)
    } else if long("Tests") || long("Test") || long("IT") {
        Some(Stereotype::Test)
    } else {
        None
    }
}

/// Clasifica el texto de cabecera de un archivo fuente. Pura (sin E/S).
pub fn classify_source(file_name: &str, raw: &str) -> SymbolInfo {
    let lower = file_name.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    let stem = file_name.rsplit_once('.').map_or(file_name, |(s, _)| s);

    if ext == "kts" {
        return SymbolInfo { kind: SymbolKind::Script, stereotype: None, public: true };
    }
    let kotlin = ext == "kt";
    let text = strip(raw);

    // Profundidad de llaves por byte: solo cuentan las declaraciones de nivel superior.
    let mut depth = Vec::with_capacity(text.len() + 1);
    let mut d: i32 = 0;
    for &c in text.as_bytes() {
        depth.push(d);
        if c == b'{' {
            d += 1;
        } else if c == b'}' {
            d = (d - 1).max(0);
        }
    }
    depth.push(d);

    let re = if kotlin { &*RE_DECL_KT } else { &*RE_DECL_JAVA };
    let decls: Vec<Decl> = re
        .captures_iter(&text)
        .filter_map(|c| {
            let whole = c.get(0)?;
            let kw = c.name("kw")?;
            // La posición real de la declaración es la de la palabra clave.
            if depth.get(kw.start()).copied().unwrap_or(0) != 0 {
                return None;
            }
            Some(Decl {
                start: whole.start(),
                end: whole.end(),
                mods: c.name("mods").map_or(String::new(), |m| m.as_str().to_string()),
                kw: kw.as_str().replace(char::is_whitespace, ""),
                name: c.name("name")?.as_str().to_string(),
            })
        })
        .collect();

    let has_top_fun = kotlin
        && RE_FUN_KT
            .find_iter(&text)
            .any(|m| depth.get(m.end().saturating_sub(1)).copied().unwrap_or(0) == 0);

    let chosen = decls.iter().find(|d| d.name == stem).or_else(|| {
        if kotlin && (decls.len() >= 2 || has_top_fun) {
            None
        } else {
            decls.first()
        }
    });

    let has = |mods: &str, w: &str| mods.split_whitespace().any(|m| m == w);
    let (mut kind, public, decl_start, header) = match chosen {
        Some(d) => {
            let kind = if kotlin {
                match d.kw.as_str() {
                    "object" => SymbolKind::Object,
                    "interface" if has(&d.mods, "sealed") => SymbolKind::SealedClass,
                    "interface" if has(&d.mods, "annotation") => SymbolKind::Annotation,
                    "interface" => SymbolKind::Interface,
                    _ if has(&d.mods, "enum") => SymbolKind::Enum,
                    _ if has(&d.mods, "annotation") => SymbolKind::Annotation,
                    _ if has(&d.mods, "data") => SymbolKind::DataClass,
                    _ if has(&d.mods, "sealed") => SymbolKind::SealedClass,
                    _ if has(&d.mods, "abstract") => SymbolKind::AbstractClass,
                    _ => SymbolKind::Class,
                }
            } else {
                match d.kw.as_str() {
                    "@interface" => SymbolKind::Annotation,
                    "interface" => SymbolKind::Interface,
                    "enum" => SymbolKind::Enum,
                    "record" => SymbolKind::Record,
                    _ if has(&d.mods, "abstract") => SymbolKind::AbstractClass,
                    _ => SymbolKind::Class,
                }
            };
            let public = if kotlin {
                !(has(&d.mods, "private") || has(&d.mods, "internal") || has(&d.mods, "protected"))
            } else {
                has(&d.mods, "public")
            };
            // Cabecera tras el nombre hasta la primera `{` (supertipos).
            let rest = &text[d.end..];
            let stop = rest.find('{').unwrap_or(rest.len()).min(600);
            let header = rest[..floor_boundary(rest, stop)].to_string();
            (kind, public, Some(d.start), header)
        }
        None if kotlin => (SymbolKind::KotlinFile, true, None, String::new()),
        None => (SymbolKind::Unknown, true, None, String::new()),
    };

    // Herencia.
    let mut repo_by_super = false;
    if matches!(kind, SymbolKind::Class | SymbolKind::AbstractClass) && RE_EXC_SUPER.is_match(&header) {
        let supers = if kotlin { header.contains(':') } else { header.contains("extends") };
        if supers {
            kind = SymbolKind::Exception;
        }
    }
    if matches!(kind, SymbolKind::Interface | SymbolKind::Class | SymbolKind::AbstractClass)
        && RE_REPO_SUPER.is_match(&header)
        && (header.contains("extends") || header.contains(':'))
    {
        repo_by_super = true;
    }

    // Anotaciones: las previas a la declaración son de clase; las posteriores solo cuentan para Test.
    let mut class_level: Vec<Stereotype> = Vec::new();
    let mut member_test = false;
    for c in RE_ANNOT.captures_iter(&text) {
        let (Some(m), Some(name)) = (c.get(0), c.get(1)) else { continue };
        let Some(st) = stereotype_of_annotation(name.as_str()) else { continue };
        let is_class_level = decl_start.is_some_and(|s| m.start() < s);
        if is_class_level {
            class_level.push(st);
        } else if st == Stereotype::Test {
            member_test = true;
        }
    }
    let mut stereotype = PRIORITY.iter().copied().find(|p| class_level.contains(p));
    if stereotype.is_none() && repo_by_super {
        stereotype = Some(Stereotype::Repository);
    }
    if stereotype.is_none() {
        let main = if kotlin { &*RE_MAIN_KT } else { &*RE_MAIN_JAVA };
        let is_main = main
            .find(&text)
            .is_some_and(|m| !kotlin || depth.get(m.end().saturating_sub(1)).copied().unwrap_or(0) == 0);
        if is_main {
            stereotype = Some(Stereotype::Main);
        }
    }
    if stereotype.is_none() && member_test {
        stereotype = Some(Stereotype::Test);
    }
    if stereotype.is_none() {
        stereotype = stereotype_from_suffix(stem);
    }
    SymbolInfo { kind, stereotype, public }
}

fn floor_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! fx {
        ($n:literal) => {
            ($n, include_str!(concat!("../../tests/fixtures/symbols/", $n)))
        };
    }

    fn c(f: (&str, &str)) -> SymbolInfo {
        classify_source(f.0, f.1)
    }

    #[test]
    fn spring_stereotypes() {
        let i = c(fx!("UserController.java"));
        assert_eq!((i.kind, i.stereotype, i.public), (SymbolKind::Class, Some(Stereotype::Controller), true));
        assert_eq!(c(fx!("UserService.java")).stereotype, Some(Stereotype::Service));
        assert_eq!(c(fx!("User.java")).stereotype, Some(Stereotype::Entity));
        assert_eq!(c(fx!("AppConfig.java")).stereotype, Some(Stereotype::Configuration));
        assert_eq!(c(fx!("UserControllerTest.java")).stereotype, Some(Stereotype::Test));
    }

    #[test]
    fn repository_by_inheritance() {
        let i = c(fx!("UserRepository.java"));
        assert_eq!((i.kind, i.stereotype), (SymbolKind::Interface, Some(Stereotype::Repository)));
    }

    #[test]
    fn spring_boot_wins_over_main() {
        let i = c(fx!("FinanceTrackerApplication.java"));
        assert_eq!(i.stereotype, Some(Stereotype::SpringBootApp));
        let m = c(fx!("Main.java"));
        assert_eq!((m.kind, m.stereotype), (SymbolKind::Class, Some(Stereotype::Main)));
    }

    #[test]
    fn java_kinds() {
        assert_eq!(c(fx!("Shape.java")).kind, SymbolKind::AbstractClass);
        assert_eq!(c(fx!("Color.java")).kind, SymbolKind::Enum);
        assert_eq!(c(fx!("Point.java")).kind, SymbolKind::Record);
        assert_eq!(c(fx!("Audited.java")).kind, SymbolKind::Annotation);
        assert_eq!(c(fx!("NotFoundException.java")).kind, SymbolKind::Exception);
    }

    #[test]
    fn comments_and_strings_do_not_count() {
        let i = c(fx!("Commented.java"));
        assert_eq!((i.kind, i.stereotype), (SymbolKind::Class, None));
    }

    #[test]
    fn kotlin_kinds() {
        assert_eq!(c(fx!("Money.kt")).kind, SymbolKind::DataClass);
        assert_eq!(c(fx!("Result.kt")).kind, SymbolKind::SealedClass);
        assert_eq!(c(fx!("Registry.kt")).kind, SymbolKind::Object);
        let u = c(fx!("Utils.kt"));
        assert_eq!((u.kind, u.stereotype), (SymbolKind::KotlinFile, None));
        let a = c(fx!("App.kt"));
        assert_eq!((a.kind, a.stereotype), (SymbolKind::Class, Some(Stereotype::SpringBootApp)));
        assert_eq!(c(fx!("build.gradle.kts")).kind, SymbolKind::Script);
    }

    #[test]
    fn kotlin_top_level_main_only() {
        let i = classify_source("Run.kt", "fun main() {\n}\n");
        assert_eq!((i.kind, i.stereotype), (SymbolKind::KotlinFile, Some(Stereotype::Main)));
        let nested = classify_source("Foo.kt", "class Foo {\n    fun main() {}\n}\n");
        assert_eq!(nested.stereotype, None);
    }

    #[test]
    fn suffix_fallback() {
        let i = classify_source("PaymentController.java", "public class PaymentController {}");
        assert_eq!(i.stereotype, Some(Stereotype::Controller));
        let t = classify_source("FooTest.java", "class FooTest {}");
        assert_eq!(t.stereotype, Some(Stereotype::Test));
        let plain = classify_source("Test.java", "class Test {}");
        assert_eq!(plain.stereotype, None);
    }

    #[test]
    fn nested_declarations_are_ignored() {
        let i = classify_source("Outer.java", "public class Outer {\n  enum Inner { A }\n}\n");
        assert_eq!(i.kind, SymbolKind::Class);
    }

    #[test]
    fn cache_uses_mtime_and_counts_reads() {
        let dir = tempfile::tempdir().unwrap();
        let root = crate::util::paths::clean_canonicalize(dir.path()).unwrap();
        let f = root.join("A.java");
        std::fs::write(&f, "public class A {}").unwrap();
        let cache = Mutex::new(SymbolCache::new());
        let req = vec![f.to_string_lossy().into_owned()];
        let r1 = classify_paths(&cache, &root, &req);
        assert_eq!(r1[0].1.kind, SymbolKind::Class);
        let r2 = classify_paths(&cache, &root, &req);
        assert_eq!(r2, r1);
        assert_eq!(cache.lock().unwrap().reads, 1, "segunda petición = 0 lecturas");
        // Reescribe con otro tamaño: la caché se invalida.
        std::fs::write(&f, "public interface A { }  ").unwrap();
        let r3 = classify_paths(&cache, &root, &req);
        assert_eq!(r3[0].1.kind, SymbolKind::Interface);
        assert_eq!(cache.lock().unwrap().reads, 2);
    }

    #[test]
    fn outside_workspace_and_unsupported_are_omitted() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let root = crate::util::paths::clean_canonicalize(a.path()).unwrap();
        std::fs::write(b.path().join("X.java"), "class X {}").unwrap();
        std::fs::write(root.join("notes.txt"), "class X {}").unwrap();
        let cache = Mutex::new(SymbolCache::new());
        let req = vec![
            b.path().join("X.java").to_string_lossy().into_owned(),
            root.join("notes.txt").to_string_lossy().into_owned(),
            root.join("missing.java").to_string_lossy().into_owned(),
        ];
        assert!(classify_paths(&cache, &root, &req).is_empty());
    }
}
