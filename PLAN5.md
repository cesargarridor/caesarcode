# PLAN5 — Implementación de SPEC5: workspace multi-raíz + selector Git por repo + iconos `@react-symbols/icons`

Orden: **A → B → C**, en secuencia. A es independiente (solo frontend de iconos/estilo). B es backend Rust y deja el frontend actual funcionando (todos los parámetros nuevos son opcionales). C consume B.
Cada bloque termina en verde con: `npx tsc --noEmit && npm run build && npm test` y, si toca Rust, `cd src-tauri && cargo check && cargo test && cargo clippy --all-targets -- -D warnings`.

---

## 0. Decisiones transversales

### 0.1 Modelo multi-raíz

```rust
// src-tauri/src/workspace/mod.rs
pub struct Root {
    pub path: PathBuf,      // canonicalizada (clean_canonicalize): identidad de la raíz
    pub name: String,       // basename; si se repite entre raíces: "api (services)" (basename del padre)
    pub exists: bool,       // false si la carpeta persistida ya no existe (no se vigila ni indexa)
}
pub struct Workspace {
    pub roots: Vec<Root>,   // roots[0] = raíz PRIMARIA (identidad del workspace)
}
```

- `AppState.workspace: RwLock<Option<PathBuf>>` → `RwLock<Option<Workspace>>`.
- Helpers en `AppState` (los comandos los usan en vez de `root()`):
  - `primary() -> AppResult<PathBuf>` (= antiguo `root()`; mantener `root()` como alias `#[deprecated]` no: simplemente renombrar y que `root()` siga existiendo devolviendo la primaria, para no romper módulos no migrados).
  - `roots() -> AppResult<Vec<PathBuf>>` (solo `exists == true`).
  - `root_for(path) -> AppResult<PathBuf>`: raíz que contiene `path` (prefijo más largo; las raíces no se solapan, ver 0.4).
- **Raíz primaria** = la carpeta abierta con `workspace_open`. Quitarla no está permitido (`workspace_remove_root` sobre `roots[0]` → `InvalidArgument("la raíz principal no se puede quitar; abre otra carpeta")`).
- `WorkspaceInfo` mantiene `root` y `name` (= primaria, compatibilidad con todo el frontend que usa `useWorkspace(s => s.root)`) y añade `roots: Vec<RootInfo { path, name, exists }>`.

### 0.2 Persistencia: **app data**, no `.caesar/workspace.json`

Decisión: `<app_data_dir>/workspaces.json` = `{ version: 1, workspaces: { "<ruta canónica primaria>": { roots: ["<abs>", ...], activeRepo?: "<abs>", updatedAt } } }`, escritura atómica (tmp + rename, mismo patrón que `projects/recent.rs`), máx. 100 entradas (LRU por `updatedAt`), JSON corrupto → backup `.bak` y vacío (igual que `recent.rs`).

Por qué:
1. Las rutas absolutas de otras carpetas son propias de la máquina del usuario: en `.caesar/` (que suele commitearse junto a `services.json` / `run-configs.json`) ensuciarían el repo o romperían en otro equipo.
2. Si la primera raíz es un repo ajeno o de solo lectura, no queremos escribir en él.
3. El conjunto de raíces se asocia a la raíz primaria: reabrir esa carpeta (recientes, `cc.lastWorkspace`) restaura las raíces adjuntas, como IntelliJ.
- No-objetivo de PLAN5: compartir el workspace en el repo. (Futuro: importar `.caesar/workspace.json` con rutas relativas si existe.)

### 0.3 Compatibilidad hacia atrás
- `workspace_open(path)` sigue igual de cara al frontend: crea `Workspace { roots: [path] ++ raíces persistidas para path }`. Una carpeta sin entrada en `workspaces.json` = exactamente el comportamiento actual.
- Todos los comandos que hoy usan `state.root()` aceptan rutas de cualquier raíz. Los comandos con config por raíz (`services_*`, `run_configs_*`, `run_detect`) reciben `root: Option<String>`; `None` = primaria → el frontend actual funciona sin cambios tras B.
- `git_*` reciben `repo: Option<String>`; `None` = repo de la raíz primaria (comportamiento actual). En C el wrapper TS lo hace **obligatorio** (`repo: string`) para que el compilador detecte cualquier llamada olvidada.
- `FsEvent::GitChanged` pasa a `GitChanged { repo: Option<String> }` (serde `tag = "type"`: el frontend actual sigue viendo `type: 'gitChanged'`).

