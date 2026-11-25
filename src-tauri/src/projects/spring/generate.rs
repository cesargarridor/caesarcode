//! Plan de microservicios y post-proceso común (online u offline): application.yml, capas,
//! clase main, Dockerfile, agregador multi-módulo, docker-compose, README y run-configs.

use std::path::{Path, PathBuf};

use super::catalog::{self, major_minor};
use crate::error::AppResult;
use crate::projects::local;
use crate::projects::template::{self, Vars};
use crate::projects::{BuildSystem, SpringRequest};
use crate::run::{self, RunConfig, RunConfigFile, RunKind};

pub const GATEWAY: &str = "api-gateway";
pub const EUREKA: &str = "eureka-server";
pub const CONFIG: &str = "config-server";

pub const DEV_USER: &str = "caesar";
pub const DEV_PASSWORD: &str = "caesar";

/// `orders-service` → `ordersservice` (como start.spring.io).
pub fn pkg_segment(name: &str) -> String {
    name.replace('-', "")
}

/// `orders-service` → `OrdersService`.
pub fn pascal(name: &str) -> String {
    name.split('-')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Service,
    Gateway,
    Discovery,
    Config,
}

#[derive(Clone, Debug)]
pub struct Module {
    pub name: String,
    pub role: Role,
    pub port: u16,
    pub deps: Vec<String>,
    pub layers: bool,
    pub package: String,
}

impl Module {
    pub fn has(&self, id: &str) -> bool {
        self.deps.iter().any(|d| d == id)
    }
    pub fn class(&self) -> String {
        pascal(&self.name)
    }
    pub fn app_class(&self) -> String {
        format!("{}Application", self.class())
    }
    pub fn package_path(&self) -> String {
        self.package.replace('.', "/")
    }
    pub fn db_name(&self) -> String {
        format!("{}_db", self.name.replace('-', "_"))
    }
    /// Necesita infraestructura externa para arrancar (sus tests de contexto fallarían sin ella).
    pub fn needs_infra(&self) -> bool {
        ["data-jpa", "postgresql", "mysql", "data-mongodb", "data-redis", "kafka", "amqp"].iter().any(|d| self.has(d))
    }
}

pub struct Plan {
    pub req: SpringRequest,
    /// Nombre del proyecto (carpeta raíz).
    pub project: String,
    pub boot: String,
    pub cloud: String,
    /// Orden: config → discovery → servicios → gateway.
    pub modules: Vec<Module>,
}

pub fn cloud_for(boot: &str) -> &'static str {
    match major_minor(boot) {
        (3, 2) | (3, 3) => "2023.0.5",
        (3, 5) => "2025.0.0",
        (4, _) => "2025.1.0",
        _ => catalog::FALLBACK_CLOUD,
    }
}

fn push_unique(v: &mut Vec<String>, id: &str) {
    if !v.iter().any(|d| d == id) {
        v.push(id.to_string());
    }
}

impl Plan {
    pub fn new(req: &SpringRequest, project: &str, boot: &str) -> Plan {
        let discovery = req.discovery.is_some();
        let config = req.config_server.is_some();
        let seg = |n: &str| format!("{}.{}", req.base_package, pkg_segment(n));
        let mut modules = Vec::new();
        let client_deps = |deps: &mut Vec<String>| {
            if discovery {
                push_unique(deps, "cloud-eureka");
            }
            if config {
                push_unique(deps, "cloud-config-client");
            }
        };
        if let Some(i) = &req.config_server {
            let mut deps = vec!["cloud-config-server".to_string(), "actuator".to_string()];
            if discovery {
                push_unique(&mut deps, "cloud-eureka");
            }
            modules.push(Module { name: CONFIG.into(), role: Role::Config, port: i.port, deps, layers: false, package: seg(CONFIG) });
        }
        if let Some(i) = &req.discovery {
            let mut deps = vec!["cloud-eureka-server".to_string(), "actuator".to_string()];
            if config {
                push_unique(&mut deps, "cloud-config-client");
            }
            modules.push(Module { name: EUREKA.into(), role: Role::Discovery, port: i.port, deps, layers: false, package: seg(EUREKA) });
        }
        for s in &req.services {
            let mut deps: Vec<String> = Vec::new();
            for d in &s.dependencies {
                push_unique(&mut deps, d);
            }
            if !deps.iter().any(|d| d == "web" || d == "webflux") {
                deps.insert(0, "web".into());
            }
            push_unique(&mut deps, "actuator");
            client_deps(&mut deps);
            if deps.iter().any(|d| d == "data-jpa") && !deps.iter().any(|d| matches!(d.as_str(), "postgresql" | "mysql" | "h2")) {
                deps.push("h2".into());
            }
            modules.push(Module { name: s.name.clone(), role: Role::Service, port: s.port, deps, layers: s.layers, package: seg(&s.name) });
        }
        if let Some(i) = &req.gateway {
            let mut deps = vec!["cloud-gateway".to_string(), "actuator".to_string()];
            client_deps(&mut deps);
            modules.push(Module { name: GATEWAY.into(), role: Role::Gateway, port: i.port, deps, layers: false, package: seg(GATEWAY) });
        }
        Plan { req: req.clone(), project: project.to_string(), boot: boot.to_string(), cloud: cloud_for(boot).to_string(), modules }
    }

