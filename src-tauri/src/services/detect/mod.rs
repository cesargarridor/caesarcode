//! Detector políglota de servicios ejecutables (Spring, Python, Node/TS, Go, Rust, compose).

mod compose;
mod go;
mod node;
mod python;
mod rust;
mod spring;
mod util;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::ServiceConfig;
use util::{unique_id, Found};

const SKIP: &[&str] = &[
    "node_modules", "target", ".git", ".venv", "venv", "dist", "build", "out", "__pycache__", ".next", ".idea",
    "coverage",
];
const MAX_DEPTH: usize = 3;

struct Ctx<'a> {
    root: &'a Path,
    root_name: &'a str,
    used: HashSet<String>,
    claimed: HashSet<PathBuf>,
    out: Vec<ServiceConfig>,
}

fn rel_cwd(root: &Path, dir: &Path) -> String {
    dir.strip_prefix(root)
        .unwrap_or(Path::new(""))
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn emit(ctx: &mut Ctx, dir: &Path, f: Found, suffix: Option<&str>) {
    let cwd = rel_cwd(ctx.root, dir);
    let mut name = f.name.unwrap_or_else(|| {
        if cwd.is_empty() {
            ctx.root_name.to_string()
        } else {
            dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| cwd.clone())
        }
    });
    if let Some(s) = suffix {
        name = format!("{name} ({s})");
    }
    ctx.out.push(ServiceConfig {
        id: unique_id(&name, &mut ctx.used),
        name,
        command: f.command,
        cwd,
        env: HashMap::new(),
        env_file: None,
        port: f.port,
        kind: f.kind,
        tech: Some(f.tech),
        framework: f.framework,
    });
}

fn walk(ctx: &mut Ctx, dir: &Path, depth: usize) {
    if ctx.claimed.contains(dir) {
        return;
    }
    let mut found: Vec<(Found, Option<&'static str>)> = Vec::new();
    found.extend(spring::detect(dir).into_iter().map(|f| (f, None)));
    found.extend(python::detect(dir).into_iter().map(|f| (f, None)));
    found.extend(node::detect(dir, ctx.root).into_iter().map(|f| (f, None)));
    found.extend(go::detect(dir).into_iter().map(|f| (f, None)));
    found.extend(rust::detect(dir).into_iter().map(|f| (f, None)));
    found.extend(compose::detect(dir).into_iter().map(|f| (f, Some("docker"))));
    ctx.claimed.extend(rust::claimed(dir));
    for (f, suffix) in found {
        emit(ctx, dir, f, suffix);
    }
    if depth >= MAX_DEPTH {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut subs: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            !SKIP.contains(&n.as_str()) && !n.starts_with('.')
        })
        .map(|e| e.path())
        .collect();
    subs.sort();
    for s in subs {
        walk(ctx, &s, depth + 1);
    }
}

