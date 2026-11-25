//! Rust: paquete simple o workspace.

use std::path::{Path, PathBuf};

use super::util::{exists, read, Found};
use crate::services::{ServiceKind, Tech};

fn parse(dir: &Path) -> Option<toml::Table> {
    read(dir, "Cargo.toml")?.parse::<toml::Table>().ok()
}

fn has_bin(dir: &Path, t: &toml::Table) -> bool {
    t.contains_key("package") && (exists(dir, "src/main.rs") || t.get("bin").is_some_and(|b| b.is_array()))
}

fn member_dirs(dir: &Path, t: &toml::Table) -> Vec<PathBuf> {
    let Some(members) = t.get("workspace").and_then(|w| w.get("members")).and_then(|m| m.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for m in members.iter().filter_map(|m| m.as_str()) {
        if let Some(base) = m.strip_suffix("/*") {
            if let Ok(rd) = std::fs::read_dir(dir.join(base)) {
                let mut subs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.join("Cargo.toml").is_file()).collect();
                subs.sort();
                out.extend(subs);
            }
        } else {
            let p = dir.join(m);
            if p.join("Cargo.toml").is_file() {
                out.push(p);
            }
        }
    }
    out
}

/// Directorios de miembros de un workspace (el recorrido no baja a ellos).
pub fn claimed(dir: &Path) -> Vec<PathBuf> {
    parse(dir).map(|t| member_dirs(dir, &t)).unwrap_or_default()
}

pub fn detect(dir: &Path) -> Vec<Found> {
    let Some(t) = parse(dir) else { return Vec::new() };
    let mk = |name: Option<String>, command: String| Found {
        name,
        command,
        kind: ServiceKind::Rust,
        tech: Tech::Rust,
        framework: None,
        port: None,
    };
    let mut out = Vec::new();
    if has_bin(dir, &t) {
        out.push(mk(None, "cargo run".into()));
    }
    for m in member_dirs(dir, &t) {
        let Some(mt) = parse(&m) else { continue };
        if !has_bin(&m, &mt) {
            continue;
        }
        if let Some(name) = mt.get("package").and_then(|p| p.get("name")).and_then(|n| n.as_str()) {
            out.push(mk(Some(name.to_string()), format!("cargo run -p {name}")));
        }
    }
    out
}
