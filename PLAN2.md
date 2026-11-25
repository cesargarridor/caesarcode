# CaesarCode — PLAN FASE 2 (Run Widget, detector políglota, árbol semántico, tema imperial)

Fuente de verdad: `SPEC.md` + `PLAN.md` (fase 1, ya construida). Este documento solo añade.
Tres bloques secuenciales; cada uno debe terminar con `cargo check`, `cargo test` (en `src-tauri/`) y `npm run build` en verde.

| Bloque | Contenido | Agente | Depende de |
|---|---|---|---|
| A | Backend Rust: RunConfig + persistencia + detector políglota + resolver Current File + runs efímeros en ServiceManager + IPC + `FsEntry.module` | coder (Sonnet) | — |
| B | Frontend: tipos/ipc, `useRunConfigStore`, header + RunToolbar + dropdown + RunConfigModal, pestañas `Run: <file>`, Services agrupado | coder (Sonnet) | A |
| C | Árbol semántico (`classifyFolder`) + tema/branding imperial (tokens, CodeMirror, xterm, icono, BrandMark) | coder (Sonnet) | A (solo `FsEntry.module`); puede ir en paralelo con B si no tocan los mismos archivos |

Archivos compartidos B/C (conflicto si van en paralelo): `tailwind.config.js`, `src/index.css`, `src/lib/types.ts`. Regla: B no toca colores/tokens; C no toca tipos de run. Si se paraleliza, C añade a `types.ts` solo el campo `module` de `FsEntry` (lo hace A en realidad, ver abajo).

---

## 0. Decisiones de arquitectura (fijas)

1. **Un solo motor de procesos.** Todo lo que se ejecuta (Current File, Auto-detected, Custom Configuration) pasa por `ServiceManager` (`src-tauri/src/services/manager.rs`) como **servicio efímero**: mismo `spawn_process` (PTY + `/bin/sh -c` / `cmd.exe /C`), mismo `kill_tree` (`services/process_tree.rs`), mismo `LogStore`, mismo canal de logs/estado (`services_subscribe`), mismas métricas `sysinfo`. Nada de un segundo spawner.
2. **Ids efímeros con prefijo** (nunca colisionan con servicios persistidos): `file:<ruta-relativa>`, `auto:<id-detectado>`, `run:<id-config>`. `config::validate` (services) y la validación de run-configs rechazan ids que contengan `:`.
3. **Stop/Restart de efímeros = comandos existentes** `service_stop` / `service_restart` con el id prefijado. Solo se añade `run_start` (que hace upsert de la config efímera y, si ya corre, stop+start = Restart).
4. **Persistencia nueva** en `.caesar/run-configs.json` (atómica tmp+rename, mismo patrón que `services/config.rs::save`). `.caesar/services.json` / `caesar.json` NO se migran ni se reescriben: siguen siendo los servicios del panel Services. Compatibilidad = el widget lista también esos servicios en la sección Custom (solo lectura, editables vía el modal de Services existente).
5. **Logs de run** se ven en el `LogViewer` existente (virtualizado) en una pestaña dinámica del panel inferior `Run: <filename>`. Salida por PTY (colores, flush por línea); no es interactivo (stdin no se conecta). Aceptado.
6. **Detección de módulo en el árbol** la hace Rust en `fs_read_dir` (campo `module` en `FsEntry`), porque el árbol es perezoso y el frontend no conoce los hijos de una carpeta sin expandir.

---

## 1. Contratos

### 1.1 Rust (nuevo módulo `src-tauri/src/run/`)

```rust
// run/mod.rs
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RunKind { SpringBoot, Python, Node, Go, Rust, Shell }   // "Custom Shell" = Shell

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunConfig {
    pub id: String,                 // slug, único, sin ':'
    pub name: String,
    pub kind: RunKind,
    #[serde(default)] pub cwd: String,          // relativo al root, "" = root, sin ".."
    pub command: String,            // entrypoint/comando base (p.ej. "./mvnw spring-boot:run", "python", "app.py")
    #[serde(default)] pub args: String,         // se concatena tal cual tras command
    #[serde(default)] pub env: HashMap<String, String>,
    #[serde(default)] pub env_file: Option<String>,
    #[serde(default)] pub port: Option<u16>,
    #[serde(default = "yes")] pub show_in_services: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunConfigFile {
    #[serde(default = "one")] pub version: u32,
    #[serde(default)] pub configs: Vec<RunConfig>,
    /// Fallback configurable del resolver: extensión (sin punto) -> plantilla.
    /// Placeholders: {file} {fileDir} {fileStem} {root}. Ej: {"lua": "lua {file}"}
    #[serde(default)] pub file_runners: HashMap<String, String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RunTemplate { pub kind: RunKind, pub label: String, pub config: RunConfig }

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedRun {
    pub command: String,   // línea completa para sh -c / cmd /C, rutas ya citadas
    pub cwd: String,       // relativo al root
    pub label: String,     // "Run: main.py"
    pub runner: String,    // "python (.venv)", "bun", "cargo run", "clang"... (para tooltip)
}

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RunTarget {
    File { path: String },                    // absoluta, dentro del workspace
    Config { id: String },                    // de run-configs.json
    Detected { service: ServiceConfig },      // tal cual vino de run_detect
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RunStarted { pub runtime_id: String, pub label: String, pub command: String }
```

