//! Detección de herramientas (`which` + `--version` con timeout) y JDKs (sin lanzar procesos:
//! se lee el archivo `release` de cada JDK) + comprobación de red (HEAD a start.spring.io).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use regex::Regex;

use super::{JdkInfo, ToolInfo, ToolsReport};
use crate::util::proc::no_window;

/// `(id, binario(s), argumentos de versión)`; `[]` = no se consulta versión (lento: `ng`).
const TOOLS: &[(&str, &[&str], &[&str])] = &[
    ("java", &["java"], &["-version"]),
    ("javac", &["javac"], &["-version"]),
    ("mvn", &["mvn"], &["-v"]),
    ("gradle", &["gradle"], &["-v"]),
    ("node", &["node"], &["--version"]),
    ("npm", &["npm"], &["--version"]),
    ("npx", &["npx"], &["--version"]),
    ("ng", &["ng"], &[]),
    ("cargo", &["cargo"], &["--version"]),
    ("go", &["go"], &["version"]),
    ("python3", &["python3", "python"], &["--version"]),
    ("git", &["git"], &["--version"]),
];

const VERSION_TIMEOUT: Duration = Duration::from_secs(3);

/// Ejecuta `program args` con timeout; devuelve stdout+stderr si sale con éxito.
fn run_with_timeout(program: &Path, args: &[&str], timeout: Duration) -> Option<String> {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    no_window(&mut cmd);
    let mut child = cmd.spawn().ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let out = child.wait_with_output().ok()?;
                if !status.success() {
                    return None;
                }
                let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
                text.push('\n');
                text.push_str(&String::from_utf8_lossy(&out.stderr));
                return Some(text);
            }
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(15)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Primer número tipo `21.0.4` / `3.9.9` / `v20.11.1` del texto.
pub fn parse_version(text: &str) -> Option<String> {
    let re = Regex::new(r"\d+(?:\.\d+)+(?:[_+-][A-Za-z0-9]+)?").ok()?;
    re.find(text).map(|m| m.as_str().to_string())
}

/// Detecta todas las herramientas en paralelo. `search_path`: PATH alternativo (tests); `None` = el del proceso.
pub fn detect_tools(search_path: Option<OsString>) -> Vec<ToolInfo> {
    let handles: Vec<_> = TOOLS
        .iter()
        .map(|(id, bins, vargs)| {
            let sp = search_path.clone();
            std::thread::spawn(move || {
                let found = bins.iter().find_map(|b| match &sp {
                    Some(p) => which::which_in(b, Some(p), std::env::current_dir().unwrap_or_default()).ok(),
                    None => which::which(b).ok(),
                });
                let (mut path, mut version) = (found, None);
                if let Some(p) = &path {
                    if !vargs.is_empty() {
                        version = run_with_timeout(p, vargs, VERSION_TIMEOUT).and_then(|t| parse_version(&t));
                        // En macOS `/usr/bin/java` es un stub que falla si no hay JDK: no cuenta como instalado.
                        if version.is_none() && matches!(*id, "java" | "javac") {
                            path = None;
                        }
                    }
                }
                ToolInfo { id: (*id).to_string(), path: path.map(|p| p.to_string_lossy().into_owned()), version }
            })
        })
        .collect();
    handles
        .into_iter()
        .zip(TOOLS)
        .map(|(h, (id, _, _))| h.join().unwrap_or(ToolInfo { id: (*id).to_string(), path: None, version: None }))
        .collect()
}

/// Lee `release` (`JAVA_VERSION="21.0.4"`, `IMPLEMENTOR="..."`) → `(version, major, vendor)`.
pub fn parse_release(text: &str) -> Option<(String, u32, Option<String>)> {
    let get = |k: &str| {
        text.lines().find_map(|l| {
            let (key, val) = l.split_once('=')?;
            (key.trim() == k).then(|| val.trim().trim_matches('"').to_string())
        })
    };
    let version = get("JAVA_VERSION")?;
    let major = if let Some(rest) = version.strip_prefix("1.") {
        rest.split(['.', '_']).next()?.parse().ok()?
    } else {
        version.split(['.', '_', '+', '-']).next()?.parse().ok()?
    };
    Some((version, major, get("IMPLEMENTOR")))
}

/// Un directorio es JDK si tiene `bin/javac` y un `release` legible.
pub fn jdk_from_home(home: &Path) -> Option<JdkInfo> {
    let javac = if cfg!(windows) { "javac.exe" } else { "javac" };
    if !home.join("bin").join(javac).is_file() {
        return None;
    }
    let text = std::fs::read_to_string(home.join("release")).ok()?;
    let (version, major, vendor) = parse_release(&text)?;
    let home = crate::util::paths::clean_canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
    Some(JdkInfo { home: home.to_string_lossy().into_owned(), version, major, vendor })
}

fn children(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default()
}

