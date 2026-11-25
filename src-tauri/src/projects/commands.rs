//! Comandos Tauri de proyectos y el orquestador `project_create`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tokio::sync::watch;

use super::cli::{self, AngularTool, CliSpec};
use super::local::{self, Cli, Post};
use super::spring::catalog;
use super::{
    fallback, http, recent, spring, template, tools, validate, BuildSystem, GenEvent, GeneratorInfo, ProjectRequest,
    RecentProject, Reporter, SpringCatalog, Strategy, ToolsReport,
};
use crate::error::{AppError, AppResult};
use crate::run::{self, RunConfig, RunConfigFile, RunKind};
use crate::state::AppState;
use crate::util::run_blocking;

pub fn data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir())
}

// ---------------------------------------------------------------- recientes

#[tauri::command]
pub async fn projects_recent_list(app: AppHandle) -> AppResult<Vec<RecentProject>> {
    let dir = data_dir(&app);
    run_blocking(move || Ok(recent::list(&dir))).await
}

#[tauri::command]
pub async fn projects_recent_remove(app: AppHandle, path: String) -> AppResult<()> {
    let dir = data_dir(&app);
    run_blocking(move || recent::remove(&dir, &path)).await
}

#[tauri::command]
pub async fn projects_recent_pin(app: AppHandle, path: String, pinned: bool) -> AppResult<()> {
    let dir = data_dir(&app);
    run_blocking(move || recent::pin(&dir, &path, pinned)).await
}

// ---------------------------------------------------------------- herramientas y generadores

#[tauri::command]
pub async fn projects_tools_detect(state: State<'_, AppState>, check_online: bool) -> AppResult<ToolsReport> {
    let report = tools::detect(check_online).await;
    if let Ok(mut c) = state.tools.lock() {
        *c = Some(report.clone());
    }
    Ok(report)
}

