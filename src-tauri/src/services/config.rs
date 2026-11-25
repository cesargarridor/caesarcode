use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::ServiceConfig;
use crate::error::{AppError, AppResult};

#[derive(Serialize, Deserialize)]
struct ConfigFile {
    #[serde(default = "one")]
    version: u32,
    #[serde(default)]
    services: Vec<ServiceConfig>,
}

fn one() -> u32 {
    1
}

pub fn primary_path(root: &Path) -> PathBuf {
    root.join(".caesar").join("services.json")
}

fn fallback_path(root: &Path) -> PathBuf {
    root.join("caesar.json")
}

/// `.caesar/services.json`; si no existe, `caesar.json`; si no, `[]`.
pub fn load(root: &Path) -> AppResult<Vec<ServiceConfig>> {
    for p in [primary_path(root), fallback_path(root)] {
        if p.is_file() {
            let text = std::fs::read_to_string(&p)?;
            let file: ConfigFile = serde_json::from_str(&text)
                .map_err(|e| AppError::Service(format!("{}: {e}", p.display())))?;
            return Ok(file.services);
        }
    }
    Ok(Vec::new())
}

pub fn bad_cwd(cwd: &str) -> bool {
    let p = Path::new(cwd);
    p.is_absolute() || p.components().any(|c| matches!(c, std::path::Component::ParentDir))
}

/// Ids únicos, nombre/comando no vacíos, cwd relativo sin `..`.
pub fn validate(services: &[ServiceConfig]) -> AppResult<()> {
    let mut seen = HashSet::new();
    for s in services {
        if s.id.trim().is_empty() {
            return Err(AppError::InvalidArgument("servicio sin id".into()));
        }
        if !seen.insert(s.id.as_str()) {
            return Err(AppError::InvalidArgument(format!("id duplicado: {}", s.id)));
        }
        if s.id.contains(':') {
            return Err(AppError::InvalidArgument(format!("el id '{}' no puede contener ':'", s.id)));
        }
        if s.name.trim().is_empty() {
            return Err(AppError::InvalidArgument(format!("el servicio {} no tiene nombre", s.id)));
        }
        if s.command.trim().is_empty() {
            return Err(AppError::InvalidArgument(format!("el servicio '{}' no tiene comando", s.name)));
        }
        if bad_cwd(&s.cwd) {
            return Err(AppError::InvalidArgument(format!(
                "cwd de '{}' debe ser relativo al workspace",
                s.name
            )));
        }
        if let Some(f) = &s.env_file {
            if bad_cwd(f) {
                return Err(AppError::InvalidArgument(format!("envFile de '{}' debe ser relativo", s.name)));
            }
        }
    }
    Ok(())
}

/// Escribe `.caesar/services.json` (pretty, atómico: tmp + rename).
pub fn save(root: &Path, services: &[ServiceConfig]) -> AppResult<()> {
    validate(services)?;
    let path = primary_path(root);
    let dir = path.parent().ok_or_else(|| AppError::Other("ruta inválida".into()))?;
    std::fs::create_dir_all(dir)?;
    let file = ConfigFile { version: 1, services: services.to_vec() };
    let mut text = serde_json::to_string_pretty(&file)?;
    text.push('\n');
    let tmp = dir.join(format!("services.json.cc-tmp-{}", std::process::id()));
    std::fs::write(&tmp, text)?;
    if let Err(e) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ServiceKind;
    use std::collections::HashMap;

    fn svc(id: &str) -> ServiceConfig {
        ServiceConfig {
            id: id.into(),
            name: id.into(),
            command: "echo hi".into(),
            cwd: String::new(),
            env: HashMap::new(),
            env_file: None,
            port: None,
            kind: ServiceKind::Custom,
            tech: None,
            framework: None,
        }
    }

    #[test]
    fn roundtrip_and_validation() {
        let dir = std::env::temp_dir().join(format!("cc-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(load(&dir).unwrap().is_empty());
        save(&dir, &[svc("a"), svc("b")]).unwrap();
        let loaded = load(&dir).unwrap();
        assert_eq!(loaded, vec![svc("a"), svc("b")]);
        assert!(save(&dir, &[svc("a"), svc("a")]).is_err());
        let mut bad = svc("c");
        bad.cwd = "../x".into();
        assert!(save(&dir, &[bad]).is_err());
        // caesar.json de respaldo
        std::fs::remove_dir_all(dir.join(".caesar")).unwrap();
        std::fs::write(
            dir.join("caesar.json"),
            r#"{"version":1,"services":[{"id":"z","name":"Z","command":"x"}]}"#,
        )
        .unwrap();
        assert_eq!(load(&dir).unwrap()[0].id, "z");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
