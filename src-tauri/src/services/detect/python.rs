//! Python: Django / FastAPI / Flask / script.

use std::path::Path;

use super::util::{exists, read, read_env_port, Found};
use crate::services::{ServiceKind, Tech};

fn interpreter(dir: &Path) -> String {
    let pyproject = read(dir, "pyproject.toml").unwrap_or_default();
    if exists(dir, "poetry.lock") && pyproject.contains("[tool.poetry]") {
        return "poetry run python".into();
    }
    if exists(dir, "uv.lock") {
        return "uv run python".into();
    }
    let candidates: &[&str] = if cfg!(windows) {
        &[".venv\\Scripts\\python.exe", "venv\\Scripts\\python.exe"]
    } else {
        &[".venv/bin/python", "venv/bin/python"]
    };
    for c in candidates {
        if exists(dir, c) {
            return (*c).to_string();
        }
    }
    if cfg!(windows) { "python".into() } else { "python3".into() }
}

pub fn detect(dir: &Path) -> Vec<Found> {
    let gate = ["pyproject.toml", "requirements.txt", "uv.lock", "poetry.lock", "manage.py"];
    if !gate.iter().any(|f| exists(dir, f)) {
        return Vec::new();
    }
    let py = interpreter(dir);
    let mk = |command: String, framework: Option<&str>, default_port: Option<u16>| Found {
        name: None,
        command,
        kind: ServiceKind::Python,
        tech: Tech::Python,
        framework: framework.map(String::from),
        port: read_env_port(dir).or(default_port),
    };
    if exists(dir, "manage.py") {
        return vec![mk(format!("{py} manage.py runserver"), Some("django"), Some(8000))];
    }
    for entry in ["main.py", "app.py", "src/main.py", "src/app.py"] {
        let Some(src) = read(dir, entry) else { continue };
        let stem = Path::new(entry).file_stem().and_then(|s| s.to_str()).unwrap_or("main");
        let in_src = entry.starts_with("src/");
        if src.contains("FastAPI(") {
            let app_dir = if in_src { " --app-dir src" } else { "" };
            return vec![mk(
                format!("{py} -m uvicorn {stem}:app --reload{app_dir}"),
                Some("fastapi"),
                Some(8000),
            )];
        }
        if src.contains("Flask(") {
            let app = if in_src { format!("src.{stem}") } else { stem.to_string() };
            return vec![mk(format!("{py} -m flask --app {app} run"), Some("flask"), Some(5000))];
        }
    }
    for entry in ["main.py", "app.py"] {
        if exists(dir, entry) {
            return vec![mk(format!("{py} {entry}"), None, None)];
        }
    }
    Vec::new()
}