/// `(id, etiqueta, grupo, estrategia, herramientas requeridas, fallback offline)`.
type GeneratorRow = (&'static str, &'static str, &'static str, Strategy, &'static [&'static str], bool);
const GENERATORS: &[GeneratorRow] = &[
    ("java", "Java", "Java/JVM", Strategy::Local, &[], false),
    ("kotlin", "Kotlin", "Java/JVM", Strategy::Local, &[], false),
    ("groovy", "Groovy", "Java/JVM", Strategy::Local, &[], false),
    ("empty", "Empty Project", "Java/JVM", Strategy::Local, &[], false),
    ("mavenArchetype", "Maven Archetype", "Java/JVM", Strategy::Cli, &["mvn"], false),
    ("javaFx", "JavaFX", "Java/JVM", Strategy::Local, &[], false),
    ("jakartaEe", "Jakarta EE", "Java/JVM", Strategy::Local, &[], false),
    ("springBoot", "Spring Boot", "Frameworks", Strategy::HttpWithFallback, &[], true),
    ("springMicroservices", "Spring Microservices", "Frameworks", Strategy::HttpWithFallback, &[], true),
    ("quarkus", "Quarkus", "Frameworks", Strategy::HttpWithFallback, &[], true),
    ("micronaut", "Micronaut", "Frameworks", Strategy::HttpWithFallback, &[], true),
    ("ktor", "Ktor", "Frameworks", Strategy::Local, &[], false),
    ("html", "HTML", "Web", Strategy::Local, &[], false),
    ("react", "React", "Web", Strategy::Cli, &["node", "npm"], false),
    ("vue", "Vue.js", "Web", Strategy::Cli, &["node", "npm"], false),
    ("vite", "Vite", "Web", Strategy::Cli, &["node", "npm"], false),
    ("angular", "Angular CLI", "Web", Strategy::Cli, &["npx"], false),
    ("express", "Express", "Web", Strategy::Local, &[], false),
    ("node", "Node.js", "Web", Strategy::Local, &[], false),
    ("python", "Python", "Otros", Strategy::Local, &[], false),
    ("go", "Go", "Otros", Strategy::Local, &[], false),
    ("rust", "Rust", "Otros", Strategy::Local, &[], false),
];

pub fn generators_for(report: &ToolsReport) -> Vec<GeneratorInfo> {
    GENERATORS
        .iter()
        .map(|(id, label, group, strategy, requires, offline)| {
            let missing: Vec<String> = requires.iter().filter(|t| !report.has(t)).map(|t| (*t).to_string()).collect();
            GeneratorInfo {
                id: (*id).into(),
                label: (*label).into(),
                group: (*group).into(),
                strategy: *strategy,
                requires: requires.iter().map(|t| (*t).to_string()).collect(),
                available: missing.is_empty(),
                missing,
                offline_fallback: *offline,
            }
        })
        .collect()
}

#[tauri::command]
pub async fn project_generators(state: State<'_, AppState>) -> AppResult<Vec<GeneratorInfo>> {
    let cached = state.tools.lock().map_err(|_| AppError::Other("lock envenenado".into()))?.clone();
    let report = match cached {
        Some(r) => r,
        None => {
            let r = tools::detect(false).await;
            if let Ok(mut c) = state.tools.lock() {
                *c = Some(r.clone());
            }
            r
        }
    };
    Ok(generators_for(&report))
}

/// `online = false` devuelve al instante el catálogo offline (el formulario nunca espera a la red).
#[tauri::command]
pub async fn spring_catalog(online: Option<bool>) -> AppResult<SpringCatalog> {
    if online == Some(false) {
        return Ok(catalog::offline_catalog());
    }
    Ok(spring::fetch_catalog().await.unwrap_or_else(|_| catalog::offline_catalog()))
}

// ---------------------------------------------------------------- creación

fn valid_job_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[tauri::command]
pub async fn project_create(
    state: State<'_, AppState>,
    request: ProjectRequest,
    job_id: String,
    on_event: Channel<GenEvent>,
) -> AppResult<String> {
    let fail = |e: AppError| {
        let _ = on_event.send(GenEvent::Failed { message: e.to_string() });
        e
    };
    if !valid_job_id(&job_id) {
        return Err(fail(AppError::InvalidArgument("job_id inválido".into())));
    }
    let (location, target) = validate::request(&request).map_err(fail)?;
    let (tx, rx) = watch::channel(false);
    {
        let mut jobs = state.project_jobs.lock().map_err(|_| AppError::Other("lock envenenado".into()))?;
        if jobs.contains_key(&job_id) {
            return Err(fail(AppError::InvalidArgument("ya hay un trabajo con ese id".into())));
        }
        jobs.insert(job_id.clone(), Arc::new(tx));
    }
    let sink_ch = on_event.clone();
    let rep = Reporter::new(Arc::new(move |ev| sink_ch.send(ev).is_ok()), rx);
    let result = create(&request, &location, &target, &rep).await;
    if let Ok(mut jobs) = state.project_jobs.lock() {
        jobs.remove(&job_id);
    }
    match result {
        Ok(n) => {
            let root = target.to_string_lossy().into_owned();
            rep.emit(GenEvent::Done { root: root.clone(), run_configs: n });
            Ok(root)
        }
        Err(e) => {
            let msg = if rep.is_cancelled() { "Cancelado".to_string() } else { e.to_string() };
            rep.emit(GenEvent::Failed { message: msg.clone() });
            Err(AppError::Other(msg))
        }
    }
}

#[tauri::command]
pub async fn project_cancel(state: State<'_, AppState>, job_id: String) -> AppResult<()> {
    let handle = state.project_jobs.lock().map_err(|_| AppError::Other("lock envenenado".into()))?.get(&job_id).cloned();
    if let Some(h) = handle {
        let _ = h.send(true);
    }
    Ok(())
}

/// Crea el proyecto; si falla o se cancela, borra lo que este job creó (nunca nada previo).
pub async fn create(req: &ProjectRequest, location: &Path, target: &Path, rep: &Reporter) -> AppResult<usize> {
    let existed = target.exists();
    let res = create_inner(req, location, target, rep).await;
    if res.is_err() {
        cleanup(target, existed);
    }
    res
}

fn cleanup(target: &Path, existed: bool) {
    if !existed {
        let _ = std::fs::remove_dir_all(target);
    } else if let Ok(rd) = std::fs::read_dir(target) {
        // Existía vacía (validado): se vacía de nuevo.
        for e in rd.flatten() {
            let p = e.path();
            let _ = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
        }
    }
}

async fn create_inner(req: &ProjectRequest, location: &Path, target: &Path, rep: &Reporter) -> AppResult<usize> {
    use ProjectRequest::*;
    rep.step(format!("Creando proyecto en {}", target.display()));
    let mut run_configs: Option<usize> = None;
    match req {
        MavenArchetype { .. } | React { .. } | Vite { .. } | Vue { .. } | Angular { .. } => {
            gen_cli(req, location, target, rep).await?;
        }
        Quarkus { .. } | Micronaut { .. } => gen_http(req, target, rep).await?,
        SpringMicroservices { common, spring: sp } => {
            std::fs::create_dir_all(target)?;
            let mut boot = sp.boot_version.clone();
            let mut online = false;
            if sp.prefer_online {
                rep.step("Consultando start.spring.io");
                match spring::fetch_catalog().await {
                    Ok(cat) => {
                        online = true;
                        boot.get_or_insert(cat.default_boot);
                    }
                    Err(e) => rep.warn(format!("Sin conexión con start.spring.io ({e}); se usa la plantilla offline")),
                }
            }
            let boot = boot.unwrap_or_else(|| catalog::FALLBACK_BOOT.to_string());
            run_configs = Some(spring::generate(sp, &common.name, &boot, target, online, rep).await?);
        }
        _ => gen_local(req, target, rep).await?,
    }
    rep.alive()?;
    ensure_gitignore(req, target);
    let n = match run_configs {
        Some(n) => n,
        None => write_run_configs(req, target)?,
    };
    if req.common().git {
        git_init(target, rep).await;
    }
    Ok(n)
}

fn which_bin(bin: &str) -> Option<PathBuf> {
    which::which(bin).ok()
}

async fn gen_local(req: &ProjectRequest, target: &Path, rep: &Reporter) -> AppResult<()> {
    std::fs::create_dir_all(target)?;
    let cli = Cli { cargo: which_bin("cargo").is_some(), go: which_bin("go").is_some() };
    let (r, t, rp) = (req.clone(), target.to_path_buf(), rep.clone());
    let posts = run_blocking(move || local::generate(&r, &t, cli, &rp)).await?;
    for p in posts {
        rep.alive()?;
        run_post(p, req, target, rep).await?;
    }
    Ok(())
}

async fn run_post(post: Post, req: &ProjectRequest, target: &Path, rep: &Reporter) -> AppResult<()> {
    let name = &req.common().name;
    match post {
        Post::GradleWrapper => {
            rep.step("Generando Gradle wrapper");
            match which_bin("gradle") {
                Some(g) => {
                    if let Err(e) = cli::run_cli(CliSpec::new(g, &["wrapper"], target), rep).await {
                        rep.alive()?;
                        rep.warn(format!("No se pudo generar el wrapper de Gradle: {e}"));
                    }
                }
                None => rep.warn("'gradle' no está en el PATH: no se genera el wrapper (instala Gradle y ejecuta 'gradle wrapper')"),
            }
        }
        Post::CargoInit { lib, .. } => {
            rep.step("cargo init");
            let pkg = local::rust_pkg_name(name);
            let mut args = vec!["init", "--vcs", "none", "--name", pkg.as_str()];
            if lib {
                args.push("--lib");
            }
            let cargo = which_bin("cargo").ok_or_else(|| AppError::Other("cargo no encontrado".into()))?;
            if let Err(e) = cli::run_cli(CliSpec::new(cargo, &args, target), rep).await {
                rep.alive()?;
                rep.warn(format!("cargo init falló ({e}); se usa la plantilla de Rust"));
                local::write_rust_template(target, name, !lib)?;
            }
        }
        Post::GoModInit { module } => {
            rep.step("go mod init");
            let go = which_bin("go").ok_or_else(|| AppError::Other("go no encontrado".into()))?;
            if let Err(e) = cli::run_cli(CliSpec::new(go, &["mod", "init", module.as_str()], target), rep).await {
                rep.alive()?;
                rep.warn(format!("go mod init falló ({e}); se escribe go.mod a mano"));
                local::write_go_mod(target, &module)?;
            }
        }
        Post::PythonVenv => {
            rep.step("Creando entorno virtual (.venv)");
            match which_bin("python3").or_else(|| which_bin("python")) {
                Some(py) => {
                    if let Err(e) = cli::run_cli(CliSpec::new(py, &["-m", "venv", ".venv"], target), rep).await {
                        rep.alive()?;
                        rep.warn(format!("No se pudo crear el entorno virtual: {e}"));
                    }
                }
                None => rep.warn("python3 no está en el PATH: no se crea el entorno virtual"),
            }
        }
        Post::NpmInstall => {
            rep.step("npm install");
            match which_bin("npm") {
                Some(npm) => {
                    if let Err(e) = cli::run_cli(CliSpec::new(npm, &["install", "--no-audit", "--no-fund"], target), rep).await {
                        rep.alive()?;
                        rep.warn(format!("npm install falló ({e}); ejecútalo más tarde"));
                    }
                }
                None => rep.warn("npm no está en el PATH: instala Node.js y ejecuta 'npm install'"),
            }
        }
    }
    Ok(())
}

/// CLIs que crean la carpeta ellas mismas (se ejecutan en `location`).
async fn gen_cli(req: &ProjectRequest, location: &Path, target: &Path, rep: &Reporter) -> AppResult<()> {
    use ProjectRequest::*;
    let name = req.common().name.clone();
    // Las CLIs crean la carpeta: si existía (vacía) se retira para que no se queje.
    if target.is_dir() {
        let _ = std::fs::remove_dir(target);
    }
    let npm_hint = "Instala Node.js (https://nodejs.org) y vuelve a intentarlo.";
    match req {
        MavenArchetype { jvm, archetype, .. } => {
            let mvn = cli::require("mvn", "Instala Apache Maven (https://maven.apache.org) y vuelve a intentarlo.")?;
            let artifact_dir = location.join(&jvm.artifact_id);
            if jvm.artifact_id != name && artifact_dir.exists() {
                return Err(AppError::AlreadyExists(artifact_dir.display().to_string()));
            }
            rep.step("mvn archetype:generate");
            let spec = cli::maven_archetype(mvn, location, archetype, &jvm.group_id, &jvm.artifact_id, &jvm.package_name, jvm.jdk_home.as_deref());
            cli::run_cli(spec, rep).await?;
            if jvm.artifact_id != name {
                std::fs::rename(&artifact_dir, target)?;
            }
        }
        React { typescript, .. } => {
            let npm = cli::require("npm", npm_hint)?;
            let tpl = if *typescript { "react-ts" } else { "react" };
            rep.step("npm create vite (React)");
            cli::run_cli(cli::npm_create(npm, location, "vite@latest", &name, &["--template", tpl]), rep).await?;
            run_post(Post::NpmInstall, req, target, rep).await?;
        }
        Vite { template, .. } => {
            let npm = cli::require("npm", npm_hint)?;
            rep.step("npm create vite");
            cli::run_cli(cli::npm_create(npm, location, "vite@latest", &name, &["--template", template.as_str()]), rep).await?;
            run_post(Post::NpmInstall, req, target, rep).await?;
        }
        Vue { typescript, router, pinia, .. } => {
            let npm = cli::require("npm", npm_hint)?;
            let mut extra = vec!["--default"];
            if *typescript {
                extra.push("--ts");
            }
            if *router {
                extra.push("--router");
            }
            if *pinia {
                extra.push("--pinia");
            }
            rep.step("npm create vue");
            cli::run_cli(cli::npm_create(npm, location, "vue@latest", &name, &extra), rep).await?;
            run_post(Post::NpmInstall, req, target, rep).await?;
        }
        Angular { routing, style, .. } => {
            let tool: AngularTool = cli::find_angular().ok_or_else(|| {
                AppError::Other("Falta 'ng' o 'npx' en el PATH. Instala Node.js (https://nodejs.org) o Angular CLI (npm i -g @angular/cli).".into())
            })?;
            rep.step("ng new");
            cli::run_cli(cli::angular_new(&tool, location, &name, *routing, style), rep).await?;
        }
        _ => unreachable!("gen_cli solo recibe generadores CLI"),
    }
    if !target.is_dir() {
        return Err(AppError::Other(format!("la herramienta terminó sin crear {}", target.display())));
    }
    Ok(())
}

fn quarkus_build(b: BuildSystem) -> &'static str {
    match b {
        BuildSystem::GradleKotlin => "GRADLE_KOTLIN_DSL",
        BuildSystem::GradleGroovy => "GRADLE",
        _ => "MAVEN",
    }
}

fn micronaut_build(b: BuildSystem) -> &'static str {
    match b {
        BuildSystem::GradleKotlin => "GRADLE_KOTLIN",
        BuildSystem::GradleGroovy => "GRADLE",
        _ => "MAVEN",
    }
}

