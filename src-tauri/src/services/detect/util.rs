//! Utilidades compartidas por los detectores.

use std::collections::HashSet;
use std::path::Path;

use crate::services::{ServiceKind, Tech};

/// Entrada detectada en un directorio (sin id ni cwd: los pone el recorrido).
#[derive(Debug, Clone)]
pub struct Found {
    pub name: Option<String>,
    pub command: String,
    pub kind: ServiceKind,
    pub tech: Tech,
    pub framework: Option<String>,
    pub port: Option<u16>,
}

pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "service".into()
    } else {
        out
    }
}

pub fn unique_id(base: &str, used: &mut HashSet<String>) -> String {
    let b = slug(base);
    let mut id = b.clone();
    let mut n = 2;
    while used.contains(&id) {
        id = format!("{b}-{n}");
        n += 1;
    }
    used.insert(id.clone());
    id
}

pub fn exists(dir: &Path, name: &str) -> bool {
    dir.join(name).exists()
}

pub fn read(dir: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(name)).ok()
}

/// `PORT=` en `.env`.
pub fn read_env_port(dir: &Path) -> Option<u16> {
    let env = read(dir, ".env")?;
    for l in env.lines() {
        if let Some(v) = l.trim().strip_prefix("PORT=") {
            if let Ok(p) = v.trim().trim_matches(|c| c == '"' || c == '\'').parse() {
                return Some(p);
            }
        }
    }
    None
}

/// Valor de una clave en `application.properties` (`k=v` o `k: v`).
pub fn properties_lookup(text: &str, key: &str) -> Option<String> {
    for l in text.lines() {
        let l = l.trim();
        if l.starts_with('#') || l.starts_with('!') {
            continue;
        }
        let Some(rest) = l.strip_prefix(key) else { continue };
        let rest = rest.trim_start();
        let Some(v) = rest.strip_prefix('=').or_else(|| rest.strip_prefix(':')) else { continue };
        return clean_value(v);
    }
    None
}

fn clean_value(v: &str) -> Option<String> {
    let mut v = v.trim();
    if let Some(i) = v.find(" #") {
        v = v[..i].trim();
    }
    let v = v.trim_matches(|c| c == '"' || c == '\'').trim();
    if v.is_empty() || v.contains("${") {
        None
    } else {
        Some(v.to_string())
    }
}

/// Parser YAML mínimo por indentación: solo claves escalares anidadas (`a.b.c`), solo el
/// primer documento (se ignoran perfiles tras `---`) y valores `${...}` -> `None`.
pub fn yaml_lookup(text: &str, path: &[&str]) -> Option<String> {
    let mut stack: Vec<(usize, Vec<String>)> = Vec::new();
    let mut seen_content = false;
    for raw in text.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with("---") {
            if seen_content {
                break;
            }
            continue;
        }
        seen_content = true;
        let indent = line.len() - trimmed.len();
        let Some(colon) = trimmed.find(':') else { continue };
        let key = trimmed[..colon].trim().trim_matches(|c| c == '"' || c == '\'');
        let value = trimmed[colon + 1..].trim();
        while stack.last().is_some_and(|(i, _)| *i >= indent) {
            stack.pop();
        }
        let mut segs: Vec<String> = stack.iter().flat_map(|(_, s)| s.clone()).collect();
        let own: Vec<String> = key.split('.').map(|s| s.to_string()).collect();
        segs.extend(own.iter().cloned());
        stack.push((indent, own));
        if segs.len() == path.len() && segs.iter().zip(path).all(|(a, b)| a == b) {
            if let Some(v) = clean_value(value) {
                return Some(v);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_nested_and_profiles() {
        let y = "spring:\n  application:\n    name: orders\n  other: 1\nserver:\n  port: 8081\n---\nserver:\n  port: 9999\n";
        assert_eq!(yaml_lookup(y, &["spring", "application", "name"]).as_deref(), Some("orders"));
        assert_eq!(yaml_lookup(y, &["server", "port"]).as_deref(), Some("8081"));
        assert_eq!(yaml_lookup("server:\n  port: ${PORT:8080}\n", &["server", "port"]), None);
        assert_eq!(yaml_lookup("server.port: 7000\n", &["server", "port"]).as_deref(), Some("7000"));
    }

    #[test]
    fn properties() {
        let p = "# c\nserver.port=9090\nspring.application.name = api\n";
        assert_eq!(properties_lookup(p, "server.port").as_deref(), Some("9090"));
        assert_eq!(properties_lookup(p, "spring.application.name").as_deref(), Some("api"));
    }
}
