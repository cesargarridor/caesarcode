//! Maven / Gradle (Spring Boot o Java genérico).

use std::path::Path;

use super::util::{exists, properties_lookup, read, yaml_lookup, Found};
use crate::services::{ServiceKind, Tech};

fn app_value(dir: &Path, yaml_path: &[&str], prop: &str) -> Option<String> {
    for y in ["application.yml", "application.yaml"] {
        if let Some(t) = read(dir, &format!("src/main/resources/{y}")) {
            if let Some(v) = yaml_lookup(&t, yaml_path) {
                return Some(v);
            }
        }
    }
    read(dir, "src/main/resources/application.properties").and_then(|t| properties_lookup(&t, prop))
}

fn wrapper(dir: &Path, unix: &str, win: &str, fallback: &str) -> String {
    if cfg!(windows) && exists(dir, win) {
        win.to_string()
    } else if !cfg!(windows) && exists(dir, unix) {
        format!("./{unix}")
    } else {
        fallback.to_string()
    }
}

pub fn detect(dir: &Path) -> Vec<Found> {
    if let Some(pom) = read(dir, "pom.xml") {
        // Agregador multi-módulo: se detectan los submódulos al bajar.
        let compact: String = pom.split_whitespace().collect();
        if compact.contains("<packaging>pom</packaging>") {
            return Vec::new();
        }
        let mvn = wrapper(dir, "mvnw", "mvnw.cmd", "mvn");
        return vec![if pom.contains("spring-boot") {
            spring(dir, format!("{mvn} spring-boot:run"), ServiceKind::Maven)
        } else {
            plain(format!("{mvn} compile exec:java"), ServiceKind::Maven)
        }];
    }
    for f in ["build.gradle", "build.gradle.kts"] {
        if let Some(g) = read(dir, f) {
            let gw = wrapper(dir, "gradlew", "gradlew.bat", "gradle");
            return vec![if g.contains("org.springframework.boot") {
                spring(dir, format!("{gw} bootRun"), ServiceKind::Gradle)
            } else {
                plain(format!("{gw} run"), ServiceKind::Gradle)
            }];
        }
    }
    Vec::new()
}

fn spring(dir: &Path, command: String, kind: ServiceKind) -> Found {
    let port = app_value(dir, &["server", "port"], "server.port")
        .and_then(|v| v.parse().ok())
        .or(Some(8080));
    Found {
        name: app_value(dir, &["spring", "application", "name"], "spring.application.name"),
        command,
        kind,
        tech: Tech::Spring,
        framework: Some("spring-boot".into()),
        port,
    }
}

fn plain(command: String, kind: ServiceKind) -> Found {
    Found { name: None, command, kind, tech: Tech::Other, framework: None, port: None }
}