/// Quarkus / Micronaut: descarga el zip del generador oficial; sin red, plantilla Maven mínima.
async fn gen_http(req: &ProjectRequest, target: &Path, rep: &Reporter) -> AppResult<()> {
    use ProjectRequest::*;
    let client = http::client(Duration::from_secs(60))?;
    rep.step("Descargando el proyecto del generador oficial");
    let (url, jvm, name) = match req {
        Quarkus { common, jvm, extensions } => {
            let java = jvm.java_version.to_string();
            let mut q: Vec<(&str, &str)> = vec![("g", &jvm.group_id), ("a", &jvm.artifact_id), ("j", &java), ("b", quarkus_build(jvm.build))];
            for e in extensions {
                q.push(("e", e));
            }
            (http::build_url(http::QUARKUS_HOST, "d", &q)?, jvm, &common.name)
        }
        Micronaut { common, jvm, features } => {
            let java = jvm.java_version.to_string();
            let mut q: Vec<(&str, &str)> = vec![("lang", "JAVA"), ("build", micronaut_build(jvm.build)), ("jdk", &java), ("test", "JUNIT")];
            for f in features {
                q.push(("features", f));
            }
            let path = format!("create/default/{}.{}", jvm.group_id, jvm.artifact_id);
            (http::build_url(http::MICRONAUT_HOST, &path, &q)?, jvm, &common.name)
        }
        _ => unreachable!("gen_http solo recibe Quarkus/Micronaut"),
    };
    let fetched = http::fetch(&client, url, None, Some(rep.cancel_rx())).await;
    let ok = match fetched {
        Ok(bytes) => {
            let t = target.to_path_buf();
            match run_blocking(move || http::extract_zip(&bytes, &t, true).map(|_| ())).await {
                Ok(()) => true,
                Err(e) => {
                    rep.warn(format!("El zip descargado no es válido ({e})"));
                    false
                }
            }
        }
        Err(e) if rep.is_cancelled() => return Err(e),
        Err(e) => {
            rep.warn(format!("Sin conexión con el generador oficial ({e})"));
            false
        }
    };
    if !ok {
        rep.warn("Se usa la plantilla Maven mínima offline (versiones fijas, sin Gradle)");
        let _ = std::fs::remove_dir_all(target);
        std::fs::create_dir_all(target)?;
        match req {
            Quarkus { extensions, .. } => fallback::write_quarkus(target, name, jvm, extensions, rep)?,
            Micronaut { features, .. } => fallback::write_micronaut(target, name, jvm, features, rep)?,
            _ => {}
        }
        template::write_text(target, "README.md", &format!("# {name}\n\nProyecto creado con CaesarCode (plantilla offline).\n"))?;
    }
    Ok(())
}

