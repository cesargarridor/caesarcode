# CaesarCode — PLAN de implementación

Fuente de verdad funcional: `SPEC.md`. Este documento fija la arquitectura, versiones, contratos y el reparto en 4 bloques.
Los coders NO deciden arquitectura: si algo no está aquí, eligen la opción más simple compatible con este plan y lo anotan en el resumen final.

Raíz del proyecto: `/Users/cesargarrido/Proyectos/caesarCode` (en adelante `ROOT`).
Toolchain verificada en la máquina: rustc 1.99.1, cargo 1.99.1, node 26.13, npm 11.20.

---

## 0. Decisiones de arquitectura (fijas)

1. **Un solo `AppState`** gestionado con `app.manage(AppState::new())`. Cada bloque añade su campo (no se crean campos vacíos antes de tiempo).
2. **Rutas siempre nativas y absolutas** entre Rust y TS. El frontend **nunca concatena rutas**: crea/renombra con `(parent, name)` y Rust hace el `join`. Toda ruta recibida se valida con `ensure_in_workspace()` (canonicaliza y comprueba `starts_with(root)`).
3. **Streaming solo por `tauri::ipc::Channel<T>`** (logs, terminal, eventos fs, LSP). Nada de `emit`. Cada suscripción es de **slot único con semántica de reemplazo**: una nueva llamada `*_subscribe` sustituye la anterior (soporta recarga del webview).
4. **Sin `React.StrictMode`** (evita doble suscripción / doble PTY en dev).
5. **Serde**: todos los DTO con `#[serde(rename_all = "camelCase")]`; enums de eventos con `#[serde(tag = "type", rename_all = "camelCase")]`. Los **argumentos de comandos** se escriben en snake_case en Rust y Tauri los expone en camelCase en `invoke` (p. ej. `case_sensitive` → `caseSensitive`, `on_event` → `onEvent`).
6. **Comandos async con `State<'_, AppState>` deben devolver `Result<T, AppError>`** (requisito de Tauri para préstamos en async).
7. Trabajo bloqueante (walk, grep, git, sysinfo, kill) → `tokio::task::spawn_blocking` o `std::thread`. Lectura de PTY → `std::thread` dedicado.
8. **PATH en macOS**: una app GUI lanzada desde Finder no hereda el PATH del shell. Al arrancar se ejecuta `$SHELL -ilc 'printf "%s" "$PATH"'` (timeout 3 s) y se fija `std::env::set_var("PATH", ...)` antes de crear nada (`env_path.rs`). Sin esto no se encuentran `git`, `npm`, `pyright-langserver`, etc.
9. **Windows**: todo `Command` (git, taskkill, netstat) con `creation_flags(0x08000000)` (CREATE_NO_WINDOW) para no abrir consolas.
10. Estado del frontend: `zustand`. Los `EditorState` de CodeMirror y los arrays grandes de logs viven **fuera** de zustand (registro en módulo) y zustand solo guarda metadatos + un contador `tick` para re-render.
11. Commits: nunca `Co-Authored-By` de Claude.

---

## 1. Versiones

### 1.1 Rust (`src-tauri/Cargo.toml`) — se escribe COMPLETO en el Bloque 1

```toml
[package]
name = "caesar-code"
version = "0.1.0"
edition = "2021"
rust-version = "1.80"

[lib]
name = "caesar_code_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2.7", features = [] }

[dependencies]
tauri = { version = "2.12", features = [] }
tauri-plugin-dialog = "2.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
portable-pty = "0.9"
sysinfo = "0.33.1"
notify = "8"
notify-debouncer-mini = "0.6"
ignore = "0.4"
grep-searcher = "0.1"
grep-regex = "0.1"
grep-matcher = "0.1"
lsp-types = "=0.95.1"          # usa url::Url (0.96+ cambió a fluent_uri, NO subir)
url = "2"
fuzzy-matcher = "0.3.7"
which = "7"
dotenvy = "0.15"
strip-ansi-escapes = "0.2"
trash = "5"
uuid = { version = "1", features = ["v4"] }
regex = "1"
log = "0.4"

[target.'cfg(unix)'.dependencies]
libc = "0.2"

[profile.dev]
incremental = true

[profile.dev.package."*"]
opt-level = 2

[profile.release]
codegen-units = 1
lto = true
opt-level = 3
panic = "abort"
strip = true
```

Notas: `notify-debouncer-mini 0.6` depende de `notify ^8` (comprobado). `sysinfo` se fija en 0.33.1 (API conocida: `ProcessesToUpdate`, `ProcessRefreshKind::nothing()`); no subir. `lsp-types` fijado exacto.

### 1.2 npm (`package.json`) — se escribe COMPLETO en el Bloque 1

```json
{
  "name": "caesar-code",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "~2.12.5",
    "@tauri-apps/plugin-dialog": "~2.8.4",
    "react": "18.3.1",
    "react-dom": "18.3.1",
    "zustand": "5.0.3",
    "react-resizable-panels": "2.1.7",
    "lucide-react": "0.469.0",
    "@tanstack/react-virtual": "3.11.2",
    "@xterm/xterm": "5.5.0",
    "@xterm/addon-webgl": "0.18.0",
    "@xterm/addon-fit": "0.10.0",
    "@fontsource/jetbrains-mono": "5.3.0",
    "@codemirror/state": "^6.5.2",
    "@codemirror/view": "^6.36.2",
    "@codemirror/commands": "^6.8.0",
    "@codemirror/language": "^6.10.8",
    "@codemirror/autocomplete": "^6.18.4",
    "@codemirror/lint": "^6.8.4",
    "@codemirror/search": "^6.5.8",
    "@codemirror/lang-python": "^6.1.7",
    "@codemirror/lang-javascript": "^6.2.2",
    "@codemirror/lang-rust": "^6.0.1",
    "@codemirror/lang-java": "^6.0.1",
    "@codemirror/lang-json": "^6.0.1",
    "@codemirror/lang-yaml": "^6.1.2",
    "@codemirror/lang-markdown": "^6.3.2",
    "@lezer/highlight": "^1.2.1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "~2.12.6",
    "@types/react": "18.3.18",
    "@types/react-dom": "18.3.5",
    "@types/node": "22.20.5",
    "@vitejs/plugin-react": "4.7.0",
    "typescript": "5.9.3",
    "vite": "6.4.6",
    "tailwindcss": "3.4.17",
    "postcss": "8.4.49",
    "autoprefixer": "10.4.20"
  }
}
```

Reglas:
- `@tauri-apps/api` y crate `tauri` deben coincidir en **major.minor** (2.12); `@tauri-apps/plugin-dialog` y crate `tauri-plugin-dialog` en 2.8. La CLI aborta si no coinciden.
- Los `@codemirror/*` con `^` para que npm deduplique: **una sola copia de `@codemirror/state`** (si hay dos, CM lanza "Unrecognized extension value"). Verificar con `npm ls @codemirror/state`.
- NO usar el meta-paquete `codemirror` ni `@codemirror/lsp-client` (el LSP va en Rust).
- Tailwind 3.4 (config JS + PostCSS), no Tailwind 4.

### 1.3 Plugins Tauri
- `tauri-plugin-dialog` (selector de carpeta raíz). Único plugin. Portapapeles con `navigator.clipboard.writeText` (gesto de usuario en menú contextual) con fallback `document.execCommand('copy')`.

---

## 2. Estructura de carpetas

```
ROOT/
├─ SPEC.md  PLAN.md  README.md  .gitignore
├─ package.json  package-lock.json  tsconfig.json  tsconfig.node.json
├─ vite.config.ts  tailwind.config.js  postcss.config.js  index.html
├─ scripts/make-icon.py                 # genera src-tauri/app-icon.png (1024x1024 ámbar sobre #0f0f0f)
├─ src/
│  ├─ main.tsx                          # sin StrictMode; importa fuentes y index.css
│  ├─ App.tsx                           # atajos globales + <AppLayout/>
│  ├─ index.css                         # @tailwind + variables + estilos scrollbar/xterm
│  ├─ lib/
│  │  ├─ types.ts                       # TODOS los tipos compartidos (§3.4)
│  │  ├─ ipc.ts                         # wrappers tipados de invoke + helpers Channel
│  │  ├─ paths.ts                       # basename, dirname, relativeTo, extOf, isSameOrInside
│  │  ├─ keys.ts                        # isMod(e) (meta en mac, ctrl en resto), useGlobalShortcut
│  │  └─ errors.ts                      # toMessage(err: unknown): string (AppError → texto)
│  ├─ store/
│  │  ├─ ui.ts                          # paneles, pestaña inferior, modales abiertos, toasts
│  │  ├─ workspace.ts                   # root, árbol (children por dir, expandidos), git
│  │  ├─ editor.ts                      # pestañas, activo, dirty, cursor; openFile/openFileAt/save/close
│  │  ├─ editorRegistry.ts              # Map<path, EditorState> + Map<path, EditorView|null> (NO zustand)
│  │  ├─ services.ts                    # configs, statuses, selección
│  │  ├─ logs.ts                        # buffers de logs fuera de zustand + tick
│  │  └─ lsp.ts                         # estado servidores + diagnósticos por path
│  ├─ components/
│  │  ├─ ui/  Button.tsx  IconButton.tsx  Select.tsx  ContextMenu.tsx  Modal.tsx
│  │  │       Badge.tsx  TextInput.tsx  Toasts.tsx  Kbd.tsx
│  │  ├─ layout/ AppLayout.tsx  StatusBar.tsx  WelcomeView.tsx  ResizeHandle.tsx
│  │  ├─ sidebar/ Sidebar.tsx  FileTree.tsx  FileTreeNode.tsx  FileIcon.tsx  InlineNameInput.tsx
│  │  ├─ palette/ QuickOpen.tsx  GlobalSearch.tsx
│  │  ├─ editor/ EditorArea.tsx  TabBar.tsx  CodeEditor.tsx  theme.ts  languages.ts
│  │  │          lsp/ completion.ts  hover.ts  definition.ts  diagnostics.ts  sync.ts  snippet.ts
│  │  └─ bottom/ BottomPanel.tsx  ServicesPanel.tsx  ServiceRow.tsx  ServiceModal.tsx
│  │             LogViewer.tsx  TerminalPanel.tsx  TerminalView.tsx
│  └─ vite-env.d.ts
└─ src-tauri/
   ├─ Cargo.toml  build.rs  tauri.conf.json  app-icon.png
   ├─ capabilities/default.json
   ├─ icons/                            # generados con `npx tauri icon src-tauri/app-icon.png`
   └─ src/
      ├─ main.rs                        # caesar_code_lib::run()
      ├─ lib.rs                         # Builder, plugins, manage, invoke_handler, RunEvent
      ├─ error.rs                       # AppError
      ├─ state.rs                       # AppState + shutdown()
      ├─ env_path.rs                    # fix PATH (macOS/Linux)
      ├─ util/ mod.rs  paths.rs  proc.rs   # ensure_in_workspace, to_utf16_col, no_window(cmd)
      ├─ app_cmds.rs                    # app_info
      ├─ workspace/ mod.rs  commands.rs
      ├─ fs/ mod.rs  tree.rs  ops.rs  watcher.rs  search.rs  quick_open.rs  commands.rs
      ├─ git/ mod.rs  commands.rs
      ├─ services/ mod.rs  config.rs  detect.rs  manager.rs  process_tree.rs
      │            metrics.rs  logs.rs  ports.rs  commands.rs
      ├─ terminal/ mod.rs  commands.rs
      └─ lsp/ mod.rs  transport.rs  client.rs  manager.rs  servers.rs  convert.rs  commands.rs
```