Extensiones a tipos existentes (`services/mod.rs`, todo `#[serde(default)]` para no romper JSON existente):
- `ServiceKind`: añadir `Go`, `Java`, `Shell` (mantener `Maven`/`Gradle`).
- `ServiceConfig`: añadir `tech: Option<Tech>` y `framework: Option<String>` (`"spring-boot" | "django" | "fastapi" | "flask" | "nestjs" | "express" | "fastify" | "next" | "vite" | ...`).
- `pub enum Tech { Spring, Python, Node, Go, Rust, Docker, Other }` (`rename_all="lowercase"`). Se usa para agrupar en Services.
- `ServiceStatus`: añadir `ephemeral: bool` y `name: String` (los efímeros no están en `useServices.configs`, el front necesita el nombre).
- `fs::FsEntry`: añadir `#[serde(skip_serializing_if = "Option::is_none")] module: Option<String>` con valores `maven | gradle | node | cargo | go | python` (solo dirs; stat de marcadores, sin leer contenido).

### 1.2 Comandos IPC nuevos (registrar en los TRES sitios: `build.rs::COMMANDS`, `lib.rs::generate_handler!`, `capabilities/default.json` como `allow-run-…`)

| Comando | Args | Devuelve |
|---|---|---|
| `run_configs_load` | — | `RunConfigFile` (si no existe el archivo: default vacío) |
| `run_configs_save` | `file: RunConfigFile` | `()` (valida: ids únicos sin `:`, nombre/comando no vacíos, cwd/envFile relativos sin `..`) |
| `run_templates` | — | `Vec<RunTemplate>` (Spring Boot, Python, Node/TypeScript, Go, Rust, Custom Shell) |
| `run_detect` | — | `Vec<ServiceConfig>` (detector nuevo; `services_detect` pasa a delegar en él) |
| `run_resolve_file` | `path: String` | `ResolvedRun` (sin ejecutar; para tooltip y preview del dropdown) |
| `run_start` | `target: RunTarget` | `RunStarted` (upsert efímero + stop si corría + start) |

Stop/Restart/estado/logs: **reutilizar** `service_stop`, `service_restart`, `services_status`, `service_logs`, `service_clear_logs`, `port_kill` con el `runtimeId`. Canales: **ninguno nuevo**; los efímeros salen por los `Channel<Vec<LogLine>>` / `Channel<Vec<ServiceStatus>>` de `services_subscribe`.

### 1.3 TypeScript (`src/lib/types.ts`, `src/lib/ipc.ts`)
Espejo 1:1 en camelCase: `RunKind = 'springBoot'|'python'|'node'|'go'|'rust'|'shell'`, `RunConfig`, `RunConfigFile`, `RunTemplate`, `ResolvedRun`, `RunTarget = {type:'file';path} | {type:'config';id} | {type:'detected';service}`, `RunStarted`, `Tech`, `ServiceConfig.tech/framework`, `ServiceStatus.ephemeral/name`, `ServiceKind` ampliado, `FsEntry.module?`.
Wrappers: `runConfigsLoad`, `runConfigsSave(file)`, `runTemplates`, `runDetect`, `runResolveFile(path)`, `runStart(target)`.

---

## 2. BLOQUE A — Backend Rust

### Archivos
- NUEVO `src-tauri/src/run/mod.rs` (tipos 1.1, `pub mod store; pub mod resolve; pub mod templates; pub mod commands;`, fn `to_service_config(&RunConfig) -> ServiceConfig` con id `run:<id>`, command = `command + " " + args` trim).
- NUEVO `src-tauri/src/run/store.rs` — load/save/validate de `.caesar/run-configs.json` (copiar patrón de `services/config.rs`: `ConfigFile` serde, tmp `run-configs.json.cc-tmp-<pid>` + rename, `bad_cwd`). Tests roundtrip/validación.
- NUEVO `src-tauri/src/run/resolve.rs` — resolver Current File (ver A.2). Puro y testeable con un trait `Probe`.
- NUEVO `src-tauri/src/run/templates.rs` — las 6 plantillas.
- NUEVO `src-tauri/src/run/commands.rs` — los 6 comandos.
- REESCRIBIR `src-tauri/src/services/detect.rs` → carpeta `src-tauri/src/services/detect/{mod.rs, spring.rs, python.rs, node.rs, go.rs, rust.rs, compose.rs, util.rs}` (mover `slug`, `unique_id`, `exists`, `read`, `read_port` a `util.rs`; conservar la firma `pub fn detect(root, workspace_name) -> Vec<ServiceConfig>` para que `services_detect` siga funcionando).
- `src-tauri/src/services/mod.rs` — extensiones de 1.1.
- `src-tauri/src/services/manager.rs` — efímeros (A.3).
- `src-tauri/src/services/config.rs` — rechazar `:` en ids; actualizar `svc()` de tests con campos nuevos.
- `src-tauri/src/fs/mod.rs` + `src-tauri/src/fs/tree.rs` — `FsEntry.module`.
- `src-tauri/src/lib.rs` (`mod run;` + handlers), `src-tauri/build.rs`, `src-tauri/capabilities/default.json`.
- `src-tauri/Cargo.toml` — añadir `toml = "0.8"` (parseo de Cargo.toml/pyproject; ya están `which = "7"` y `regex`). Dev: usar `std::env::temp_dir()` + nombre único (como los tests actuales), sin crate `tempfile`.

