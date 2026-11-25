//! Catálogo de dependencias Spring: versiones de respaldo, mapeo id de start.spring.io → coordenadas
//! (plantilla offline) y parseo del catálogo online (`/metadata/client`).

use serde_json::Value;

use crate::projects::{SpringCatalog, SpringDep, SpringDepGroup};

/// Spring Boot de la plantilla offline. Debe ser compatible con `FALLBACK_CLOUD`
/// (Spring Cloud 2024.0.x ↔ Boot 3.4.x). Cambiar ambas a la vez.
pub const FALLBACK_BOOT: &str = "3.4.5";
/// Release train de Spring Cloud compatible con `FALLBACK_BOOT`.
pub const FALLBACK_CLOUD: &str = "2024.0.1";
pub const JAVA_VERSIONS: &[u32] = &[17, 21, 25];
/// BOM de Quarkus y Micronaut para sus plantillas offline mínimas.
pub const QUARKUS_FALLBACK: &str = "3.17.4";
pub const MICRONAUT_FALLBACK: &str = "4.7.6";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Compile,
    Runtime,
    /// `optional` + annotation processor.
    Lombok,
    /// `runtime` + `optional` (Maven) / `developmentOnly` (Gradle).
    DevTools,
}

#[derive(Clone, Copy, Debug)]
pub struct Coord {
    pub group: &'static str,
    pub artifact: &'static str,
    pub scope: Scope,
}

const fn c(group: &'static str, artifact: &'static str, scope: Scope) -> Coord {
    Coord { group, artifact, scope }
}

const BOOT: &str = "org.springframework.boot";
const CLOUD: &str = "org.springframework.cloud";

pub struct DepDef {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub group: &'static str,
    pub coords: &'static [Coord],
    /// Requiere el BOM de Spring Cloud.
    pub cloud: bool,
}

