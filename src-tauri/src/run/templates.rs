//! Las 6 plantillas de nueva configuración.

use std::collections::HashMap;

use super::{RunConfig, RunKind, RunTemplate};

fn t(kind: RunKind, label: &str, id: &str, command: &str, args: &str, port: Option<u16>) -> RunTemplate {
    RunTemplate {
        kind,
        label: label.into(),
        config: RunConfig {
            id: id.into(),
            name: label.into(),
            kind,
            cwd: String::new(),
            command: command.into(),
            args: args.into(),
            env: HashMap::new(),
            env_file: None,
            port,
            show_in_services: true,
        },
    }
}

pub fn templates() -> Vec<RunTemplate> {
    let mvn = if cfg!(windows) { "mvnw.cmd spring-boot:run" } else { "./mvnw spring-boot:run" };
    let py = if cfg!(windows) { "python" } else { "python3" };
    vec![
        t(RunKind::SpringBoot, "Spring Boot", "spring-boot", mvn, "", Some(8080)),
        t(RunKind::Python, "Python", "python", py, "main.py", None),
        t(RunKind::Node, "Node/TypeScript", "node", "npm run dev", "", Some(3000)),
        t(RunKind::Go, "Go", "go", "go run", ".", None),
        t(RunKind::Rust, "Rust", "rust", "cargo run", "", None),
        t(RunKind::Shell, "Custom Shell", "shell", "echo", "\"edita este comando\"", None),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_valid_templates() {
        let ts = templates();
        assert_eq!(ts.len(), 6);
        let file = crate::run::RunConfigFile { configs: ts.iter().map(|t| t.config.clone()).collect(), ..Default::default() };
        crate::run::store::validate(&file).unwrap();
    }
}
