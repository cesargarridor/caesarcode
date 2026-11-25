//! Node / TypeScript.

use std::path::Path;

use super::util::{exists, read, read_env_port, Found};
use crate::services::{ServiceKind, Tech};

const LOCKS: &[(&str, &str)] = &[
    ("bun.lockb", "bun"),
    ("bun.lock", "bun"),
    ("pnpm-lock.yaml", "pnpm"),
    ("yarn.lock", "yarn"),
    ("package-lock.json", "npm"),
];

/// Gestor por lockfile en `dir` o, si no hay, en el ancestro más cercano hasta `root`.
fn package_manager(dir: &Path, root: &Path) -> &'static str {
    let mut cur = Some(dir);
    while let Some(d) = cur {
        for (lock, pm) in LOCKS {
            if exists(d, lock) {
                return pm;
            }
        }
        if d == root {
            break;
        }
        cur = d.parent();
    }
    "npm"
}

fn script_port(script: &str) -> Option<u16> {
    let toks: Vec<&str> = script.split_whitespace().collect();
    for w in toks.windows(2) {
        if w[0] == "--port" || w[0] == "-p" {
            if let Ok(p) = w[1].parse() {
                return Some(p);
            }
        }
    }
    toks.iter().find_map(|t| t.strip_prefix("--port=").and_then(|v| v.parse().ok()))
}

pub fn detect(dir: &Path, root: &Path) -> Vec<Found> {
    let Some(pkg) = read(dir, "package.json") else { return Vec::new() };
    let json: serde_json::Value = serde_json::from_str(&pkg).unwrap_or(serde_json::Value::Null);
    let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) else { return Vec::new() };
    let Some(script) = ["dev", "start:dev", "start", "serve"].into_iter().find(|k| scripts.contains_key(*k)) else {
        return Vec::new();
    };
    let pm = package_manager(dir, root);
    let command = if script == "start" && pm == "npm" {
        "npm start".to_string()
    } else {
        format!("{pm} run {script}")
    };
    let has_dep = |n: &str| {
        ["dependencies", "devDependencies"]
            .iter()
            .any(|k| json.get(*k).and_then(|d| d.get(n)).is_some())
    };
    let (framework, default_port) = if has_dep("@nestjs/core") {
        (Some("nestjs"), Some(3000))
    } else if has_dep("next") {
        (Some("next"), Some(3000))
    } else if has_dep("vite") {
        (Some("vite"), Some(5173))
    } else if has_dep("fastify") {
        (Some("fastify"), Some(3000))
    } else if has_dep("express") {
        (Some("express"), Some(3000))
    } else {
        (None, None)
    };
    let vite_cfg = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().starts_with("vite.config.")))
        .unwrap_or(false);
    let port = read_env_port(dir)
        .or_else(|| scripts.get(script).and_then(|s| s.as_str()).and_then(script_port))
        .or(default_port)
        .or(if vite_cfg { Some(5173) } else { None });
    vec![Found {
        name: None,
        command,
        kind: ServiceKind::Node,
        tech: Tech::Node,
        framework: framework.map(String::from),
        port,
    }]
}