### A.1 Detector políglota (`services/detect/`)
- Recorrido: profundidad máx **3** (hoy `MAX_DEPTH = 2`), `SKIP` = `node_modules, target, .git, .venv, venv, dist, build, out, __pycache__, .next, .idea, coverage` + cualquier dir que empiece por `.`. Orden estable (sort). Un directorio puede producir **varias** entradas (`Vec<Detected>`), y docker compose ya **no** corta la detección del resto (se añade como una entrada más, `tech: Docker`).
- **Spring Boot**: `pom.xml` con `spring-boot` → `./mvnw spring-boot:run` si existe `mvnw` (Windows `mvnw.cmd`), si no `mvn spring-boot:run`. `build.gradle(.kts)` con `org.springframework.boot` → `./gradlew bootRun` / `gradle bootRun`. Nombre: `spring.application.name` leído de `src/main/resources/application.{yml,yaml,properties}`; puerto `server.port` (default 8080 si es Spring y no hay valor). Parser YAML mínimo por indentación (solo claves `spring.application.name` y `server.port`, ignorar perfiles `---`, ignorar `${...}` → None). Maven/Gradle sin Spring: conservar comportamiento actual (`compile exec:java` / `run`), `tech: Spring` solo si es Spring, si no `Other`… usar `Tech::Other` con `kind` Maven/Gradle. Multi-módulo Maven: si el `pom.xml` raíz es `<packaging>pom</packaging>`, no emitir la raíz; los submódulos se detectan al bajar.
- **Python**: intérprete por orden: `.venv/bin/python`, `venv/bin/python` (Win `Scripts\python.exe`) → ruta relativa; `poetry.lock` + `[tool.poetry]` → prefijo `poetry run`; `uv.lock` → prefijo `uv run`; si no `python3` (`python` en Windows). Django: `manage.py` → `<py> manage.py runserver` (puerto 8000). FastAPI: `FastAPI(` en `main.py`/`app.py`/`src/main.py` → `<py> -m uvicorn <module>:app --reload` (puerto 8000). Flask: `Flask(` → `<py> -m flask --app <module> run` (puerto 5000). Si solo hay `pyproject.toml`/`requirements.txt` + `main.py`/`app.py` → `<py> main.py`. `framework` rellenado.
- **Node/TS**: gestor por lockfile con prioridad `bun.lockb|bun.lock → bun`, `pnpm-lock.yaml → pnpm`, `yarn.lock → yarn`, si no `npm`. Si no hay lockfile en el dir, buscar en ancestros hasta root (monorepos). Script por prioridad `dev`, `start:dev`, `start`, `serve` → `<pm> run <script>` (`npm start` para start con npm). Framework por `dependencies`/`devDependencies`: `@nestjs/core`→nestjs, `next`→next (3000), `vite`→vite (5173), `fastify`, `express`. Puerto: `PORT=` en `.env`, `--port N` en el script, o el default del framework. Ignorar `package.json` sin scripts útiles (como hoy). Si un dir tiene `package.json` con `"workspaces"` y sin script dev/start, no emitir.
- **Go**: `go.mod` → si existe `main.go` en el dir: `go run .`; por cada `cmd/<svc>/main.go`: una entrada `go run ./cmd/<svc>` con nombre `<svc>`. `kind: Go`, `tech: Go`.
- **Rust**: `Cargo.toml` con `[package]` + (`src/main.rs` | `[[bin]]`) → `cargo run`. Con `[workspace]`: expandir `members` (soportar glob de un nivel `crates/*`), por cada crate con binario una entrada `cargo run -p <package.name>`, `cwd` = raíz del workspace; no bajar después a los miembros (evita duplicados: marcar dirs visitados).
- Ids estables: `slug(name)` + `unique_id` (existente). Nombre: el del framework (Spring app name) > nombre de carpeta > nombre del workspace en la raíz.
- `services_detect` y `run_detect` llaman al mismo `detect()`.

### A.2 Resolver Current File (`run/resolve.rs`)
```rust
pub trait Probe { fn which(&self, bin: &str) -> bool; }       // real: which::which(bin).is_ok()
pub fn resolve_file(root: &Path, file: &Path, runners: &HashMap<String,String>, probe: &dyn Probe) -> AppResult<ResolvedRun>
```
Orden: 1) `file_runners[ext]` del usuario si existe (gana siempre); 2) shebang (primera línea `#!`; `#!/usr/bin/env X args` → `X args <file>`; ruta absoluta → `<ruta> <file>`) para archivos sin extensión conocida o `.sh`; 3) tabla por extensión; 4) error `InvalidArgument("No hay runner para .<ext>; añade fileRunners en .caesar/run-configs.json")`.
- `.py` → buscar `.venv/bin/python` y luego `venv/bin/python` subiendo desde el dir del archivo hasta `root` (Win `Scripts\python.exe`); si no, `python3`/`python`. cwd = dir del archivo.
- `.ts/.mts/.cts` → `bun <file>` si `which bun`; si no `node_modules/.bin/tsx` en ancestros → esa ruta; si no `which tsx` → `tsx`; si no `npx --yes tsx <file>`.
- `.js/.mjs/.cjs` → `node <file>`.
- `.go` → `go run <file>` (cwd dir del archivo).
- `.rs` → subir hasta root buscando `Cargo.toml` con `[package]`: si el archivo es `src/main.rs` → `cargo run`; si es `src/bin/<x>.rs` → `cargo run --bin <x>`; si pertenece al crate pero es otro archivo → `cargo run` igualmente (cwd = dir del crate). Sin Cargo: `rustc <file> -o <tmp> && <tmp>`.
- `.java` → `java <file>` (single-file launch, JDK 11+).
- `.sh` → `bash <file>`, `.zsh` → `zsh <file>`.
- `.c` → `clang` si existe, si no `gcc`: `<cc> <file> -o <tmp> && <tmp>`; `.cpp/.cc/.cxx` → `clang++` o `g++`.
- `<tmp>` = `std::env::temp_dir()/caesarcode-run/<fileStem>-<hash8 de ruta absoluta>` (+`.exe` en Windows); crear el dir. Windows: encadenar con `&&` funciona en `cmd /C`.
- **Citado obligatorio**: helper `shell_quote(&str) -> String` (unix: comillas simples con escape `'\''`; windows: comillas dobles). Toda ruta va citada. Test con ruta con espacios y comilla.
- `cwd` devuelto relativo a root (ruta con `/`); el archivo debe estar dentro del workspace (`util::paths::ensure_in_workspace`).
- `label` = `Run: <filename>`; runtime id = `file:<ruta relativa>`.