---

## 3. Contratos

### 3.1 AppError (`src-tauri/src/error.rs`, Bloque 1)

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("I/O: {0}")] Io(#[from] std::io::Error),
    #[error("JSON: {0}")] Json(#[from] serde_json::Error),
    #[error("No hay workspace abierto")] NoWorkspace,
    #[error("Ruta fuera del workspace: {0}")] OutsideWorkspace(String),
    #[error("No encontrado: {0}")] NotFound(String),
    #[error("Ya existe: {0}")] AlreadyExists(String),
    #[error("Argumento inválido: {0}")] InvalidArgument(String),
    #[error("Git: {0}")] Git(String),
    #[error("Servicio: {0}")] Service(String),
    #[error("PTY: {0}")] Pty(String),
    #[error("LSP: {0}")] Lsp(String),
    #[error("Watcher: {0}")] Watch(String),
    #[error("{0}")] Other(String),
}
pub type AppResult<T> = Result<T, AppError>;
```
`impl Serialize for AppError` → `{ "kind": "<Variante>", "message": self.to_string() }` (serializar a mano con `serialize_struct`). `impl From<notify::Error>`, `From<anyhow-like String>` no; usar `.map_err(|e| AppError::Pty(e.to_string()))` para `portable-pty` (devuelve `anyhow::Error`).
TS: `interface AppError { kind: string; message: string }`; `toMessage(e)` devuelve `e.message` si existe, si no `String(e)`.

### 3.2 AppState (`src-tauri/src/state.rs`)

```rust
pub struct AppState {
    // Bloque 1
    pub workspace: std::sync::RwLock<Option<PathBuf>>,          // raíz canonicalizada
    // Bloque 2
    pub watcher: std::sync::Mutex<Option<WatcherHandle>>,        // drop = deja de vigilar
    pub file_index: std::sync::Mutex<FileIndex>,                 // cache Quick Open + flag dirty
    pub search_gen: std::sync::atomic::AtomicU64,                // cancelación de búsquedas
    pub git_last_run: std::sync::Mutex<Option<std::time::Instant>>,
    // Bloque 3
    pub services: Arc<ServiceManager>,
    pub terminals: Arc<TerminalManager>,
    // Bloque 4
    pub lsp: Arc<LspManager>,
}
impl AppState {
    pub fn root(&self) -> AppResult<PathBuf>;   // Err(NoWorkspace) si None
    pub fn shutdown(&self);                     // síncrono; ver §3.6
}
```

### 3.3 Comandos Tauri (todos devuelven `Result<T, AppError>`)

Nombre Rust (snake) → permiso generado `allow-<nombre-con-guiones>`; args en invoke en camelCase.

| Bloque | Comando | Args | Retorno |
|---|---|---|---|
| 1 | `app_info` | — | `AppInfo { version, platform: "macos"\|"windows"\|"linux", pathSep }` |
| 2 | `workspace_open` | `path: String` | `WorkspaceInfo { root, name }` (canonicaliza, resetea índice, para watcher previo; Bloque 3 recarga servicios, Bloque 4 cierra LSPs del root anterior) |
| 2 | `workspace_current` | — | `Option<WorkspaceInfo>` |
| 2 | `fs_read_dir` | `path` | `Vec<FsEntry>` (dirs primero, alfabético case-insensitive; omite `.git`, `.DS_Store`) |
| 2 | `fs_read_file` | `path` | `FileContent { path, text, isBinary, tooLarge }` (>10 MB → tooLarge, text ""; NUL en primeros 8 KB → isBinary) |
| 2 | `fs_write_file` | `path, text` | `()` (escritura atómica: tmp en mismo dir + rename) |
| 2 | `fs_create_file` | `parent, name` | `String` (ruta nueva; AlreadyExists si existe) |
| 2 | `fs_create_dir` | `parent, name` | `String` |
| 2 | `fs_rename` | `path, new_name` | `String` (ruta nueva) |
| 2 | `fs_delete` | `path` | `()` (envía a la papelera con `trash::delete`) |
| 2 | `fs_watch_subscribe` | `on_event: Channel<FsEvent>` | `()` (vigila el root actual; reemplaza al anterior) |
| 2 | `git_status` | — | `GitStatus` |
| 2 | `quick_open` | `query, limit: u32` | `Vec<QuickOpenItem>` |
| 2 | `search_global` | `query, options: SearchOptions` | `SearchResult` |
| 3 | `services_load` | — | `Vec<ServiceConfig>` |
| 3 | `services_save` | `services: Vec<ServiceConfig>` | `()` (escribe `.caesar/services.json`) |
| 3 | `services_detect` | — | `Vec<ServiceConfig>` (sugerencias, no guarda) |
| 3 | `services_subscribe` | `on_log: Channel<Vec<LogLine>>, on_status: Channel<Vec<ServiceStatus>>` | `()` |
| 3 | `service_start` / `service_stop` / `service_restart` | `id` | `()` |
| 3 | `services_start_all` / `services_stop_all` | — | `()` |
| 3 | `services_status` | — | `Vec<ServiceStatus>` |
| 3 | `service_logs` | `id: Option<String>, limit: u32` | `Vec<LogLine>` (backlog; None = todos ordenados por seq) |
| 3 | `service_clear_logs` | `id: Option<String>` | `()` |
| 3 | `port_kill` | `port: u16` | `Vec<u32>` (pids matados) |
| 3 | `terminal_open` | `cwd: Option<String>, cols: u16, rows: u16, on_event: Channel<TerminalEvent>` | `String` (id uuid) |
| 3 | `terminal_write` | `id, data: String` | `()` |
| 3 | `terminal_resize` | `id, cols, rows` | `()` |
| 3 | `terminal_close` | `id` | `()` |
| 4 | `lsp_subscribe` | `on_event: Channel<LspEvent>` | `()` |
| 4 | `lsp_did_open` | `path, text` | `LspDocStatus` |
| 4 | `lsp_did_change` | `path, version: i32, text` | `()` (full sync) |
| 4 | `lsp_did_save` | `path, text` | `()` |
| 4 | `lsp_did_close` | `path` | `()` |
| 4 | `lsp_completion` | `path, line: u32, character: u32` | `Vec<CompletionDto>` |
| 4 | `lsp_hover` | `path, line, character` | `Option<HoverDto>` |
| 4 | `lsp_definition` | `path, line, character` | `Vec<LocationDto>` |
| 4 | `lsp_status` | — | `Vec<LspServerStatus>` |

### 3.4 Tipos compartidos (Rust DTO ↔ `src/lib/types.ts`)

Todos camelCase en JSON. Las rutas son `string` nativas absolutas.

```ts
export interface AppInfo { version: string; platform: 'macos'|'windows'|'linux'; pathSep: string }
export interface WorkspaceInfo { root: string; name: string }
export interface FsEntry { name: string; path: string; isDir: boolean; isSymlink: boolean; size: number; ext: string | null }
export interface FileContent { path: string; text: string; isBinary: boolean; tooLarge: boolean }
export type FsEvent =
  | { type: 'changed'; paths: string[] }      // rutas afectadas (deduplicadas); el front recarga dirs padre expandidos
  | { type: 'gitChanged' }                     // .git/HEAD|index|refs cambió
  | { type: 'error'; message: string };
export type GitFileStatus = 'M' | 'U' | 'D' | 'A' | 'R';
export interface GitStatus { isRepo: boolean; branch: string | null; files: Record<string, GitFileStatus> } // clave = ruta absoluta nativa
export interface QuickOpenItem { path: string; relPath: string; score: number; indices: number[] } // indices sobre relPath (chars)
export interface SearchOptions { caseSensitive: boolean; regex: boolean; wholeWord: boolean; include?: string | null; maxResults?: number | null }
export interface SearchMatch { path: string; relPath: string; line: number /*1-based*/; column: number /*0-based UTF-16*/;
  preview: string; matchStart: number; matchEnd: number /*UTF-16 sobre preview*/ }
