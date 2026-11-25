//! Puente LSP: cliente JSON-RPC sobre stdin/stdout de servidores externos
//! (pyright, typescript-language-server, rust-analyzer, jdtls).

pub mod client;
pub mod commands;
pub mod convert;
pub mod manager;
pub mod servers;
pub mod transport;

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub use manager::LspManager;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LspState {
    Starting,
    Ready,
    Unavailable,
    Crashed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LspServerStatus {
    pub language: String,
    pub server: String,
    pub state: LspState,
    pub message: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LspDocStatus {
    pub language: Option<String>,
    pub server: Option<String>,
    pub available: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiagSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticDto {
    pub from_line: u32,
    pub from_char: u32,
    pub to_line: u32,
    pub to_char: u32,
    pub severity: DiagSeverity,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LspEvent {
    #[serde(rename_all = "camelCase")]
    Diagnostics { path: String, diagnostics: Vec<DiagnosticDto> },
    #[serde(rename_all = "camelCase")]
    Status { status: LspServerStatus },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextEditDto {
    pub from_line: u32,
    pub from_char: u32,
    pub to_line: u32,
    pub to_char: u32,
    pub new_text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompletionDto {
    pub label: String,
    pub kind: Option<String>,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: String,
    pub is_snippet: bool,
    pub sort_text: Option<String>,
    pub filter_text: Option<String>,
    pub text_edit: Option<TextEditDto>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HoverDto {
    pub markdown: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocationDto {
    pub path: String,
    pub line: u32,
    pub character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

/// Destino de eventos (el `Channel` de Tauri). Devuelve `false` si el receptor desapareció.
pub type EventSink = Box<dyn Fn(LspEvent) -> bool + Send + Sync>;

/// Slot único con semántica de reemplazo: cada `lsp_subscribe` sustituye al anterior.
#[derive(Default)]
pub struct Emitter {
    sink: Mutex<Option<EventSink>>,
}

impl Emitter {
    pub fn set(&self, sink: EventSink) {
        if let Ok(mut g) = self.sink.lock() {
            *g = Some(sink);
        }
    }

    pub fn emit(&self, ev: LspEvent) {
        if let Ok(mut g) = self.sink.lock() {
            if let Some(s) = g.as_ref() {
                if !s(ev) {
                    *g = None; // webview recargado: se descarta la suscripción
                }
            }
        }
    }
}