### A.3 Efímeros en `ServiceManager`
- Nuevo campo `ephemeral: Mutex<Vec<ServiceConfig>>` (+ `HashSet` de cuáles marcar `show_in_services` no hace falta en Rust: el front decide).
- `pub fn upsert_ephemeral(&self, cfg: ServiceConfig)` (reemplaza por id) y `pub fn run_ephemeral(self: &Arc<Self>, cfg) -> AppResult<()>`: upsert; si el runtime está busy → `stop_blocking`; `start`.
- `config_of` busca en `configs` y luego en `ephemeral`.
- **`statuses()` debe iterar `configs` + `ephemeral`** (si no, el widget nunca recibe estado) con `ephemeral: true` y `name`.
- `set_workspace` (root distinto) limpia `ephemeral`. `save_configs`/`load_configs` NO tocan `ephemeral`. `start_all`/`stop_all` solo persistidos; `stop_all_blocking`/`shutdown` ya usan `active_ids()` (runtimes) → cubre efímeros (verificar).
- `run_start` (en `run/commands.rs`): resuelve target → `ServiceConfig` efímero (`file:` con `kind` acorde, `run:` desde `to_service_config`, `auto:` = `service` recibido con id re-prefijado y validado: cwd relativo sin `..`) → `run_blocking(move || m.run_ephemeral(cfg))` → `RunStarted`. `env`/`env_file` funcionan solos porque `spawn_process` ya los aplica.
- Al iniciar un efímero, limpiar su backlog de logs (`clear_logs(Some(id))`) para que cada Run empiece limpio, y `push_log` de una cabecera `[CaesarCode] $ <command>  (cwd)`.

### A.4 `FsEntry.module`
En `fs/tree.rs::read_dir`, para cada dir (no symlink): primer marcador existente de `pom.xml→maven, build.gradle|build.gradle.kts→gradle, Cargo.toml→cargo, go.mod→go, package.json→node, pyproject.toml→python`. Solo `Path::is_file`, sin leer. No aplicar dentro de dirs excluidos (`node_modules` etc. → None, evita miles de stats).

### Tests headless (Rust, `cargo test`)
- `services/detect/` — fixtures en tempdir:
  1. Spring Maven con `mvnw`, `application.yml` (`spring.application.name: orders`, `server.port: 8081`) → `./mvnw spring-boot:run`, name `orders`, port 8081, tech spring.
  2. Spring Gradle kts + `application.properties` → `./gradlew bootRun`, port.
  3. Maven multi-módulo `packaging pom` raíz + 2 submódulos → 2 entradas, ninguna en la raíz.
  4. Python: Django con `.venv/bin/python` → `.venv/bin/python manage.py runserver`; FastAPI con `uv.lock` → `uv run … uvicorn main:app --reload`; Flask con poetry.
  5. Node: `pnpm-lock.yaml` + `start:dev` + `@nestjs/core` → `pnpm run start:dev`, framework nestjs; bun.lockb gana a npm; lockfile en ancestro.
  6. Go: `go.mod` + `cmd/api/main.go` + `cmd/worker/main.go` → 2 entradas `go run ./cmd/api|worker`.
  7. Rust workspace `members = ["crates/*"]` con 1 bin y 1 lib → 1 entrada `cargo run -p <bin>`, sin duplicado al bajar.
  8. Profundidad 3 encuentra `a/b/c/package.json`; `node_modules/x/package.json` y `.venv` ignorados; profundidad 4 no.
  9. docker-compose + package.json en el mismo dir → 2 entradas.
- `run/resolve.rs` con `FakeProbe { bins: HashSet<&str> }`: py con/sin `.venv` en ancestro; ts con bun / tsx local / npx; rs en crate (`src/main.rs`, `src/bin/x.rs`) y suelto (contiene `rustc` y `&&`); c con clang vs gcc; shebang `#!/usr/bin/env node`; `fileRunners` sobrescribe; extensión desconocida → error; ruta con espacio y `'` citada; archivo fuera del root → error.
- `run/store.rs`: roundtrip, ids duplicados, id con `:`, cwd `../x` → error; archivo ausente → default.
- `manager.rs`: efímero `file:x` con `sleep 30` aparece en `statuses()` con `ephemeral: true`; `run_ephemeral` dos veces = restart (pid cambia); `stop_blocking` mata árbol (reusar helpers `wait_for`, `is_alive`); `save_configs` no borra efímeros; `set_workspace` a otro root sí.
- `fs/tree.rs`: dir con `Cargo.toml` → `module: Some("cargo")`.

