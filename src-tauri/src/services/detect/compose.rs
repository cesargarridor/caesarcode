//! docker compose.

use std::path::Path;

use super::util::{exists, Found};
use crate::services::{ServiceKind, Tech};

pub fn detect(dir: &Path) -> Vec<Found> {
    for f in ["docker-compose.yml", "docker-compose.yaml", "compose.yaml", "compose.yml"] {
        if exists(dir, f) {
            return vec![Found {
                name: None,
                command: "docker compose up".into(),
                kind: ServiceKind::Docker,
                tech: Tech::Docker,
                framework: None,
                port: None,
            }];
        }
    }
    Vec::new()
}