export interface SearchResult { matches: SearchMatch[]; truncated: boolean; filesSearched: number; cancelled: boolean }

export interface ServiceConfig { id: string; name: string; command: string; cwd: string /*relativo al root, "" = root*/;
  env: Record<string,string>; envFile: string | null /*relativo a cwd*/; port: number | null; kind: ServiceKind }
export type ServiceKind = 'node'|'python'|'rust'|'maven'|'gradle'|'docker'|'custom';
export type ServiceState = 'stopped'|'starting'|'running'|'stopping'|'error';
export interface ServiceStatus { id: string; state: ServiceState; pid: number | null; exitCode: number | null;
  cpu: number /*% suma árbol*/; memoryMb: number /*suma árbol*/; portOpen: boolean; port: number | null; startedAt: number | null /*epoch ms*/; message: string | null }
export type LogLevel = 'error'|'warn'|'info';
export interface LogLine { seq: number; serviceId: string; ts: number /*epoch ms*/; level: LogLevel; text: string /*sin ANSI*/ }
export type TerminalEvent = { type: 'data'; data: string } | { type: 'exit'; code: number | null };

export type LspState = 'starting'|'ready'|'unavailable'|'crashed';
export interface LspServerStatus { language: string; server: string; state: LspState; message: string | null }
export interface LspDocStatus { language: string | null; server: string | null; available: boolean }
export interface DiagnosticDto { fromLine: number; fromChar: number; toLine: number; toChar: number; // 0-based, UTF-16
  severity: 'error'|'warning'|'info'|'hint'; message: string; source: string | null }
export type LspEvent =
  | { type: 'diagnostics'; path: string; diagnostics: DiagnosticDto[] }
  | { type: 'status'; status: LspServerStatus };
export interface TextEditDto { fromLine: number; fromChar: number; toLine: number; toChar: number; newText: string }
export interface CompletionDto { label: string; kind: string | null /*'function'|'method'|'variable'|'class'|'keyword'|...*/;
  detail: string | null; documentation: string | null; insertText: string; isSnippet: boolean;
  sortText: string | null; filterText: string | null; textEdit: TextEditDto | null }
export interface HoverDto { markdown: string }
export interface LocationDto { path: string; line: number; character: number; endLine: number; endCharacter: number }
```

Rust: mismos structs en el módulo que los produce (`fs/mod.rs`, `git/mod.rs`, `services/mod.rs`, `terminal/mod.rs`, `lsp/mod.rs`), `#[derive(Serialize, Deserialize, Clone, Debug)]`, `#[serde(rename_all = "camelCase")]`. Enums string (`ServiceState`, `LogLevel`, etc.) con `#[serde(rename_all = "lowercase")]` / `camelCase`. `GitFileStatus` serializa como `"M"` etc. con `#[serde(rename = "M")]` por variante.

### 3.5 Channels

| Canal | Tipo | Productor | Ritmo |
|---|---|---|---|
| `fs_watch_subscribe.onEvent` | `FsEvent` | debouncer 200 ms | por lote |
| `services_subscribe.onLog` | `Vec<LogLine>` | flusher | cada 50 ms si hay líneas (máx 2000/lote) |
| `services_subscribe.onStatus` | `Vec<ServiceStatus>` | metrics loop | cada 1000 ms + inmediato en cambios de estado |
| `terminal_open.onEvent` | `TerminalEvent` | hilo lector PTY | por lectura (buffer 16 KB) |
| `lsp_subscribe.onEvent` | `LspEvent` | lector LSP | por notificación |

TS: `import { Channel, invoke } from '@tauri-apps/api/core'`; `const ch = new Channel<T>(); ch.onmessage = fn;` **antes** del `invoke`. Rust: si `channel.send(x)` devuelve `Err`, se descarta la suscripción (webview recargado).

### 3.6 Shutdown
`lib.rs`: `.build(tauri::generate_context!())?.run(|app, ev| if let RunEvent::Exit = ev { app.state::<AppState>().shutdown() })`. Además en `RunEvent::ExitRequested` no se hace nada especial. `shutdown()` (síncrono, se ejecuta en hilo principal, NO usar `block_on` dentro de un hilo de tokio):
1. `services.stop_all_blocking()` (kill de árboles, máx 3 s total).
2. `terminals.close_all()` (kill de cada child + árbol).
3. `lsp.shutdown_blocking()` (envía `shutdown`/`exit` con timeout 1 s vía `tauri::async_runtime::block_on`, luego `kill`).
4. `watcher.take()` (drop).

### 3.7 Capabilities y permisos de comandos propios

`build.rs`:
```rust
const COMMANDS: &[&str] = &["app_info", /* cada bloque añade los suyos */];
fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    ).expect("tauri build");
}
```
`capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Ventana principal",
  "windows": ["main"],
  "permissions": ["core:default", "dialog:allow-open", "allow-app-info"]
}
```
Trampa: con `app_manifest` activo, **todo comando no listado en `COMMANDS` y en la capability falla en runtime** con "not allowed". Cada bloque añade sus comandos en ambos sitios. Tras `cargo check`, confirmar el nombre exacto del permiso en `src-tauri/gen/schemas/acl-manifests.json` (debería ser `allow-fs-read-dir` para `fs_read_dir`; si el esquema genera otro nombre, usar ese).

### 3.8 `tauri.conf.json` (Bloque 1)
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "CaesarCode",
  "version": "0.1.0",
  "identifier": "com.caesarcode.app",
  "build": { "beforeDevCommand": "npm run dev", "devUrl": "http://localhost:1420",
             "beforeBuildCommand": "npm run build", "frontendDist": "../dist" },
  "app": {
    "windows": [{ "label": "main", "title": "CaesarCode", "width": 1440, "height": 900,
                  "minWidth": 900, "minHeight": 560, "backgroundColor": "#0f0f0f" }],
    "security": { "csp": "default-src 'self'; connect-src 'self' ipc: http://ipc.localhost; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset: http://asset.localhost; font-src 'self' data:; script-src 'self'" }
  },
  "bundle": { "active": true, "targets": "all",
    "icon": ["icons/32x32.png","icons/128x128.png","icons/128x128@2x.png","icons/icon.icns","icons/icon.ico"] }
}
```
`style-src 'unsafe-inline'` es obligatorio (CodeMirror y xterm inyectan `<style>`).

---

## 4. Paleta y UI comunes

`tailwind.config.js` → `theme.extend.colors`:
`base #0f0f0f`, `panel #161616`, `border #262626`, `fg #ededed`, `muted #888888`, `accent #f59e0b`, `accent-hover #d97706`, `ok #22c55e`, `danger #ef4444`, `hover #1f1f1f` (filas), `selection #f59e0b33`.
`fontFamily.mono = ['"JetBrains Mono"', '"Geist Mono"', 'ui-monospace', 'monospace']`, `fontFamily.sans = ['Inter', 'system-ui', 'sans-serif']`. Tamaños: UI 12–13 px, editor 13 px.
Componentes `ui/`: botones planos (borde 1 px `border`, hover `hover`), `Select` custom (popover con lista, teclado ↑↓ Enter Esc, sin `<select>`), `ContextMenu` (posicionado en `clientX/Y`, cierra con Esc/click fuera), `Modal` (overlay `#000a`, panel `panel`, foco atrapado), `Toasts` (errores de `AppError`). Scrollbars finos oscuros en `index.css`.

---

## 5. Bloques

Verificación estándar de cada bloque (desde `ROOT`):
```
npm install            # solo Bloque 1 o si cambia package.json
npm run build          # tsc -b && vite build — 0 errores
(cd src-tauri && cargo check)            # 0 errores
(cd src-tauri && cargo clippy -- -D warnings)   # deseable; no bloqueante salvo errores
npm ls @codemirror/state                 # una sola versión
```
Prueba manual (la hace el orquestador): `npm run tauri dev`.

---

### BLOQUE 1 — Scaffolding, AppState, AppError, layout base

**Archivos**: `package.json`, `tsconfig.json` (strict, `noUnusedLocals`, `noUnusedParameters`, `jsx: react-jsx`, `moduleResolution: bundler`, `target ES2022`, references a `tsconfig.node.json`), `tsconfig.node.json` (para `vite.config.ts`, `composite: true`), `vite.config.ts` (react plugin; `server: { port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } }`, `clearScreen: false`, `build.target: 'es2022'`), `tailwind.config.js`, `postcss.config.js`, `index.html` (`<html class="dark">`, body `bg-base`), `src/main.tsx`, `src/App.tsx`, `src/index.css`, `src/vite-env.d.ts`, `src/lib/{types.ts,ipc.ts,paths.ts,keys.ts,errors.ts}`, `src/store/ui.ts`, `src/components/ui/*`, `src/components/layout/{AppLayout,StatusBar,WelcomeView,ResizeHandle}.tsx`, `scripts/make-icon.py`, `src-tauri/*` (Cargo.toml completo, build.rs, tauri.conf.json, capabilities, `src/{main,lib,error,state,env_path,app_cmds}.rs`, `src/util/{mod,paths,proc}.rs`), `.gitignore` (`node_modules`, `dist`, `src-tauri/target`, `src-tauri/gen/schemas`? NO: `gen/schemas` se versiona), `README.md` breve.

