//! Agentes IA de terminal (Claude Code, Codex, Gemini CLI). Se lanzan en un PTY del gestor de
//! terminales SIN argumentos: nunca flags de autoaprobación.
pub mod commands;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentId {
    Claude,
    Codex,
    Gemini,
}

impl AgentId {
    pub const ALL: [AgentId; 3] = [AgentId::Claude, AgentId::Codex, AgentId::Gemini];

    pub fn id(self) -> &'static str {
        match self {
            AgentId::Claude => "claude",
            AgentId::Codex => "codex",
            AgentId::Gemini => "gemini",
        }
    }

    /// Binario buscado en el PATH.
    pub fn bin(self) -> &'static str {
        self.id()
    }

    pub fn install_cmd(self) -> &'static str {
        match self {
            AgentId::Claude => "npm install -g @anthropic-ai/claude-code",
            AgentId::Codex => "npm install -g @openai/codex",
            AgentId::Gemini => "npm install -g @google/gemini-cli",
        }
    }

    /// Argumentos con los que se lanza: siempre vacío (nunca autoaprobación).
    pub fn argv(self) -> Vec<String> {
        Vec::new()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub id: String,
    pub bin: String,
    pub found: bool,
    pub path: Option<String>,
    pub install_cmd: String,
}

pub fn locate(a: AgentId) -> Option<PathBuf> {
    which::which(a.bin()).ok()
}

pub fn detect() -> Vec<AgentInfo> {
    AgentId::ALL
        .iter()
        .map(|a| {
            let path = locate(*a);
            AgentInfo {
                id: a.id().into(),
                bin: a.bin().into(),
                found: path.is_some(),
                path: path.map(|p| p.to_string_lossy().into_owned()),
                install_cmd: a.install_cmd().into(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_is_empty_and_never_auto_approves() {
        for a in AgentId::ALL {
            let argv = a.argv();
            assert!(argv.is_empty(), "{:?} lleva args", a);
            for bad in ["--dangerously-skip-permissions", "--full-auto", "--yolo"] {
                assert!(!argv.iter().any(|x| x.contains(bad)));
            }
        }
    }

    #[test]
    fn whitelist_rejects_unknown_agent() {
        assert!(serde_json::from_str::<AgentId>("\"claude\"").is_ok());
        assert!(serde_json::from_str::<AgentId>("\"codex\"").is_ok());
        assert!(serde_json::from_str::<AgentId>("\"gemini\"").is_ok());
        assert!(serde_json::from_str::<AgentId>("\"bash\"").is_err());
        assert!(serde_json::from_str::<AgentId>("\"rm\"").is_err());
    }

    #[test]
    fn detect_lists_three_with_install_cmds() {
        let d = detect();
        assert_eq!(d.len(), 3);
        assert!(d.iter().all(|i| i.install_cmd.starts_with("npm install -g ")));
    }
}