    pub fn dir_of(&self, m: &Module, target: &Path) -> PathBuf {
        if self.req.single {
            target.to_path_buf()
        } else {
            target.join(&m.name)
        }
    }

    pub fn has_discovery(&self) -> bool {
        self.req.discovery.is_some()
    }
    pub fn has_config(&self) -> bool {
        self.req.config_server.is_some()
    }
    pub fn uses_any(&self, id: &str) -> bool {
        self.modules.iter().any(|m| m.has(id))
    }
}

// ---------------------------------------------------------------- application.yml

pub fn app_yml(plan: &Plan, m: &Module) -> String {
    let mut y = format!("server:\n  port: {}\n\nspring:\n  application:\n    name: {}\n", m.port, m.name);
    let discovery_client = plan.has_discovery() && m.has("cloud-eureka");
    if plan.has_config() && m.role != Role::Config {
        y.push_str("  config:\n    import: optional:configserver:http://localhost:8888\n");
    }
    if m.role == Role::Config {
        y.push_str("  profiles:\n    active: native\n  cloud:\n    config:\n      server:\n        native:\n          search-locations: classpath:/config\n");
    }
    if m.has("postgresql") {
        y.push_str(&format!(
            "  datasource:\n    url: jdbc:postgresql://localhost:5432/{}\n    username: {DEV_USER}\n    password: {DEV_PASSWORD}\n",
            m.db_name()
        ));
    } else if m.has("mysql") {
        y.push_str(&format!(
            "  datasource:\n    url: jdbc:mysql://localhost:3306/{}\n    username: {DEV_USER}\n    password: {DEV_PASSWORD}\n",
            m.db_name()
        ));
    }
    if m.has("data-jpa") {
        y.push_str("  jpa:\n    hibernate:\n      ddl-auto: update\n    open-in-view: false\n");
    }
    let mut data = String::new();
    if m.has("data-mongodb") {
        data.push_str(&format!("    mongodb:\n      uri: mongodb://localhost:27017/{}\n", m.db_name()));
    }
    if m.has("data-redis") {
        data.push_str("    redis:\n      host: localhost\n      port: 6379\n");
    }
    if !data.is_empty() {
        y.push_str("  data:\n");
        y.push_str(&data);
    }
    if m.has("kafka") {
        y.push_str("  kafka:\n    bootstrap-servers: localhost:9092\n");
    }
    if m.has("amqp") {
        y.push_str("  rabbitmq:\n    host: localhost\n    port: 5672\n    username: guest\n    password: guest\n");
    }
    if m.role == Role::Gateway {
        // Spring Cloud 2025.0 (Boot 3.5+) movió las propiedades a `server.webflux`.
        let nested = major_minor(&plan.boot) >= (3, 5);
        let (indent, head) = if nested {
            ("          ", "  cloud:\n    gateway:\n      server:\n        webflux:\n          routes:\n")
        } else {
            ("        ", "  cloud:\n    gateway:\n      routes:\n")
        };
        y.push_str(head);
        for s in plan.modules.iter().filter(|s| s.role == Role::Service) {
            let uri = if plan.has_discovery() { format!("lb://{}", s.name) } else { format!("http://localhost:{}", s.port) };
            y.push_str(&format!(
                "{indent}- id: {n}\n{indent}  uri: {uri}\n{indent}  predicates:\n{indent}    - Path=/api/{n}/**\n",
                n = s.name
            ));
        }
    }
    if discovery_client {
        y.push_str("\neureka:\n  client:\n    service-url:\n      defaultZone: http://localhost:8761/eureka/\n");
    }
    if m.role == Role::Discovery {
        y.push_str("\neureka:\n  instance:\n    hostname: localhost\n  client:\n    register-with-eureka: false\n    fetch-registry: false\n");
    }
    if m.has("actuator") {
        y.push_str("\nmanagement:\n  endpoints:\n    web:\n      exposure:\n        include: health,info\n");
    }
    y
}