fn ensure_gitignore(req: &ProjectRequest, target: &Path) {
    use ProjectRequest::*;
    if target.join(".gitignore").exists() {
        return;
    }
    let text = match req {
        React { .. } | Vue { .. } | Vite { .. } | Angular { .. } | Express { .. } | Node { .. } => local::GITIGNORE_NODE,
        Python { .. } => local::GITIGNORE_PYTHON,
        Go { .. } => local::GITIGNORE_GO,
        Rust { .. } => local::GITIGNORE_RUST,
        Java { .. } | Kotlin { .. } | Groovy { .. } | MavenArchetype { .. } | JavaFx { .. } | JakartaEe { .. }
        | Quarkus { .. } | Micronaut { .. } | Ktor { .. } | SpringMicroservices { .. } => local::GITIGNORE_JAVA,
        Html { .. } | Empty { .. } => local::GITIGNORE_GENERIC,
    };
    let _ = std::fs::write(target.join(".gitignore"), text);
}

async fn git_init(target: &Path, rep: &Reporter) {
    rep.step("git init");
    if which_bin("git").is_none() {
        rep.warn("git no está en el PATH: no se inicializa el repositorio");
        return;
    }
    if crate::git::run_git_in(target, &["init", "-b", "main"]).await.is_ok() {
        return;
    }
    // git < 2.28 no conoce `-b`.
    if let Err(e) = crate::git::run_git_in(target, &["init"]).await {
        rep.warn(format!("git init falló: {e}"));
    }
}