/// Sugerencias de servicios (no guarda nada). Usado por `services_detect` y `run_detect`.
pub fn detect(root: &Path, workspace_name: &str) -> Vec<ServiceConfig> {
    let mut ctx = Ctx { root, root_name: workspace_name, used: HashSet::new(), claimed: HashSet::new(), out: Vec::new() };
    walk(&mut ctx, root, 0);
    ctx.out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{ServiceKind, Tech};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-detect-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(d: &Path, rel: &str, content: &str) {
        let p = d.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    fn by_cwd(found: &[ServiceConfig], cwd: &str) -> ServiceConfig {
        found.iter().find(|s| s.cwd == cwd).unwrap_or_else(|| panic!("sin {cwd}: {found:?}")).clone()
    }

    #[test]
    fn spring_maven_wrapper_yaml() {
        let d = tmp("spring-mvn");
        put(&d, "pom.xml", "<artifactId>spring-boot-starter</artifactId>");
        put(&d, "mvnw", "");
        put(&d, "mvnw.cmd", "");
        put(&d, "src/main/resources/application.yml", "spring:\n  application:\n    name: orders\nserver:\n  port: 8081\n");
        let f = detect(&d, "proj");
        assert_eq!(f.len(), 1, "{f:?}");
        let s = &f[0];
        assert_eq!(s.command, if cfg!(windows) { "mvnw.cmd spring-boot:run" } else { "./mvnw spring-boot:run" });
        assert_eq!((s.name.as_str(), s.port, s.tech), ("orders", Some(8081), Some(Tech::Spring)));
        assert_eq!(s.framework.as_deref(), Some("spring-boot"));
        assert_eq!(s.kind, ServiceKind::Maven);
        // sin puerto configurado: 8080 por defecto
        let d2 = tmp("spring-mvn2");
        put(&d2, "pom.xml", "spring-boot");
        assert_eq!(detect(&d2, "x")[0].port, Some(8080));
        assert_eq!(detect(&d2, "x")[0].command, "mvn spring-boot:run");
    }

    #[test]
    fn spring_gradle_kts_properties() {
        let d = tmp("spring-gr");
        put(&d, "build.gradle.kts", "plugins { id(\"org.springframework.boot\") version \"3\" }");
        put(&d, "gradlew", "");
        put(&d, "gradlew.bat", "");
        put(&d, "src/main/resources/application.properties", "server.port=9090\nspring.application.name=billing\n");
        let f = detect(&d, "proj");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].command, if cfg!(windows) { "gradlew.bat bootRun" } else { "./gradlew bootRun" });
        assert_eq!((f[0].name.as_str(), f[0].port), ("billing", Some(9090)));
        assert_eq!(f[0].kind, ServiceKind::Gradle);
    }

    #[test]
    fn maven_multi_module_skips_aggregator() {
        let d = tmp("multi");
        put(&d, "pom.xml", "<project><packaging>pom</packaging><modules/></project>");
        put(&d, "a/pom.xml", "spring-boot");
        put(&d, "b/pom.xml", "<artifactId>plain</artifactId>");
        let f = detect(&d, "proj");
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(f.iter().all(|s| !s.cwd.is_empty()));
        assert_eq!(by_cwd(&f, "a").tech, Some(Tech::Spring));
        assert_eq!(by_cwd(&f, "b").tech, Some(Tech::Other));
        assert_eq!(by_cwd(&f, "b").command, "mvn compile exec:java");
    }

    #[test]
    fn python_django_fastapi_flask() {
        let d = tmp("py");
        put(&d, "dj/manage.py", "");
        put(&d, "dj/requirements.txt", "django");
        put(&d, "dj/.venv/bin/python", "");
        put(&d, "dj/venv/Scripts/python.exe", "");
        put(&d, "fa/main.py", "app = FastAPI()");
        put(&d, "fa/pyproject.toml", "[project]");
        put(&d, "fa/uv.lock", "");
        put(&d, "fl/app.py", "app = Flask(__name__)");
        put(&d, "fl/pyproject.toml", "[tool.poetry]\nname='x'");
        put(&d, "fl/poetry.lock", "");
        put(&d, "sc/main.py", "print(1)");
        put(&d, "sc/requirements.txt", "");
        let f = detect(&d, "proj");
        let dj = by_cwd(&f, "dj");
        if !cfg!(windows) {
            assert_eq!(dj.command, ".venv/bin/python manage.py runserver");
        }
        assert_eq!((dj.port, dj.framework.as_deref()), (Some(8000), Some("django")));
        let fa = by_cwd(&f, "fa");
        assert_eq!(fa.command, "uv run python -m uvicorn main:app --reload");
        assert_eq!(fa.framework.as_deref(), Some("fastapi"));
        let fl = by_cwd(&f, "fl");
        assert_eq!(fl.command, "poetry run python -m flask --app app run");
        assert_eq!(fl.port, Some(5000));
        let sc = by_cwd(&f, "sc");
        assert_eq!(sc.command, format!("{} main.py", if cfg!(windows) { "python" } else { "python3" }));
        assert_eq!(sc.tech, Some(Tech::Python));
    }

    #[test]
    fn node_managers_and_frameworks() {
        let d = tmp("node");
        put(&d, "nest/package.json", r#"{"scripts":{"start:dev":"nest start --watch"},"dependencies":{"@nestjs/core":"1"}}"#);
        put(&d, "nest/pnpm-lock.yaml", "");
        put(&d, "bunapp/package.json", r#"{"scripts":{"dev":"vite --port 4000"},"devDependencies":{"vite":"5"}}"#);
        put(&d, "bunapp/bun.lockb", "");
        put(&d, "bunapp/package-lock.json", "");
        put(&d, "mono/yarn.lock", "");
        put(&d, "mono/pkgs/web/package.json", r#"{"scripts":{"start":"node x.js"},"dependencies":{"express":"4"}}"#);
        put(&d, "mono/pkgs/web/.env", "PORT=4321\n");
        put(&d, "mono/package.json", r#"{"workspaces":["pkgs/*"]}"#);
        put(&d, "npmapp/package.json", r#"{"scripts":{"start":"node ."}}"#);
        let f = detect(&d, "proj");
        let n = by_cwd(&f, "nest");
        assert_eq!((n.command.as_str(), n.framework.as_deref(), n.port), ("pnpm run start:dev", Some("nestjs"), Some(3000)));
        let b = by_cwd(&f, "bunapp");
        assert_eq!((b.command.as_str(), b.port), ("bun run dev", Some(4000)));
        let w = by_cwd(&f, "mono/pkgs/web");
        assert_eq!((w.command.as_str(), w.port, w.framework.as_deref()), ("yarn run start", Some(4321), Some("express")));
        assert!(f.iter().all(|s| s.cwd != "mono"), "workspace raíz sin script no se emite");
        assert_eq!(by_cwd(&f, "npmapp").command, "npm start");
    }

    #[test]
    fn go_cmd_entries() {
        let d = tmp("go");
        put(&d, "go.mod", "module x");
        put(&d, "cmd/api/main.go", "package main");
        put(&d, "cmd/worker/main.go", "package main");
        put(&d, "cmd/empty/readme.md", "");
        let f = detect(&d, "proj");
        let cmds: Vec<_> = f.iter().map(|s| s.command.as_str()).collect();
        assert_eq!(cmds, vec!["go run ./cmd/api", "go run ./cmd/worker"], "{f:?}");
        assert_eq!(f[0].name, "api");
        assert_eq!(f[0].kind, ServiceKind::Go);
        put(&d, "main.go", "package main");
        assert_eq!(detect(&d, "proj")[0].command, "go run .");
    }

    #[test]
    fn rust_workspace_no_duplicates() {
        let d = tmp("rs");
        put(&d, "Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n");
        put(&d, "crates/server/Cargo.toml", "[package]\nname = \"my-server\"\n");
        put(&d, "crates/server/src/main.rs", "fn main(){}");
        put(&d, "crates/core/Cargo.toml", "[package]\nname = \"core\"\n");
        put(&d, "crates/core/src/lib.rs", "");
        let f = detect(&d, "proj");
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].command, "cargo run -p my-server");
        assert_eq!(f[0].cwd, "");
        assert_eq!(f[0].name, "my-server");
        // paquete simple
        let d2 = tmp("rs2");
        put(&d2, "Cargo.toml", "[package]\nname=\"x\"");
        put(&d2, "src/main.rs", "fn main(){}");
        assert_eq!(detect(&d2, "proj")[0].command, "cargo run");
        assert_eq!(detect(&d2, "proj")[0].name, "proj");
    }

    #[test]
    fn depth_and_skips() {
        let d = tmp("depth");
        put(&d, "a/b/c/package.json", r#"{"scripts":{"dev":"x"}}"#);
        put(&d, "a/b/c/d/package.json", r#"{"scripts":{"dev":"x"}}"#);
        put(&d, "node_modules/x/package.json", r#"{"scripts":{"start":"x"}}"#);
        put(&d, ".venv/lib/package.json", r#"{"scripts":{"start":"x"}}"#);
        put(&d, "target/p/package.json", r#"{"scripts":{"start":"x"}}"#);
        let f = detect(&d, "proj");
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].cwd, "a/b/c");
    }

    #[test]
    fn compose_does_not_hide_others() {
        let d = tmp("compose");
        put(&d, "compose.yaml", "services: {}");
        put(&d, "package.json", r#"{"scripts":{"dev":"vite"}}"#);
        let f = detect(&d, "proj");
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(f.iter().any(|s| s.command == "docker compose up" && s.tech == Some(Tech::Docker)));
        assert!(f.iter().any(|s| s.command == "npm run dev"));
        let ids: HashSet<_> = f.iter().map(|s| s.id.clone()).collect();
        assert_eq!(ids.len(), 2);
    }
}