// ---------------------------------------------------------------- Java

pub fn main_class(m: &Module) -> String {
    let mut imports = vec![
        "org.springframework.boot.SpringApplication".to_string(),
        "org.springframework.boot.autoconfigure.SpringBootApplication".to_string(),
    ];
    let mut annots: Vec<&str> = Vec::new();
    if m.role == Role::Discovery {
        imports.push("org.springframework.cloud.netflix.eureka.server.EnableEurekaServer".into());
        annots.push("@EnableEurekaServer");
    }
    if m.role == Role::Config {
        imports.push("org.springframework.cloud.config.server.EnableConfigServer".into());
        annots.push("@EnableConfigServer");
    }
    if m.has("cloud-eureka") {
        imports.push("org.springframework.cloud.client.discovery.EnableDiscoveryClient".into());
        annots.push("@EnableDiscoveryClient");
    }
    if m.has("cloud-feign") {
        imports.push("org.springframework.cloud.openfeign.EnableFeignClients".into());
        annots.push("@EnableFeignClients");
    }
    imports.sort();
    let mut s = format!("package {};\n\n", m.package);
    for i in &imports {
        s.push_str(&format!("import {i};\n"));
    }
    s.push_str("\n@SpringBootApplication\n");
    for a in annots {
        s.push_str(a);
        s.push('\n');
    }
    let c = m.app_class();
    s.push_str(&format!("public class {c} {{\n\n    public static void main(String[] args) {{\n        SpringApplication.run({c}.class, args);\n    }}\n}}\n"));
    s
}

