use std::sync::{Arc, Mutex};

use super::catalog::FALLBACK_BOOT;
use super::generate::{app_yml, Plan};
use super::*;
use crate::projects::{BuildSystem, GenEvent, InfraSpec, MicroserviceSpec, Reporter, SpringRequest};

fn req(build: BuildSystem) -> SpringRequest {
    let svc = |n: &str, p: u16, d: &[&str]| MicroserviceSpec {
        name: n.into(),
        port: p,
        dependencies: d.iter().map(|s| s.to_string()).collect(),
        layers: true,
    };
    SpringRequest {
        group_id: "com.example".into(),
        base_package: "com.example.shop".into(),
        java_version: 21,
        boot_version: None,
        build,
        services: vec![svc("orders-service", 8081, &["web", "data-jpa", "postgresql", "lombok"]), svc("users-service", 8082, &["web", "security"])],
        gateway: Some(InfraSpec { port: 8080 }),
        discovery: Some(InfraSpec { port: 8761 }),
        config_server: None,
        docker_compose: true,
        dockerfiles: true,
        prefer_online: false,
        single: false,
    }
}

fn rep() -> (Reporter, Arc<Mutex<Vec<GenEvent>>>) {
    let ev = Arc::new(Mutex::new(Vec::new()));
    let e2 = ev.clone();
    (Reporter::detached(Arc::new(move |g| { e2.lock().unwrap().push(g); true })), ev)
}

