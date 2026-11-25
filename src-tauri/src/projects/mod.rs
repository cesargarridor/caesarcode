//! Proyectos: recientes, detección de herramientas, asistente New Project y generadores
//! (plantillas locales, CLIs con streaming, descargas HTTP) + inicializador Spring Microservices.

pub mod cli;
pub mod commands;
pub mod fallback;
pub mod http;
pub mod local;
pub mod recent;
pub mod spring;
pub mod template;
pub mod tools;
pub mod validate;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use crate::error::{AppError, AppResult};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecentProject {
    pub path: String,
    pub name: String,
    /// Milisegundos desde epoch.
    pub last_opened: u64,
    /// Módulo detectado (`maven|gradle|node|cargo|go|python`).
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    /// Calculado al listar.
    #[serde(default)]
    pub exists: bool,
    /// Raíces adjuntas persistidas en `workspaces.json` (calculado al listar; no se guarda aquí).
    #[serde(default, skip_deserializing)]
    pub extra_roots: u32,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub id: String,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JdkInfo {
    pub home: String,
    pub version: String,
    pub major: u32,
    pub vendor: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ToolsReport {
    pub tools: Vec<ToolInfo>,
    pub jdks: Vec<JdkInfo>,
    pub online: Option<bool>,
}

impl ToolsReport {
    pub fn has(&self, id: &str) -> bool {
        self.tools.iter().any(|t| t.id == id && t.path.is_some())
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Strategy {
    Local,
    Cli,
    #[allow(dead_code)] // parte del contrato con el frontend (descarga sin fallback)
    Http,
    HttpWithFallback,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorInfo {
    pub id: String,
    pub label: String,
    pub group: String,
    pub strategy: Strategy,
    pub requires: Vec<String>,
    pub available: bool,
    pub missing: Vec<String>,
    pub offline_fallback: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CommonOpts {
    pub name: String,
    /// Carpeta padre existente.
    pub location: String,
    #[serde(default)]
    pub git: bool,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BuildSystem {
    Intellij,
    Maven,
    GradleKotlin,
    GradleGroovy,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JvmOpts {
    pub build: BuildSystem,
    pub java_version: u32,
    #[serde(default)]
    pub jdk_home: Option<String>,
    pub group_id: String,
    pub artifact_id: String,
    pub package_name: String,
    #[serde(default)]
    pub sample_code: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeRef {
    pub group: String,
    pub artifact: String,
    pub version: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "generator", rename_all = "camelCase")]
pub enum ProjectRequest {
    Java { common: CommonOpts, jvm: JvmOpts },
    Kotlin { common: CommonOpts, jvm: JvmOpts },
    Groovy { common: CommonOpts, jvm: JvmOpts },
    Empty { common: CommonOpts },
    MavenArchetype { common: CommonOpts, jvm: JvmOpts, archetype: ArchetypeRef },
    JavaFx { common: CommonOpts, jvm: JvmOpts },
    JakartaEe { common: CommonOpts, jvm: JvmOpts },
    Quarkus { common: CommonOpts, jvm: JvmOpts, #[serde(default)] extensions: Vec<String> },
    Micronaut { common: CommonOpts, jvm: JvmOpts, #[serde(default)] features: Vec<String> },
    Ktor { common: CommonOpts, jvm: JvmOpts },
    SpringMicroservices { common: CommonOpts, spring: SpringRequest },
    Html { common: CommonOpts },
    Express { common: CommonOpts, #[serde(default)] typescript: bool },
    Node { common: CommonOpts, #[serde(default)] typescript: bool },
    React { common: CommonOpts, #[serde(default)] typescript: bool },
    Vue { common: CommonOpts, #[serde(default)] typescript: bool, #[serde(default)] router: bool, #[serde(default)] pinia: bool },
    Vite { common: CommonOpts, template: String },
    Angular { common: CommonOpts, #[serde(default)] routing: bool, style: String },
    Python { common: CommonOpts, #[serde(default)] venv: bool },
    Go { common: CommonOpts, module: String },
    Rust { common: CommonOpts, #[serde(default)] bin: bool },
}

impl ProjectRequest {
    pub fn common(&self) -> &CommonOpts {
        use ProjectRequest::*;
        match self {
            Java { common, .. } | Kotlin { common, .. } | Groovy { common, .. } | Empty { common }
            | MavenArchetype { common, .. } | JavaFx { common, .. } | JakartaEe { common, .. }
            | Quarkus { common, .. } | Micronaut { common, .. } | Ktor { common, .. }
            | SpringMicroservices { common, .. } | Html { common } | Express { common, .. }
            | Node { common, .. } | React { common, .. } | Vue { common, .. } | Vite { common, .. }
            | Angular { common, .. } | Python { common, .. } | Go { common, .. } | Rust { common, .. } => common,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SpringRequest {
    pub group_id: String,
    pub base_package: String,
    pub java_version: u32,
    #[serde(default)]
    pub boot_version: Option<String>,
    pub build: BuildSystem,
    pub services: Vec<MicroserviceSpec>,
    #[serde(default)]
    pub gateway: Option<InfraSpec>,
    #[serde(default)]
    pub discovery: Option<InfraSpec>,
    #[serde(default)]
    pub config_server: Option<InfraSpec>,
    #[serde(default)]
    pub docker_compose: bool,
    #[serde(default)]
    pub dockerfiles: bool,
    #[serde(default)]
    pub prefer_online: bool,
    /// Extensión del plan: proyecto Spring Boot de un solo servicio en la raíz (sin agregador).
    #[serde(default)]
    pub single: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MicroserviceSpec {
    pub name: String,
    pub port: u16,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub layers: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InfraSpec {
    pub port: u16,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum GenEvent {
    Step { label: String },
    Output { line: String, stream: String },
    Warning { message: String },
    #[serde(rename_all = "camelCase")]
    Done { root: String, run_configs: usize },
    Failed { message: String },
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SpringCatalog {
    pub boot_versions: Vec<String>,
    pub default_boot: String,
    pub java_versions: Vec<u32>,
    pub dependencies: Vec<SpringDepGroup>,
    pub source: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SpringDepGroup {
    pub name: String,
    pub items: Vec<SpringDep>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SpringDep {
    pub id: String,
    pub name: String,
    pub description: String,
    pub offline: bool,
}

/// Cancelación de un job de creación (vive en `AppState.project_jobs`).
pub type CancelHandle = Arc<watch::Sender<bool>>;

type Sink = Arc<dyn Fn(GenEvent) -> bool + Send + Sync>;

/// Emisor de eventos del job + estado de cancelación. Si el canal falla (frontend cerrado)
/// se considera cancelado: la operación aborta de forma limpia.
#[derive(Clone)]
pub struct Reporter {
    sink: Sink,
    cancel: watch::Receiver<bool>,
    closed: Arc<AtomicBool>,
}

impl Reporter {
    pub fn new(sink: Sink, cancel: watch::Receiver<bool>) -> Self {
        Self { sink, cancel, closed: Arc::new(AtomicBool::new(false)) }
    }

    /// Sin cancelación posible (tests).
    #[cfg(test)]
    pub fn detached(sink: Sink) -> Self {
        let (tx, rx) = watch::channel(false);
        std::mem::forget(tx); // el receptor nunca ve cambios ni cierre
        Self::new(sink, rx)
    }

    pub fn emit(&self, ev: GenEvent) {
        if !(self.sink)(ev) {
            self.closed.store(true, Ordering::SeqCst);
        }
    }
    pub fn step(&self, label: impl Into<String>) {
        self.emit(GenEvent::Step { label: label.into() });
    }
    pub fn warn(&self, message: impl Into<String>) {
        self.emit(GenEvent::Warning { message: message.into() });
    }
    pub fn output(&self, line: impl Into<String>, stream: &str) {
        self.emit(GenEvent::Output { line: line.into(), stream: stream.into() });
    }
    pub fn is_cancelled(&self) -> bool {
        *self.cancel.borrow() || self.closed.load(Ordering::SeqCst)
    }
    /// `Err` si el job fue cancelado o el canal se cerró.
    pub fn alive(&self) -> AppResult<()> {
        if self.is_cancelled() {
            Err(AppError::Other("Cancelado".into()))
        } else {
            Ok(())
        }
    }
    pub fn cancel_rx(&self) -> watch::Receiver<bool> {
        self.cancel.clone()
    }
}

/// Espera a que el job se cancele (nunca resuelve si el emisor desaparece sin cancelar).
pub async fn wait_cancel(rx: &mut watch::Receiver<bool>) {
    loop {
        if *rx.borrow() {
            return;
        }
        if rx.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}