### Criterios de aceptación A
`cargo check`, `cargo test` verdes; `cargo clippy` sin warnings nuevos; los 6 comandos en los 3 registros; `services_detect` sigue funcionando y `npm run build` no se rompe (si los tipos TS no se actualizan aún, los campos nuevos son opcionales en serde, no rompen).

### Trampas A
- Olvidar uno de los tres registros de comandos → error de permiso en runtime, no en compilación.
- `statuses()` sin efímeros; `emit_status()` nunca con `runtimes` bloqueado (deadlock, ver comentario existente).
- `run_ephemeral` en hilo bloqueante (`util::run_blocking`), nunca en el hilo async (stop puede tardar ~6 s).
- Rutas sin citar con espacios. Windows: `python` vs `python3`, `.exe`, `Scripts\`.
- No bajar en el detector a dirs ya cubiertos por un workspace Cargo / multi-módulo (duplicados).
- `serde(default)` en TODOS los campos nuevos de `ServiceConfig` para leer `services.json` viejos.
- Los tests que lanzan procesos: comandos `sh` portables (`sleep`, `echo`), `#[cfg(unix)]` si dependen de señales.

---

## 3. BLOQUE B — Frontend Run Widget

### Archivos
- `src/lib/types.ts`, `src/lib/ipc.ts` — contratos 1.3.
- NUEVO `src/store/runConfigs.ts` — `useRunConfigStore`.
- NUEVO `src/components/layout/AppHeader.tsx` — barra superior 36 px (`bg-panel`, borde inferior `border-border`): izquierda nombre del workspace (placeholder de BrandMark; C lo sustituye), derecha `<RunToolbar/>`. Montar en `AppLayout.tsx` encima del `PanelGroup`.
- NUEVO `src/components/run/RunToolbar.tsx`, `RunDropdown.tsx`, `RunConfigModal.tsx`.
- NUEVO `src/components/ui/EnvEditor.tsx` — EXTRAER el editor clave/valor + `.env` de `src/components/bottom/ServiceModal.tsx` y usarlo en ambos modales (no duplicar).
- `src/store/ui.ts` — pestañas dinámicas: `BottomTab = 'services' | 'logs' | 'terminal' | \`run:${string}\``; `runTabs: {id: string; label: string}[]`, `openRunTab(id, label)` (añade si no existe y activa), `closeRunTab(id)`.
- `src/components/bottom/BottomPanel.tsx` — render de `runTabs` tras las fijas, con botón `x` y punto de estado; contenido `<LogViewer key={id} serviceId={id} />` (reuso exacto).
- `src/components/bottom/ServicesPanel.tsx` + `ServiceRow.tsx` — agrupar por `tech` (cabeceras plegables: Spring Boot, Python, Node/TS, Go, Rust, Docker, Other); incluir efímeros con `showInServices` (vienen de `useRunConfigStore`); Start/Stop/Restart de un efímero → acciones del run store.
- `src/store/services.ts` — `init` ya recibe statuses por canal; asegurarse de que `statuses` guarda también los ids efímeros (es un `Record` por id: sí). `runningCount` debe ignorar efímeros o contarlos aparte.
- Atajos en `src/App.tsx` (donde estén los globales): `Ctrl+Shift+R`/`Shift+F10` Run seleccionado, `Ctrl+F2` Stop. NO usar `Cmd+R` (recarga del webview en dev).

### `useRunConfigStore` (contrato)
```ts
type RunSelection = { kind: 'file' } | { kind: 'detected'; id: string } | { kind: 'config'; id: string } | { kind: 'service'; id: string };
interface RunConfigStore {
  file: RunConfigFile;              // configs + fileRunners
  templates: RunTemplate[];
  detected: ServiceConfig[];
  selection: RunSelection;          // persistida en localStorage por root (try/catch)
  launched: Record<string, { runtimeId: string; label: string; showInServices: boolean; tech?: Tech }>; // clave = selectionKey
  init(): Promise<void>;            // load + templates + detect (en paralelo), llamado donde se llama useServices.init
  reset(): void;                    // al cambiar de workspace, junto a useServices.reset
  refreshDetected(): Promise<void>;
  save(file: RunConfigFile): Promise<boolean>;
  select(sel: RunSelection): void;
  runtimeIdFor(sel): string | null; // 'file:<rel>' para file (de la pestaña activa), 'run:<id>', 'auto:<id>', o el id del servicio
  run(sel?): Promise<void>;         // file: guarda antes el buffer sucio (useEditor.save(path)); runStart(target); openRunTab(runtimeId, label)
  stop(sel?): Promise<void>;        // serviceStop(runtimeId)
}
```
Estado del botón: `useServices(s => s.statuses[runtimeId])` → `running|starting` = Run se convierte en Restart (icono `RotateCw`) y Stop activo; punto verde `running`, ámbar `starting/stopping`, gris parado, rojo `error`. Errores → `useUi.pushToast`.

