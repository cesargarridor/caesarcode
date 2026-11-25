//! Run Widget: configuraciones de ejecución, detección, resolver de "Current File" y arranque
//! como servicios efímeros del `ServiceManager`.

pub mod commands;
pub mod resolve;
pub mod store;
pub mod templates;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::services::{ServiceConfig, ServiceKind, Tech};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RunKind {
    SpringBoot,
    Python,
    Node,
    Go,
    Rust,
    /// "Custom Shell"
    Shell,
}

fn yes() -> bool {
    true
}

fn one() -> u32 {
    1
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunConfig {
    /// Slug único, sin ':'.
    pub id: String,
    pub name: String,
    pub kind: RunKind,
    /// Relativo al root; "" = root; sin "..".
    #[serde(default)]
    pub cwd: String,
    /// Entrypoint/comando base.
    pub command: String,
    /// Se concatena tal cual tras `command`.
    #[serde(default)]
    pub args: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub env_file: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default = "yes")]
    pub show_in_services: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RunConfigFile {
    #[serde(default = "one")]
    pub version: u32,
    #[serde(default)]
    pub configs: Vec<RunConfig>,
    /// Fallback del resolver: extensión (sin punto) -> plantilla.
    /// Placeholders (ya citados): {file} {fileDir} {root}; {fileStem} va sin citar.
    #[serde(default)]
    pub file_runners: HashMap<String, String>,
}

impl Default for RunConfigFile {
    fn default() -> Self {
        Self { version: 1, configs: Vec::new(), file_runners: HashMap::new() }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RunTemplate {
    pub kind: RunKind,
    pub label: String,
    pub config: RunConfig,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedRun {
    /// Línea completa para `sh -c` / `cmd /C`, rutas ya citadas.
    pub command: String,
    /// Relativo al root.
    pub cwd: String,
    /// "Run: main.py"
    pub label: String,
    /// "python (.venv)", "bun", "cargo run"... (tooltip)
    pub runner: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RunTarget {
    File { path: String },
    Config { id: String },
    Detected { service: ServiceConfig },
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RunStarted {
    pub runtime_id: String,
    pub label: String,
    pub command: String,
}

pub fn kind_to_service(kind: RunKind) -> (ServiceKind, Tech) {
    match kind {
        RunKind::SpringBoot => (ServiceKind::Java, Tech::Spring),
        RunKind::Python => (ServiceKind::Python, Tech::Python),
        RunKind::Node => (ServiceKind::Node, Tech::Node),
        RunKind::Go => (ServiceKind::Go, Tech::Go),
        RunKind::Rust => (ServiceKind::Rust, Tech::Rust),
        RunKind::Shell => (ServiceKind::Shell, Tech::Other),
    }
}

/// Servicio efímero `run:<id>` de una configuración guardada.
pub fn to_service_config(c: &RunConfig) -> ServiceConfig {
    let (kind, tech) = kind_to_service(c.kind);
    ServiceConfig {
        id: format!("run:{}", c.id),
        name: c.name.clone(),
        command: format!("{} {}", c.command.trim(), c.args).trim().to_string(),
        cwd: c.cwd.clone(),
        env: c.env.clone(),
        env_file: c.env_file.clone(),
        port: c.port,
        kind,
        tech: Some(tech),
        framework: if c.kind == RunKind::SpringBoot { Some("spring-boot".into()) } else { None },
    }
}