pub static DEPS: &[DepDef] = &[
    DepDef { id: "devtools", name: "Spring Boot DevTools", description: "Reinicio rápido y LiveReload.", group: "Developer Tools", coords: &[c(BOOT, "spring-boot-devtools", Scope::DevTools)], cloud: false },
    DepDef { id: "lombok", name: "Lombok", description: "Reduce el código repetitivo con anotaciones.", group: "Developer Tools", coords: &[c("org.projectlombok", "lombok", Scope::Lombok)], cloud: false },
    DepDef { id: "web", name: "Spring Web", description: "API REST con Spring MVC y Tomcat.", group: "Web", coords: &[c(BOOT, "spring-boot-starter-web", Scope::Compile)], cloud: false },
    DepDef { id: "webflux", name: "Spring Reactive Web", description: "Web reactiva con Spring WebFlux y Netty.", group: "Web", coords: &[c(BOOT, "spring-boot-starter-webflux", Scope::Compile)], cloud: false },
    DepDef { id: "security", name: "Spring Security", description: "Autenticación y autorización.", group: "Security", coords: &[c(BOOT, "spring-boot-starter-security", Scope::Compile)], cloud: false },
    DepDef { id: "validation", name: "Validation", description: "Bean Validation con Hibernate Validator.", group: "I/O", coords: &[c(BOOT, "spring-boot-starter-validation", Scope::Compile)], cloud: false },
    DepDef { id: "data-jpa", name: "Spring Data JPA", description: "Persistencia SQL con Hibernate.", group: "SQL", coords: &[c(BOOT, "spring-boot-starter-data-jpa", Scope::Compile)], cloud: false },
    DepDef { id: "postgresql", name: "PostgreSQL Driver", description: "Driver JDBC de PostgreSQL.", group: "SQL", coords: &[c("org.postgresql", "postgresql", Scope::Runtime)], cloud: false },
    DepDef { id: "mysql", name: "MySQL Driver", description: "Driver JDBC de MySQL.", group: "SQL", coords: &[c("com.mysql", "mysql-connector-j", Scope::Runtime)], cloud: false },
    DepDef { id: "h2", name: "H2 Database", description: "Base de datos SQL en memoria.", group: "SQL", coords: &[c("com.h2database", "h2", Scope::Runtime)], cloud: false },
    DepDef { id: "data-mongodb", name: "Spring Data MongoDB", description: "Base de datos documental MongoDB.", group: "NoSQL", coords: &[c(BOOT, "spring-boot-starter-data-mongodb", Scope::Compile)], cloud: false },
    DepDef { id: "data-redis", name: "Spring Data Redis", description: "Almacén clave-valor Redis.", group: "NoSQL", coords: &[c(BOOT, "spring-boot-starter-data-redis", Scope::Compile)], cloud: false },
    DepDef { id: "kafka", name: "Spring for Apache Kafka", description: "Mensajería con Apache Kafka.", group: "Messaging", coords: &[c("org.springframework.kafka", "spring-kafka", Scope::Compile)], cloud: false },
    DepDef { id: "amqp", name: "Spring for RabbitMQ", description: "Mensajería AMQP con RabbitMQ.", group: "Messaging", coords: &[c(BOOT, "spring-boot-starter-amqp", Scope::Compile)], cloud: false },
    DepDef { id: "actuator", name: "Spring Boot Actuator", description: "Endpoints de salud y métricas.", group: "Ops", coords: &[c(BOOT, "spring-boot-starter-actuator", Scope::Compile)], cloud: false },
    DepDef { id: "cloud-feign", name: "OpenFeign", description: "Clientes HTTP declarativos.", group: "Spring Cloud", coords: &[c(CLOUD, "spring-cloud-starter-openfeign", Scope::Compile)], cloud: true },
    DepDef { id: "cloud-eureka", name: "Eureka Discovery Client", description: "Registro y descubrimiento de servicios.", group: "Spring Cloud", coords: &[c(CLOUD, "spring-cloud-starter-netflix-eureka-client", Scope::Compile)], cloud: true },
    DepDef { id: "cloud-eureka-server", name: "Eureka Server", description: "Servidor de registro de servicios.", group: "Spring Cloud", coords: &[c(CLOUD, "spring-cloud-starter-netflix-eureka-server", Scope::Compile)], cloud: true },
    DepDef { id: "cloud-config-client", name: "Config Client", description: "Configuración centralizada (cliente).", group: "Spring Cloud", coords: &[c(CLOUD, "spring-cloud-starter-config", Scope::Compile)], cloud: true },
    DepDef { id: "cloud-config-server", name: "Config Server", description: "Configuración centralizada (servidor).", group: "Spring Cloud", coords: &[c(CLOUD, "spring-cloud-config-server", Scope::Compile)], cloud: true },
    DepDef { id: "cloud-gateway", name: "Gateway", description: "API Gateway reactivo.", group: "Spring Cloud", coords: &[c(CLOUD, "spring-cloud-starter-gateway", Scope::Compile)], cloud: true },
];

pub fn find(id: &str) -> Option<&'static DepDef> {
    DEPS.iter().find(|d| d.id == id)
}

/// Catálogo offline completo (lo que soporta la plantilla local).
pub fn offline_catalog() -> SpringCatalog {
    let mut groups: Vec<SpringDepGroup> = Vec::new();
    for d in DEPS {
        let item = SpringDep { id: d.id.into(), name: d.name.into(), description: d.description.into(), offline: true };
        match groups.iter_mut().find(|g| g.name == d.group) {
            Some(g) => g.items.push(item),
            None => groups.push(SpringDepGroup { name: d.group.into(), items: vec![item] }),
        }
    }
    SpringCatalog {
        boot_versions: vec![FALLBACK_BOOT.into()],
        default_boot: FALLBACK_BOOT.into(),
        java_versions: JAVA_VERSIONS.to_vec(),
        dependencies: groups,
        source: "offline".into(),
    }
}

fn is_stable(v: &str) -> bool {
    !(v.contains("SNAPSHOT") || v.contains("-M") || v.contains("-RC") || v.contains("-m") || v.contains("-rc"))
}