**Orden**:
1. `git init` en ROOT (si no existe). Escribir `package.json` y `Cargo.toml` completos (§1). `npm install`.
2. Configs front (vite/ts/tailwind/postcss/index.html).
3. Icono: `python3 scripts/make-icon.py` (genera PNG 1024 con `struct`+`zlib`, sin dependencias: cuadrado `#0f0f0f` con círculo/rombo `#f59e0b`) → `npx tauri icon src-tauri/app-icon.png`. Sin iconos, `generate_context!` falla en `cargo check`.
4. Rust: `error.rs`, `state.rs` (solo `workspace` + `root()` + `shutdown()` que de momento solo hace `take` de lo existente), `env_path.rs`, `util/paths.rs` (`ensure_in_workspace(root, p) -> AppResult<PathBuf>`: canonicaliza `p` si existe, si no canonicaliza el padre y re-une el nombre; comprueba `starts_with(root)`; `utf16_len(s: &str) -> usize`), `util/proc.rs` (`fn no_window(cmd: &mut std::process::Command)` y versión tokio; no-op fuera de Windows), `app_cmds.rs` (`app_info`), `lib.rs` (`env_path::fix_path()` primero, plugin dialog, `manage`, `invoke_handler(generate_handler![app_cmds::app_info])`, `run` con RunEvent::Exit → shutdown), `main.rs` (`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`).
5. `build.rs` + capability (§3.7).
6. Front: `types.ts` completo (§3.4, aunque los bloques siguientes lo usen), `ipc.ts` con `appInfo()` (el resto lo añade cada bloque), `ui` store, componentes ui, `AppLayout`:
   - `PanelGroup direction="horizontal" autoSaveId="cc-main"`: `Panel` Sidebar (defaultSize 20, minSize 12, collapsible) | handle | `PanelGroup direction="vertical" autoSaveId="cc-center"`: `Panel` editor (70) | handle | `Panel` bottom (30, minSize 10, collapsible).
   - Sidebar placeholder real: `WelcomeView`-style botón "Open Folder" que abre `open({ directory: true })` del plugin dialog y guarda la ruta en `ui` store (Bloque 2 la conectará a `workspace_open`). No es stub: muestra la ruta elegida.
   - Área editor: `WelcomeView` (logo texto "CaesarCode", atajos ⌘P / ⌘⇧F / ⌘S con `Kbd`).
   - Panel inferior: barra de pestañas `Services | Logs | Terminal` (estado en `ui` store), contenido vacío con texto "Abre una carpeta" hasta Bloque 3.
   - `StatusBar` (h-6, `panel`, borde superior): versión y plataforma de `app_info`, hueco para rama git, LSP, cursor.
   - `ResizeHandle`: 1 px `border`, hover `accent`.
7. Verificación.

**Criterios de aceptación**: `npm run build` y `cargo check` sin errores; `npm run tauri dev` abre ventana oscura con 3 paneles redimensionables que persisten tamaño; diálogo nativo de carpeta funciona; StatusBar muestra versión/plataforma reales; ningún `<select>` nativo.

**Trampas**: iconos ausentes → error en `generate_context!`; versión `@tauri-apps/api` ≠ crate; `tsc -b` falla por variables no usadas; `PATH` en macOS; capability sin `dialog:allow-open` → el diálogo falla en silencio (promesa rechazada); `crate-type` con `staticlib/cdylib/rlib` y `main.rs` llamando a `caesar_code_lib::run()`.

**Prompt coder Bloque 1** (copiar tal cual):
> Eres el coder del Bloque 1 de CaesarCode. Proyecto en `/Users/cesargarrido/Proyectos/caesarCode`. Lee `SPEC.md` y en `PLAN.md` las secciones 0, 1, 2, 3 (entera), 4 y "BLOQUE 1". Implementa exactamente el Bloque 1 siguiendo el orden indicado: `git init`, `package.json` y `src-tauri/Cargo.toml` COMPLETOS con las versiones de §1 (no cambies versiones; si una no resuelve, usa la más cercana del mismo major.minor y avísalo), configs de Vite (puerto 1420 strictPort)/TS strict/Tailwind 3.4 con la paleta de §4, `scripts/make-icon.py` + `npx tauri icon`, Rust (`error.rs` con el AppError de §3.1 serializado como `{kind,message}`, `state.rs` solo con el campo `workspace`, `env_path.rs`, `util/paths.rs`, `util/proc.rs`, `app_cmds.rs` con `app_info`, `lib.rs`, `main.rs`), `build.rs` con `AppManifest::commands(COMMANDS)` y `capabilities/default.json` de §3.7, `tauri.conf.json` de §3.8, y el frontend: `src/lib/types.ts` con TODOS los tipos de §3.4, `ipc.ts`, `paths.ts`, `keys.ts`, `errors.ts`, store `ui.ts`, componentes `ui/` (Select custom sin `<select>`, ContextMenu, Modal, Toasts, Button, IconButton, Badge, TextInput, Kbd) y layout con react-resizable-panels 2.1.7 (`PanelGroup/Panel/PanelResizeHandle`, `autoSaveId`), StatusBar real y botón Open Folder con `@tauri-apps/plugin-dialog`. Sin `React.StrictMode`. Sin funciones vacías ni mocks. No crees aún módulos Rust de bloques posteriores. Al terminar ejecuta `npm run build` y `cd src-tauri && cargo check` y corrige hasta 0 errores. No hagas commit. Devuelve: lista de archivos creados, salida final de ambos comandos (resumida) y cualquier desviación del plan.

---

### BLOQUE 2 — Sistema de archivos, Git, Quick Open, Global Search, editor mínimo

