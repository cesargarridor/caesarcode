pub mod commands;
pub mod config;
pub mod detect;
pub mod logs;
pub mod manager;
pub mod metrics;
pub mod ports;
pub mod process_tree;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ServiceKind {
    Node,
    Python,
    Rust,
    Maven,
    Gradle,
    Docker,
    Go,
    Java,
    Shell,
    #[default]
    Custom,
}

/// Tecnología de un servicio (agrupación en el panel Services).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Tech {
    Spring,
    Python,
    Node,
    Go,
    Rust,
    Docker,
    Other,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServiceState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceConfig {
    pub id: String,
    pub name: String,
    pub command: String,
    /// Relativo al root del workspace; "" = root.
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Relativo a `cwd`.
    #[serde(default)]
    pub env_file: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub kind: ServiceKind,
    #[serde(default)]
    pub tech: Option<Tech>,
    /// `spring-boot | django | fastapi | flask | nestjs | express | fastify | next | vite | ...`
    #[serde(default)]
    pub framework: Option<String>,
}

/// `ServiceConfig` expuesto al frontend con su raíz (no se persiste en `services.json`).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RootedService {
    #[serde(flatten)]
    pub cfg: ServiceConfig,
    pub root: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// Raíz del workspace a la que pertenece (ruta absoluta).
    #[serde(default)]
    pub root: String,
    /// Servicio efímero de Run (no persistido en services.json).
    #[serde(default)]
    pub ephemeral: bool,
    pub state: ServiceState,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    /// % de CPU sumado sobre el árbol de procesos.
    pub cpu: f32,
    /// RAM en MB sumada sobre el árbol de procesos.
    pub memory_mb: f64,
    pub port_open: bool,
    pub port: Option<u16>,
    /// epoch ms
    pub started_at: Option<u64>,
    pub message: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub seq: u64,
    pub service_id: String,
    /// epoch ms
    pub ts: u64,
    pub level: LogLevel,
    /// Sin secuencias ANSI.
    pub text: String,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
