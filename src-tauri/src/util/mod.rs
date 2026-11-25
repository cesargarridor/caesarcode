pub mod paths;
pub mod proc;

use crate::error::{AppError, AppResult};

/// Ejecuta trabajo bloqueante (disco, procesos) fuera del runtime async.
pub async fn run_blocking<T, F>(f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Other(format!("tarea abortada: {e}")))?
}