**Rust**:
- `workspace/commands.rs`: `workspace_open` (canonicaliza, `is_dir`, guarda en state, `file_index.mark_dirty()`, suelta watcher previo), `workspace_current`.
- `fs/mod.rs`: DTOs (`FsEntry`, `FileContent`, `FsEvent`, `QuickOpenItem`, `SearchOptions`, `SearchMatch`, `SearchResult`).
- `fs/tree.rs`: `read_dir` ordenado. Omite `.git` y `.DS_Store`; NO aplica .gitignore en el árbol (se ve `node_modules`, perezoso).
- `fs/ops.rs`: `read_file` (límite 10 MB, detección binario, `String::from_utf8_lossy`), `write_file` atómico (`<name>.cc-tmp-<uuid>` + `rename`), create/rename/delete (`trash::delete`). Validar `name` (sin `/`, `\`, `..`, no vacío) → `InvalidArgument`.
- `fs/watcher.rs`: `WatcherHandle { _debouncer: Debouncer<RecommendedWatcher> }`. `new_debouncer(Duration::from_millis(200), move |res: DebounceEventResult| ...)`; `debouncer.watcher().watch(&root, RecursiveMode::Recursive)`. Clasificación: rutas bajo `.git/` → si terminan en `HEAD`, `index`, o están bajo `refs/` → `gitChanged` (ignorar si `git_last_run` < 700 ms para evitar bucle); resto de `.git/` se ignora. Rutas bajo `node_modules/`, `target/`, `.cc-tmp-` se envían (el front filtra por expandidos) salvo los tmp. Envía `changed` con rutas dedup; además marca `file_index` dirty. El closure captura un `Arc<Mutex<Option<Channel<FsEvent>>>>` y un `AppHandle` (para leer state). Error de envío → poner el slot a `None`.
- `fs/quick_open.rs`: `FileIndex { files: Vec<(PathBuf, String /*rel con '/'*/)>, dirty: bool }`. Rebuild con `ignore::WalkBuilder::new(root).hidden(false).git_ignore(true).git_global(true).git_exclude(true).filter_entry(|e| e.file_name() != ".git").build()`, máx 200 000 archivos. Búsqueda: `SkimMatcherV2::default().fuzzy_indices(rel, query)`; bonus si el `basename` contiene la query; orden por score desc, luego longitud; `limit`. Query vacía → primeros `limit` archivos (o recientes del front). Ejecutar en `spawn_blocking`.
- `fs/search.rs`: `RegexMatcherBuilder::new().case_insensitive(!cs).word(whole_word).fixed_strings(!regex).build(query)`; `SearcherBuilder::new().line_number(true).binary_detection(BinaryDetection::quit(b'\x00')).build()`; walker `ignore` igual que quick_open (si `include` presente, `OverrideBuilder` con ese glob). Sink `grep_searcher::sinks::UTF8(|lnum, line| { ... })`: localizar match con `matcher.find(line.as_bytes())`, `column = utf16_len(&line[..m.start()])`, `preview` = línea sin `\n`, recortada a 300 chars alrededor del match (recalcular `matchStart/End` en UTF-16 sobre el preview). Máx `maxResults` (default 2000) → `truncated`. Cancelación: al empezar `let gen = search_gen.fetch_add(1)+1`; en cada archivo si `search_gen.load() != gen` → `cancelled: true` y salir. Query vacía → resultado vacío. Regex inválida → `InvalidArgument`.
- `git/mod.rs`: `run_git(root, args) -> AppResult<String>` con `tokio::process::Command::new("git")`, `-c core.quotepath=false --no-optional-locks`, `no_window`, actualiza `git_last_run`. `git_status`: si `git rev-parse --is-inside-work-tree` falla → `{isRepo:false}`. `git status --porcelain=v1 -z --untracked-files=all` → parse por `\0`: `XY path`; si X o Y es `R`/`C`, el siguiente token es la ruta original (consumirlo). Mapear: `??` → `U`; contiene `D` → `D`; `A` → `A`; `R` → `R`; resto → `M`. Ruta: `root.join(rel)` (convertir `/` a componentes) como String nativo. Rama: `git rev-parse --abbrev-ref HEAD` (si `HEAD` → "detached"). Si `git` no está en PATH → `isRepo:false` (no error).
- Añadir a `generate_handler!`, `COMMANDS` y capability: `workspace_open, workspace_current, fs_read_dir, fs_read_file, fs_write_file, fs_create_file, fs_create_dir, fs_rename, fs_delete, fs_watch_subscribe, git_status, quick_open, search_global`.

**Frontend**:
- `ipc.ts`: wrappers de todos los comandos del bloque.
- `store/workspace.ts`: `root`, `name`, `children: Record<dirPath, FsEntry[]>`, `expanded: Set<string>` (o Record), `selected`, `git: GitStatus`, acciones `openWorkspace(path)` (invoke, guarda en `localStorage['cc.lastWorkspace']`, carga raíz, suscribe watcher, carga git), `toggleDir`, `refreshDir`, `refreshGit` (debounce 300 ms), `handleFsEvent(ev)` (para cada path: si su padre está cargado → `refreshDir(padre)`; si el path es un archivo abierto y no dirty → recargar contenido en el editor; luego `refreshGit`). Al iniciar App: si hay `cc.lastWorkspace` → abrir.
- `Sidebar.tsx`: cabecera con nombre del workspace + botones (Open Folder, New File, New Folder, Collapse All). `FileTree` virtualizado NO necesario; render recursivo de `FileTreeNode` (indentación 12 px, chevron, `FileIcon` por extensión con iconos lucide: `FileCode`, `FileJson`, `FileText`, `Folder/FolderOpen`, `Settings` para configs, color por lenguaje).
- Badges git: archivo con estado → letra a la derecha y color del nombre (`M` accent, `U`/`A` ok, `D` danger, `R` accent). Carpeta: punto ámbar si algún descendiente tiene estado (`isSameOrInside`).
- Menú contextual (clic derecho en nodo o fondo): New File, New Folder, Rename, Delete (con `Modal` de confirmación: "Se moverá a la papelera"), Copy Path, Copy Relative Path. New/Rename usan `InlineNameInput` dentro del árbol (Enter confirma, Esc cancela). Tras la operación: `refreshDir`; si se renombra/borra un archivo abierto → actualizar/cerrar pestaña.
- `QuickOpen.tsx` (`Mod+P`, captura en `window` con `capture: true` y `preventDefault` para evitar imprimir): modal arriba-centro, input, lista con resaltado de `indices`, ↑↓ Enter Esc, debounce 60 ms, `limit 50`.
- `GlobalSearch.tsx` (`Mod+Shift+F`): se muestra como vista en el Sidebar (pestaña Files/Search en la cabecera del sidebar). Input + toggles `Aa`, `ab` (palabra), `.*`, campo include; debounce 250 ms; resultados agrupados por archivo (relPath + contador), líneas con match resaltado; clic → `openFileAt(path, line, column)`.
- Editor mínimo (se completa en Bloque 4): `store/editor.ts` (`tabs: {path,name,dirty}[]`, `active`, `openFile(path)`, `openFileAt(path,line,col)`, `save(path)`, `close(path)`, `markDirty`), `editorRegistry.ts`, `EditorArea.tsx` + `TabBar.tsx` (punto `•` si dirty, `X` al hover, clic medio cierra, cerrar dirty pide confirmación con `Modal`), `CodeEditor.tsx` con UN `EditorView` reutilizado: al cambiar de pestaña `view.setState(registry.get(path))` (guardando antes el estado saliente). Extensiones mínimas en `theme.ts`/`CodeEditor.tsx`: `lineNumbers, highlightActiveLine, history, drawSelection, keymap(defaultKeymap, historyKeymap)`, tema Graphite & Amber, `Mod-s` → `save`. `openFileAt` hace `view.dispatch({ selection: {anchor: pos}, effects: EditorView.scrollIntoView(pos, {y:'center'}) })` y `view.focus()`. Binario/tooLarge → panel informativo en vez de editor.
- `StatusBar`: rama git y Ln/Col.

**Criterios de aceptación**: abrir carpeta muestra árbol perezoso; crear/renombrar/borrar desde menú se refleja; cambios externos (`touch`, `rm` en terminal del sistema) aparecen en ≤1 s; badges git M/U/D correctos y se actualizan tras `git add`/commit externos; ⌘P encuentra archivos fuzzy respetando .gitignore; ⌘⇧F encuentra texto y el clic abre el archivo en la línea; ⌘S guarda; `•` de sin guardar; ningún bucle de refresco git (CPU en reposo ~0 %).

**Trampas**: bucle watcher↔git (usar `--no-optional-locks` y ventana de 700 ms); el `Debouncer` se para si se dropea (guardarlo en state); macOS `/private/var` vs `/var` al canonicalizar (comparar siempre con root canonicalizado); en Windows rutas `\\?\` de `canonicalize` → usar `dunce`-like: quitar prefijo `\\?\` manualmente en `util/paths.rs`; columnas UTF-16; `grep-searcher` UTF8 sink falla con texto no UTF-8 → devolver `Ok(true)` y seguir (usar `sinks::Lossy`); el atajo ⌘P debe registrarse en fase de captura para ganarle a CodeMirror.

**Prompt coder Bloque 2**:
> Eres el coder del Bloque 2 de CaesarCode en `/Users/cesargarrido/Proyectos/caesarCode` (el Bloque 1 ya está hecho y compila). Lee `PLAN.md` secciones 0, 2, 3 y "BLOQUE 2" completas antes de tocar nada, y revisa el código existente de `src-tauri/src/{lib,state,error}.rs`, `build.rs`, `capabilities/default.json`, `src/lib/*` y `src/components/layout/*`. Implementa el Bloque 2 exactamente: módulos Rust `workspace/`, `fs/` (tree, ops, watcher con notify-debouncer-mini 0.6, quick_open con `ignore` + `fuzzy-matcher` SkimMatcherV2, search con `grep-searcher`/`grep-regex`/`grep-matcher` y cancelación por generación) y `git/` (binario git con `--no-optional-locks`, porcelain v1 -z), añadiendo los campos de `AppState` del Bloque 2 (§3.2) y registrando los 13 comandos en `generate_handler!`, en `COMMANDS` de `build.rs` y en `capabilities/default.json` (permisos `allow-...`; verifica el nombre exacto en `src-tauri/gen/schemas/` tras `cargo check`). Frontend: wrappers en `ipc.ts`, `store/workspace.ts`, `store/editor.ts`, `store/editorRegistry.ts`, Sidebar con árbol perezoso, iconos, badges git en vivo, menú contextual (New File, New Folder, Rename, Delete a papelera con confirmación, Copy Path, Copy Relative Path), QuickOpen (Mod+P en fase captura), GlobalSearch (Mod+Shift+F) y el editor mínimo CodeMirror (pestañas con `•`, Mod+S, `openFileAt` con scroll a línea) usando UN solo `EditorView` y `setState` por pestaña. Usa los DTO y nombres de §3.4 tal cual (camelCase). Respeta las trampas listadas (bucle git/watcher, UTF-16, `\\?\` en Windows, `/private/var` en macOS, debouncer guardado en state). Sin stubs ni mocks. Termina con `npm run build` y `cd src-tauri && cargo check` en 0 errores y `npm ls @codemirror/state` con una sola versión. No hagas commit. Devuelve archivos tocados, resultado de los comandos y desviaciones.

---

### BLOQUE 3 — Microservicios, logs, terminal

**Rust**:
- `services/config.rs`: archivo `ROOT/.caesar/services.json` con forma `{ "version": 1, "services": ServiceConfig[] }`. `load`: si existe `.caesar/services.json` úsalo; si no, `caesar.json` en raíz (misma forma); si no, `[]`. `save` siempre escribe `.caesar/services.json` (crea dir, JSON pretty, escritura atómica). Validar ids únicos y nombre no vacío.
- `services/detect.rs`: recorre root y subcarpetas hasta profundidad 2 (omitir `node_modules, target, .git, dist, build, .venv, venv, __pycache__, .idea`). Reglas (una sugerencia por carpeta, prioridad en este orden):
  - `docker-compose.yml|docker-compose.yaml|compose.yaml|compose.yml` → `docker compose up`, kind docker.
  - `package.json` → gestor por lockfile (`pnpm-lock.yaml`→pnpm, `yarn.lock`→yarn, `bun.lockb|bun.lock`→bun, si no npm); script `dev` → `<pm> run dev`, si no `start` → `<pm> run start` (npm: `npm start`); sin ninguno → omitir. kind node.
  - `Cargo.toml` con `[package]` y (`src/main.rs` o `[[bin]]`) → `cargo run`, kind rust.
  - `pom.xml` → si existe `mvnw` → `./mvnw` (Windows `mvnw.cmd`) si no `mvn`; si el pom contiene `spring-boot` → `<mvn> spring-boot:run`, si no `<mvn> compile exec:java`. kind maven.
  - `build.gradle|build.gradle.kts` → `./gradlew` (Windows `gradlew.bat`) o `gradle`; contiene `org.springframework.boot` → `bootRun`, si no `run`. kind gradle.
  - `pyproject.toml|requirements.txt` → `manage.py` → `python manage.py runserver`; si `main.py` o `app.py` contiene `FastAPI(` → `uvicorn <mod>:app --reload`; si no `python <main.py|app.py>`; sin entrypoint → omitir. Si hay `.venv/bin/python` (o `.venv\Scripts\python.exe`) usar esa ruta. kind python.
  - Puerto: `.env` `PORT=`; `src/main/resources/application.properties` `server.port=`; `application.yml` `port:` bajo `server:` (regex simple); `vite.config.*` presente → 5173; si no `null`.
  - `name` = nombre de la carpeta (root → nombre del workspace); `id` = slug único; `cwd` relativo con `/`.
- `services/logs.rs`: `LogStore { per_service: HashMap<String, VecDeque<LogLine>> (cap 20 000), seq: AtomicU64, pending: Vec<LogLine> }`. `LineSplitter` por servicio: acumula bytes, separa por `\n`, quita `\r` final, `strip_ansi_escapes::strip`, `from_utf8_lossy`; parte pendiente sin `\n` se emite si pasan 200 ms. Nivel por regex case-insensitive: `\b(error|err|fatal|panic|exception|traceback)\b` → error; `\b(warn|warning)\b` → warn; resto info.
- `services/process_tree.rs`:
  - `descendants(sys: &System, root: u32) -> Vec<u32>`: mapa padre→hijos con `sys.processes()` y BFS. **Tomar la foto del árbol ANTES de matar** (huérfanos se re-parentan a launchd/init).
  - `kill_tree(root: u32, grace: Duration)`: unix → `refresh_processes(ProcessesToUpdate::All, true)`, `pids = descendants + root`; `libc::killpg(root as i32, SIGTERM)` (el hijo de portable-pty es líder de sesión/grupo) y `libc::kill(pid, SIGTERM)` a cada pid; sondear cada 100 ms hasta `grace` (3 s) si siguen vivos (`kill(pid,0)==0`); luego `SIGKILL` a supervivientes. Windows → `taskkill /PID <root> /T /F` con `no_window`, y además `kill` a descendientes capturados que sigan vivos.
- `services/ports.rs`: `is_port_open(port)` → `TcpStream::connect_timeout(127.0.0.1:port, 150 ms)` (probar también `[::1]`). `pids_on_port(port)`: unix `lsof -nP -iTCP:<port> -sTCP:LISTEN -t`; Windows `netstat -ano -p tcp` y filtrar líneas `LISTENING` con `:<port>` exacto. `port_kill` → `kill_tree` de cada pid.
- `services/manager.rs`: `ServiceManager { inner: Mutex<HashMap<String, ServiceRuntime>>, configs: Mutex<Vec<ServiceConfig>>, log_tx: Mutex<Option<Channel<Vec<LogLine>>>>, status_tx: Mutex<Option<Channel<Vec<ServiceStatus>>>>, logs: Mutex<LogStore>, root: RwLock<Option<PathBuf>> }`. `ServiceRuntime { state, pid, child: Box<dyn portable_pty::Child + Send + Sync>, master: Box<dyn MasterPty + Send>, started_at, exit_code, user_stopped: bool, message }`.
  - `start(id)`: si ya running → no-op. `native_pty_system().openpty(PtySize{rows:40, cols:200, ..})`. Comando: unix `CommandBuilder::new("/bin/sh")` args `["-c", command]`; Windows `cmd.exe /C command`. `cwd = root.join(cfg.cwd)`. Env: `std::env::vars()` (PATH ya corregido) + `.env` de `envFile` (con `dotenvy::from_path_iter`) + `cfg.env` + `TERM=xterm-256color`, `FORCE_COLOR=1`, `PYTHONUNBUFFERED=1`. `slave.spawn_command(cmd)`; **`drop(pair.slave)` inmediatamente** (si no, el lector nunca recibe EOF). Hilo lector `master.try_clone_reader()` → `LineSplitter` → `LogStore` + `pending`. Hilo waiter: `child.wait()` no es posible si el child está en el mapa → en su lugar el metrics loop hace `child.try_wait()` cada 250 ms (loop dedicado de 250 ms para exit, y de 1 s para métricas). Al salir: `exit_code`, estado `error` si `!user_stopped && code != 0`, si no `stopped`; añade línea de log `[CaesarCode] proceso terminado con código X`; emite status inmediato.
  - `stop(id)`: `user_stopped = true`, estado `stopping`, `kill_tree(pid, 3s)` en `spawn_blocking`, luego `child.kill()` por si acaso, estado `stopped`. `restart` = stop + start. `start_all`/`stop_all` en paralelo. `stop_all_blocking()` para shutdown.
  - Metrics loop (tokio task lanzada en `services_subscribe` una sola vez, guardada con `JoinHandle` o flag): `System::new()`, cada 1 s `refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cpu().with_memory())` (la CPU requiere dos refrescos: el primer valor es 0, es normal); por servicio running: suma cpu y `memory()` (bytes → MB, f64 1 decimal) del root + descendientes; `portOpen` si `port`. Envía `Vec<ServiceStatus>` de TODOS los configs (los no arrancados → stopped).
  - Flusher de logs: task cada 50 ms → `take(pending)` → `log_tx.send(lote)`; si `Err` → `log_tx = None`.
  - Al `workspace_open`: `stop_all` del root anterior, recargar configs.
- `terminal/mod.rs`: `TerminalManager { terms: Mutex<HashMap<String, Term>> }`, `Term { writer: Box<dyn Write + Send>, master: Box<dyn MasterPty + Send>, child: Box<dyn Child + Send + Sync>, pid }`. `open`: shell unix `$SHELL` (fallback `/bin/zsh`, luego `/bin/bash`) con arg `-l`; Windows `powershell.exe -NoLogo` (fallback `%COMSPEC%`). Env `TERM=xterm-256color`, `COLORTERM=truecolor`. `drop(slave)` tras spawn. Hilo lector 16 KB: **decodificación UTF-8 con arrastre** (si `from_utf8` falla con `error_len()==None`, guardar los últimos ≤3 bytes para la siguiente lectura; si `Some`, lossy). Envía `TerminalEvent::Data`. Al EOF: `try_wait` → `Exit{code}` y eliminar del mapa. `write` → `writer.write_all + flush`. `resize` → `master.resize(PtySize{..})`. `close` → `kill_tree(pid, 500ms)` + `child.kill()` + quitar del mapa. `close_all` para shutdown.
- `services/commands.rs`, `terminal/commands.rs`; añadir los 15 comandos (§3.3 bloque 3) a handler, `COMMANDS` y capability. Campos `services`, `terminals` en `AppState`; `shutdown()` los usa (§3.6).

**Frontend**:
- `store/services.ts`: `configs`, `statuses: Record<id, ServiceStatus>`, `selectedId: string | 'all'`, acciones load/save/detect/start/stop/restart/startAll/stopAll/killPort. Suscripción única al abrir workspace (`services_subscribe` con dos `Channel`).
- `store/logs.ts`: módulo con `buffers: Map<string, LogLine[]>` + `all: LogLine[]` (cap 20 000 c/u, recorte en bloques de 2 000) y zustand mínimo `{ tick }` incrementado con `requestAnimationFrame` como máximo 1 vez por frame. Al seleccionar servicio por primera vez → `service_logs(id, 5000)` para backlog.
- `ServicesPanel.tsx` (pestaña Services): split horizontal (`PanelGroup`): izquierda lista (toolbar: Start All `Play`, Stop All `Square`, Add `Plus`, Detect `Wand2`; fila "All Services"; `ServiceRow` por servicio con punto de estado (ok/danger/muted, ámbar parpadeante en starting/stopping), nombre, badge Running/Stopped/Error, `RAM xx MB`, `CPU x.x%`, `:port` en verde si abierto con botón `Kill Port` (`Zap`), botones Start/Stop/Restart/Edit/Delete (confirmación)). Derecha `LogViewer` del seleccionado.
- `ServiceModal.tsx`: campos nombre, comando, cwd (con botón carpeta → dialog `open({directory:true, defaultPath: root})` y convertir a relativo; si queda fuera del root → error), env (editor de pares clave/valor, añadir/quitar filas), envFile, puerto (numérico), kind (`Select` custom). Modo "Detectados": lista de `services_detect` con checkboxes para añadir en bloque. Guardar → `services_save`.
- `LogViewer.tsx`: `useVirtualizer` (`estimateSize: () => 18`, `overscan: 20`), fuente mono 12 px; toolbar: filtro texto (debounce 150 ms, case-insensitive), `Select` nivel (All/ERROR/WARN/INFO = mínimo nivel), selector de servicio en modo "All" (prefijo coloreado con nombre del servicio), botón Clear, toggle "Follow" (auto-scroll al final si el usuario está abajo; se desactiva al hacer scroll arriba). Colores: error danger, warn accent, info fg.
- Pestaña Logs = `LogViewer` en modo All Services a tamaño completo.
- `TerminalPanel.tsx`: sub-pestañas de terminales (`+` nueva, `x` cerrar), cada una `TerminalView` montada (ocultas con `display:none`, no desmontadas). `TerminalView.tsx`: `new Terminal({ fontFamily: 'JetBrains Mono, monospace', fontSize: 13, cursorBlink: true, allowProposedApi: true, theme: { background:'#0f0f0f', foreground:'#ededed', cursor:'#f59e0b', selectionBackground:'#f59e0b55', ... ANSI } })`, `open(div)`, `loadAddon(fit)`, `try { const gl = new WebglAddon(); gl.onContextLoss(() => gl.dispose()); term.loadAddon(gl) } catch {}` (fallback DOM), `fit.fit()` y `terminal_open(cwd=root, cols, rows, channel)`; `term.onData(d => terminal_write(id, d))`; `ResizeObserver` → `fit.fit()` + `terminal_resize` (debounce 50 ms; no llamar si el contenedor mide 0 por estar oculto); al desmontar `terminal_close`. Importar `@xterm/xterm/css/xterm.css`. Con `Exit` → mostrar "[proceso terminado]" y permitir reabrir.
- StatusBar: "n/m services running".

**Criterios de aceptación**: crear servicio `node -e "setInterval(()=>console.log('tick'),500)"`, Start → Running con RAM/CPU >0 y logs fluyendo; Stop → ningún proceso hijo queda (`ps -ef | grep node`); servicio `npx http-server -p 8099` → `:8099` verde, "Kill Port" lo libera; un servicio que sale con código 1 → Error; detect encuentra un `package.json` con script dev; 100 000 líneas de log sin bloquear la UI; filtro texto/nivel funciona; terminal interactiva (`vim`, colores, resize) funciona; cerrar la app mata servicios y shells (verificar con `ps`).

**Trampas**: no dropear `slave` → sin EOF y zombis; tomar la foto del árbol antes de matar; `killpg` además de `kill` (npm/yarn lanzan nietos); Windows ConPTY: el lector solo hace EOF al cerrar el master → en `close` dropear master; `child.wait()` bloquea y requiere `&mut` → usar `try_wait` en bucle; CPU de sysinfo necesita 2 muestras; `Channel` dentro de `Mutex<Option<..>>` y descartar en error; xterm WebGL puede perder el contexto (manejar `onContextLoss`); `fit()` con contenedor oculto da 0 cols → saltar; UTF-8 partido entre lecturas; `lsof` puede no estar en PATH → usar `/usr/sbin/lsof` como fallback.

**Prompt coder Bloque 3**:
> Eres el coder del Bloque 3 de CaesarCode en `/Users/cesargarrido/Proyectos/caesarCode` (Bloques 1 y 2 hechos y compilando). Lee `PLAN.md` secciones 0, 2, 3 y "BLOQUE 3" completas y revisa el código existente (`src-tauri/src/{lib,state,error}.rs`, `util/*`, `build.rs`, capability, `src/lib/{types,ipc}.ts`, `src/store/*`, `src/components/ui/*`, `src/components/layout/AppLayout.tsx`). Implementa el Bloque 3 exactamente: Rust `services/` (config `.caesar/services.json`/`caesar.json`, detect con las reglas del plan, logs con LineSplitter + strip ANSI + nivel, process_tree con foto del árbol vía sysinfo 0.33.1 y kill SIGTERM→SIGKILL con `killpg` en unix y `taskkill /T /F` en Windows, ports con `lsof`/`netstat`, manager con portable-pty 0.9 dropeando el `slave` tras spawn, bucle de exit con `try_wait` 250 ms, bucle de métricas 1 s, flusher de logs 50 ms por `Channel<Vec<LogLine>>` y estados por `Channel<Vec<ServiceStatus>>`) y `terminal/` (PTY con shell de login, decodificación UTF-8 con arrastre, `Channel<TerminalEvent>`). Añade `services` y `terminals` a `AppState`, actualiza `shutdown()` (§3.6) y registra los 15 comandos del bloque en `generate_handler!`, `COMMANDS` y la capability. Frontend: `store/services.ts`, `store/logs.ts` (buffers fuera de zustand + tick), `ServicesPanel`, `ServiceRow`, `ServiceModal` (con Detect y editor de env), `LogViewer` virtualizado con `@tanstack/react-virtual` (filtro texto, nivel, follow, clear, modo All Services), `TerminalPanel` + `TerminalView` con `@xterm/xterm` 5.5 + WebGL (fallback si falla) + fit + ResizeObserver. Usa exactamente los tipos de §3.4. Nada de `<select>` nativo, nada de stubs. Termina con `npm run build` y `cd src-tauri && cargo check` en 0 errores. No hagas commit. Devuelve archivos tocados, resultado de comandos, desviaciones y una lista de pruebas manuales sugeridas.

---

### BLOQUE 4 — Editor completo + LSP en Rust + limpieza final

**Rust `lsp/`**:
- `servers.rs`: `language_for(path) -> Option<&'static str>`: `.py/.pyi`→`python`; `.ts/.tsx/.js/.jsx/.mjs/.cjs`→`typescript` (un servidor para ambos); `.rs`→`rust`; `.java`→`java`. `language_id_for(path)` (el `languageId` del didOpen): `python`, `typescript`, `typescriptreact`, `javascript`, `javascriptreact`, `rust`, `java`. `server_command(lang, root, app_data_dir) -> Option<(PathBuf, Vec<String>)>` usando `which::which`: python → `pyright-langserver --stdio` (fallback `basedpyright-langserver --stdio`); typescript → `typescript-language-server --stdio`; rust → `rust-analyzer`; java → `jdtls -data <app_data_dir>/jdtls/<hash del root>`. Ninguno → `None` (estado `unavailable`).
- `transport.rs`: escritura `Content-Length: <len bytes>\r\n\r\n<body>` + flush (longitud en **bytes**). Lectura: leer líneas de cabecera hasta línea vacía (`\r\n`), parsear `Content-Length` case-insensitive (ignorar `Content-Type`), `read_exact` del cuerpo, `serde_json::from_slice::<Value>`. Sobre `tokio::io::BufReader<ChildStdout>`.
- `client.rs`: `LspClient { lang, server_name, stdin: tokio::sync::Mutex<ChildStdin>, next_id: AtomicI64, pending: Mutex<HashMap<i64, oneshot::Sender<Result<Value, String>>>>, state, child, open_docs: Mutex<HashMap<String /*uri*/, PathBuf>>, ready: tokio::sync::watch<bool>, capabilities }`.
  - `start`: `tokio::process::Command` con stdin/stdout/stderr piped, `current_dir(root)`, `kill_on_drop(true)`, `no_window`. **Drenar stderr** en una task (si no, el pipe se llena y el servidor se bloquea); se guarda en `log::debug`.
  - Task lectora: mensaje con `id` y `method` → petición servidor→cliente: responder `workspace/configuration` → `[null; items.len()]`, `client/registerCapability`/`unregisterCapability`/`window/workDoneProgress/create` → `null`, `workspace/workspaceFolders` → carpeta root, otro → error `-32601`. Con `id` sin `method` → resolver `pending`. Sin `id`: `textDocument/publishDiagnostics` → convertir y enviar `LspEvent::Diagnostics` (mapear URI→path vía `open_docs`, si no `Url::to_file_path`); `window/logMessage`/`$/progress` → ignorar. EOF → estado `crashed`, emitir status, fallar todos los pending.
  - `request(method, params, timeout 10 s)`; `notify(method, params)`.
  - `initialize` con `processId`, `rootUri`, `workspaceFolders: [{uri, name}]`, `initializationOptions` vacías, `capabilities`: `general.positionEncodings: ["utf-16"]`, `textDocument.synchronization { didSave: true, dynamicRegistration: false }`, `completion.completionItem { snippetSupport: true, documentationFormat: ["markdown","plaintext"], resolveSupport: none }`, `hover.contentFormat ["markdown","plaintext"]`, `definition { linkSupport: true }`, `publishDiagnostics {}`, `workspace { configuration: true, workspaceFolders: true }`. Al responder → `initialized {}`, `ready=true`, estado `ready`. **Encolar `didOpen` hasta `ready`** (esperar el `watch`). Construir params con `lsp_types` + `serde_json::to_value`, o `json!` directamente (preferir tipos de `lsp_types` para respuestas).
- `convert.rs`: `CompletionResponse` (Array|List) → `CompletionDto` (kind enum → string en minúscula: Function→"function", Method→"method", Variable→"variable", Class→"class", Interface→"interface", Keyword→"keyword", Property/Field→"property", Module→"namespace", Constant→"constant", Enum→"enum", Snippet→"snippet", resto→"text"); `insertText` = `textEdit.newText` || `insert_text` || `label`; `isSnippet` = `insert_text_format == Snippet`; `textEdit` solo si es `TextEdit` (para `InsertReplaceEdit` usar `replace`). Máx 200 items. `Hover` (`MarkedString`, `Vec`, `MarkupContent`) → markdown string. `GotoDefinitionResponse` (Scalar|Array|Link → `target_selection_range`) → `LocationDto` con path de `to_file_path`. Diagnósticos → `DiagnosticDto`.
- `manager.rs`: `LspManager { clients: tokio::sync::Mutex<HashMap<(PathBuf, String), Arc<LspClient>>>, event_tx: Mutex<Option<Channel<LspEvent>>>, statuses }`. `ensure_client(root, lang)`: arranca una vez por (workspace, lenguaje); si `unavailable` lo recuerda y no reintenta (hasta reabrir workspace); si `crashed` permite reiniciar en el siguiente didOpen (máx 3 reinicios). `shutdown_blocking()`. Al `workspace_open` cerrar clientes del root anterior.
- `commands.rs`: los 9 comandos de §3.3. `lsp_did_open` devuelve `LspDocStatus` inmediatamente (no espera a `ready`; la apertura real se encola). `lsp_completion/hover/definition` sin cliente/ready → `Ok(vec![])`/`Ok(None)` (no error). Añadir a handler, `COMMANDS`, capability; campo `lsp` en `AppState`; `shutdown()` actualizado.
- URIs: `Url::from_file_path(path)`; guardar el string exacto enviado en `open_docs` y usarlo en todas las notificaciones del documento (evita divergencias de percent-encoding / mayúsculas de unidad en Windows).

**Frontend editor**:
- `languages.ts`: `languageFor(path): Extension` y `lspLanguage(path)`: python(), javascript({ jsx, typescript }) según ext, rust(), java(), json(), yaml(), markdown(); otros → sin lenguaje. Usar `Compartment` para cambiar de lenguaje por pestaña (cada `EditorState` se crea con el suyo).
- `theme.ts`: `EditorView.theme({...}, {dark:true})` (fondo `#0f0f0f`, gutter `#0f0f0f` con texto `#555`, línea activa `#161616`, gutter activo `#ededed`, selección `#f59e0b33`, cursor `#f59e0b`, match de búsqueda `#f59e0b40`, panel de búsqueda con estilos `panel/border`, tooltips `panel` + borde, autocompletado seleccionado `#f59e0b22`) + `HighlightStyle.define` (keyword `#f59e0b`, string `#a3be8c`-like verde suave `#9fca7e`, number `#e5a663`, comment `#666` itálica, function `#ededed` bold suave, type/class `#d4b483`, property `#c9c9c9`, operator `#888`, invalid `#ef4444`).
- `CodeEditor.tsx`: estado por archivo creado con: `lineNumbers, highlightActiveLineGutter, highlightActiveLine, foldGutter, drawSelection, dropCursor, EditorState.allowMultipleSelections.of(true), rectangularSelection, crosshairCursor, indentOnInput, bracketMatching, closeBrackets, history, highlightSelectionMatches, autocompletion({ override: [lspCompletionSource, ...fallback del lenguaje] }), lintGutter(), keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...searchKeymap, ...historyKeymap, ...foldKeymap, ...completionKeymap, ...lintKeymap, indentWithTab, {key:'Mod-s', run: save}, {key:'F12', run: goToDefinition}])`, `search({ top: true })`, `syntaxHighlighting(style)`, tema, `languageCompartment`, `lspPlugin(path)`, `hoverTooltip(lspHover)`, `EditorView.updateListener` (dirty + cursor Ln/Col + `scheduleDidChange`). Multicursor: `Mod-d` → `selectNextOccurrence`, `Alt-click` (por defecto `addCursor` con Alt en CM).
- `lsp/sync.ts`: por path `version` incremental; `lsp_did_open` al crear el estado (si `lspLanguage` no es null); `didChange` con debounce 150 ms enviando el texto completo; `flushChange(path)` síncrono antes de completion/hover/definition (**si no, el servidor responde sobre texto viejo**); `didSave` tras guardar; `didClose` al cerrar pestaña.
- `lsp/completion.ts`: `CompletionSource` → si `!context.explicit` y no hay palabra antes (`/[\w$]+$/` o tras `.`/`::`/`->`), `null`; `pos → {line, character}` con `state.doc.lineAt(pos)` (`character = pos - line.from`, ya es UTF-16). Mapear a `Completion { label, type: kind, detail, info: documentation, apply }`; si `isSnippet` → `apply: snippet(lspSnippetToCm(insertText))`; si `textEdit` → `apply` personalizado que reemplaza el rango del textEdit; `from` = inicio de la palabra; `validFor: /^[\w$]*$/`. Si el resultado está vacío, dejar que actúe la completion del lenguaje.
- `lsp/snippet.ts`: convertir `$N` → `${N}`, `${N:txt}` se mantiene, `$0`/`${0}` → `${}` al final, escapar `#{` y `}` sueltos, quitar choices `${1|a,b|}` → `${1:a}`.
- `lsp/hover.ts`: `hoverTooltip(async (view, pos) => ...)` → `flushChange`, `lsp_hover`; DOM: bloques ``` → `<pre>` mono, resto texto plano en `<div>` con `white-space: pre-wrap`, máx 480 px ancho y 300 px alto con scroll. NO usar `innerHTML` con contenido del servidor (usar `textContent`).
- `lsp/definition.ts`: `F12` y `Mod+Click` (`EditorView.domEventHandlers({ mousedown(e, view) { if (isMod(e)) { const pos = view.posAtCoords({x:e.clientX,y:e.clientY}); ... e.preventDefault(); return true } } })`) → `lsp_definition` → primera ubicación → `openFileAt(path, line+1, character)`. Subrayado del token bajo el cursor con Mod pulsado (opcional pero deseable: decoración `cm-lsp-link`).
- `lsp/diagnostics.ts` + `store/lsp.ts`: `lsp_subscribe` al abrir workspace; `diagnostics` por path guardados en store; si hay `EditorView` activo con ese path → `view.dispatch(setDiagnostics(view.state, toCm(diags)))` (convertir line/char→offset con `doc.line(l+1).from + min(char, line.length)`, clamp a `doc.length`); al hacer `setState` de otra pestaña volver a aplicar sus diagnósticos. Contadores de errores/warnings en StatusBar y punto rojo/ámbar en la pestaña.
- `StatusBar`: indicador LSP del archivo activo: `● pyright` (verde ready, ámbar starting), `○ LSP no instalado (pyright-langserver)` en `muted` con tooltip; `crashed` en danger. Sin lenguaje LSP → nada.
- Pestañas: cerrar con `Mod+W`, `Mod+Shift+[ / ]` cambiar pestaña, clic medio cierra, cerrar dirty pide confirmar (Guardar / Descartar / Cancelar), al cerrar la ventana con dirty → `getCurrentWindow().onCloseRequested` pregunta con `Modal` (requiere permiso `core:window:allow-close`/`core:window:default` incluido en `core:default`; si no se puede prevenir, documentarlo).

**Limpieza final**: buscar `TODO|FIXME|unimplemented!|todo!|console.log` y eliminarlos; `cargo clippy -- -D warnings` limpio; revisar que todos los comandos están en `COMMANDS` y capability (contar: 1 + 13 + 15 + 9 = 38); README con requisitos (git, servidores LSP opcionales: `npm i -g pyright typescript-language-server typescript`, `rustup component add rust-analyzer`, `brew install jdtls`) y `npm run tauri dev`/`npm run tauri build`.

**Criterios de aceptación**: abrir `.py` con pyright instalado → diagnósticos subrayados en vivo (≤1 s), completion al escribir `os.` y hover con firma; `.ts` igual con typescript-language-server; F12/⌘Click navega a otro archivo y línea; sin servidores instalados el editor funciona igual y la StatusBar muestra el indicador discreto, sin errores en consola; buscar/reemplazar (⌘F, ⌘⌥F / Mod-Shift-h según keymap), plegado, multicursor (⌘D, Alt+clic); un solo proceso de servidor por lenguaje aunque se abran 10 archivos; cerrar la app mata los servidores LSP (comprobar con `ps`); `npm run build`, `cargo check` y `cargo clippy -- -D warnings` limpios; `npm run tauri build` genera el bundle (en macOS, solo prueba local, sin publicar).

**Trampas**: framing con longitud en bytes, no chars; leer cabeceras hasta `\r\n\r\n` (puede haber `Content-Type`); responder peticiones servidor→cliente o pyright/tsserver se quedan colgados; drenar stderr; enviar `initialized` tras la respuesta y encolar didOpen hasta entonces; didChange pendiente antes de pedir completion; rust-analyzer tarda en indexar (estado `ready` no implica diagnósticos inmediatos); jdtls necesita `-data` escribible y Java 21+; posiciones UTF-16 (CM ya usa UTF-16, no convertir); diagnósticos con rangos fuera del doc → clamp; `publishDiagnostics` de archivos no abiertos → guardar en store igualmente; Windows: URIs `file:///c%3A/...` → normalizar comparando `to_file_path` en minúsculas; un solo `EditorView` + `setState` pierde el `scrollTop` → guardarlo en el registro y restaurarlo con `EditorView.scrollIntoView` / `scrollDOM.scrollTop`.