### UI
- Toolbar: `[● nombre-selección ▾] [▶ Run | ⟳ Restart] [■ Stop]`. Fondo `#161616`, hover `#1f1f1f`, Run ámbar `#f59e0b`, Stop carmesí `#dc2626` (deshabilitado gris si nada corre). Tooltip del Run con `ResolvedRun.command` (para Current File, `runResolveFile` al abrir el dropdown, debounced, no en cada tecla).
- Dropdown custom (sin `<select>`), popover anclado, cierra con Esc/click fuera, navegación ↑/↓/Enter, foco visible ámbar. Secciones con cabecera muted: **Current File** (nombre del archivo activo o "Sin archivo activo" deshabilitado), **Auto-detected** (agrupado por tech, icono + framework + puerto), **Custom Configurations** (run-configs + servicios de `services.json` con etiqueta "Services"), separador, **+ Edit / New Configuration…** → abre `RunConfigModal`. Reutilizar patrón de `components/ui/ContextMenu.tsx`/`Select.tsx` para posición y cierre.
- `RunConfigModal` (reusa `components/ui/Modal.tsx`, `Button`, `Select`): lista izquierda de configs (+ Nuevo desde plantilla ▾, Duplicar, Eliminar con `useUi.ask`), formulario derecha: Nombre, Tipo (Select custom con las 6 plantillas; cambiar tipo en una config nueva rellena command/args de la plantilla), Working Directory (texto relativo + botón carpeta con `@tauri-apps/plugin-dialog` → convertir a relativo, rechazar fuera del root), Comando/Entrypoint, Argumentos, Env (`EnvEditor`, incluye `.env`), Puerto esperado, checkbox "Mostrar en panel Services". Guardar → `save()`; validación en cliente igual que Rust (id slug único generado del nombre, sin `:`). Sin perder lo escrito al cambiar de config en la lista (estado borrador por id).

### Criterios de aceptación B
1. Con `main.py` activo y `.venv` en el proyecto: Run abre pestaña `Run: main.py`, logs en vivo, punto verde; Run otra vez = Restart (pid nuevo); Stop mata el árbol (verificar con `ps` que no quedan hijos).
2. Un proyecto Spring/Node detectado aparece en Auto-detected y arranca desde el widget; aparece en Services bajo su grupo si `showInServices`.
3. Crear "API dev" desde plantilla Node, editar, duplicar, eliminar → `.caesar/run-configs.json` refleja los cambios; `.caesar/services.json` intacto.
4. Cerrar la app con runs activos → ningún proceso huérfano (shutdown existente).
5. `npm run build` verde, cero `<select>` nativos (`grep -r "<select" src` vacío).

### Trampas B
- No guardar el buffer sucio antes de "Current File" → ejecuta la versión vieja.
- `BottomTab` como string template: revisar todos los `switch`/comparaciones existentes.
- Selectores zustand que devuelven objetos nuevos en cada render (bucle de renders): seleccionar primitivos o usar `useShallow`.
- La selección persistida puede apuntar a una config borrada → caer a `{kind:'file'}`.
- `reset()` al cambiar de workspace (mismo sitio que `useServices.reset`) y cerrar las `runTabs`.

---

## 4. BLOQUE C — Árbol semántico + tema imperial

### Archivos
- NUEVO `src/lib/palette.ts` — fuente única de colores TS (base `#0f0f0f`, panel `#161616`, border `#262626`, fg `#ededed`, muted `#888888`, accent `#f59e0b`, accentHover `#d97706`, gold `#fbbf24`, crimson `#dc2626`, crimsonDeep `#991b1b`, crimsonDark `#7f1d1d`, sky `#38bdf8`, green `#10b981`, warm `#eab308`, violet `#a78bfa`, excluded `#52525b`, ok `#22c55e`, danger `#ef4444`).
- `tailwind.config.js` — añadir tokens `crimson`, `crimson-deep`, `gold`, `tree-src`, `tree-test`, `tree-cfg`, `tree-infra`, `tree-excluded` (valores = palette). `src/index.css` — CSS vars `--cc-*` en `:root` (caret, selección `#f59e0b33`, scrollbar), foco ámbar.
- NUEVO `src/lib/classifyFolder.ts`:
```ts
export type FolderClass = 'module' | 'source' | 'test' | 'config' | 'infra' | 'excluded' | 'plain';
export interface FolderStyle { cls: FolderClass; color: string; icon: LucideIcon; iconOpen: LucideIcon; badge?: string; dimName: boolean; collapsedByDefault: boolean }
export function classifyFolder(name: string, path: string, children?: readonly string[] | null, opts?: { isRoot?: boolean; module?: string | null }): FolderStyle
```
  Prioridad: excluded > module (opts.module o children contiene marcador, o isRoot con marcador) > test > source > config/infra > plain. Nombres case-insensitive. Excluded: `node_modules, target, dist, build, out, .venv, venv, __pycache__, .next, .git, .idea, coverage` → `#52525b` icono Y nombre (`dimName`), `collapsedByDefault`. Module → ámbar `#f59e0b`, icono `FolderGit2`/`Package`, badge corto (`mvn`, `gradle`, `npm`, `cargo`, `go`, `py`). Source (`src, app, lib, pkg, cmd, internal, core`) → `#38bdf8` (`FolderCode`). Test (`test, tests, __tests__, spec, e2e, integration`) → `#10b981` (`FlaskConical`/`FolderCheck`). Config (`config, resources, assets, .caesar, .vscode`) → `#eab308` (`FolderCog`); infra (`.github, docker, k8s, helm, infra, terraform, scripts, migrations`) → `#a78bfa` (`FolderKanban`/`Container`). Solo se tinta el **icono**; el nombre mantiene `text-fg` salvo excluidos (no saturar). Memoizar por `path`.
