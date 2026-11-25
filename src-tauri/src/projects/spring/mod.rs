//! Inicializador de microservicios Spring: start.spring.io con fallback offline por servicio.

pub mod catalog;
pub mod generate;
pub mod offline;

use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use self::generate::{Module, Plan, Role};
use super::http;
use super::{BuildSystem, Reporter, SpringCatalog, SpringRequest};
use crate::error::{AppError, AppResult};

pub const METADATA_ACCEPT: &str = "application/vnd.initializr.v2.2+json";

/// Catálogo online (timeout 6 s). `Err` si no hay red o la respuesta no se entiende.
pub async fn fetch_catalog() -> AppResult<SpringCatalog> {
    let client = http::client(Duration::from_secs(6))?;
    let url = http::build_url(http::SPRING_HOST, "metadata/client", &[])?;
    let bytes = http::fetch(&client, url, Some(METADATA_ACCEPT), None).await?;
    let v: Value = serde_json::from_slice(&bytes)?;
    catalog::parse_metadata(&v).ok_or_else(|| AppError::Other("catálogo de start.spring.io no reconocido".into()))
}

fn project_type(b: BuildSystem) -> &'static str {
    match b {
        BuildSystem::GradleKotlin => "gradle-project-kotlin",
        BuildSystem::GradleGroovy => "gradle-project",
        _ => "maven-project",
    }
}

async fn download_module(client: &reqwest::Client, plan: &Plan, m: &Module, dir: &Path, rep: &Reporter) -> AppResult<()> {
    let deps = m.deps.join(",");
    let java = plan.req.java_version.to_string();
    let url = http::build_url(
        http::SPRING_HOST,
        "starter.zip",
        &[
            ("type", project_type(plan.req.build)),
            ("language", "java"),
            ("bootVersion", &plan.boot),
            ("groupId", &plan.req.group_id),
            ("artifactId", &m.name),
            ("name", &m.name),
            ("description", &m.name),
            ("packageName", &m.package),
            ("javaVersion", &java),
            ("packaging", "jar"),
            ("dependencies", &deps),
            ("baseDir", &m.name),
        ],
    )?;
    let bytes = http::fetch(client, url, None, Some(rep.cancel_rx())).await?;
    let (dir, bytes) = (dir.to_path_buf(), bytes);
    tokio::task::spawn_blocking(move || http::extract_zip(&bytes, &dir, true))
        .await
        .map_err(|e| AppError::Other(format!("tarea abortada: {e}")))??;
    Ok(())
}

/// Genera todo el proyecto en `target` (ya creado). `online`: usar start.spring.io para cada servicio.
/// Devuelve el número de run-configs escritas.
pub async fn generate(req: &SpringRequest, project: &str, boot: &str, target: &Path, online: bool, rep: &Reporter) -> AppResult<usize> {
    let plan = Plan::new(req, project, boot);
    let client = if online { Some(http::client(Duration::from_secs(60))?) } else { None };
    let mut warnings: Vec<String> = Vec::new();
    for (i, m) in plan.modules.iter().enumerate() {
        rep.alive()?;
        let dir = plan.dir_of(m, target);
        let role = match m.role {
            Role::Service => "microservicio",
            Role::Gateway => "API Gateway",
            Role::Discovery => "Eureka Server",
            Role::Config => "Config Server",
        };
        rep.step(format!("[{}/{}] {} ({role}, puerto {})", i + 1, plan.modules.len(), m.name, m.port));
        let mut done_online = false;
        if let Some(c) = &client {
            match download_module(c, &plan, m, &dir, rep).await {
                Ok(()) => done_online = true,
                Err(e) if rep.is_cancelled() => return Err(e),
                Err(e) => {
                    rep.warn(format!("{}: start.spring.io no disponible ({e}); se usa la plantilla offline", m.name));
                    // Lo que haya quedado a medias se descarta antes del fallback.
                    if !plan.req.single {
                        let _ = std::fs::remove_dir_all(&dir);
                    }
                }
            }
        }
        if !done_online {
            offline::write_build(&plan, m, &dir, rep)?;
        }
        generate::finish_module(&plan, m, &dir)?;
    }
    rep.step("Archivos del proyecto (agregador, docker-compose, README)");
    generate::write_root(&plan, target, &mut warnings)?;
    for w in warnings {
        rep.warn(w);
    }
    rep.step("Configuraciones de ejecución");
    generate::save_run_configs(target, generate::run_configs(&plan, target))
}

#[cfg(test)]
mod tests;
