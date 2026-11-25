//! Persistencia de `.caesar/run-configs.json` (atómica: tmp + rename).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::RunConfigFile;
use crate::error::{AppError, AppResult};
use crate::services::config::bad_cwd;

pub fn path(root: &Path) -> PathBuf {
    root.join(".caesar").join("run-configs.json")
}

/// Si el archivo no existe: valor por defecto vacío.
pub fn load(root: &Path) -> AppResult<RunConfigFile> {
    let p = path(root);
    if !p.is_file() {
        return Ok(RunConfigFile::default());
    }
    let text = std::fs::read_to_string(&p)?;
    serde_json::from_str(&text).map_err(|e| AppError::Service(format!("{}: {e}", p.display())))
}

/// Ids únicos sin ':', nombre/comando no vacíos, cwd/envFile relativos sin `..`.
pub fn validate(file: &RunConfigFile) -> AppResult<()> {
    let mut seen = HashSet::new();
    for c in &file.configs {
        if c.id.trim().is_empty() {
            return Err(AppError::InvalidArgument("configuración sin id".into()));
        }
        if c.id.contains(':') {
            return Err(AppError::InvalidArgument(format!("el id '{}' no puede contener ':'", c.id)));
        }
        if !seen.insert(c.id.as_str()) {
            return Err(AppError::InvalidArgument(format!("id duplicado: {}", c.id)));
        }
        if c.name.trim().is_empty() {
            return Err(AppError::InvalidArgument(format!("la configuración {} no tiene nombre", c.id)));
        }
        if c.command.trim().is_empty() {
            return Err(AppError::InvalidArgument(format!("la configuración '{}' no tiene comando", c.name)));
        }
        if bad_cwd(&c.cwd) {
            return Err(AppError::InvalidArgument(format!("cwd de '{}' debe ser relativo al workspace", c.name)));
        }
        if let Some(f) = &c.env_file {
            if bad_cwd(f) {
                return Err(AppError::InvalidArgument(format!("envFile de '{}' debe ser relativo", c.name)));
            }
        }
    }
    Ok(())
}

pub fn save(root: &Path, file: &RunConfigFile) -> AppResult<()> {
    validate(file)?;
    let p = path(root);
    let dir = p.parent().ok_or_else(|| AppError::Other("ruta inválida".into()))?;
    std::fs::create_dir_all(dir)?;
    let mut text = serde_json::to_string_pretty(file)?;
    text.push('\n');
    let tmp = dir.join(format!("run-configs.json.cc-tmp-{}", std::process::id()));
    std::fs::write(&tmp, text)?;
    if let Err(e) = std::fs::rename(&tmp, &p) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::{RunConfig, RunKind};
    use std::collections::HashMap;

    fn cfg(id: &str) -> RunConfig {
        RunConfig {
            id: id.into(),
            name: id.into(),
            kind: RunKind::Node,
            cwd: String::new(),
            command: "npm run dev".into(),
            args: String::new(),
            env: HashMap::new(),
            env_file: None,
            port: Some(3000),
            show_in_services: true,
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-runstore-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn missing_file_is_default() {
        let d = tmp("missing");
        let f = load(&d).unwrap();
        assert!(f.configs.is_empty() && f.file_runners.is_empty());
        assert_eq!(f.version, 1);
    }

    #[test]
    fn roundtrip() {
        let d = tmp("rt");
        let mut f = RunConfigFile { configs: vec![cfg("a"), cfg("b")], ..Default::default() };
        f.file_runners.insert("lua".into(), "lua {file}".into());
        save(&d, &f).unwrap();
        let l = load(&d).unwrap();
        assert_eq!(l.configs, f.configs);
        assert_eq!(l.file_runners["lua"], "lua {file}");
        assert!(std::fs::read_dir(d.join(".caesar")).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().contains("tmp")));
        let raw = std::fs::read_to_string(path(&d)).unwrap();
        assert!(raw.contains("showInServices") && raw.contains("fileRunners"));
    }

    #[test]
    fn validation_errors() {
        let d = tmp("val");
        let with = |c: Vec<RunConfig>| RunConfigFile { configs: c, ..Default::default() };
        assert!(save(&d, &with(vec![cfg("a"), cfg("a")])).is_err());
        assert!(save(&d, &with(vec![cfg("a:b")])).is_err());
        let mut bad = cfg("c");
        bad.cwd = "../x".into();
        assert!(save(&d, &with(vec![bad])).is_err());
        let mut bad = cfg("c");
        bad.env_file = Some("/etc/passwd".into());
        assert!(save(&d, &with(vec![bad])).is_err());
        let mut bad = cfg("c");
        bad.command = "  ".into();
        assert!(save(&d, &with(vec![bad])).is_err());
        let mut bad = cfg("c");
        bad.name = String::new();
        assert!(save(&d, &with(vec![bad])).is_err());
        assert!(!path(&d).exists());
    }

    #[test]
    fn old_json_without_optional_fields() {
        let d = tmp("old");
        std::fs::create_dir_all(d.join(".caesar")).unwrap();
        std::fs::write(
            path(&d),
            r#"{"configs":[{"id":"x","name":"X","kind":"springBoot","command":"mvn"}]}"#,
        )
        .unwrap();
        let l = load(&d).unwrap();
        assert!(l.configs[0].show_in_services);
        assert_eq!(l.configs[0].kind, RunKind::SpringBoot);
    }
}