fn layer_files(m: &Module) -> Vec<(String, String)> {
    let (pkg, cls, name) = (&m.package, m.class(), &m.name);
    let jpa = m.has("data-jpa");
    let base = m.package_path();
    let mut out = Vec::new();
    let repo_import = if jpa { format!("import {pkg}.repository.{cls}Repository;\n") } else { String::new() };
    let repo_field = if jpa { format!("    private final {cls}Repository repository;\n\n    public {cls}Service({cls}Repository repository) {{\n        this.repository = repository;\n    }}\n\n    public long count() {{\n        return repository.count();\n    }}\n\n") } else { String::new() };
    out.push((
        format!("src/main/java/{base}/service/{cls}Service.java"),
        format!(
            "package {pkg}.service;\n\n{repo_import}import org.springframework.stereotype.Service;\n\nimport java.util.Map;\n\n@Service\npublic class {cls}Service {{\n\n{repo_field}    public Map<String, String> health() {{\n        return Map.of(\"service\", \"{name}\", \"status\", \"UP\");\n    }}\n}}\n"
        ),
    ));
    let count_endpoint = if jpa { "\n    @GetMapping(\"/count\")\n    public long count() {\n        return service.count();\n    }\n".to_string() } else { String::new() };
    out.push((
        format!("src/main/java/{base}/controller/{cls}Controller.java"),
        format!(
            "package {pkg}.controller;\n\nimport {pkg}.service.{cls}Service;\nimport org.springframework.web.bind.annotation.GetMapping;\nimport org.springframework.web.bind.annotation.RequestMapping;\nimport org.springframework.web.bind.annotation.RestController;\n\nimport java.util.Map;\n\n@RestController\n@RequestMapping(\"/api/{name}\")\npublic class {cls}Controller {{\n\n    private final {cls}Service service;\n\n    public {cls}Controller({cls}Service service) {{\n        this.service = service;\n    }}\n\n    @GetMapping(\"/health\")\n    public Map<String, String> health() {{\n        return service.health();\n    }}\n{count_endpoint}}}\n"
        ),
    ));
    if jpa {
        out.push((
            format!("src/main/java/{base}/model/{cls}Entity.java"),
            format!(
                "package {pkg}.model;\n\nimport jakarta.persistence.Entity;\nimport jakarta.persistence.GeneratedValue;\nimport jakarta.persistence.GenerationType;\nimport jakarta.persistence.Id;\n\n@Entity\npublic class {cls}Entity {{\n\n    @Id\n    @GeneratedValue(strategy = GenerationType.IDENTITY)\n    private Long id;\n\n    private String name;\n\n    public Long getId() {{\n        return id;\n    }}\n\n    public void setId(Long id) {{\n        this.id = id;\n    }}\n\n    public String getName() {{\n        return name;\n    }}\n\n    public void setName(String name) {{\n        this.name = name;\n    }}\n}}\n"
            ),
        ));
        out.push((
            format!("src/main/java/{base}/repository/{cls}Repository.java"),
            format!(
                "package {pkg}.repository;\n\nimport {pkg}.model.{cls}Entity;\nimport org.springframework.data.jpa.repository.JpaRepository;\n\npublic interface {cls}Repository extends JpaRepository<{cls}Entity, Long> {{\n}}\n"
            ),
        ));
    }
    out
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_files(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}

/// Deja el módulo listo: limpia lo genérico de start.spring.io y escribe nuestras piezas.
pub fn finish_module(plan: &Plan, m: &Module, dir: &Path) -> AppResult<()> {
    let _ = std::fs::remove_file(dir.join("src/main/resources/application.properties"));
    let mut files = Vec::new();
    collect_files(&dir.join("src/main/java"), &mut files);
    for f in files {
        if f.file_name().is_some_and(|n| n.to_string_lossy().ends_with("Application.java")) {
            let _ = std::fs::remove_file(f);
        }
    }
    if m.needs_infra() {
        // Los tests de contexto generados necesitan la infraestructura levantada: se omiten.
        let mut tests = Vec::new();
        collect_files(&dir.join("src/test/java"), &mut tests);
        for f in tests {
            if f.file_name().is_some_and(|n| n.to_string_lossy().ends_with("ApplicationTests.java")) {
                let _ = std::fs::remove_file(f);
            }
        }
    }
    template::write_text(dir, &format!("src/main/java/{}/{}.java", m.package_path(), m.app_class()), &main_class(m))?;
    template::write_text(dir, "src/main/resources/application.yml", &app_yml(plan, m))?;
    if m.role == Role::Config {
        template::write_text(
            dir,
            "src/main/resources/config/application.yml",
            "# Configuración compartida por todos los servicios (profile native).\n# Añade aquí propiedades comunes o archivos <servicio>.yml.\n",
        )?;
    }
    if m.role == Role::Service && m.layers {
        for (rel, text) in layer_files(m) {
            template::write_text(dir, &rel, &text)?;
        }
    }
    if plan.req.dockerfiles {
        let mut v = Vars::new();
        v.insert("java_version", plan.req.java_version.to_string());
        v.insert("port", m.port.to_string());
        let tpl = if plan.req.build == BuildSystem::Maven { local::DOCKERFILE_MAVEN } else { local::DOCKERFILE_GRADLE };
        template::write_text(dir, "Dockerfile", &template::render(tpl, &v)?)?;
        template::write_text(dir, ".dockerignore", "target/\nbuild/\n.gradle/\n.idea/\n*.iml\n")?;
    }
    Ok(())
}

// ---------------------------------------------------------------- raíz del proyecto

pub fn aggregator_pom(plan: &Plan) -> String {
    let artifact = local_artifact(&plan.project);
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<project xmlns=\"http://maven.apache.org/POM/4.0.0\"\n         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\n         xsi:schemaLocation=\"http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd\">\n    <modelVersion>4.0.0</modelVersion>\n\n");
    s.push_str(&format!(
        "    <groupId>{}</groupId>\n    <artifactId>{artifact}</artifactId>\n    <version>0.0.1-SNAPSHOT</version>\n    <packaging>pom</packaging>\n    <name>{}</name>\n\n    <modules>\n",
        plan.req.group_id, plan.project
    ));
    for m in &plan.modules {
        s.push_str(&format!("        <module>{}</module>\n", m.name));
    }
    s.push_str("    </modules>\n</project>\n");
    s
}

fn local_artifact(name: &str) -> String {
    let a: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    a.trim_matches('-').to_string()
}

pub fn gradle_settings(plan: &Plan, kts: bool) -> String {
    let names: Vec<String> = plan.modules.iter().map(|m| if kts { format!("\"{}\"", m.name) } else { format!("'{}'", m.name) }).collect();
    if kts {
        format!("rootProject.name = \"{}\"\n\ninclude({})\n", local_artifact(&plan.project), names.join(", "))
    } else {
        format!("rootProject.name = '{}'\n\ninclude {}\n", local_artifact(&plan.project), names.join(", "))
    }
}