/// Parsea `GET /metadata/client` (formato initializr v2.2).
pub fn parse_metadata(v: &Value) -> Option<SpringCatalog> {
    let boot = v.get("bootVersion")?;
    let default_boot = boot.get("default")?.as_str()?.to_string();
    let mut boot_versions: Vec<String> = boot
        .get("values")?
        .as_array()?
        .iter()
        .filter_map(|x| x.get("id")?.as_str().map(str::to_string))
        .filter(|id| is_stable(id))
        .collect();
    if !boot_versions.contains(&default_boot) {
        boot_versions.insert(0, default_boot.clone());
    }
    let mut java_versions: Vec<u32> = v
        .get("javaVersion")
        .and_then(|j| j.get("values"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.get("id")?.as_str()?.parse().ok()).collect())
        .unwrap_or_default();
    java_versions.retain(|j| *j >= 17);
    if java_versions.is_empty() {
        java_versions = JAVA_VERSIONS.to_vec();
    }
    let mut dependencies = Vec::new();
    for g in v.get("dependencies")?.get("values")?.as_array()? {
        let name = g.get("name")?.as_str()?.to_string();
        let items: Vec<SpringDep> = g
            .get("values")?
            .as_array()?
            .iter()
            .filter_map(|d| {
                let id = d.get("id")?.as_str()?.to_string();
                Some(SpringDep {
                    offline: find(&id).is_some(),
                    name: d.get("name")?.as_str()?.to_string(),
                    description: d.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
                    id,
                })
            })
            .collect();
        if !items.is_empty() {
            dependencies.push(SpringDepGroup { name, items });
        }
    }
    if dependencies.is_empty() {
        return None;
    }
    Some(SpringCatalog { boot_versions, default_boot, java_versions, dependencies, source: "online".into() })
}

/// `3.5.7` → `(3, 5)`.
pub fn major_minor(v: &str) -> (u32, u32) {
    let mut it = v.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_catalog_has_presets() {
        let c = offline_catalog();
        let all: Vec<&str> = c.dependencies.iter().flat_map(|g| g.items.iter().map(|i| i.id.as_str())).collect();
        for id in ["web", "data-jpa", "postgresql", "mysql", "security", "validation", "lombok", "actuator", "kafka", "amqp", "data-redis", "cloud-feign", "cloud-eureka", "cloud-config-client", "devtools", "webflux", "data-mongodb", "cloud-gateway", "cloud-eureka-server", "cloud-config-server"] {
            assert!(all.contains(&id), "{id}");
        }
        assert_eq!(c.source, "offline");
    }

    #[test]
    fn parses_online_metadata() {
        let json: Value = serde_json::from_str(
            r#"{"bootVersion":{"default":"3.5.7","values":[{"id":"4.0.0-SNAPSHOT"},{"id":"3.5.7"},{"id":"3.4.11"},{"id":"3.6.0-RC1"}]},
                "javaVersion":{"default":"21","values":[{"id":"25"},{"id":"21"},{"id":"17"},{"id":"11"}]},
                "dependencies":{"values":[{"name":"Web","values":[{"id":"web","name":"Spring Web","description":"d"},{"id":"nuevo","name":"Nuevo"}]}]}}"#,
        )
        .unwrap();
        let c = parse_metadata(&json).unwrap();
        assert_eq!(c.default_boot, "3.5.7");
        assert_eq!(c.boot_versions, ["3.5.7", "3.4.11"]);
        assert_eq!(c.java_versions, [25, 21, 17]);
        assert!(c.dependencies[0].items[0].offline && !c.dependencies[0].items[1].offline);
        assert!(parse_metadata(&serde_json::json!({})).is_none());
    }

    #[test]
    fn versions() {
        assert_eq!(major_minor("3.5.7"), (3, 5));
        assert_eq!(major_minor("4.0.0-RC1"), (4, 0));
    }
}