**Prompt coder Bloque 4**:
> Eres el coder del Bloque 4 (final) de CaesarCode en `/Users/cesargarrido/Proyectos/caesarCode` (Bloques 1–3 hechos y compilando). Lee `PLAN.md` secciones 0, 2, 3 y "BLOQUE 4" completas y revisa el código existente (`src-tauri/src/{lib,state,error}.rs`, `util/*`, `build.rs`, capability, `src/lib/*`, `src/store/{editor,editorRegistry,workspace}.ts`, `src/components/editor/*`, `StatusBar.tsx`). Implementa el Bloque 4 exactamente: Rust `lsp/` (servers con `which`, transport con framing `Content-Length` en bytes, client sobre `tokio::process` con task lectora, pending por id con oneshot, respuestas a peticiones servidor→cliente, drenado de stderr, initialize/initialized y cola de didOpen hasta ready, convert con `lsp-types` 0.95.1, manager con un cliente por (workspace, lenguaje) y `Channel<LspEvent>`), los 9 comandos LSP registrados en `generate_handler!`, `COMMANDS` y capability, campo `lsp` en `AppState` y `shutdown()` actualizado. Frontend: editor CodeMirror 6 completo (lenguajes con Compartment, tema Graphite & Amber + HighlightStyle, buscar/reemplazar, plegado, multicursor, closeBrackets, lintGutter), y la capa LSP en `src/components/editor/lsp/` (sync con debounce y `flushChange`, completion con snippets convertidos, hover con `textContent` sin innerHTML, definition con F12 y Mod+Click, diagnósticos con `setDiagnostics`), `store/lsp.ts`, indicador LSP discreto en StatusBar, atajos de pestañas y confirmación al cerrar con cambios sin guardar. Si no hay servidor LSP instalado todo debe funcionar sin errores. Después haz la limpieza final del plan (sin TODO/console.log/unimplemented, clippy limpio, 38 comandos registrados, README). Termina con `npm run build`, `cd src-tauri && cargo check` y `cargo clippy -- -D warnings` en 0 errores. No hagas commit ni publiques nada. Devuelve archivos tocados, resultados de comandos, desviaciones y pruebas manuales sugeridas.

---

## 6. Orquestación (para el agente principal)

| Paso | Agente | Tarea |
|---|---|---|
| 1 | coder (Sonnet) | Bloque 1 (prompt de arriba) |
| 2 | principal | Revisar diff, `npm run build`, `cargo check`, `npm run tauri dev` (smoke) |
| 3 | coder | Bloque 2 |
| 4 | principal | Revisión + prueba manual de árbol, watcher, git, ⌘P, ⌘⇧F |
| 5 | coder | Bloque 3 |
| 6 | principal | Revisión + prueba de kill tree con `ps`, terminal, logs |
| 7 | coder | Bloque 4 |
| 8 | principal | Verificación completa y criterios de aceptación de los 4 bloques |
| — | explorer | Solo si un coder se bloquea con la API de una crate: consultar docs.rs de la versión fijada (sysinfo 0.33.1, portable-pty 0.9.0, notify-debouncer-mini 0.6.0, grep-searcher 0.1, lsp-types 0.95.1) y devolver firmas exactas |

Los bloques son secuenciales (cada uno depende de `AppState`, `build.rs`, capability y `types.ts` del anterior); no paralelizar coders sobre el mismo repo.