struct Infra {
    compose: String,
    files: Vec<(&'static str, String)>,
}

fn compose(plan: &Plan) -> Option<Infra> {
    let mut services = String::new();
    let mut volumes: Vec<&str> = Vec::new();
    let mut files: Vec<(&'static str, String)> = Vec::new();
    let pg: Vec<&Module> = plan.modules.iter().filter(|m| m.has("postgresql")).collect();
    let my: Vec<&Module> = plan.modules.iter().filter(|m| m.has("mysql")).collect();
    if !pg.is_empty() {
        services.push_str(&format!("  postgres:\n    image: postgres:16-alpine\n    environment:\n      POSTGRES_USER: {DEV_USER}\n      POSTGRES_PASSWORD: {DEV_PASSWORD}\n      POSTGRES_DB: postgres\n    ports:\n      - \"5432:5432\"\n    volumes:\n      - pgdata:/var/lib/postgresql/data\n      - ./docker/postgres/init.sql:/docker-entrypoint-initdb.d/init.sql:ro\n\n"));
        volumes.push("pgdata");
        files.push(("docker/postgres/init.sql", pg.iter().map(|m| format!("CREATE DATABASE {};\n", m.db_name())).collect()));
    }
    if !my.is_empty() {
        services.push_str(&format!("  mysql:\n    image: mysql:8.4\n    environment:\n      MYSQL_ROOT_PASSWORD: {DEV_PASSWORD}\n      MYSQL_USER: {DEV_USER}\n      MYSQL_PASSWORD: {DEV_PASSWORD}\n    ports:\n      - \"3306:3306\"\n    volumes:\n      - mysqldata:/var/lib/mysql\n      - ./docker/mysql/init.sql:/docker-entrypoint-initdb.d/init.sql:ro\n\n"));
        volumes.push("mysqldata");
        files.push((
            "docker/mysql/init.sql",
            my.iter()
                .map(|m| format!("CREATE DATABASE IF NOT EXISTS {db};\nGRANT ALL PRIVILEGES ON {db}.* TO '{DEV_USER}'@'%';\n", db = m.db_name()))
                .collect(),
        ));
    }
    if plan.uses_any("data-redis") {
        services.push_str("  redis:\n    image: redis:7-alpine\n    ports:\n      - \"6379:6379\"\n\n");
    }
    if plan.uses_any("data-mongodb") {
        services.push_str("  mongodb:\n    image: mongo:7\n    ports:\n      - \"27017:27017\"\n    volumes:\n      - mongodata:/data/db\n\n");
        volumes.push("mongodata");
    }
    if plan.uses_any("kafka") {
        services.push_str("  kafka:\n    image: apache/kafka:3.9.0\n    ports:\n      - \"9092:9092\"\n    environment:\n      KAFKA_NODE_ID: 1\n      KAFKA_PROCESS_ROLES: broker,controller\n      KAFKA_LISTENERS: PLAINTEXT://:9092,CONTROLLER://:9093\n      KAFKA_ADVERTISED_LISTENERS: PLAINTEXT://localhost:9092\n      KAFKA_CONTROLLER_LISTENER_NAMES: CONTROLLER\n      KAFKA_LISTENER_SECURITY_PROTOCOL_MAP: CONTROLLER:PLAINTEXT,PLAINTEXT:PLAINTEXT\n      KAFKA_CONTROLLER_QUORUM_VOTERS: 1@localhost:9093\n      KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR: 1\n      KAFKA_TRANSACTION_STATE_LOG_REPLICATION_FACTOR: 1\n      KAFKA_TRANSACTION_STATE_LOG_MIN_ISR: 1\n      KAFKA_GROUP_INITIAL_REBALANCE_DELAY_MS: 0\n\n");
    }
    if plan.uses_any("amqp") {
        services.push_str("  rabbitmq:\n    image: rabbitmq:3-management\n    ports:\n      - \"5672:5672\"\n      - \"15672:15672\"\n\n");
    }
    if services.is_empty() {
        return None;
    }
    let mut compose = format!("services:\n{}", services.trim_end());
    compose.push('\n');
    if !volumes.is_empty() {
        compose.push_str("\nvolumes:\n");
        for v in volumes {
            compose.push_str(&format!("  {v}:\n"));
        }
    }
    Some(Infra { compose, files })
}

pub fn readme(plan: &Plan, compose: bool) -> String {
    let maven = plan.req.build == BuildSystem::Maven;
    let mut s = format!("# {}\n\nProyecto de microservicios Spring Boot {} (Java {}) generado con CaesarCode.\n\n## Servicios\n\n| Módulo | Rol | Puerto |\n|---|---|---|\n", plan.project, plan.boot, plan.req.java_version);
    for m in &plan.modules {
        let role = match m.role {
            Role::Service => "Microservicio",
            Role::Gateway => "API Gateway",
            Role::Discovery => "Eureka Server",
            Role::Config => "Config Server",
        };
        s.push_str(&format!("| `{}` | {} | {} |\n", m.name, role, m.port));
    }
    s.push_str("\n## Orden de arranque sugerido\n\n");
    let mut n = 1;
    if compose {
        s.push_str("1. `docker compose up -d` (bases de datos y mensajería).\n");
        n += 1;
    }
    s.push_str(&format!("{n}. Config Server → Eureka Server → microservicios → API Gateway (los que existan).\n\nCada módulo tiene su configuración de ejecución en el widget Run (`.caesar/run-configs.json`).\n\n## Compilar\n\n"));
    if plan.req.single {
        s.push_str(if maven { "```\nmvn -DskipTests package\n```\n" } else { "```\ngradle build -x test\n```\n" });
    } else if maven {
        s.push_str("```\nmvn -DskipTests package\n```\n\nEl `pom.xml` de la raíz es solo un agregador de módulos.\n");
    } else {
        s.push_str("```\ngradle build -x test\n```\n");
    }
    if plan.req.dockerfiles {
        s.push_str("\n## Docker\n\nCada servicio incluye un `Dockerfile` multi-stage: `docker build -t <servicio> <carpeta>`.\n");
    }
    s.push_str(&format!("\nCredenciales de desarrollo de las bases de datos: usuario `{DEV_USER}` / contraseña `{DEV_PASSWORD}`.\n"));
    s
}

/// Archivos de la raíz: agregador, docker-compose, `.gitignore`, README. Devuelve si se escribió compose.
pub fn write_root(plan: &Plan, target: &Path, warn: &mut Vec<String>) -> AppResult<()> {
    if !plan.req.single {
        match plan.req.build {
            BuildSystem::Maven => template::write_text(target, "pom.xml", &aggregator_pom(plan))?,
            BuildSystem::GradleKotlin => template::write_text(target, "settings.gradle.kts", &gradle_settings(plan, true))?,
            _ => template::write_text(target, "settings.gradle", &gradle_settings(plan, false))?,
        }
    }
    let mut wrote_compose = false;
    if plan.req.docker_compose {
        match compose(plan) {
            Some(infra) => {
                template::write_text(target, "docker-compose.yml", &infra.compose)?;
                for (rel, text) in infra.files {
                    template::write_text(target, rel, &text)?;
                }
                wrote_compose = true;
            }
            None => warn.push("docker-compose omitido: ningún servicio usa bases de datos ni mensajería".into()),
        }
    }
    template::write_text(target, ".gitignore", &format!("{}.env\n", local::GITIGNORE_JAVA))?;
    template::write_text(target, "README.md", &readme(plan, wrote_compose))?;
    Ok(())
}

pub fn run_configs(plan: &Plan, target: &Path) -> Vec<RunConfig> {
    let maven = plan.req.build == BuildSystem::Maven;
    plan.modules
        .iter()
        .map(|m| {
            let dir = plan.dir_of(m, target);
            let command = if maven {
                if dir.join("mvnw").is_file() {
                    if cfg!(windows) { "mvnw spring-boot:run" } else { "./mvnw spring-boot:run" }
                } else {
                    "mvn spring-boot:run"
                }
            } else if dir.join("gradlew").is_file() {
                if cfg!(windows) { "gradlew bootRun" } else { "./gradlew bootRun" }
            } else {
                "gradle bootRun"
            };
            RunConfig {
                id: m.name.clone(),
                name: m.name.clone(),
                kind: RunKind::SpringBoot,
                cwd: if plan.req.single { String::new() } else { m.name.clone() },
                command: command.to_string(),
                args: String::new(),
                env: Default::default(),
                env_file: None,
                port: Some(m.port),
                show_in_services: true,
            }
        })
        .collect()
}

pub fn save_run_configs(target: &Path, configs: Vec<RunConfig>) -> AppResult<usize> {
    let n = configs.len();
    run::store::save(target, &RunConfigFile { configs, ..Default::default() })?;
    Ok(n)
}