fn read(p: &std::path::Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[tokio::test]
async fn offline_maven_multi_module() {
    let d = tempfile::tempdir().unwrap();
    let (r, _) = rep();
    let n = generate(&req(BuildSystem::Maven), "shop", FALLBACK_BOOT, d.path(), false, &r).await.unwrap();
    assert_eq!(n, 4, "orders, users, gateway, eureka");
    let t = d.path();

    let parent = read(&t.join("pom.xml"));
    assert!(parent.contains("<packaging>pom</packaging>"));
    for m in ["eureka-server", "orders-service", "users-service", "api-gateway"] {
        assert!(parent.contains(&format!("<module>{m}</module>")), "{m}");
    }

    let pom = read(&t.join("orders-service/pom.xml"));
    assert!(pom.contains("<artifactId>orders-service</artifactId>") && pom.contains("<java.version>21</java.version>"));
    assert!(pom.contains("spring-boot-starter-data-jpa") && pom.contains("<artifactId>postgresql</artifactId>"));
    assert!(pom.contains("<artifactId>lombok</artifactId>") && pom.contains("annotationProcessorPaths"));
    assert!(pom.contains("spring-cloud-starter-netflix-eureka-client"), "cliente Eureka añadido al haber discovery");
    assert!(pom.contains("<spring-cloud.version>2024.0.1</spring-cloud.version>"));

    let yml = read(&t.join("orders-service/src/main/resources/application.yml"));
    assert!(yml.contains("port: 8081") && yml.contains("name: orders-service"));
    assert!(yml.contains("jdbc:postgresql://localhost:5432/orders_service_db"));
    assert!(yml.contains("defaultZone: http://localhost:8761/eureka/"));
    assert!(!t.join("orders-service/src/main/resources/application.properties").exists());

    let base = t.join("orders-service/src/main/java/com/example/shop/ordersservice");
    let app = read(&base.join("OrdersServiceApplication.java"));
    assert!(app.contains("package com.example.shop.ordersservice;") && app.contains("@EnableDiscoveryClient"));
    for f in ["controller/OrdersServiceController.java", "service/OrdersServiceService.java", "repository/OrdersServiceRepository.java", "model/OrdersServiceEntity.java"] {
        assert!(base.join(f).is_file(), "{f}");
    }
    assert!(read(&base.join("controller/OrdersServiceController.java")).contains("/api/orders-service"));
    // users no tiene JPA: sin repository ni model
    let ub = t.join("users-service/src/main/java/com/example/shop/usersservice");
    assert!(ub.join("controller/UsersServiceController.java").is_file() && !ub.join("repository").exists());

    let gw = read(&t.join("api-gateway/src/main/resources/application.yml"));
    assert!(gw.contains("port: 8080") && gw.contains("lb://orders-service") && gw.contains("Path=/api/users-service/**"));
    let eu = read(&t.join("eureka-server/src/main/java/com/example/shop/eurekaserver/EurekaServerApplication.java"));
    assert!(eu.contains("@EnableEurekaServer"));
    assert!(read(&t.join("eureka-server/src/main/resources/application.yml")).contains("register-with-eureka: false"));

    let compose = read(&t.join("docker-compose.yml"));
    assert!(compose.contains("postgres:16") && compose.contains("5432:5432") && !compose.contains("redis"));
    assert!(read(&t.join("docker/postgres/init.sql")).contains("CREATE DATABASE orders_service_db;"));
    assert!(read(&t.join("orders-service/Dockerfile")).contains("maven:3-eclipse-temurin-21") && read(&t.join("orders-service/Dockerfile")).contains("EXPOSE 8081"));
    assert!(t.join(".gitignore").is_file() && read(&t.join("README.md")).contains("| `orders-service` | Microservicio | 8081 |"));

    let rc = crate::run::store::load(t).unwrap();
    assert_eq!(rc.configs.len(), 4);
    assert_eq!(rc.configs.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["eureka-server", "orders-service", "users-service", "api-gateway"]);
    let o = rc.configs.iter().find(|c| c.id == "orders-service").unwrap();
    assert_eq!((o.cwd.as_str(), o.command.as_str(), o.port), ("orders-service", "mvn spring-boot:run", Some(8081)));
}

#[tokio::test]
async fn offline_gradle_kts_multi_module() {
    let d = tempfile::tempdir().unwrap();
    let (r, _) = rep();
    let mut rq = req(BuildSystem::GradleKotlin);
    rq.docker_compose = false;
    let n = generate(&rq, "shop", FALLBACK_BOOT, d.path(), false, &r).await.unwrap();
    assert_eq!(n, 4);
    let t = d.path();
    let settings = read(&t.join("settings.gradle.kts"));
    assert!(settings.contains("include(\"eureka-server\", \"orders-service\", \"users-service\", \"api-gateway\")"));
    let b = read(&t.join("orders-service/build.gradle.kts"));
    assert!(b.contains("id(\"org.springframework.boot\") version \"3.4.5\"") && b.contains("runtimeOnly(\"org.postgresql:postgresql\")"));
    assert!(b.contains("annotationProcessor(\"org.projectlombok:lombok\")") && b.contains("mavenBom("));
    assert!(read(&t.join("orders-service/Dockerfile")).contains("gradle bootJar"));
    let rc = crate::run::store::load(t).unwrap();
    assert_eq!(rc.configs.iter().find(|c| c.id == "users-service").unwrap().command, "gradle bootRun");
    assert!(!t.join("docker-compose.yml").exists());
}

#[tokio::test]
async fn offline_gradle_groovy_and_single() {
    let d = tempfile::tempdir().unwrap();
    let (r, _) = rep();
    let mut rq = req(BuildSystem::GradleGroovy);
    rq.services.truncate(1);
    rq.gateway = None;
    rq.discovery = None;
    rq.single = true;
    rq.docker_compose = false;
    let n = generate(&rq, "orders-service", FALLBACK_BOOT, d.path(), false, &r).await.unwrap();
    assert_eq!(n, 1);
    let t = d.path();
    assert!(read(&t.join("build.gradle")).contains("id 'org.springframework.boot' version '3.4.5'"));
    assert!(t.join("src/main/java/com/example/shop/ordersservice/OrdersServiceApplication.java").is_file());
    assert!(!t.join("pom.xml").exists() && !t.join("orders-service").exists());
    let rc = crate::run::store::load(t).unwrap();
    assert_eq!(rc.configs[0].cwd, "");
}

#[test]
fn config_server_and_unsupported_deps() {
    let mut rq = req(BuildSystem::Maven);
    rq.config_server = Some(InfraSpec { port: 8888 });
    rq.services[0].dependencies.push("nuevo-desconocido".into());
    let plan = Plan::new(&rq, "shop", FALLBACK_BOOT);
    assert_eq!(plan.modules[0].name, "config-server", "config primero");
    let svc = plan.modules.iter().find(|m| m.name == "orders-service").unwrap();
    assert!(svc.has("cloud-config-client") && svc.has("cloud-eureka") && svc.has("actuator"));
    assert!(app_yml(&plan, svc).contains("import: optional:configserver:http://localhost:8888"));
    let (_, missing) = offline::resolve(svc);
    assert_eq!(missing, ["nuevo-desconocido"]);
    let cfg = plan.modules.iter().find(|m| m.name == "config-server").unwrap();
    assert!(app_yml(&plan, cfg).contains("search-locations: classpath:/config"));
    let gw = plan.modules.last().unwrap();
    assert_eq!(gw.name, "api-gateway");
}

#[test]
fn gateway_routes_without_eureka_and_newer_boot_prefix() {
    let mut rq = req(BuildSystem::Maven);
    rq.discovery = None;
    let plan = Plan::new(&rq, "shop", "3.4.5");
    let gw = plan.modules.last().unwrap();
    let y = app_yml(&plan, gw);
    assert!(y.contains("uri: http://localhost:8081") && y.contains("      routes:"));
    let plan = Plan::new(&rq, "shop", "3.5.7");
    let y = app_yml(&plan, plan.modules.last().unwrap());
    assert!(y.contains("webflux:") && y.contains("uri: http://localhost:8082"));
    assert_eq!(plan.cloud, "2025.0.0");
}

#[test]
fn compose_for_every_infra_dependency() {
    let mut rq = req(BuildSystem::Maven);
    rq.services[1].dependencies = vec!["web".into(), "mysql".into(), "data-redis".into(), "kafka".into(), "amqp".into(), "data-mongodb".into()];
    let plan = Plan::new(&rq, "shop", FALLBACK_BOOT);
    let d = tempfile::tempdir().unwrap();
    let mut w = Vec::new();
    generate::write_root(&plan, d.path(), &mut w).unwrap();
    let c = read(&d.path().join("docker-compose.yml"));
    for needle in ["postgres:16", "mysql:8.4", "redis:7", "apache/kafka", "rabbitmq:3-management", "mongo:7"] {
        assert!(c.contains(needle), "{needle}");
    }
    assert!(read(&d.path().join("docker/mysql/init.sql")).contains("users_service_db"));
}

#[test]
fn compose_without_infra_warns() {
    let mut rq = req(BuildSystem::Maven);
    rq.services[0].dependencies = vec!["web".into()];
    let plan = Plan::new(&rq, "shop", FALLBACK_BOOT);
    let d = tempfile::tempdir().unwrap();
    let mut w = Vec::new();
    generate::write_root(&plan, d.path(), &mut w).unwrap();
    assert!(!d.path().join("docker-compose.yml").exists());
    assert_eq!(w.len(), 1);
}

#[tokio::test]
#[ignore = "red real (start.spring.io)"]
async fn online_generation_with_real_start_spring_io() {
    let d = tempfile::tempdir().unwrap();
    let (r, ev) = rep();
    let cat = fetch_catalog().await.unwrap();
    let mut rq = req(BuildSystem::Maven);
    rq.docker_compose = false;
    generate(&rq, "shop", &cat.default_boot, d.path(), true, &r).await.unwrap();
    assert!(d.path().join("orders-service/pom.xml").is_file());
    let warnings = ev.lock().unwrap().iter().filter(|e| matches!(e, GenEvent::Warning { .. })).count();
    assert_eq!(warnings, 0, "todos los módulos deben venir de start.spring.io");
}
