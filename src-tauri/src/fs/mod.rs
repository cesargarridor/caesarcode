pub mod commands;
pub mod ops;
pub mod quick_open;
pub mod roles;
pub mod search;
pub mod symbols;
pub mod tree;
pub mod watcher;

use serde::{Deserialize, Serialize};

pub use roles::{CompactChain, FolderRole};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub ext: Option<String>,
    /// Solo directorios: `maven | gradle | node | cargo | go | python`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// Rol de carpeta estilo IntelliJ (source root, paquete, excluida...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<FolderRole>,
    /// Cadena de paquetes compactada (`com.example.app`) hasta la carpeta hoja.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact: Option<CompactChain>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileContent {
    pub path: String,
    pub text: String,
    pub is_binary: bool,
    pub too_large: bool,
    /// Solo se devuelven los primeros 5 MB (archivo entre 5 y 10 MB): la UI lo abre en solo lectura.
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FsEvent {
    Changed { paths: Vec<String> },
    /// `repo`: toplevel del repo afectado (None = desconocido: refrescar todos).
    GitChanged { repo: Option<String> },
    /// Una raíz del workspace desapareció o cambió: el frontend vuelve a pedir `workspace_roots`.
    RootsChanged,
    Error { message: String },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QuickOpenItem {
    pub path: String,
    pub rel_path: String,
    /// Nombre de la raíz dueña.
    #[serde(default)]
    pub root: String,
    pub score: i64,
    pub indices: Vec<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub regex: bool,
    pub whole_word: bool,
    #[serde(default)]
    pub include: Option<String>,
    #[serde(default)]
    pub max_results: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatch {
    pub path: String,
    pub rel_path: String,
    /// Nombre de la raíz dueña.
    #[serde(default)]
    pub root: String,
    pub line: u64,
    pub column: usize,
    pub preview: String,
    pub match_start: usize,
    pub match_end: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
    pub truncated: bool,
    pub files_searched: u64,
    pub cancelled: bool,
}
