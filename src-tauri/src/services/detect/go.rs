//! Go: `go run .` y `cmd/<svc>`.

use std::path::Path;

use super::util::{exists, read_env_port, Found};
use crate::services::{ServiceKind, Tech};

pub fn detect(dir: &Path) -> Vec<Found> {
    if !exists(dir, "go.mod") {
        return Vec::new();
    }
    let port = read_env_port(dir);
    let mk = |name: Option<String>, command: String| Found {
        name,
        command,
        kind: ServiceKind::Go,
        tech: Tech::Go,
        framework: None,
        port,
    };
    let mut out = Vec::new();
    if exists(dir, "main.go") {
        out.push(mk(None, "go run .".into()));
    }
    if let Ok(rd) = std::fs::read_dir(dir.join("cmd")) {
        let mut names: Vec<String> = rd
            .flatten()
            .filter(|e| e.path().join("main.go").is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        for n in names {
            out.push(mk(Some(n.clone()), format!("go run ./cmd/{n}")));
        }
    }
    out
}