- `src/components/sidebar/FileTreeNode.tsx` / `FileIcon.tsx` — usar `classifyFolder` para dirs (pasando `entry.module` e `isRoot`), badge de módulo como pill pequeña tras el nombre (antes del badge Git), mantener badges Git, menú contextual CRUD, `InlineNameInput` y notify intactos. Excluidos: no auto-expandir al revelar archivos (si existe lógica de reveal en `store/workspace.ts`, saltarlos salvo que el archivo esté dentro).
- `src/components/editor/theme.ts` — importar de `palette.ts`: caret ámbar, línea activa `#161616`, gutter activo ámbar, selección ámbar 20 %, matching bracket ámbar, búsqueda ámbar/oro; sintaxis sin carmesí salvo `invalid`.
- `src/components/bottom/TerminalView.tsx` — `THEME` xterm desde `palette.ts` (background `#0f0f0f`, cursor `#f59e0b`, selection `#f59e0b40`, red `#dc2626`, brightRed `#ef4444`, yellow `#eab308`, brightYellow `#fbbf24`, blue `#38bdf8`, green `#10b981`, magenta `#a78bfa`).
- `src/components/editor/TabBar.tsx` — pestaña activa: borde superior 2 px ámbar.
- Branding: NUEVO `scripts/brand-grid.txt` (pixel-art 32×32, un carácter por píxel: `.` transparente, `G` oro `#fbbf24`, `g` ámbar `#f59e0b`, `L` laurel verde oscuro `#4d7c0f`, `R` carmesí `#dc2626`, `r` `#991b1b`, `K` contorno `#0f0f0f`): letra C dorada, corona de laurel arriba, capa carmesí detrás/abajo. Reescribir `scripts/make-icon.py` para leer ese grid y generar `src-tauri/app-icon.png` 1024×1024 (fondo `#0f0f0f`, esquinas redondeadas, escalado nearest ×28 centrado), luego `npx tauri icon src-tauri/app-icon.png` para regenerar `src-tauri/icons/*`. NUEVO `src/components/layout/BrandMark.tsx`: SVG de `<rect>` desde el MISMO grid (`import grid from '../../../scripts/brand-grid.txt?raw'`, `shape-rendering="crispEdges"`), props `size`. Usarlo en `AppHeader` (16 px, con línea inferior carmesí 1 px sutil `#7f1d1d`) y en `WelcomeView.tsx` (96 px + título con detalle carmesí). Carmesí SOLO en: splash/WelcomeView, BrandMark/cabecera, badges de módulo opcional en hover, botón Stop.

### Tests C
- Añadir `vitest` (devDependency) + script `"test": "vitest run"`; `src/lib/classifyFolder.test.ts`: cada categoría, prioridad excluded > module (`node_modules` con package.json sigue excluido), case-insensitive (`Tests`), root con módulo, `opts.module` sin children, carpeta plain.
- Rust: ya cubierto `FsEntry.module` en A.

### Criterios de aceptación C
- Árbol: raíz/subcarpetas con marcador en ámbar con badge; `src` azul; `tests` verde; `.github` violeta; `node_modules`/`target` grises y colapsados; badges Git y menú contextual funcionan como antes.
- Icono nuevo en Dock/ventana (`npm run tauri build` o dev), BrandMark en header y WelcomeView idénticos al icono.
- Editor y terminal coherentes (cursor ámbar, sin carmesí salvo errores). `npm run build` + `npm test` verdes.

### Trampas C
- `?raw` necesita el tipo en `vite-env.d.ts` (ya incluye `vite/client`); `tsc -b` debe ver el archivo fuera de `src` → ajustar `include` o mover el grid a `src/assets/brand-grid.txt` y que el script Python lea esa ruta (preferible).
- No romper `memo` de `FileTreeNode` (classifyFolder puro/memoizado).
- `npx tauri icon` sobrescribe `icons/`; confirmar que `tauri.conf.json` sigue apuntando a los mismos nombres.
- Contraste: verificar texto `#52525b` sobre `#161616` solo para excluidos (intencionadamente apagado).

---

## 5. Prompts para coders (autocontenidos)

### Prompt BLOQUE A (coder Sonnet)
> Proyecto Tauri 2 + Rust en `/Users/cesargarrido/Proyectos/caesarCode`. Lee `/Users/cesargarrido/Proyectos/caesarCode/PLAN2.md` secciones 0, 1 y 2 (Bloque A) y síguelas al pie de la letra. Antes de editar lee: `src-tauri/src/services/{mod.rs,config.rs,commands.rs,detect.rs,manager.rs,process_tree.rs}`, `src-tauri/src/fs/{mod.rs,tree.rs}`, `src-tauri/src/util/paths.rs`, `src-tauri/src/lib.rs`, `src-tauri/build.rs`, `src-tauri/capabilities/default.json`, `src-tauri/Cargo.toml`. Tareas: (1) extender `ServiceKind`/`ServiceConfig`(tech, framework)/`ServiceStatus`(ephemeral, name)/`FsEntry`(module) con `serde(default)`; (2) reescribir el detector como `services/detect/` políglota (profundidad 3, Spring/Python/Node/Go/Rust workspace/compose, varias entradas por dir) manteniendo `detect(root, name)`; (3) crear `src-tauri/src/run/` (tipos, store `.caesar/run-configs.json` atómico, resolver Current File con trait `Probe` y `shell_quote`, 6 plantillas, comandos `run_configs_load/run_configs_save/run_templates/run_detect/run_resolve_file/run_start`); (4) añadir servicios efímeros a `ServiceManager` (ids `file:`/`auto:`/`run:`, incluidos en `statuses()`, limpiados en cambio de workspace, reutilizando `spawn_process`/`kill_tree`/`LogStore`; Stop/Restart vía `service_stop`/`service_restart` existentes); (5) registrar los comandos en `build.rs`, `generate_handler!` y capabilities; (6) escribir TODOS los tests de la sección "Tests headless" con fixtures en `std::env::temp_dir()`. No toques el frontend salvo que `npm run build` se rompa. Termina con `cd src-tauri && cargo check && cargo test && cargo clippy` en verde y `npm run build` en verde. No hagas commits. Reporta: archivos tocados, firmas públicas nuevas, resultado de tests.