// ---------------------------------------------------------------- run configs (no Spring)

fn rc(id: &str, name: &str, kind: RunKind, command: &str, port: Option<u16>) -> RunConfig {
    RunConfig {
        id: id.into(),
        name: name.into(),
        kind,
        cwd: String::new(),
        command: command.into(),
        args: String::new(),
        env: Default::default(),
        env_file: None,
        port,
        show_in_services: true,
    }
}

/// Comando con el wrapper del proyecto (`./mvnw`) si existe; si no, la herramienta del PATH.
fn wrapper(target: &Path, script: &str, plain: &str) -> String {
    if target.join(script).is_file() {
        if cfg!(windows) { script.to_string() } else { format!("./{script}") }
    } else {
        plain.to_string()
    }
}

pub fn run_configs_for(req: &ProjectRequest, target: &Path) -> Vec<RunConfig> {
    use ProjectRequest::*;
    let mvn = |goal: &str| format!("{} {goal}", wrapper(target, "mvnw", "mvn"));
    let gradle = |task: &str| format!("{} {task}", wrapper(target, "gradlew", "gradle"));
    let with_jdk = |mut c: RunConfig, jdk: &Option<String>| {
        if let Some(h) = jdk {
            c.env.insert("JAVA_HOME".into(), h.clone());
        }
        c
    };
    match req {
        Java { jvm, .. } | Kotlin { jvm, .. } | Groovy { jvm, .. } => match jvm.build {
            BuildSystem::Intellij => vec![],
            BuildSystem::Maven => vec![with_jdk(rc("run", "Run", RunKind::Shell, &mvn("-q compile exec:java"), None), &jvm.jdk_home)],
            _ => vec![with_jdk(rc("run", "Run", RunKind::Shell, &gradle("run"), None), &jvm.jdk_home)],
        },
        JavaFx { jvm, .. } => vec![with_jdk(rc("run", "Run", RunKind::Shell, &mvn("javafx:run"), None), &jvm.jdk_home)],
        Ktor { jvm, .. } => vec![with_jdk(rc("run", "Run", RunKind::Shell, &gradle("run"), Some(8080)), &jvm.jdk_home)],
        Quarkus { jvm, .. } => {
            let gradle_project = target.join("build.gradle").exists() || target.join("build.gradle.kts").exists();
            let cmd = if gradle_project {
                gradle("quarkusDev")
            } else {
                mvn("quarkus:dev")
            };
            vec![with_jdk(rc("dev", "Quarkus dev", RunKind::Shell, &cmd, Some(8080)), &jvm.jdk_home)]
        }
        Micronaut { jvm, .. } => {
            let cmd = if target.join("pom.xml").exists() { mvn("mn:run") } else { gradle("run") };
            vec![with_jdk(rc("run", "Run", RunKind::Shell, &cmd, Some(8080)), &jvm.jdk_home)]
        }
        Express { .. } => vec![rc("dev", "Dev server", RunKind::Node, "npm run dev", Some(3000))],
        Node { .. } => vec![rc("dev", "Dev", RunKind::Node, "npm run dev", None)],
        React { .. } | Vite { .. } | Vue { .. } => vec![rc("dev", "Dev server", RunKind::Node, "npm run dev", Some(5173))],
        Angular { .. } => vec![rc("start", "Dev server", RunKind::Node, "npm start", Some(4200))],
        Python { common, .. } => {
            let pkg = common.name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>();
            let pkg = if pkg.starts_with(|c: char| c.is_ascii_digit()) { format!("_{pkg}") } else { pkg };
            vec![rc("run", "Run", RunKind::Python, &format!("python3 src/{pkg}/main.py"), None)]
        }
        Go { .. } => vec![rc("run", "Run", RunKind::Go, "go run .", None)],
        Rust { bin, .. } => {
            if *bin {
                vec![rc("run", "Run", RunKind::Rust, "cargo run", None)]
            } else {
                vec![]
            }
        }
        Empty { .. } | Html { .. } | JakartaEe { .. } | MavenArchetype { .. } | SpringMicroservices { .. } => vec![],
    }
}

