use std::path::{Path, PathBuf};

use tauri::State;

use super::resolve::{self, SystemProbe};
use super::{store, templates, to_service_config, ResolvedRun, RunConfigFile, RunStarted, RunTarget, RunTemplate};
use crate::error::{AppError, AppResult};
use crate::services::config::bad_cwd;
use crate::services::{detect, ServiceConfig, ServiceKind, Tech};
use crate::state::AppState;
use crate::util::run_blocking;

fn kind_for_ext(path: &Path) -> (ServiceKind, Tech) {
    match path.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
        Some("py") => (ServiceKind::Python, Tech::Python),
        Some("ts" | "mts" | "cts" | "js" | "mjs" | "cjs") => (ServiceKind::Node, Tech::Node),
        Some("go") => (ServiceKind::Go, Tech::Go),
        Some("rs") => (ServiceKind::Rust, Tech::Rust),
        Some("java") => (ServiceKind::Java, Tech::Other),
        _ => (ServiceKind::Shell, Tech::Other),
    }
}

fn check_relative(cfg: &ServiceConfig) -> AppResult<()> {
    if cfg.command.trim().is_empty() {
        return Err(AppError::InvalidArgument("comando vacío".into()));
    }
    if bad_cwd(&cfg.cwd) || cfg.env_file.as_deref().is_some_and(bad_cwd) {
        return Err(AppError::InvalidArgument("cwd/envFile deben ser relativos al workspace".into()));
    }
    Ok(())
}

/// Convierte un objetivo de Run en el servicio efímero a lanzar y su etiqueta.
pub fn target_to_service(root: &Path, target: RunTarget) -> AppResult<(ServiceConfig, String)> {
    match target {
        RunTarget::File { path } => {
            let file = PathBuf::from(&path);
            let runners = store::load(root)?.file_runners;
            let r = resolve::resolve_file(root, &file, &runners, &SystemProbe)?;
            let abs = crate::util::paths::ensure_in_workspace(root, &file)?;
            let (kind, tech) = kind_for_ext(&abs);
            let cfg = ServiceConfig {
                id: resolve::runtime_id(root, &abs),
                name: abs.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                command: r.command,
                cwd: r.cwd,
                env: Default::default(),
                env_file: None,
                port: None,
                kind,
                tech: Some(tech),
                framework: None,
            };
            Ok((cfg, r.label))
        }
        RunTarget::Config { id } => {
            let file = store::load(root)?;
            let c = file
                .configs
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| AppError::NotFound(format!("configuración {id}")))?;
            let cfg = to_service_config(c);
            check_relative(&cfg)?;
            Ok((cfg, c.name.clone()))
        }
        RunTarget::Detected { mut service } => {
            let base = service.id.strip_prefix("auto:").unwrap_or(&service.id).to_string();
            if base.trim().is_empty() || base.contains(':') {
                return Err(AppError::InvalidArgument(format!("id inválido: {}", service.id)));
            }
            service.id = format!("auto:{base}");
            check_relative(&service)?;
            let label = service.name.clone();
            Ok((service, label))
        }
    }
}

/// `root`: raíz del workspace (None = primaria).
#[tauri::command]
pub async fn run_configs_load(state: State<'_, AppState>, root: Option<String>) -> AppResult<RunConfigFile> {
    let root = state.resolve_root(root.as_deref())?;
    run_blocking(move || store::load(&root)).await
}

#[tauri::command]
pub async fn run_configs_save(state: State<'_, AppState>, root: Option<String>, file: RunConfigFile) -> AppResult<()> {
    let root = state.resolve_root(root.as_deref())?;
    run_blocking(move || store::save(&root, &file)).await
}

#[tauri::command]
pub async fn run_templates() -> AppResult<Vec<RunTemplate>> {
    Ok(templates::templates())
}

#[tauri::command]
pub async fn run_detect(state: State<'_, AppState>, root: Option<String>) -> AppResult<Vec<ServiceConfig>> {
    let root = state.resolve_root(root.as_deref())?;
    run_blocking(move || {
        let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "root".into());
        Ok(detect::detect(&root, &name))
    })
    .await
}

/// El archivo se resuelve con la config (`file_runners`) y la raíz que lo contiene.
#[tauri::command]
pub async fn run_resolve_file(state: State<'_, AppState>, path: String) -> AppResult<ResolvedRun> {
    let root = owner_root(&state, &path)?;
    run_blocking(move || {
        let runners = store::load(&root)?.file_runners;
        resolve::resolve_file(&root, Path::new(&path), &runners, &SystemProbe)
    })
    .await
}

/// Raíz del workspace que contiene `path` (el archivo puede no existir aún).
fn owner_root(state: &AppState, path: &str) -> AppResult<PathBuf> {
    Ok(crate::util::paths::ensure_in_roots(&state.roots()?, Path::new(path))?.1)
}

/// `root`: raíz de la config/servicio detectado (None = primaria); un objetivo `File` usa la raíz del archivo
/// y su cwd por defecto es esa raíz.
#[tauri::command]
pub async fn run_start(state: State<'_, AppState>, target: RunTarget, root: Option<String>) -> AppResult<RunStarted> {
    let root = match &target {
        RunTarget::File { path } => owner_root(&state, path)?,
        _ => state.resolve_root(root.as_deref())?,
    };
    let m = state.services.clone();
    run_blocking(move || {
        let (cfg, label) = target_to_service(&root, target)?;
        let command = cfg.command.clone();
        let runtime_id = m.run_ephemeral_in(&root, cfg)?;
        Ok(RunStarted { runtime_id, label, command })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::{RunConfig, RunKind};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-runcmd-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::canonicalize(&d).unwrap()
    }

    #[test]
    fn targets_become_prefixed_services() {
        let d = tmp("t");
        std::fs::write(d.join("a b.js"), "").unwrap();
        let (c, label) = target_to_service(&d, RunTarget::File { path: d.join("a b.js").to_string_lossy().into_owned() }).unwrap();
        assert_eq!(c.id, "file:a b.js");
        assert_eq!(label, "Run: a b.js");
        assert!(c.command.starts_with("node "));

        let file = RunConfigFile {
            configs: vec![RunConfig {
                id: "api".into(),
                name: "API".into(),
                kind: RunKind::Node,
                cwd: String::new(),
                command: "npm".into(),
                args: "run dev".into(),
                env: Default::default(),
                env_file: None,
                port: Some(3000),
                show_in_services: true,
            }],
            ..Default::default()
        };
        store::save(&d, &file).unwrap();
        let (c, label) = target_to_service(&d, RunTarget::Config { id: "api".into() }).unwrap();
        assert_eq!((c.id.as_str(), c.command.as_str(), label.as_str()), ("run:api", "npm run dev", "API"));
        assert!(target_to_service(&d, RunTarget::Config { id: "nope".into() }).is_err());

        let svc = |id: &str, cwd: &str| ServiceConfig {
            id: id.into(),
            name: "n".into(),
            command: "x".into(),
            cwd: cwd.into(),
            env: Default::default(),
            env_file: None,
            port: None,
            kind: ServiceKind::Custom,
            tech: None,
            framework: None,
        };
        let (c, _) = target_to_service(&d, RunTarget::Detected { service: svc("web", "web") }).unwrap();
        assert_eq!(c.id, "auto:web");
        assert!(target_to_service(&d, RunTarget::Detected { service: svc("web", "../x") }).is_err());
        assert!(target_to_service(&d, RunTarget::Detected { service: svc("w:eb", "") }).is_err());
    }
}