### Prompt BLOQUE B (coder Sonnet)
> Proyecto Tauri 2 + React/TS/Tailwind/zustand en `/Users/cesargarrido/Proyectos/caesarCode`. El Bloque A (backend) ya está hecho. Lee `/Users/cesargarrido/Proyectos/caesarCode/PLAN2.md` secciones 0, 1 y 3 (Bloque B) y síguelas. Lee antes: `src-tauri/src/run/mod.rs` (contratos reales), `src/lib/{types.ts,ipc.ts}`, `src/store/{services.ts,ui.ts,editor.ts,workspace.ts,logs.ts}`, `src/components/layout/AppLayout.tsx`, `src/components/bottom/{BottomPanel,ServicesPanel,ServiceRow,ServiceModal,LogViewer}.tsx`, `src/components/ui/{Modal,Select,ContextMenu,Button,IconButton}.tsx`, `src/App.tsx`. Tareas: tipos+ipc espejo de Rust; `src/store/runConfigs.ts` (`useRunConfigStore`, init/reset junto a `useServices`); `AppHeader` montado en `AppLayout`; `RunToolbar` + `RunDropdown` custom (3 secciones + "+ Edit / New Configuration…", teclado, Esc) + `RunConfigModal` (plantillas, crear/duplicar/editar/eliminar, campos completos, `EnvEditor` extraído de `ServiceModal`); pestañas dinámicas `Run: <file>` en `BottomPanel` con `LogViewer`; Services agrupado por `tech` incluyendo efímeros con `showInServices`; Run→Restart cuando corre, Stop carmesí `#dc2626`, punto verde/ámbar; guardar buffer sucio antes de ejecutar Current File; atajos `Shift+F10`/`Ctrl+Shift+R` y `Ctrl+F2` (nunca Cmd+R). Sin `<select>` nativo. NO cambies tokens de color de Tailwind ni el árbol (eso es el Bloque C); usa hex inline o clases existentes. Termina con `npm run build` verde y `cd src-tauri && cargo check`. No hagas commits. Reporta archivos tocados y cómo probar los criterios de aceptación.

### Prompt BLOQUE C (coder Sonnet)
> Proyecto Tauri 2 + React/TS/Tailwind en `/Users/cesargarrido/Proyectos/caesarCode`. El backend ya expone `FsEntry.module` (`maven|gradle|node|cargo|go|python`). Lee `/Users/cesargarrido/Proyectos/caesarCode/PLAN2.md` secciones 0 y 4 (Bloque C) y síguelas. Lee antes: `tailwind.config.js`, `src/index.css`, `src/lib/types.ts` (FsEntry), `src/components/sidebar/{FileTree,FileTreeNode,FileIcon}.tsx`, `src/store/workspace.ts`, `src/components/editor/{theme.ts,TabBar.tsx}`, `src/components/bottom/TerminalView.tsx`, `src/components/layout/{WelcomeView,AppHeader}.tsx`, `scripts/make-icon.py`, `src-tauri/tauri.conf.json`. Tareas: `src/lib/palette.ts` como fuente única; tokens Tailwind/CSS vars nuevos; `src/lib/classifyFolder.ts` con la firma y prioridades del plan y su uso en `FileTreeNode` (tintar solo icono, badge de módulo, excluidos grises y colapsados, sin romper badges Git, menú contextual CRUD ni notify); tema CodeMirror y xterm coherentes (ámbar foco/cursor/pestaña activa, carmesí solo marca y errores); grid pixel-art 32×32 en `src/assets/brand-grid.txt` (C dorada, laurel, capa carmesí) usado por `BrandMark.tsx` (SVG crispEdges) y por `scripts/make-icon.py` reescrito → `src-tauri/app-icon.png` → `npx tauri icon src-tauri/app-icon.png`; BrandMark en `AppHeader` y `WelcomeView`. Añade `vitest` + script `test` y `src/lib/classifyFolder.test.ts`. Termina con `npm run build` y `npm test` verdes. No hagas commits. Reporta archivos tocados y una captura/descripción del icono generado.

---

## 6. Orquestación
1. explorer: no necesario (este plan ya fija archivos y contratos).
2. coder A → revisión principal (`cargo test`, leer diff de `manager.rs` y registros IPC).
3. coder B y coder C en paralelo (worktrees separados o secuencial si se prefiere evitar conflictos en `types.ts`/`AppHeader.tsx`: C solo edita `AppHeader` para insertar `BrandMark`, hacerlo después de B si van en el mismo árbol).
4. Verificación final manual en `npm run tauri dev`: criterios de aceptación A/B/C + `ps -ef | grep` sin huérfanos tras Stop y tras cerrar la app.