### 0.4 Reglas de raíces
- Añadir: debe existir y ser carpeta; se canonicaliza (resuelve symlinks; dos rutas que apuntan a lo mismo = duplicada → error `"ya está en el workspace"`).
- **Sin solapes**: rechazar si la nueva está dentro de una existente (`"ya está dentro de <name>"`) o si contiene a una existente (`"contiene a <name>: abre esa carpeta como principal"`). Así `root_for` es inequívoco y no hay eventos/índices duplicados.
- Límite blando: 64 raíces (error claro por encima).
- Raíz persistida que ya no existe: se devuelve con `exists: false`; no se vigila, no se indexa, no se busca; en el árbol aparece atenuada con acción "Quitar del workspace". **Nunca** se borra automáticamente del JSON (puede ser un disco desmontado).
- Raíz borrada en caliente: el watcher recibe evento sobre la propia raíz → `FsEvent::RootsChanged` → el frontend vuelve a pedir `workspace_roots` (que recalcula `exists`).

### 0.5 Detección de repos Git
- Por raíz: (1) `git rev-parse --show-toplevel` desde la raíz (cubre raíz = subcarpeta de un repo); (2) recorrido de subcarpetas con **profundidad ≤ 3** (constante `REPO_SCAN_DEPTH`) buscando entradas `.git` (dir **o archivo**: worktrees/submódulos), sin seguir symlinks, saltando `node_modules target .venv venv dist build out .next .idea coverage __pycache__ .gradle` y carpetas ocultas salvo `.git`. No descender dentro de un `.git`. Máx. 200 repos (truncar + `truncated: true`).
- Dedupe por top canónico. Cada repo: `{ top, name, root, depth, nested: bool (top dentro de otro repo detectado), submodule: bool (.git es archivo con gitdir .../modules/...) }`. Orden: por raíz, luego profundidad, luego nombre.
- Repos anidados: se listan todos; el repo "dueño" de un archivo = el top más largo que lo contiene (badges y "abrir en Git").

### 0.6 Modelo Git por repo
- Todos los `git_*` (≈30 en `git/commands.rs`) + `git_remote` + `git_init` reciben `repo: Option<String>`.
- Validación de `repo` (`git/exec.rs::ctx_for(state, repo)`): canonicalizar; aceptar si está en la caché `git_repos` de la última detección; si no, ejecutar `rev-parse --show-toplevel` dentro y exigir que coincida con `repo` y que el repo esté relacionado con alguna raíz (top dentro de una raíz o una raíz dentro del top). Si no → `InvalidArgument("repo fuera del workspace")`.
- `Ctx { ws, top }` → `Ctx { roots: Vec<PathBuf>, top }`. `rel_path` (validate.rs) valida rutas absolutas contra **el top** del repo validado (más la comprobación léxica actual para borrados) en vez de contra `ws`.
- Concurrencia por repo: `git_op: tokio::Mutex<()>` → `git_ops: Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>`; `git_busy: AtomicU32` → `git_busy: Mutex<HashMap<PathBuf, u32>>`; `git_last_run` → `Mutex<HashMap<PathBuf, Instant>>`. `OpGuard::acquire(state, &top)` / `BusyGuard::new(state, &top)`. Así un pull lento en el repo A no bloquea un stage en B ni silencia el `gitChanged` de B.
- `git_top` cache se elimina (el repo llega explícito); `git_untracked_normal` pasa a `HashSet<PathBuf>` por repo.
- `RemoteOp::Pull { rebase, #[serde(default)] ff_only: bool }` para "Pull all".

### 0.7 Iconos (Parte 2) — convivencia
- Se **reemplaza**: `src/components/sidebar/icons/FileTypeIcons.tsx` y `FolderIcons.tsx` (SVG propios de tipos de archivo y carpetas), los `lucide` de `run/TechIcon.tsx`.
- Se **conserva**: clasificación Java/Kotlin (`fs_classify`, `store/symbols.ts`, `SymbolInfo`), círculos C/I/E/R/@ y overlays Spring/run/test (se mueven a `src/components/icons/JvmSymbolIcon.tsx`), `classifyFolder.ts` (roles source/test/resources/excluded/module), `iconColors.ts` (colores de símbolos/overlays; quitar solo claves que queden sin uso), `fileIconRegistry.ts` como **resolver puro** (sin React) testeable.
- Regla: archivo `.java/.kt/.kts/.groovy/.scala` con símbolo clasificado (`kind !== 'unknown'`) → `JvmSymbolIcon`; si no → icono del paquete.
- `lucide-react` queda solo para controles de UI.