/// Posibles `JAVA_HOME` bajo los directorios contenedores habituales.
fn candidate_homes(extra_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(h) = std::env::var_os("JAVA_HOME") {
        out.push(PathBuf::from(h));
    }
    if let Ok(p) = which::which("javac") {
        if let Ok(real) = p.canonicalize() {
            if let Some(home) = real.parent().and_then(|b| b.parent()) {
                out.push(home.to_path_buf());
            }
        }
    }
    let home_dir = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let mut containers: Vec<PathBuf> = extra_roots.to_vec();
    #[cfg(target_os = "macos")]
    {
        containers.push("/Library/Java/JavaVirtualMachines".into());
        if let Some(h) = &home_dir {
            containers.push(h.join("Library/Java/JavaVirtualMachines"));
        }
        for brew in ["/opt/homebrew/opt", "/usr/local/opt"] {
            for p in children(Path::new(brew)) {
                if p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("openjdk")) {
                    out.push(p.join("libexec/openjdk.jdk/Contents/Home"));
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        containers.extend(["/usr/lib/jvm".into(), "/usr/java".into(), "/opt/java".into()]);
    }
    #[cfg(windows)]
    {
        for pf in ["C:\\Program Files\\Java", "C:\\Program Files\\Eclipse Adoptium", "C:\\Program Files\\Microsoft", "C:\\Program Files\\Zulu", "C:\\Program Files\\Amazon Corretto"] {
            containers.push(pf.into());
        }
    }
    if let Some(h) = &home_dir {
        containers.push(h.join(".sdkman/candidates/java"));
        containers.push(h.join(".jdks"));
        containers.push(h.join(".asdf/installs/java"));
    }
    for c in containers {
        for jdk in children(&c) {
            out.push(jdk.clone());
            out.push(jdk.join("Contents/Home")); // macOS
        }
    }
    out
}

/// JDKs instalados, sin duplicados (por ruta real), de mayor a menor versión.
pub fn detect_jdks(extra_roots: &[PathBuf]) -> Vec<JdkInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut jdks: Vec<JdkInfo> = candidate_homes(extra_roots)
        .iter()
        .filter_map(|h| jdk_from_home(h))
        .filter(|j| seen.insert(j.home.clone()))
        .collect();
    jdks.sort_by(|a, b| b.major.cmp(&a.major).then_with(|| a.home.cmp(&b.home)));
    jdks
}

/// HEAD a start.spring.io con timeout de 3 s.
pub async fn check_online() -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .connect_timeout(Duration::from_secs(3))
        .build()
    else {
        return false;
    };
    client.head("https://start.spring.io/").send().await.map(|r| !r.status().is_server_error()).unwrap_or(false)
}

pub async fn detect(check: bool) -> ToolsReport {
    let tools = tokio::task::spawn_blocking(|| detect_tools(None)).await.unwrap_or_default();
    let jdks = tokio::task::spawn_blocking(|| detect_jdks(&[])).await.unwrap_or_default();
    let online = if check { Some(check_online().await) } else { None };
    ToolsReport { tools, jdks, online }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        assert_eq!(parse_version("openjdk version \"21.0.4\" 2024-07-16").as_deref(), Some("21.0.4"));
        assert_eq!(parse_version("v20.11.1").as_deref(), Some("20.11.1"));
        assert_eq!(parse_version("Apache Maven 3.9.9 (abc)").as_deref(), Some("3.9.9"));
        assert_eq!(parse_version("nada"), None);
    }

    #[test]
    fn parses_release_files() {
        let (v, m, vendor) = parse_release("JAVA_VERSION=\"21.0.4\"\nIMPLEMENTOR=\"Eclipse Adoptium\"\n").unwrap();
        assert_eq!((v.as_str(), m, vendor.as_deref()), ("21.0.4", 21, Some("Eclipse Adoptium")));
        assert_eq!(parse_release("JAVA_VERSION=\"1.8.0_292\"").unwrap().1, 8);
        assert_eq!(parse_release("JAVA_VERSION=\"17\"").unwrap().1, 17);
        assert!(parse_release("OTRA=1").is_none());
    }

    #[test]
    fn finds_fake_jdk_in_root() {
        let d = tempfile::tempdir().unwrap();
        let home = d.path().join("jdk-21");
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::write(home.join("bin").join(if cfg!(windows) { "javac.exe" } else { "javac" }), "").unwrap();
        std::fs::write(home.join("release"), "JAVA_VERSION=\"21.0.2\"\n").unwrap();
        std::fs::create_dir_all(d.path().join("sin-javac")).unwrap();
        let found = detect_jdks(&[d.path().to_path_buf()]);
        assert!(found.iter().any(|j| j.major == 21 && j.version == "21.0.2"));
    }

    #[test]
    fn detect_without_tools_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        let tools = detect_tools(Some(d.path().as_os_str().to_owned()));
        assert_eq!(tools.len(), TOOLS.len());
        assert!(tools.iter().all(|t| t.path.is_none() && t.version.is_none()));
    }
}