fn write_run_configs(req: &ProjectRequest, target: &Path) -> AppResult<usize> {
    let configs = run_configs_for(req, target);
    if configs.is_empty() {
        return Ok(0);
    }
    let n = configs.len();
    run::store::save(target, &RunConfigFile { configs, ..Default::default() })?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{CommonOpts, JvmOpts};

    fn common(name: &str, loc: &Path) -> CommonOpts {
        CommonOpts { name: name.into(), location: loc.to_string_lossy().into(), git: false }
    }

    fn jvm(build: BuildSystem) -> JvmOpts {
        JvmOpts { build, java_version: 21, jdk_home: None, group_id: "com.example".into(), artifact_id: "demo".into(), package_name: "com.example.demo".into(), sample_code: true }
    }

    fn collector() -> (Reporter, Arc<std::sync::Mutex<Vec<GenEvent>>>) {
        let ev = Arc::new(std::sync::Mutex::new(Vec::new()));
        let e2 = ev.clone();
        (Reporter::detached(Arc::new(move |g| { e2.lock().unwrap().push(g); true })), ev)
    }

    #[tokio::test]
    async fn create_local_projects_end_to_end() {
        let d = tempfile::tempdir().unwrap();
        for req in [
            ProjectRequest::Java { common: common("app-java", d.path()), jvm: jvm(BuildSystem::Maven) },
            ProjectRequest::Html { common: common("site", d.path()) },
            ProjectRequest::Python { common: common("py-app", d.path()), venv: false },
            ProjectRequest::Empty { common: common("empty", d.path()) },
        ] {
            let (location, target) = validate::request(&req).unwrap();
            let (rep, _) = collector();
            let n = create(&req, &location, &target, &rep).await.unwrap();
            assert!(target.join(".gitignore").is_file(), "{target:?}");
            match &req {
                ProjectRequest::Java { .. } => {
                    assert_eq!(n, 1);
                    assert_eq!(run::store::load(&target).unwrap().configs[0].command, "mvn -q compile exec:java");
                }
                ProjectRequest::Python { .. } => assert_eq!(run::store::load(&target).unwrap().configs[0].command, "python3 src/py_app/main.py"),
                _ => assert_eq!(n, 0),
            }
        }
    }

    #[tokio::test]
    async fn spring_offline_via_create_opens_with_run_configs() {
        let d = tempfile::tempdir().unwrap();
        let sp = crate::projects::SpringRequest {
            group_id: "com.example".into(),
            base_package: "com.example.shop".into(),
            java_version: 21,
            boot_version: None,
            build: BuildSystem::Maven,
            services: vec![crate::projects::MicroserviceSpec { name: "orders".into(), port: 8081, dependencies: vec!["web".into()], layers: true }],
            gateway: Some(crate::projects::InfraSpec { port: 8080 }),
            discovery: None,
            config_server: None,
            docker_compose: false,
            dockerfiles: false,
            prefer_online: false,
            single: false,
        };
        let mut c = common("shop", d.path());
        c.git = false;
        let req = ProjectRequest::SpringMicroservices { common: c, spring: sp };
        let (location, target) = validate::request(&req).unwrap();
        let (rep, ev) = collector();
        assert_eq!(create(&req, &location, &target, &rep).await.unwrap(), 2);
        assert!(target.join("pom.xml").is_file() && target.join("orders/pom.xml").is_file());
        assert!(ev.lock().unwrap().iter().any(|e| matches!(e, GenEvent::Step { .. })));
    }

    #[tokio::test]
    async fn failure_cleans_up_created_dir_only() {
        let d = tempfile::tempdir().unwrap();
        let (rep, _) = collector();
        // Maven Archetype sin mvn en PATH real puede existir; usamos Angular con nombre válido pero forzamos fallo
        // con un fallo propio: ProjectRequest::Go con módulo válido pero cancelación previa.
        let (tx, rx) = watch::channel(false);
        let cancelled = Reporter::new(Arc::new(|_| true), rx);
        tx.send(true).unwrap();
        let req = ProjectRequest::Empty { common: common("will-fail", d.path()) };
        let (location, target) = validate::request(&req).unwrap();
        assert!(create(&req, &location, &target, &cancelled).await.is_err());
        assert!(!target.exists(), "lo creado por el job se borra al cancelar");
        // carpeta preexistente vacía: se conserva (vaciada)
        std::fs::create_dir(d.path().join("pre")).unwrap();
        let req = ProjectRequest::Empty { common: common("pre", d.path()) };
        let (location, target) = validate::request(&req).unwrap();
        assert!(create(&req, &location, &target, &cancelled).await.is_err());
        assert!(target.is_dir() && std::fs::read_dir(&target).unwrap().next().is_none());
        let _ = rep;
    }

    #[test]
    fn generators_report_missing_tools() {
        let empty = ToolsReport { tools: vec![], jdks: vec![], online: None };
        let g = generators_for(&empty);
        let by = |id: &str| g.iter().find(|x| x.id == id).unwrap();
        assert!(!by("react").available && by("react").missing == ["node", "npm"]);
        assert!(!by("mavenArchetype").available && by("mavenArchetype").missing == ["mvn"]);
        assert!(by("java").available && by("springMicroservices").offline_fallback && by("quarkus").offline_fallback);
        assert_eq!(g.len(), 22);
    }

    #[test]
    fn job_ids() {
        assert!(valid_job_id("abc-123_x") && !valid_job_id("") && !valid_job_id("a b") && !valid_job_id("../x"));
    }

    #[tokio::test]
    #[ignore = "requiere npm y red"]
    async fn vite_react_with_real_npm() {
        let d = tempfile::tempdir().unwrap();
        let req = ProjectRequest::React { common: common("web", d.path()), typescript: true };
        let (location, target) = validate::request(&req).unwrap();
        let (rep, _) = collector();
        create(&req, &location, &target, &rep).await.unwrap();
        assert!(target.join("package.json").is_file());
    }

    #[tokio::test]
    #[ignore = "requiere cargo"]
    async fn rust_with_real_cargo() {
        let d = tempfile::tempdir().unwrap();
        let req = ProjectRequest::Rust { common: common("rs-app", d.path()), bin: true };
        let (location, target) = validate::request(&req).unwrap();
        let (rep, _) = collector();
        create(&req, &location, &target, &rep).await.unwrap();
        assert!(target.join("Cargo.toml").is_file() && target.join("src/main.rs").is_file());
    }

    #[tokio::test]
    #[ignore = "requiere red (code.quarkus.io / launch.micronaut.io)"]
    async fn quarkus_online() {
        let d = tempfile::tempdir().unwrap();
        let req = ProjectRequest::Quarkus { common: common("qk", d.path()), jvm: jvm(BuildSystem::Maven), extensions: vec![] };
        let (location, target) = validate::request(&req).unwrap();
        let (rep, _) = collector();
        create(&req, &location, &target, &rep).await.unwrap();
        assert!(target.join("pom.xml").is_file());
    }
}