### 0.8 Trampas (todas)
1. **Migración de cwd implícito**: grep `state.root()` (≈25 usos: fs, git, lsp, run, services, terminal, symbols) y migrar uno a uno a `roots()`/`root_for()`/`ctx_for()`. Ninguno debe quedar usando la primaria cuando el path pertenece a otra raíz. Test: comando fs/git sobre archivo de la raíz 2.
2. **git_busy / git_op por repo** (0.6). El watcher decide `busy/recent` con el repo del evento.
3. **Repos anidados**: dueño = top más largo; detección no entra en `.git`; el evento `.git/HEAD` de un repo interno se atribuye a ese repo (padre de `.git`), no al externo.
4. **Symlinks**: identidad por ruta canónica; la detección y el walker no siguen symlinks de carpetas (evita bucles y repos duplicados); `ensure_in_roots` compara contra rutas canónicas (igual que hoy).
5. **Rendimiento con decenas de raíces**: un único debouncer con `watch()` por raíz (no N debouncers); índice de Quick Open por raíz y perezoso; búsqueda global con presupuesto compartido `max_results` y cancelación por `search_gen` entre raíces; resúmenes git con semáforo de 4 en paralelo y `--untracked-files=normal` si el repo está marcado lento; en el árbol las raíces arrancan colapsadas salvo la primaria si hay > 5; en Linux, error de límite inotify → log + toast una vez, sin abortar.
6. **Estado persistido obsoleto** (0.4): `exists:false`, nunca borrado silencioso; `activeRepo` persistido que ya no existe → caer al repo de la primaria.
7. **Ids de servicios repetidos entre raíces**: id visible = `id` en la primaria y `"<rootName>/<id>"` en las demás (ver B.5). Al guardar se envía el id crudo + `root`.
8. **Archivos sin guardar** al quitar una raíz: preguntar como hace `openWorkspace` y cerrar las pestañas de esa raíz.
9. **Raíz que deja de existir mientras hay servicios corriendo en ella**: `set_roots` para solo los servicios de raíces quitadas, no los de las que siguen.

---

## Bloque A — Iconos `@react-symbols/icons` + ajustes visuales (independiente)

