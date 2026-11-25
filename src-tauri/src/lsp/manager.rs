//! Un servidor LSP por (raíz, lenguaje).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use super::client::LspClient;
use super::servers::{expected_server, server_command};
use super::{Emitter, EventSink, LspEvent, LspServerStatus, LspState};

const MAX_RESTARTS: u32 = 3;

type Key = (PathBuf, String);

#[derive(Default)]
struct Entry {
    client: Option<Arc<LspClient>>,
    /// Servidor no instalado o imposible de arrancar: se recuerda y no se reintenta hasta reabrir el workspace.
    unavailable: Option<LspServerStatus>,
    restarts: u32,
}

/// Resultado de `ensure_client`.
pub enum Ensured {
    Client(Arc<LspClient>),
    Unavailable(LspServerStatus),
}

#[derive(Default)]
pub struct LspManager {
    entries: Mutex<HashMap<Key, Entry>>,
    emitter: Arc<Emitter>,
}

impl LspManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_sink(&self, sink: EventSink) {
        self.emitter.set(sink);
    }

    /// Arranca (una sola vez) el servidor del lenguaje para el workspace.
    pub async fn ensure_client(&self, root: &Path, lang: &str, app_data_dir: &Path) -> Ensured {
        let key: Key = (root.to_path_buf(), lang.to_string());
        let mut entries = self.entries.lock().await;
        let entry = entries.entry(key).or_default();

        if let Some(u) = &entry.unavailable {
            return Ensured::Unavailable(u.clone());
        }
        if let Some(c) = &entry.client {
            if c.state() != LspState::Crashed {
                return Ensured::Client(c.clone());
            }
            if entry.restarts >= MAX_RESTARTS {
                return Ensured::Unavailable(c.status());
            }
            entry.restarts += 1;
            entry.client = None;
        }

        let unavailable = |name: &str, message: String| LspServerStatus {
            language: lang.to_string(),
            server: name.to_string(),
            state: LspState::Unavailable,
            message: Some(message),
        };
        let Some(spec) = server_command(lang, root, app_data_dir) else {
            let name = expected_server(lang);
            let st = unavailable(name, format!("{name} no está instalado"));
            entry.unavailable = Some(st.clone());
            self.emitter.emit(LspEvent::Status { status: st.clone() });
            return Ensured::Unavailable(st);
        };
        match LspClient::spawn(lang, &spec.name, &spec.program, &spec.args, root, self.emitter.clone()) {
            Ok(c) => {
                entry.client = Some(c.clone());
                self.emitter.emit(LspEvent::Status { status: c.status() });
                Ensured::Client(c)
            }
            Err(e) => {
                let st = unavailable(&spec.name, e);
                entry.unavailable = Some(st.clone());
                self.emitter.emit(LspEvent::Status { status: st.clone() });
                Ensured::Unavailable(st)
            }
        }
    }

    /// Cliente ya existente (no arranca nada).
    pub async fn client(&self, root: &Path, lang: &str) -> Option<Arc<LspClient>> {
        let entries = self.entries.lock().await;
        entries.get(&(root.to_path_buf(), lang.to_string()))?.client.clone()
    }

    pub async fn statuses(&self) -> Vec<LspServerStatus> {
        let entries = self.entries.lock().await;
        let mut out: Vec<LspServerStatus> = entries
            .values()
            .filter_map(|e| e.client.as_ref().map(|c| c.status()).or_else(|| e.unavailable.clone()))
            .collect();
        out.sort_by(|a, b| a.language.cmp(&b.language));
        out
    }

    /// Olvida y apaga todo lo del workspace anterior (al abrir otro). Devuelve los clientes a cerrar.
    pub async fn take_all(&self) -> Vec<Arc<LspClient>> {
        let mut entries = self.entries.lock().await;
        entries.drain().filter_map(|(_, e)| e.client).collect()
    }

    /// Olvida los servidores de una raíz (al quitarla del workspace). Devuelve los clientes a cerrar.
    pub async fn take_root(&self, root: &Path) -> Vec<Arc<LspClient>> {
        let mut entries = self.entries.lock().await;
        let keys: Vec<Key> = entries.keys().filter(|(r, _)| r == root).cloned().collect();
        keys.into_iter().filter_map(|k| entries.remove(&k)).filter_map(|e| e.client).collect()
    }

    /// Cierra los clientes dados con `shutdown`/`exit` y, si no salen, mata su árbol.
    pub async fn shutdown_clients(clients: Vec<Arc<LspClient>>, grace: Duration) {
        futures_join_all(clients.into_iter().map(|c| async move { c.shutdown(grace).await })).await;
    }

    /// Apagado síncrono para `RunEvent::Exit` (hilo principal, sin estar dentro de un hilo de tokio).
    pub fn shutdown_blocking(&self) {
        tauri::async_runtime::block_on(async {
            let clients = self.take_all().await;
            let _ = tokio::time::timeout(
                Duration::from_millis(2500),
                Self::shutdown_clients(clients, Duration::from_secs(1)),
            )
            .await;
        });
    }
}

/// `join_all` mínimo sin añadir dependencia: lanza tareas y espera a todas.
async fn futures_join_all<I, F>(futs: I)
where
    I: IntoIterator<Item = F>,
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let handles: Vec<_> = futs.into_iter().map(tokio::spawn).collect();
    for h in handles {
        let _ = h.await;
    }
}
