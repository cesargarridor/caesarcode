//! Cliente LSP sobre stdin/stdout de un proceso hijo (o de cualquier par de streams async).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lsp_types::{PublishDiagnosticsParams, Url};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader};
use tokio::process::Child;
use tokio::sync::{mpsc, oneshot, watch};

use super::convert;
use super::transport::{read_message, write_message};
use super::{CompletionDto, Emitter, HoverDto, LocationDto, LspEvent, LspServerStatus, LspState};
use crate::services::process_tree::kill_tree;
use crate::util::proc::no_window_tokio;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// `initialize` puede tardar (rust-analyzer, jdtls).
const INIT_TIMEOUT: Duration = Duration::from_secs(120);

type BoxWriter = Box<dyn AsyncWrite + Send + Unpin>;
type Pending = HashMap<i64, oneshot::Sender<Result<Value, String>>>;

/// Cola ordenada de notificaciones hacia el servidor. Garantiza que `didOpen`/`didChange`
/// salen en orden y no antes de `initialized`.
enum Out {
    Notify { method: &'static str, params: Value },
    /// Se resuelve cuando todo lo anterior ya se escribió (antes de completion/hover/definition).
    Flush(oneshot::Sender<()>),
    Stop,
}

#[derive(Default)]
struct Docs {
    /// ruta → uri enviada al servidor (string exacto, se reutiliza en todas las notificaciones).
    by_path: HashMap<PathBuf, String>,
    /// uri → ruta tal como la conoce el frontend.
    by_uri: HashMap<String, PathBuf>,
}

pub struct LspClient {
    pub lang: String,
    pub server_name: String,
    root: PathBuf,
    writer: tokio::sync::Mutex<BoxWriter>,
    next_id: AtomicI64,
    pending: Mutex<Pending>,
    state_tx: watch::Sender<LspState>,
    message: Mutex<Option<String>>,
    child: Mutex<Option<Child>>,
    pid: Option<u32>,
    docs: Mutex<Docs>,
    out_tx: mpsc::UnboundedSender<Out>,
    emitter: Arc<Emitter>,
    closed: AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn uri_string(path: &Path) -> Result<String, String> {
    Url::from_file_path(path)
        .map(|u| u.to_string())
        .map_err(|_| format!("ruta no válida para URI: {}", path.display()))
}

impl LspClient {
    /// Arranca el servidor como proceso hijo y lanza el handshake `initialize` en segundo plano.
    pub fn spawn(
        lang: &str,
        server_name: &str,
        program: &Path,
        args: &[String],
        root: &Path,
        emitter: Arc<Emitter>,
    ) -> Result<Arc<Self>, String> {
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        no_window_tokio(&mut cmd);
        let mut child = cmd.spawn().map_err(|e| format!("no se pudo arrancar {}: {e}", program.display()))?;
        let stdin = child.stdin.take().ok_or("sin stdin")?;
        let stdout = child.stdout.take().ok_or("sin stdout")?;
        let stderr = child.stderr.take().ok_or("sin stderr")?;
        // Drenar stderr: si el pipe se llena el servidor se bloquea.
        let label = server_name.to_string();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                log::debug!("[lsp {label}] {line}");
            }
        });
        Ok(Self::from_io(lang, server_name, root, stdout, stdin, Some(child), emitter))
    }

    /// Construye el cliente sobre streams arbitrarios (proceso real o servidor falso en tests).
    pub fn from_io<R, W>(
        lang: &str,
        server_name: &str,
        root: &Path,
        reader: R,
        writer: W,
        child: Option<Child>,
        emitter: Arc<Emitter>,
    ) -> Arc<Self>
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let (out_tx, out_rx) = mpsc::unbounded_channel();
        let (state_tx, _) = watch::channel(LspState::Starting);
        let pid = child.as_ref().and_then(|c| c.id());
        let client = Arc::new(Self {
            lang: lang.to_string(),
            server_name: server_name.to_string(),
            root: root.to_path_buf(),
            writer: tokio::sync::Mutex::new(Box::new(writer)),
            next_id: AtomicI64::new(1),
            pending: Mutex::new(HashMap::new()),
            state_tx,
            message: Mutex::new(None),
            child: Mutex::new(child),
            pid,
            docs: Mutex::new(Docs::default()),
            out_tx,
            emitter,
            closed: AtomicBool::new(false),
        });
        tokio::spawn(Self::reader_loop(client.clone(), BufReader::new(reader)));
        tokio::spawn(Self::queue_loop(client.clone(), out_rx));
        tokio::spawn(Self::initialize(client.clone()));
        client
    }

    // ---- Estado ----

    pub fn state(&self) -> LspState {
        *self.state_tx.borrow()
    }

    pub fn status(&self) -> LspServerStatus {
        LspServerStatus {
            language: self.lang.clone(),
            server: self.server_name.clone(),
            state: self.state(),
            message: lock(&self.message).clone(),
        }
    }

    fn set_state(&self, state: LspState, message: Option<String>) {
        *lock(&self.message) = message;
        self.state_tx.send_replace(state);
        if !self.closed.load(Ordering::SeqCst) {
            self.emitter.emit(LspEvent::Status { status: self.status() });
        }
    }

    fn root_folder(&self) -> Value {
        let uri = Url::from_directory_path(&self.root).map(|u| u.to_string()).unwrap_or_default();
        let name = self.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        json!({ "uri": uri, "name": name })
    }

    // ---- Handshake ----

    async fn initialize(self: Arc<Self>) {
        let folder = self.root_folder();
        let params = json!({
            "processId": std::process::id(),
            "clientInfo": { "name": "CaesarCode" },
            "rootUri": folder["uri"],
            "workspaceFolders": [folder],
            "initializationOptions": {},
            "capabilities": {
                "general": { "positionEncodings": ["utf-16"] },
                "textDocument": {
                    "synchronization": { "dynamicRegistration": false, "didSave": true },
                    "completion": { "completionItem": {
                        "snippetSupport": true,
                        "documentationFormat": ["markdown", "plaintext"]
                    } },
                    "hover": { "contentFormat": ["markdown", "plaintext"] },
                    "definition": { "linkSupport": true },
                    "publishDiagnostics": {}
                },
                "workspace": { "configuration": true, "workspaceFolders": true }
            }
        });
        match self.request("initialize", params, INIT_TIMEOUT).await {
            Ok(_) => {
                if let Err(e) = self.write(&json!({"jsonrpc": "2.0", "method": "initialized", "params": {}})).await {
                    self.set_state(LspState::Crashed, Some(format!("initialized: {e}")));
                    return;
                }
                self.set_state(LspState::Ready, None);
            }
            Err(e) => {
                if !self.closed.load(Ordering::SeqCst) {
                    self.set_state(LspState::Crashed, Some(format!("initialize: {e}")));
                    self.kill().await;
                }
            }
        }
    }

    // ---- E/S ----

    async fn write(&self, msg: &Value) -> Result<(), String> {
        let mut w = self.writer.lock().await;
        write_message(&mut *w, msg).await.map_err(|e| e.to_string())
    }

    pub async fn request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        lock(&self.pending).insert(id, tx);
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        if let Err(e) = self.write(&msg).await {
            lock(&self.pending).remove(&id);
            return Err(e);
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err("servidor terminado".into()),
            Err(_) => {
                lock(&self.pending).remove(&id);
                Err(format!("timeout en {method}"))
            }
        }
    }

    async fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        self.write(&json!({"jsonrpc": "2.0", "method": method, "params": params})).await
    }

    fn enqueue(&self, method: &'static str, params: Value) {
        let _ = self.out_tx.send(Out::Notify { method, params });
    }

    async fn queue_loop(self: Arc<Self>, mut rx: mpsc::UnboundedReceiver<Out>) {
        let mut st = self.state_tx.subscribe();
        // Hasta `initialized` no sale nada: las notificaciones esperan en la cola.
        loop {
            let s = *st.borrow_and_update();
            if s != LspState::Starting {
                break;
            }
            if st.changed().await.is_err() {
                break;
            }
        }
        if self.state() == LspState::Ready {
            while let Some(item) = rx.recv().await {
                match item {
                    Out::Notify { method, params } => {
                        if let Err(e) = self.notify(method, params).await {
                            log::debug!("[lsp {}] {method}: {e}", self.server_name);
                        }
                    }
                    Out::Flush(tx) => {
                        let _ = tx.send(());
                    }
                    Out::Stop => break,
                }
                if matches!(self.state(), LspState::Crashed | LspState::Unavailable) {
                    break;
                }
            }
        }
        // Servidor caído o cerrado: no se escribe más, pero nadie se queda esperando un flush.
        rx.close();
        while let Ok(item) = rx.try_recv() {
            if let Out::Flush(tx) = item {
                let _ = tx.send(());
            }
        }
    }

    async fn flush(&self) {
        let (tx, rx) = oneshot::channel();
        if self.out_tx.send(Out::Flush(tx)).is_ok() {
            let _ = tokio::time::timeout(REQUEST_TIMEOUT, rx).await;
        }
    }

    // ---- Lectura ----

    async fn reader_loop<R: AsyncRead + Unpin>(self: Arc<Self>, mut reader: BufReader<R>) {
        loop {
            match read_message(&mut reader).await {
                Ok(Some(msg)) => self.dispatch(msg).await,
                Ok(None) => break,
                Err(e) => {
                    log::debug!("[lsp {}] lectura: {e}", self.server_name);
                    break;
                }
            }
        }
        // EOF: se fallan las peticiones en vuelo y se marca caído (salvo cierre voluntario).
        let pending: Vec<_> = lock(&self.pending).drain().collect();
        for (_, tx) in pending {
            let _ = tx.send(Err("servidor terminado".into()));
        }
        if !self.closed.load(Ordering::SeqCst) && self.state() != LspState::Crashed {
            self.set_state(LspState::Crashed, Some("El servidor terminó inesperadamente".into()));
        }
    }

    async fn dispatch(&self, msg: Value) {
        let method = msg.get("method").and_then(Value::as_str).map(str::to_owned);
        let id = msg.get("id").cloned().filter(|v| !v.is_null());
        match (method, id) {
            (Some(method), Some(id)) => self.answer_server_request(id, &method, msg.get("params")).await,
            (Some(method), None) => {
                if method == "textDocument/publishDiagnostics" {
                    self.on_diagnostics(msg.get("params").cloned().unwrap_or(Value::Null));
                }
                // window/logMessage, $/progress y demás: se ignoran.
            }
            (None, Some(id)) => {
                let Some(id) = id.as_i64() else { return };
                let Some(tx) = lock(&self.pending).remove(&id) else { return };
                let result = match msg.get("error") {
                    Some(err) if !err.is_null() => {
                        Err(err.get("message").and_then(Value::as_str).unwrap_or("error del servidor").to_string())
                    }
                    _ => Ok(msg.get("result").cloned().unwrap_or(Value::Null)),
                };
                let _ = tx.send(result);
            }
            (None, None) => {}
        }
    }

    /// Peticiones servidor→cliente: si no se contestan, pyright/tsserver se quedan colgados.
    async fn answer_server_request(&self, id: Value, method: &str, params: Option<&Value>) {
        let result: Result<Value, (i64, &str)> = match method {
            "workspace/configuration" => {
                let n = params.and_then(|p| p.get("items")).and_then(Value::as_array).map_or(0, Vec::len);
                Ok(Value::Array(vec![Value::Null; n]))
            }
            "client/registerCapability"
            | "client/unregisterCapability"
            | "window/workDoneProgress/create"
            | "window/showMessageRequest" => Ok(Value::Null),
            "workspace/workspaceFolders" => Ok(json!([self.root_folder()])),
            _ => Err((-32601, "method not found")),
        };
        let reply = match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((code, message)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
        };
        if let Err(e) = self.write(&reply).await {
            log::debug!("[lsp {}] respuesta a {method}: {e}", self.server_name);
        }
    }

    fn path_for_uri(&self, uri: &Url) -> Option<String> {
        let docs = lock(&self.docs);
        if let Some(p) = docs.by_uri.get(uri.as_str()) {
            return Some(p.to_string_lossy().into_owned());
        }
        let fallback = convert::file_path_of(uri)?;
        // Windows: `file:///c%3A/...` vs `C:\...`; se compara sin distinguir mayúsculas.
        if cfg!(windows) {
            let want = fallback.to_lowercase();
            if let Some(p) = docs.by_uri.values().find(|p| p.to_string_lossy().to_lowercase() == want) {
                return Some(p.to_string_lossy().into_owned());
            }
        }
        Some(fallback)
    }

    fn on_diagnostics(&self, params: Value) {
        let Ok(p) = serde_json::from_value::<PublishDiagnosticsParams>(params) else { return };
        let Some(path) = self.path_for_uri(&p.uri) else { return };
        let diagnostics = p.diagnostics.into_iter().map(convert::diagnostic_dto).collect();
        self.emitter.emit(LspEvent::Diagnostics { path, diagnostics });
    }

    // ---- Documentos ----

    fn doc_uri(&self, path: &Path) -> Option<String> {
        lock(&self.docs).by_path.get(path).cloned()
    }

    pub fn did_open(&self, path: &Path, language_id: &str, text: String) {
        let Ok(uri) = uri_string(path) else { return };
        let already = {
            let mut docs = lock(&self.docs);
            let already = docs.by_path.contains_key(path);
            if !already {
                docs.by_path.insert(path.to_path_buf(), uri.clone());
                docs.by_uri.insert(uri.clone(), path.to_path_buf());
            }
            already
        };
        if already {
            // Reapertura (p. ej. recarga del webview): se resincroniza el contenido.
            self.enqueue("textDocument/didChange", json!({
                "textDocument": { "uri": uri, "version": 1 },
                "contentChanges": [{ "text": text }]
            }));
        } else {
            self.enqueue("textDocument/didOpen", json!({
                "textDocument": { "uri": uri, "languageId": language_id, "version": 1, "text": text }
            }));
        }
    }

    pub fn did_change(&self, path: &Path, version: i32, text: String) {
        let Some(uri) = self.doc_uri(path) else { return };
        self.enqueue("textDocument/didChange", json!({
            "textDocument": { "uri": uri, "version": version },
            "contentChanges": [{ "text": text }]
        }));
    }

    pub fn did_save(&self, path: &Path, text: String) {
        let Some(uri) = self.doc_uri(path) else { return };
        self.enqueue("textDocument/didSave", json!({ "textDocument": { "uri": uri }, "text": text }));
    }

    pub fn did_close(&self, path: &Path) {
        let uri = {
            let mut docs = lock(&self.docs);
            let Some(uri) = docs.by_path.remove(path) else { return };
            docs.by_uri.remove(&uri);
            uri
        };
        self.enqueue("textDocument/didClose", json!({ "textDocument": { "uri": uri } }));
    }

    // ---- Peticiones de lenguaje ----

    /// `Ok(None)` si el servidor aún no está listo o el documento no está abierto.
    async fn position_request(&self, method: &str, path: &Path, line: u32, character: u32, extra: Value) -> Result<Option<Value>, String> {
        if self.state() != LspState::Ready {
            return Ok(None);
        }
        let Some(uri) = self.doc_uri(path) else { return Ok(None) };
        // Los didChange pendientes deben llegar antes, o el servidor responde sobre texto viejo.
        self.flush().await;
        let mut params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        });
        if let (Some(p), Some(e)) = (params.as_object_mut(), extra.as_object()) {
            p.extend(e.clone());
        }
        self.request(method, params, REQUEST_TIMEOUT).await.map(Some)
    }

    pub async fn completion(&self, path: &Path, line: u32, character: u32) -> Result<Vec<CompletionDto>, String> {
        let r = self.position_request("textDocument/completion", path, line, character, json!({"context": {"triggerKind": 1}})).await?;
        Ok(r.map(convert::completion_from).unwrap_or_default())
    }

    pub async fn hover(&self, path: &Path, line: u32, character: u32) -> Result<Option<HoverDto>, String> {
        let r = self.position_request("textDocument/hover", path, line, character, Value::Null).await?;
        Ok(r.and_then(convert::hover_from))
    }

    pub async fn definition(&self, path: &Path, line: u32, character: u32) -> Result<Vec<LocationDto>, String> {
        let r = self.position_request("textDocument/definition", path, line, character, Value::Null).await?;
        Ok(r.map(convert::locations_from).unwrap_or_default())
    }

    // ---- Cierre ----

    async fn kill(&self) {
        let child = lock(&self.child).take();
        if let Some(mut c) = child {
            if let Some(pid) = self.pid {
                let _ = tokio::task::spawn_blocking(move || kill_tree(pid, Duration::from_millis(200), false)).await;
            }
            let _ = c.kill().await;
        }
    }

    /// `shutdown` + `exit` con plazo `grace`; si el proceso no sale, se mata su árbol.
    pub async fn shutdown(&self, grace: Duration) {
        self.closed.store(true, Ordering::SeqCst);
        if self.state() == LspState::Ready {
            let _ = self.request("shutdown", Value::Null, grace).await;
            let _ = tokio::time::timeout(grace, self.notify("exit", Value::Null)).await;
        }
        let _ = self.out_tx.send(Out::Stop);
        self.state_tx.send_replace(LspState::Crashed);
        let child = lock(&self.child).take();
        if let Some(mut c) = child {
            if tokio::time::timeout(Duration::from_millis(300), c.wait()).await.is_err() {
                if let Some(pid) = self.pid {
                    let _ = tokio::task::spawn_blocking(move || kill_tree(pid, Duration::from_millis(200), false)).await;
                }
                let _ = c.kill().await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{duplex, split, DuplexStream};

    /// Servidor LSP falso del otro lado de un `duplex`. Registra los métodos recibidos.
    struct FakeServer {
        log: Arc<Mutex<Vec<Value>>>,
    }

    impl FakeServer {
        fn start(stream: DuplexStream, ask_config: bool) -> Self {
            let log = Arc::new(Mutex::new(Vec::new()));
            let log2 = log.clone();
            tokio::spawn(async move {
                let (r, mut w) = split(stream);
                let mut r = BufReader::new(r);
                while let Ok(Some(msg)) = read_message(&mut r).await {
                    log2.lock().unwrap().push(msg.clone());
                    let method = msg.get("method").and_then(Value::as_str).unwrap_or("").to_string();
                    let id = msg.get("id").cloned();
                    let reply = |result: Value| json!({"jsonrpc": "2.0", "id": id.clone(), "result": result});
                    match method.as_str() {
                        "initialize" => {
                            if ask_config {
                                let _ = write_message(&mut w, &json!({"jsonrpc": "2.0", "id": "cfg-1", "method": "workspace/configuration", "params": {"items": [{}, {}]}})).await;
                                let _ = write_message(&mut w, &json!({"jsonrpc": "2.0", "id": 99, "method": "foo/unknown", "params": {}})).await;
                            }
                            let _ = write_message(&mut w, &reply(json!({"capabilities": {}}))).await;
                        }
                        "textDocument/didOpen" => {
                            let uri = msg["params"]["textDocument"]["uri"].clone();
                            let _ = write_message(&mut w, &json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
                                "params": {"uri": uri, "diagnostics": [
                                    {"range": {"start": {"line": 0, "character": 1}, "end": {"line": 0, "character": 4}},
                                     "severity": 2, "message": "ojo", "source": "fake"}]}})).await;
                        }
                        "textDocument/completion" => {
                            let _ = write_message(&mut w, &reply(json!([{"label": "alpha", "kind": 3}]))).await;
                        }
                        "textDocument/hover" => {
                            // El texto visto por el servidor debe ser el último didChange.
                            let _ = write_message(&mut w, &reply(json!({"contents": {"kind": "markdown", "value": "hover!"}}))).await;
                        }
                        "textDocument/definition" => {
                            let _ = write_message(&mut w, &reply(json!({"uri": "file:///tmp/def.py",
                                "range": {"start": {"line": 7, "character": 1}, "end": {"line": 7, "character": 3}}}))).await;
                        }
                        "shutdown" => {
                            let _ = write_message(&mut w, &reply(Value::Null)).await;
                        }
                        "exit" => break,
                        _ => {}
                    }
                }
            });
            FakeServer { log }
        }

        fn methods(&self) -> Vec<String> {
            self.log
                .lock()
                .unwrap()
                .iter()
                .filter_map(|m| m.get("method").and_then(Value::as_str).map(str::to_owned))
                .collect()
        }
    }

    fn collector() -> (Arc<Emitter>, Arc<Mutex<Vec<LspEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let e2 = events.clone();
        let em = Arc::new(Emitter::default());
        em.set(Box::new(move |ev| {
            e2.lock().unwrap().push(ev);
            true
        }));
        (em, events)
    }

    async fn wait_state(c: &LspClient, want: LspState) {
        let mut rx = c.state_tx.subscribe();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if *rx.borrow_and_update() == want {
                    return;
                }
                rx.changed().await.unwrap();
            }
        })
        .await
        .expect("estado esperado");
    }

    fn connect(ask_config: bool) -> (Arc<LspClient>, FakeServer, Arc<Mutex<Vec<LspEvent>>>) {
        let (a, b) = duplex(1 << 16);
        let server = FakeServer::start(b, ask_config);
        let (em, events) = collector();
        let (r, w) = split(a);
        let client = LspClient::from_io("python", "fake", Path::new("/tmp"), r, w, None, em);
        (client, server, events)
    }

    #[tokio::test]
    async fn handshake_queues_did_open_until_initialized_and_answers_server_requests() {
        let (client, server, events) = connect(true);
        // Se encola ANTES de que el handshake termine.
        client.did_open(Path::new("/tmp/a.py"), "python", "import os\n".into());
        wait_state(&client, LspState::Ready).await;

        // Diagnóstico publicado tras el didOpen, ruta mapeada a la del frontend.
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if events.lock().unwrap().iter().any(|e| matches!(e, LspEvent::Diagnostics { .. })) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("diagnóstico");
        let diag = events.lock().unwrap().iter().find_map(|e| match e {
            LspEvent::Diagnostics { path, diagnostics } => Some((path.clone(), diagnostics.clone())),
            _ => None,
        });
        let (path, diags) = diag.unwrap();
        assert_eq!(path, "/tmp/a.py");
        assert_eq!(diags[0].message, "ojo");
        assert_eq!((diags[0].from_char, diags[0].to_char), (1, 4));

        // Orden: initialize → initialized → didOpen.
        let methods = server.methods();
        let pos = |m: &str| methods.iter().position(|x| x == m).unwrap_or(usize::MAX);
        assert!(pos("initialize") < pos("initialized"));
        assert!(pos("initialized") < pos("textDocument/didOpen"));
        assert!(events.lock().unwrap().iter().any(|e| matches!(e, LspEvent::Status { status } if status.state == LspState::Ready)));

        // El servidor recibió respuestas a sus peticiones (configuración → [null, null]; desconocida → -32601).
        let log = server.log.lock().unwrap().clone();
        let cfg = log.iter().find(|m| m["id"] == "cfg-1").expect("respuesta a workspace/configuration");
        assert_eq!(cfg["result"], json!([null, null]));
        let unk = log.iter().find(|m| m["id"] == 99).expect("respuesta a método desconocido");
        assert_eq!(unk["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn requests_flush_pending_changes_first() {
        let (client, server, _events) = connect(false);
        wait_state(&client, LspState::Ready).await;
        let p = Path::new("/tmp/b.py");
        client.did_open(p, "python", "x = 1\n".into());
        client.did_change(p, 2, "x = 12\n".into());

        let items = client.completion(p, 0, 1).await.unwrap();
        assert_eq!(items[0].label, "alpha");
        assert_eq!(items[0].kind.as_deref(), Some("function"));
        let methods = server.methods();
        let change = methods.iter().position(|m| m == "textDocument/didChange").unwrap();
        let comp = methods.iter().position(|m| m == "textDocument/completion").unwrap();
        assert!(change < comp, "didChange debe llegar antes que completion: {methods:?}");

        assert_eq!(client.hover(p, 0, 0).await.unwrap().unwrap().markdown, "hover!");
        let def = client.definition(p, 0, 0).await.unwrap();
        assert_eq!((def[0].path.as_str(), def[0].line), ("/tmp/def.py", 7));

        // Documento no abierto → respuesta vacía, sin error.
        assert!(client.completion(Path::new("/tmp/nope.py"), 0, 0).await.unwrap().is_empty());
        assert!(client.hover(Path::new("/tmp/nope.py"), 0, 0).await.unwrap().is_none());

        // didSave / didClose salen con la misma URI.
        client.did_save(p, "x = 12\n".into());
        client.did_close(p);
        client.flush().await;
        let log = server.log.lock().unwrap().clone();
        let uris: Vec<_> = log
            .iter()
            .filter(|m| m["method"].as_str().is_some_and(|s| s.starts_with("textDocument/did")))
            .map(|m| m["params"]["textDocument"]["uri"].clone())
            .collect();
        assert!(uris.len() >= 4 && uris.iter().all(|u| u == &uris[0]));
    }

    #[tokio::test]
    async fn shutdown_sends_shutdown_then_exit() {
        let (client, server, _events) = connect(false);
        wait_state(&client, LspState::Ready).await;
        client.shutdown(Duration::from_secs(1)).await;
        // El servidor falso registra `exit` de forma asíncrona.
        for _ in 0..100 {
            if server.methods().iter().any(|m| m == "exit") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let methods = server.methods();
        let s = methods.iter().position(|m| m == "shutdown").expect("shutdown");
        let e = methods.iter().position(|m| m == "exit").expect("exit");
        assert!(s < e);
    }

    #[tokio::test]
    async fn eof_marks_client_crashed_and_fails_pending() {
        let (a, b) = duplex(1 << 16);
        let (em, events) = collector();
        let (r, w) = split(a);
        let client = LspClient::from_io("python", "fake", Path::new("/tmp"), r, w, None, em);
        // El "servidor" muere sin contestar al initialize.
        drop(b);
        wait_state(&client, LspState::Crashed).await;
        assert!(events.lock().unwrap().iter().any(|e| matches!(e, LspEvent::Status { status } if status.state == LspState::Crashed)));
        // Sin servidor no hay respuestas ni errores: vacío.
        assert!(client.completion(Path::new("/tmp/a.py"), 0, 0).await.unwrap().is_empty());
    }

    /// Proceso real (python3 como servidor LSP mínimo): comprueba `spawn`, pipes, stderr drenado,
    /// framing real sobre stdio y que `shutdown` deja el proceso muerto.
    #[cfg(unix)]
    #[tokio::test]
    async fn spawned_process_speaks_lsp_and_dies_on_shutdown() {
        const SCRIPT: &str = r#"
import sys, json
sys.stderr.write("x" * 1000000)  # si no se drena stderr, el servidor se bloquea aquí
sys.stderr.flush()
def read():
    n = None
    while True:
        l = sys.stdin.buffer.readline()
        if not l: return None
        l = l.strip()
        if not l: break
        k, v = l.split(b":", 1)
        if k.lower() == b"content-length": n = int(v)
    return json.loads(sys.stdin.buffer.read(n))
def send(o):
    b = json.dumps(o, ensure_ascii=False).encode()
    sys.stdout.buffer.write(b"Content-Length: %d\r\n\r\n" % len(b) + b); sys.stdout.buffer.flush()
while True:
    m = read()
    if m is None: break
    meth = m.get("method")
    if meth == "initialize": send({"jsonrpc": "2.0", "id": m["id"], "result": {"capabilities": {}}})
    elif meth == "textDocument/hover": send({"jsonrpc": "2.0", "id": m["id"], "result": {"contents": "ñandú 日本"}})
    elif meth == "shutdown": send({"jsonrpc": "2.0", "id": m["id"], "result": None})
    elif meth == "exit": break
"#;
        let Ok(py) = which::which("python3") else { return };
        let (em, _events) = collector();
        let args = vec!["-u".to_string(), "-c".to_string(), SCRIPT.to_string()];
        let client = LspClient::spawn("python", "pyfake", &py, &args, Path::new("/tmp"), em).unwrap();
        let pid = client.pid.expect("pid");
        let p = Path::new("/tmp/real.py");
        client.did_open(p, "python", "x".into());
        wait_state(&client, LspState::Ready).await;
        assert_eq!(client.hover(p, 0, 0).await.unwrap().unwrap().markdown, "ñandú 日本");
        client.shutdown(Duration::from_secs(2)).await;
        for _ in 0..50 {
            if !crate::services::process_tree::is_alive(pid) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("el proceso LSP sigue vivo tras shutdown");
    }

    /// Contra un servidor real si está en el PATH (rust-analyzer). Se omite si no funciona.
    #[tokio::test]
    async fn real_rust_analyzer_handshake_if_available() {
        let Ok(program) = which::which("rust-analyzer") else { return };
        if !std::process::Command::new(&program).arg("--version").output().is_ok_and(|o| o.status.success()) {
            return; // shim de rustup sin el componente instalado
        }
        let dir = std::env::temp_dir().join(format!("caesar-lsp-ra-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();
        std::fs::write(dir.join("src/main.rs"), "fn main() {\n    let x: i32 = 1;\n}\n").unwrap();
        let root = dir.canonicalize().unwrap();
        let (em, _events) = collector();
        let client = LspClient::spawn("rust", "rust-analyzer", &program, &[], &root, em).unwrap();
        let file = root.join("src/main.rs");
        client.did_open(&file, "rust", std::fs::read_to_string(&file).unwrap());
        wait_state(&client, LspState::Ready).await;
        assert_eq!(client.status().state, LspState::Ready);
        client.shutdown(Duration::from_secs(3)).await;
        let _ = std::fs::remove_dir_all(&dir);
    }
}