**Prompt (coder Sonnet):**
> Proyecto `/Users/cesargarrido/Proyectos/caesarCode` (Tauri 2 + React 18 + Tailwind 3 + vitest). Lee `SPEC5.md` (Parte 2) y la sección 0.7 y Bloque A de `PLAN5.md`. Implementa solo el Bloque A. No toques Rust, `src/store/**` (salvo imports), ni `src/lib/ipc.ts`.
> 1. `npm install @react-symbols/icons` (versión exacta en package.json, sin `^`). ANTES de escribir código, inspecciona `node_modules/@react-symbols/icons/package.json` (`exports`, subrutas como `/utils`, `/files`, `/folders`) y TODOS los `.d.ts`; escribe en un comentario de cabecera de `src/components/icons/SymbolIcon.tsx` la lista de exports que usas, con la subruta exacta. Si el paquete trae componentes auto-resolutores (p. ej. `FileIcon`/`FolderIcon` con `fileName`/`folderName`), impórtalos con alias (`PkgFileIcon`) para no chocar con los nuestros. Usa SOLO nombres verificados en los `.d.ts`; nada inventado.
> 2. Resolver puro `src/lib/fileIconRegistry.ts`: amplía `fileIconKey(name, ext, symbol)` para cubrir todos los nombres/patrones/extensiones de SPEC5 Paso 2.1 (orden: símbolo JVM > nombre exacto > patrón > extensión > `generic`). Claves propias (string union `FileIconName`), p. ej. `pom, gradle, mvnw, springApp, java, kotlin, jar, properties, pyproject, requirements, pipfile, poetry, uv, djangoManage, python, notebook, packageJson, tsconfig, vite, next, tailwind, pnpm, yarn, bun, typescript, react, javascript, html, css, scss, svg, cargo, rust, gomod, go, docker, dockerignore, git, env, readme, license, makefile, caesar, json, yaml, xml, toml, ini, sql, graphql, proto, shell, markdown, text, c, cpp, h, image, lock, testFile, generic`. `*Application.java` → `springApp` solo si no hay símbolo clasificado (el símbolo con estereotipo `springBootApp` ya lleva overlay). Añade `folderIconName(name, style)` puro: por nombre (listas de SPEC5 Paso 2.2, case-insensitive, `controllers?`, `services?`, `models?`, `repositor(y|ies)`) y si no por rol de `classifyFolder`.
> 3. `src/components/icons/SymbolIcon.tsx` (NUEVO) exporta `FileIcon({ filename, path?, className?, symbol? })`, `FolderIcon({ name, path?, isOpen, isModule?, style?, className? })` y `TechIcon({ tech, className? })` con `tech: 'spring'|'java'|'python'|'node'|'typescript'|'go'|'rust'|'docker'|'shell'|'other'` (superset del `Tech` de types.ts). Tablas `Record<FileIconName, ComponentType>` y equivalente para carpetas → componente del paquete. Si una clave no tiene icono específico en el paquete, usa el más cercano verificado o el genérico del paquete (documenta en comentario). Carpetas: variante abierta del paquete si existe; si no, misma con `brightness-110`. Excluidas → `opacity-55` (el nombre `text-zinc-500` lo pone el nodo). `isModule` → anillo/tono ámbar (`text-accent` / `drop-shadow` sutil), sin hex nuevo. Tamaño por defecto `w-4 h-4 shrink-0`.
> 4. Mueve `src/components/sidebar/icons/SymbolIcon.tsx` → `src/components/icons/JvmSymbolIcon.tsx` (con `Svg`, `SPRING_LEAF` y colores que necesite desde `src/lib/iconColors.ts`). Borra `FileTypeIcons.tsx`, `FolderIcons.tsx` e `index.ts` de `sidebar/icons/` cuando no queden imports. `src/components/sidebar/FileIcon.tsx` pasa a adaptador fino sobre los nuevos componentes (mismas props que hoy, para no reescribir `FileTreeNode`), o actualiza `FileTreeNode` directamente; elige uno y quita el otro.
> 5. Aplica en: FileTree (incluidas carpetas compactadas `com.example.x` → icono por el último segmento), `editor/TabBar.tsx`, `editor/Breadcrumbs.tsx` (último segmento), `palette/QuickOpen.tsx` y `palette/GlobalSearch.tsx` (icono por resultado), `run/RunDropdown.tsx` + `run/RunConfigModal.tsx` (TechIcon; en modo Current File, FileIcon del archivo), `bottom/ServicesPanel.tsx` + `bottom/ServiceRow.tsx` (TechIcon en cabecera de grupo y cada card). `src/components/run/TechIcon.tsx` queda como re-export del nuevo `TechIcon` (o actualiza sus imports y bórralo). Mapear `Tech` de Rust: `spring→spring, python, node, go, rust, docker, other`.
> 6. Paso 4 visual: `FileTreeNode` seleccionado → `bg-white/[0.08] text-white rounded-md` + indicador `before:` de **2px** `before:bg-accent` (accent = #f59e0b; no hex en componentes), nombre seleccionado `text-white` (no `text-accent-text`). `ServicesPanel` botón "All Services" → `bg-white/[0.06] text-zinc-200 rounded-lg` cuando está activo (sin `bg-accent/12`), hover `hover:bg-white/[0.04]` cuando no.
> 7. Tests vitest: adapta `src/lib/fileIconRegistry.test.ts` (claves que cambian: p. ej. `svg` deja de ser `image`, `makefile` deja de ser `config`, `Cargo.toml`/`go.mod`/`package.json` tienen clave propia) y añade casos para cada grupo de SPEC5 Paso 2.1 y para `folderIconName` (nombre gana a rol; excluida; module). `classifyFolder.test.ts` no debería cambiar; si cambia, justifica. Los tests siguen siendo `.ts` puros (sin React/jsdom).
> Verificación: `npx tsc --noEmit && npm run build && npm test`. `grep -rn "sidebar/icons" src` vacío. `grep -rn "lucide-react" src/components/run/TechIcon.tsx src/components/icons` vacío.

**Criterios de aceptación A**
- Paquete instalado con versión fija; cabecera de `SymbolIcon.tsx` lista los exports usados (verificados en `.d.ts`).
- Árbol, pestañas, breadcrumbs, Quick Open, búsqueda global, Run widget/modal y Services usan los nuevos componentes; Java/Kotlin clasificados siguen mostrando C/I/E/R/@ y overlays.
- Selección del árbol = píldora neutra con barra ámbar de 2px; "All Services" = pill limpio.
- tsc/build/test verdes; sin imports rotos; bundle no crece de forma absurda (si el paquete no hace tree-shaking por subruta, importar por subruta).

---

## Bloque B — Backend multi-raíz + Git por repo (Rust)

**Prompt (coder Sonnet):**
> Proyecto `/Users/cesargarrido/Proyectos/caesarCode/src-tauri`. Lee `SPEC5.md` (Parte 1) y las secciones 0.1–0.6, 0.8 y Bloque B de `PLAN5.md`. Implementa solo Rust. El frontend actual NO se toca y debe seguir funcionando: todos los parámetros nuevos son `Option` (None = primaria / repo de la primaria). Mantén el estilo existente (errores `AppError`, `run_blocking`, escritura atómica tmp+rename, tests en el propio módulo con tempdirs).

### B.1 Estado y workspace (`state.rs`, `workspace/{mod,commands,store}.rs`)
- Tipos `Root`, `Workspace`, `RootInfo`, `WorkspaceInfo { root, name, roots }` (0.1). `AppState.workspace: RwLock<Option<Workspace>>`; helpers `primary()`, `root()` (= primary, para no romper), `roots()`, `root_for(&Path)`.
- `workspace/store.rs` (NUEVO): `load(data_dir, primary) -> Vec<PathBuf>`, `save(data_dir, primary, roots)`, `set_active_repo(...)`, LRU 100, JSON corrupto → `.bak` (copiar patrón de `projects/recent.rs`). Tests: roundtrip, corrupto, sin tmp residual, LRU.
- `workspace/validate.rs` (NUEVO o dentro de mod): `check_new_root(existing: &[PathBuf], new: &Path) -> AppResult<PathBuf>` con las reglas 0.4 (duplicado, dentro, contiene, límite 64). Tests puros con tempdirs incluyendo symlink a una raíz existente (`#[cfg(unix)]`).
- Comandos nuevos (registrar en `lib.rs`):
  - `workspace_roots() -> Vec<RootInfo>` (recalcula `exists`).
  - `workspace_add_root(path) -> WorkspaceInfo`: valida, añade, persiste, `watch` en el watcher vivo, marca índice sucio, `services.set_roots(..)`.
  - `workspace_remove_root(path) -> WorkspaceInfo`: no la primaria; `unwatch`, quita índice, para servicios de esa raíz, apaga clientes LSP con esa raíz (`lsp.take_root(&root)` NUEVO + `shutdown_clients`), limpia `symbols` de rutas bajo la raíz, persiste.
  - `workspace_open(path)`: como hoy + restaura raíces persistidas (inexistentes → `exists:false`).
- `util/paths.rs`: `ensure_in_roots(roots: &[PathBuf], p) -> AppResult<(PathBuf /*resuelta*/, PathBuf /*raíz*/)>` y `_nofollow`; implementadas sobre `check_inside` existente. Tests.

### B.2 fs (`fs/{commands,watcher,quick_open,search,symbols}.rs`)
- Todos los `fs_*` validan con `ensure_in_roots`.
- Watcher: un único debouncer; `WatcherHandle` expone `watch_root(&Path)` / `unwatch_root(&Path)`; `fs_watch_subscribe` vigila todas las raíces existentes. El callback obtiene las raíces del `AppState` en cada lote y clasifica cada ruta respecto a su raíz. Ruta dentro de `.git` → `GitChanged { repo: Some(padre_del_.git) }` (repos anidados correctos), filtrando `busy/recent` **de ese repo**. Evento sobre la propia raíz (borrada/renombrada) → `FsEvent::RootsChanged`. Error de `watch` (límite inotify) → log + continuar.
- Quick Open: `FileIndex` → índice por raíz (`HashMap<PathBuf, RootIndex{files, dirty}>`), reconstrucción perezosa solo de las sucias. `QuickOpenItem` añade `root: String` (nombre de la raíz); `rel_path` se prefija con `"<rootName>/"` solo si hay > 1 raíz. Scoring sin cambios.
- `search_global`: recorre raíces en orden, mismo `gen` de cancelación, presupuesto `max_results` compartido; `SearchMatch` añade `root`; `files_searched` acumulado.
- `fs_classify`: rutas validadas contra `roots()`.

### B.3 Git (`git/*.rs`)
- `exec.rs`: `ctx_for(state, repo: Option<String>) -> AppResult<Ctx>` (0.6); `Ctx { roots, top }`; guards por repo (`OpGuard::acquire(state, &ctx.top)`); `mark_run(state, &top)`. Quitar `git_top`.
- `state.rs`: `git_ops`, `git_busy`, `git_last_run`, `git_untracked_normal` por repo; `git_repos: Mutex<Vec<RepoEntry>>` (caché de detección).
- `validate.rs::rel_path`: contra `ctx.top` (+ comprobación léxica existente para borrados).
- Todos los `#[tauri::command] git_*` de `commands.rs` añaden `repo: Option<String>` y lo pasan a `ctx_for`. `git_status` (badges, formato v1 actual) también. `git_init(path)`: `path` dentro de alguna raíz; al terminar invalida `git_repos`.
- `remote.rs`: `Pull { rebase, #[serde(default)] ff_only }` → `git pull --ff-only` si `ff_only`.
- `git/repos.rs` (NUEVO): `detect(roots) -> ReposResult { repos: Vec<RepoEntry>, truncated }` (0.5) y `summary(state, top) -> RepoSummary { top, branch, detached, upstream, ahead, behind, changed, staged, untracked, conflicted, state: Option<String> /*merging|rebasing...*/ }` reutilizando el parseo de `status2.rs` (`status --porcelain=v2 --branch -z`).
- Comandos nuevos: `git_repos() -> ReposResult` (detecta y cachea), `git_repo_summary(repo) -> RepoSummary`, `git_repos_summary() -> Vec<RepoSummary>` (semáforo 4, un repo que falla → entrada con `error: Some(msg)`, no aborta el resto).

### B.4 LSP (`lsp/{commands,manager}.rs`)
- `resolve()` usa `ensure_in_roots` y devuelve la raíz dueña del archivo → la clave `(root, lang)` ya existente da "un servidor por raíz+lenguaje" sin más cambios. Revisar `servers.rs::server_command`: si usa un data dir por workspace (jdtls), debe derivarse de la raíz concreta (hash de la ruta) para no compartirlo.
- `LspManager::take_root(&Path) -> Vec<Arc<LspClient>>` (NUEVO) para `workspace_remove_root`. `workspace_open` sigue con `take_all`.

### B.5 Servicios y Run (`services/*`, `run/*`)
- `ServiceManager`: `root: RwLock<Option<PathBuf>>` → `roots: RwLock<Vec<PathBuf>>` (primaria = [0]); configs como `Vec<LoadedService { root, cfg }>`. `set_workspace(root)` queda como `set_roots(vec![root])` (los tests existentes de `set_workspace` deben seguir pasando). `set_roots(new)`: para solo servicios/logs/efímeros de raíces que desaparecen; recarga config de todas.
- `cwd` de cada servicio relativo a **su** raíz; `ensure_in_workspace(&its_root, ...)`.
- Id visible: `cfg.id` si raíz primaria, `"<rootName>/<id>"` si no. `ServiceStatus` y `ServiceConfig` expuestos añaden `root: String` (`#[serde(default)]`, no se persiste en `services.json` → usar un DTO o `skip_serializing` en el guardado).
- Comandos: `services_load(root: Option<String>)`, `services_save(root: Option<String>, services)`, el resto (start/stop/logs) por id visible sin cambios de firma.
- Run: `run_configs_load/save(root: Option<String>)`, `run_detect(root: Option<String>)`, `run_resolve_file(path)` usa `root_for(path)` y los `file_runners` de esa raíz; `run_start` con target de archivo → cwd por defecto = raíz del archivo. `RunConfig` expuesto con `root`.
- Terminal: `cwd` validado con `ensure_in_roots`; por defecto primaria.

### B.6 Recientes (`projects/recent.rs`)
- `RecentProject` añade `#[serde(default)] extra_roots: u32` (rellenado al listar desde `workspaces.json`); `touch` solo con la primaria.

### B.7 Tests headless (tempdirs, `git` real en PATH; si no hay git, `return` temprano como en `tests_repo.rs`)
- `workspace`: add/remove/duplicado/dentro/contiene/symlink/primaria no removible/persistencia y restauración con raíz borrada (`exists:false`).
- `repos::detect`: tempdir con `a/` (repo), `b/` (sin repo), `b/svc1/` y `b/deep/x/y/z/` (repo a profundidad 4 → NO detectado), `node_modules/pkg/.git` (ignorado), repo anidado `a/vendor/lib` (nested=true), worktree (`git worktree add` → `.git` archivo), symlink a `a/` dentro de `b/` (no duplica).
- `repos::summary`: repo con remoto bare local, commit adelantado → `ahead=1`; tras `fetch` de un commit remoto → `behind=1`; archivos modificado/staged/untracked/conflicto contados.
- Git por repo: dos repos en dos raíces; `git_stage` con `repo=A` de un archivo de B → error; `OpGuard` de A no bloquea a B (adquirir A, `try_lock` de B ok); `busy` de A no silencia `GitChanged` de B (test de la función de decisión, sin notify real).
- `ensure_in_roots`, `root_for` (prefijo más largo, rutas `..`).
- Servicios: dos raíces con el mismo id `api` → ids visibles `api` y `svc2/api`; `set_roots` quitando la 2ª para solo ese servicio (proceso `sleep`, como los tests actuales).

**Criterios de aceptación B**
- `cargo check && cargo test && cargo clippy --all-targets -- -D warnings` verdes; tests existentes sin tocar su intención.
- `grep -n "state.root()" src-tauri/src -r` → solo usos justificados (primaria explícita: recientes, terminal por defecto, `services_*`/`run_*` con `None`).
- El frontend actual (sin cambios) abre una carpeta, ve árbol, git, servicios y run igual que antes (`npm run tauri dev`, prueba manual rápida).

---

## Bloque C — Frontend multi-raíz + selector Git + árbol multi-raíz + Add project

**Prompt (coder Sonnet):**
> Proyecto `/Users/cesargarrido/Proyectos/caesarCode`. Lee `SPEC5.md` (Parte 1), las secciones 0.x y Bloque C de `PLAN5.md`, y el Rust de B (`workspace/`, `git/repos.rs`, firmas de `git/commands.rs`, `services/commands.rs`, `run/commands.rs`). Implementa solo frontend. Estilo visual: tokens de PLAN4 (`bg-elevated`, `border-border-strong`, `rounded-lg`, sin hex en componentes) e iconos del Bloque A (`FolderIcon isModule` para raíces).

### C.1 Tipos e IPC (`src/lib/types.ts`, `src/lib/ipc.ts`)
- `RootInfo`, `WorkspaceInfo.roots`, `RepoEntry`, `ReposResult`, `RepoSummary`, `QuickOpenItem.root`, `SearchMatch.root`, `FsEvent` (`gitChanged` con `repo?: string | null`, nuevo `rootsChanged`), `ServiceConfig/ServiceStatus/RunConfig.root`, `RecentProject.extraRoots`, `RemoteOp` pull con `ffOnly`.
- Wrappers: `workspaceRoots`, `workspaceAddRoot`, `workspaceRemoveRoot`, `gitRepos`, `gitRepoSummary`, `gitReposSummary`. **Todos los `git*` añaden `repo: string` como PRIMER parámetro obligatorio** (tsc encontrará cada llamada a migrar). `servicesLoad/Save`, `runConfigsLoad/Save`, `runDetect` con `root?: string`.

### C.2 Store workspace (`src/store/workspace.ts`)
- Estado: `roots: RootInfo[]` (además de `root`/`name` = primaria), `gitByRepo: Record<string, GitStatus>`, `git` = agregado (unión de `files`) para que `FileTree`/badges sigan funcionando; `branch` del agregado = el del repo de la primaria.
- Acciones: `addRoot(path)` (llama IPC, actualiza `roots`, `expanded[root]=true` salvo > 5 raíces, `refreshDir`, recarga repos, servicios y run configs de esa raíz), `removeRoot(path)` (confirma si hay pestañas sucias de esa raíz → cierra pestañas de la raíz, limpia `children/expanded` bajo ella, quita su git), `refreshRoots()`.
- `refreshGit()` → por repo (debounce actual), solo el repo afectado cuando `gitChanged.repo` viene; sin repo → todos. `rootsChanged` → `refreshRoots()`.
- `openWorkspace` sigue igual + carga `roots` de la respuesta. Exporta `addRootDialog()` (diálogo de carpeta `open({directory:true, multiple:true, title:'Añadir proyecto al workspace'})`, añade cada una, toasts con errores de validación).
- `src/lib/paths.ts`: `rootFor(roots, path)`, `repoFor(repos, path)` (top más largo) + tests vitest nuevos `paths.test.ts`.

### C.3 Store git (`src/store/git.ts`)
- `repos: RepoEntry[]`, `summaries: Record<string, RepoSummary>`, `activeRepo: string | null`, `view: 'repo' | 'all'`.
- `setActiveRepo(top)`: resetea log/selección/diff/fileHistory/búsqueda y `refreshAll()`; persiste en `localStorage` `cc.activeRepo:<primaria>` (try/catch). Al cargar: persistido si sigue en `repos`, si no el repo de la primaria, si no el primero, si no `null` (pantalla "no hay repos" con `initRepo`).
- Todas las acciones pasan `get().activeRepo`; `busyCount`/`busy`/`remoteJob` **por repo** (`busyByRepo: Record<string,string>`), para que el selector muestre spinner por repo y no se bloquee otro repo.
- `refreshSoft` solo del repo activo; resumen del repo afectado tras cada op y en `gitChanged`.
- `loadRepos()` (al abrir workspace, al añadir/quitar raíz, tras `initRepo`), `loadSummaries()` (al abrir el selector o la vista All; caché 10 s).
- `fetchAll()` / `pullAll()`: cola con concurrencia 3 usando `gitRemote(repo, ...)` y `jobId` por repo; `pullAll` usa `ffOnly: true` y salta repos con cambios sin commitear, sin upstream o en merge/rebase (los reporta). Al final toast resumen "7 ok · 2 saltados · 1 error" + panel con detalle en la vista All.
- "Abrir en Git" desde un archivo/raíz: `openInGit(path)` = `setActiveRepo(repoFor(path))` + `setCenterView('git')`.

### C.4 UI Git
- `src/components/git/RepoSelector.tsx` (NUEVO): dropdown (patrón de `RunDropdown`) con buscador si > 8 repos; cada fila: `FolderIcon` + nombre (+ raíz en gris si anidado), rama, `●N` cambios, `↑a ↓b`, spinner si `busyByRepo`; entrada superior "All repos". Teclado: flechas/Enter/Escape.
- `GitView.tsx`: cabecera con `RepoSelector`; si `view==='all'` renderiza `AllReposView.tsx` (NUEVO): tabla de repos (nombre, raíz, rama, cambios, ahead/behind, estado, error), botones Fetch all / Pull all (ff-only) / Refresh, clic en fila → `setActiveRepo` + `view='repo'`. Filas sin `transition` ni sombras (PLAN4 trampa 5).
- `AppHeader.tsx` `BranchChip` → `RepoBranchChip`: `nombreRepo · rama · ●N ↑a ↓b` (oculta nombre si solo hay 1 repo, así queda como hoy); clic abre el mismo `RepoSelector`; elegir → vista Git.
- `StatusBar`: rama del repo activo.

### C.5 Árbol multi-raíz (`components/sidebar/*`)
- `Sidebar.tsx`: renderiza un `RootNode` por raíz (si solo hay 1, se puede mantener el aspecto actual pero con el nodo raíz visible: decidir uno y aplicarlo siempre; recomendado: nodo raíz siempre visible, como IntelliJ). Botón de cabecera "Añadir proyecto" (`FolderPlus` de lucide) → `addRootDialog()`.
- `RootNode.tsx` (NUEVO): fila de nivel 0 con `FolderIcon isModule`, nombre en `font-medium`, rama del repo de la raíz en `text-faint`, contador de cambios; atenuado + "(no encontrado)" si `exists:false`. Menú contextual (`ui/ContextMenu`): "Quitar del workspace" (deshabilitado en la primaria con tooltip), "Abrir en Git" (`openInGit(root)`; deshabilitado si no hay repo), "Copiar ruta" (`lib/clipboard.ts`), "Revelar en Finder" solo si ya existe utilidad. Debajo, `<FileTree root={root.path} />` existente.
- `FileTree.useDirtyDirs` y badges: usar `gitByRepo`/agregado; sin cambios de lógica más allá de que `root` sea cada raíz.
- `FileTreeNode` menú contextual: añadir "Abrir en Git" (repo dueño del archivo).
- Rendimiento: raíces colapsadas no piden `fs_read_dir`.

### C.6 Resto de subsistemas
- `palette/QuickOpen.tsx` y `GlobalSearch.tsx`: con > 1 raíz, mostrar `root` como chip gris delante de la ruta relativa.
- `editor/Breadcrumbs.tsx` y `TabBar` (tooltip): ruta relativa a `rootFor(path)` con el nombre de la raíz como primer segmento si > 1 raíz.
- `store/services.ts`: carga por raíz (`servicesLoad(root)` para cada raíz existente), agrupa por raíz y luego por tech en `ServicesPanel` (cabecera de raíz solo si > 1); `ServiceModal` guarda con su `root`; detección "Detectar servicios" por raíz (`runDetect(root)`).
- `store/runConfigs.ts`: configs por raíz, clave de selección `"<root>::<id>"`; `RunConfigModal` agrupa por raíz y al crear pide raíz (por defecto la del archivo activo). "Current File" resuelve con la raíz del archivo (backend ya lo hace).
- Terminal nueva: cwd = raíz del archivo activo o primaria.

### C.7 Add project desde recientes (`components/projects/*`)
- Con un workspace abierto, `ProjectsScreen` muestra en cada fila de `RecentList` una acción "Añadir al workspace" (icono `Plus`, y en el menú contextual). Abrir un reciente con workspace abierto pregunta (`useUi.ask`): "Abrir aquí (reemplaza)" / "Añadir al workspace" / "Cancelar" — equivalente a "This window / Attach" de IntelliJ (no hay multiventana). Filas con `extraRoots > 0` muestran badge "+N".
- Botón "Añadir carpeta al workspace…" en la cabecera de `ProjectsScreen` (solo con workspace abierto).
- Adaptar `RecentList.test.ts` si cambia alguna función pura que testea.

**Criterios de aceptación C**
- tsc/build/test verdes; nuevos tests vitest para `rootFor`/`repoFor` y cola `fetchAll`/`pullAll` si se extrae a función pura (`src/lib/repoQueue.ts` recomendado).
- Manual (`npm run tauri dev`): abrir carpeta A; añadir B y C (diálogo multi-selección) → tres nodos raíz; Quick Open y búsqueda encuentran archivos de las tres; abrir `.ts` de C → LSP arranca para C (`lsp_status` muestra 2 servidores si A también tiene TS); servicio de B arranca con cwd en B; selector Git lista A, B, C (y un repo anidado) con rama/cambios/ahead-behind; stage+commit en B no afecta a A; vista All repos → Fetch all; cerrar y reabrir A restaura B y C; borrar C del disco → nodo atenuado con "Quitar del workspace"; quitar B con pestaña sucia de B → pregunta.
- Con una sola carpeta todo se ve y funciona como antes (salvo el nodo raíz visible).

---

## Reparto

| Bloque | Agente | Depende de | Paralelizable |
|---|---|---|---|
| A — iconos + visual | coder Sonnet (prog-sonnet-1) | — | sí, con B (archivos disjuntos: A solo `src/`, B solo `src-tauri/`) |
| B — backend multi-raíz + git por repo | coder Sonnet (prog-sonnet-2) | — | sí, con A |
| C — frontend multi-raíz + selector + árbol + attach | coder Sonnet (prog-sonnet-3) | A y B terminados | no |

Aunque A y B pueden ir en paralelo (sin conflictos de archivos salvo `package.json`/`package-lock.json`, que solo toca A), el pedido es en secuencia: A → B → C. Tras cada bloque, el coordinador revisa el diff y corre la verificación completa antes de lanzar el siguiente.
