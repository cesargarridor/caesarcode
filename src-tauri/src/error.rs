use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("No hay workspace abierto")]
    NoWorkspace,
    #[error("Ruta fuera del workspace: {0}")]
    OutsideWorkspace(String),
    #[error("No encontrado: {0}")]
    NotFound(String),
    #[error("Ya existe: {0}")]
    AlreadyExists(String),
    #[error("Argumento inválido: {0}")]
    InvalidArgument(String),
    #[error("Git: {0}")]
    Git(String),
    #[error("Servicio: {0}")]
    Service(String),
    #[error("PTY: {0}")]
    Pty(String),
    #[error("LSP: {0}")]
    Lsp(String),
    #[error("Watcher: {0}")]
    Watch(String),
    #[error("{0}")]
    Other(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            AppError::Io(_) => "Io",
            AppError::Json(_) => "Json",
            AppError::NoWorkspace => "NoWorkspace",
            AppError::OutsideWorkspace(_) => "OutsideWorkspace",
            AppError::NotFound(_) => "NotFound",
            AppError::AlreadyExists(_) => "AlreadyExists",
            AppError::InvalidArgument(_) => "InvalidArgument",
            AppError::Git(_) => "Git",
            AppError::Service(_) => "Service",
            AppError::Pty(_) => "Pty",
            AppError::Lsp(_) => "Lsp",
            AppError::Watch(_) => "Watch",
            AppError::Other(_) => "Other",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut st = serializer.serialize_struct("AppError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}
