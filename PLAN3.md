# CaesarCode — PLAN FASE 3 (árbol estilo IntelliJ, Proyectos + New Project + Spring Microservices, Git estilo GitKraken, icono macOS)

Fuente de verdad: `SPEC.md` + `PLAN.md` (fase 1) + `PLAN2.md` (fase 2). Todo ya construido. Este documento solo añade.
Bloques **secuenciales**. Cada bloque termina con, en este orden y en verde:
`cd src-tauri && cargo check && cargo test` · `cd .. && npm run build && npm test` (vitest).

| Bloque | Contenido | Agente | Depende de |
|---|---|---|---|
| A | Iconos SVG propios + clasificador de símbolos Java/Kotlin en Rust (caché, perezoso) + roles de carpeta (source/test/resources root, package, excluded, module) + paquetes compactados | coder Sonnet | — |
| B | Proyectos recientes (app data) + pantalla Projects + asistente New Project (generadores locales y por CLI/HTTP con streaming) + inicializador Spring Microservices | coder Sonnet | A (usa iconos de generador; si no, independiente) |
| C1 | Backend Git completo (status v2, log NUL, refs, diff, hunks, ops, stash, tags, remotos con progreso, blame, historial, conflictos) | coder Sonnet | — (puede ir tras B) |
| C2 | Frontend Git estilo GitKraken (vista Git, grafo con lanes en TS virtualizado, refs, cambios, diff CM merge, commit, menú contextual, blame en editor) | coder Sonnet | C1 |
| D | Icono macOS: diagnóstico y verificación (lo investiga el usuario; aquí solo criterios) | usuario / principal | — |

Orden recomendado: A → B → C1 → C2. Nunca dos bloques a la vez (todos tocan `build.rs`, `lib.rs`, `capabilities/default.json`, `src/lib/types.ts`, `src/lib/ipc.ts`).

---

## 0. Decisiones de arquitectura (fijas)

1. **Registro de comandos en TRES sitios** (igual que fases 1-2): `src-tauri/build.rs::COMMANDS`, `src-tauri/src/lib.rs::generate_handler!`, `src-tauri/capabilities/default.json` como `allow-<nombre-con-guiones>`. Un comando sin los tres = fallo en runtime. Cada bloque añade una prueba manual: `grep -c` del nombre en los tres archivos.
2. **Streaming** solo por `tauri::ipc::Channel<T>` como argumento del comando (patrón de `services_subscribe` / `terminal_open`); nunca `emit` global. Si el canal falla (`send` err) se aborta la operación de forma limpia.
3. **Procesos externos** (git, mvn, npm, cargo, go, ng): siempre `tokio::process::Command` con **vector de argumentos**, `stdin(null)`, `kill_on_drop(true)`, `no_window_tokio`. **Prohibido** `sh -c` / `cmd /C` con texto del usuario. Resolución del binario con `which::which` (ya es dependencia). En Windows los `.cmd` (npm, ng) solo se lanzan con argumentos ya validados por regex (ver §5.3).
4. **Sin libgit2.** Git = binario del sistema vía `git::run_git*` (`src-tauri/src/git/mod.rs`). Se reutiliza su configuración (`--no-optional-locks`, `core.quotepath=false`, `GIT_TERMINAL_PROMPT=0`, marca de `git_last_run`).
5. **Grafo de commits: lanes en TypeScript** (`src/lib/gitGraph.ts`, función pura e incremental, testeada con vitest). Rust solo devuelve commits paginados. Motivo: paginación natural (el estado de lanes vive en el store y se continúa página a página), test barato, sin estado por repo en Rust.
6. **Clasificación de símbolos en Rust** (cabecera ≤ 8 KiB, caché por `(path, mtime, len)`), pedida **en lote y perezosamente** por el frontend solo para archivos visibles `java|kt|kts|groovy|scala`. `fs_read_dir` NO lee contenido de archivos (sigue rápido).
7. **Roles de carpeta en Rust** (`FsEntry.role`, `FsEntry.compact`), porque el árbol es perezoso y la detección de source roots depende de la ruta y de marcadores (`pom.xml`, `build.gradle*`).
8. **HTTP** (start.spring.io, code.quarkus.io, launch.micronaut.io): `reqwest` con `rustls`, sin features por defecto; zips con crate `zip` (solo deflate). El webview NO hace peticiones (CSP intacta).
9. **Persistencia global** (proyectos recientes) en `app.path().app_data_dir()/recent-projects.json`, escritura atómica tmp+rename (mismo patrón que `run/store.rs::save`).

### Dependencias nuevas
`src-tauri/Cargo.toml`:
```toml
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "stream"] }
zip = { version = "2", default-features = false, features = ["deflate"] }
[dev-dependencies]
tempfile = "3"
```
`package.json`: `"@codemirror/merge": "^6.10.0"` (solo C2). Nada más.

---

## 1. BLOQUE A — Árbol e iconos estilo IntelliJ

### 1.1 Archivos
Crear:
- `src-tauri/src/fs/symbols.rs` — clasificador + `SymbolCache` + tests con fixtures.
- `src-tauri/src/fs/roles.rs` — `folder_role(path, root) -> Option<FolderRole>` + compactación de paquetes + tests.
- `src-tauri/tests/fixtures/symbols/` — `.java` / `.kt` de prueba (ver §1.6).
- `src/components/sidebar/icons/` — `SymbolIcon.tsx` (círculo/cuadrado + letra + overlays), `FileTypeIcons.tsx` (registro de SVG por tipo de archivo), `FolderIcons.tsx` (carpetas por rol), `index.ts`.
- `src/lib/fileIconRegistry.ts` (+ `fileIconRegistry.test.ts`) — función pura `fileIconKey(name, ext, symbol?) -> IconKey`.
- `src/store/symbols.ts` — caché frontend + cola de lotes.

Tocar:
- `src-tauri/src/fs/mod.rs` (FsEntry: `role`, `compact`), `src-tauri/src/fs/tree.rs` (rellenarlos), `src-tauri/src/fs/commands.rs` (`fs_classify`), `src-tauri/src/state.rs` (`symbols: Mutex<SymbolCache>`), `build.rs`, `lib.rs`, `capabilities/default.json`.
- `src/lib/types.ts`, `src/lib/ipc.ts`, `src/lib/classifyFolder.ts` (acepta `opts.role`; el rol de Rust gana), `src/components/sidebar/FileIcon.tsx` (delegar en iconos nuevos; mantener firma actual + prop opcional `symbol`), `src/components/sidebar/FileTreeNode.tsx` (nombre compacto, hijos de `compact.path`), `src/store/workspace.ts` (tras `refreshDir` encolar clasificación; en `handleFsEvent` invalidar símbolos de rutas cambiadas). También `QuickOpen.tsx` y `TabBar.tsx` usan `FileIcon`: deben seguir funcionando (pasar `symbol` si está en caché).

### 1.2 Contratos
```rust
// fs/mod.rs — FsEntry gana (ambos skip_serializing_if None):
pub role: Option<FolderRole>,
pub compact: Option<CompactChain>,

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FolderRole { SourceRoot, TestRoot, ResourcesRoot, TestResourcesRoot, GeneratedRoot, Package, Excluded, Module }

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompactChain { pub name: String /* "com.example.finance_tracker" */, pub path: String /* carpeta hoja absoluta */ }

// fs/symbols.rs
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SymbolKind { Class, AbstractClass, Interface, Enum, Record, Annotation, Exception, Object /*kotlin*/, DataClass, SealedClass, KotlinFile /*varias decl o solo funciones*/, Script, Unknown }

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Stereotype { SpringBootApp, Main, Controller, Service, Repository, Configuration, Entity, Component, Test }

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInfo { pub kind: SymbolKind, pub stereotype: Option<Stereotype>, pub public: bool }

#[tauri::command] async fn fs_classify(state, paths: Vec<String>) -> AppResult<Vec<(String, SymbolInfo)>>
```
TS (`types.ts`): `FolderRole`, `CompactChain`, `SymbolKind`, `Stereotype`, `SymbolInfo`; `FsEntry` añade `role?: FolderRole | null; compact?: CompactChain | null`. `ipc.ts`: `fsClassify(paths: string[]) => invoke<[string, SymbolInfo][]>('fs_classify', { paths })`.

Comando nuevo: `fs_classify` (permiso `allow-fs-classify`).

### 1.3 Clasificador (`symbols.rs`)
- Solo extensiones `java kt kts groovy scala`; resto → no se pide. Máx 512 rutas por llamada; cada ruta pasa `ensure_in_workspace` (`util/paths.rs`); fuera → se omite (no error global).
- Lee como máximo **8 KiB** (`File::take(8192)`), `run_blocking` / `spawn_blocking`. Si no es UTF-8 → lossy.
- Pre-lexer simple (máquina de estados): elimina comentarios `//`, `/* */`, strings `"..."`, text blocks `"""..."""`, chars `'x'`. Sobre el texto limpio:
  - Anotaciones (regex `@([A-Za-z_][\w.]*)`), última parte del nombre: `SpringBootApplication`→SpringBootApp; `RestController|Controller`→Controller; `Service`→Service; `Repository`→Repository; `Configuration|ConfigurationProperties`→Configuration; `Entity|Table|Document|MappedSuperclass|Embeddable`→Entity; `Component`→Component; `Test|ParameterizedTest|SpringBootTest|WebMvcTest|DataJpaTest`→Test.
  - Herencia: `extends\s+(JpaRepository|CrudRepository|PagingAndSortingRepository|MongoRepository|ReactiveCrudRepository)` → Repository; `extends\s+\w*(Exception|Error|Throwable)\b` → kind Exception.
  - Main: Java `static\s+void\s+main\s*\(`; Kotlin `^\s*fun\s+main\s*\(` (top-level). SpringBootApp tiene prioridad sobre Main.
  - Si no hay anotación, sufijo del nombre de archivo como fallback: `*Controller`, `*Service|*ServiceImpl`, `*Repository|*Repo`, `*Config|*Configuration`, `*Test|*Tests|*IT` (Test solo si además está bajo un TestRoot o tiene `@Test`; sufijo solo → Test igualmente).
  - Kind: primera declaración top-level cuyo nombre == stem del archivo (si no, la primera). Java: `(public\s+)?(abstract\s+|final\s+|sealed\s+|non-sealed\s+|static\s+|strictfp\s+)*(class|interface|enum|record|@interface)\s+(\w+)`. `abstract class`→AbstractClass; `@interface`→Annotation. Kotlin: modificadores `data|enum|sealed|abstract|annotation|value|inline|open|internal|private` + `class|interface|object`; `enum class`→Enum, `annotation class`→Annotation, `data class`→DataClass, `sealed class|interface`→SealedClass, `object`→Object; `.kts` → Script; sin clase pero con `fun` → KotlinFile.
- `SymbolCache`: `HashMap<PathBuf, (SystemTime mtime, u64 len, SymbolInfo)>`, tope 20 000 entradas (al superarlo `clear()`), se limpia en `workspace_open`. Hit si mtime y len coinciden (un `metadata` por ruta, sin leer).

### 1.4 Roles de carpeta (`roles.rs`) y compactación
- Rol por **ruta relativa al módulo más cercano** (sin recorrer disco salvo `is_file` de marcadores ya usado en `tree.rs`):
  - `src/main/{java,kotlin,groovy,scala}` → SourceRoot; `src/test/{...}` → TestRoot; `src/main/resources` → ResourcesRoot; `src/test/resources` → TestResourcesRoot; `target/generated-sources/*`, `build/generated/*` → GeneratedRoot. Fallback no-Maven: `src` en raíz de módulo con `package.json|Cargo.toml|go.mod|pyproject.toml` → SourceRoot; `test|tests|__tests__` → TestRoot.
  - Carpeta **dentro** de un Source/TestRoot JVM → `Package`.
  - `target build out dist node_modules .gradle .idea .venv venv __pycache__ .next coverage` en raíz de módulo → Excluded (reusar `NO_MODULE` de `tree.rs`).
  - Carpeta con marcador (`module` ya calculado) → Module.
- **Compactación** (como “Compact Middle Packages” de IntelliJ), solo si el padre es SourceRoot/TestRoot/Package: para cada hijo carpeta, bajar mientras la carpeta tenga **exactamente una entrada visible y sea carpeta** (máx 16 niveles, sin seguir symlinks). Si bajó ≥1 nivel: `compact = { name: "com.example.finance_tracker", path: <hoja> }`. Coste: un `read_dir` por nivel solo de esa cadena.
- Frontend: nodo con `compact` muestra `compact.name`, expande/carga hijos de `compact.path`, y **todas** las acciones (New File/Folder, Rename, Delete, Copy Path, badges git, `dirtyDirs`, `data-path`) usan `compact.path` salvo Rename, que renombra solo la hoja (documentar en tooltip). `expanded` se indexa por `compact.path`. El watcher refresca el padre real: en `handleFsEvent`, si cambia un dir intermedio de una cadena, refrescar el padre del nodo compacto (basta refrescar los padres de las rutas cambiadas, como ya hace, más `refreshDir` del ancestro que contiene la cadena: buscar el nodo cuyo `compact.path` es prefijo).

### 1.5 Iconos SVG (frontend)
- 16×16, `viewBox="0 0 16 16"`, inline (componentes React, sin archivos de imagen, sin fetch), `shrink-0`, `aria-hidden`. Colores aproximados a IntelliJ New UI dark, definidos en un único objeto `ICON_COLORS` en `FileTypeIcons.tsx`:
  - Símbolos (círculo r=6.5 relleno tenue + borde, letra 9px bold): Class `C` azul `#548AF7`; AbstractClass `C` azul con borde discontinuo; Interface `I` verde `#5FB865`; Enum `E` ámbar `#E5A541`; Record `R` violeta `#B07EE6`; Annotation `@` verde `#5FB865`; Exception `C` rojo `#E06C75`; Object `O` azul; DataClass `C` azul + punto; SealedClass `C` azul + candado mínimo; KotlinFile = logo K violeta; Script = K con `S`.
  - Overlays (esquina inferior derecha, 7×7): SpringBootApp hoja verde `#6DB33F` + triángulo run; Main triángulo verde run; Controller/Service/Repository/Configuration/Entity/Component hoja verde Spring pequeña (title con el estereotipo); Test triángulo rojo/verde. Tooltip `title` del nodo: `"Clase · @RestController"`.
  - Archivos: `pom.xml` (m rojizo), `build.gradle*`/`settings.gradle*` (elefante simplificado o `G` verde), `Dockerfile`/`docker-compose*.yml` (ballena azul simplificada), `html` naranja `<>`, `xml` naranja, `properties` engranaje gris, `yml|yaml` (morado `Y`; `application*.yml|properties` con hoja Spring), `json` `{}` amarillo, `md` `M↓` azul, `sql` cilindro amarillo, `ts/tsx/js/jsx` cuadrado con `TS`/`JS`, `py`, `rs`, `go`, `kt` (sin símbolo aún = K), `java` (sin símbolo aún = `J` taza gris: placeholder hasta que llegue la clasificación), `css/scss`, `sh`, `gitignore`, `.env`, `lock`, imágenes, `txt/log`, genérico.
  - Carpetas (silueta de carpeta IntelliJ): normal gris-dorado del tema; SourceRoot azul; TestRoot verde; ResourcesRoot carpeta con líneas ámbar; TestResourcesRoot con líneas verdes; GeneratedRoot azul con engranaje; Excluded naranja/marrón atenuado; Module carpeta con cuadradito azul; Package icono “paquete” (carpeta pequeña con punto) gris claro.
- `fileIconRegistry.ts`: `fileIconKey(name, ext, symbol?)` pura → clave del registro; `FileIcon` usa el registro. Conservar: badges Git (`STATUS_COLOR`), badge de módulo (`folder.badge`), punto `hasChanges`, menú contextual, `dimName` de excluidas.

### 1.6 Carga perezosa en frontend (`store/symbols.ts`)
- `symbols: Record<path, SymbolInfo>`, `pending: Set<path>`. `request(paths)` añade a cola; flush con debounce 60 ms, lotes de 256 → `fsClassify`. Selector por nodo: `useSymbols((s) => s.symbols[entry.path])` (un selector por fila, sin re-render global).
- Disparo: tras `refreshDir(dir)` exitoso y si `expanded[dir]`, encolar hijos con extensión JVM. Invalidación: en `handleFsEvent` `changed` → `forget(paths)` y re-encolar si visibles.
- Sin clasificación todavía → icono placeholder por extensión (nunca vacío, sin parpadeo de tamaño).

### 1.7 Trampas A
- No clasificar en `fs_read_dir` (directorios con 2 000 archivos `.java` congelarían la expansión). Lote + caché + mtime.
- No usar regex sobre el archivo entero sin quitar comentarios: `// @Service` o un `"class "` en un string darían falsos positivos.
- Archivos enormes/minificados: el tope 8 KiB lo resuelve; no aumentar.
- La compactación cambia la clave del nodo: cuidado con `expanded`, `selected`, `inlineEdit.parent` y `NewEntryRow` (crear dentro de la hoja).
- `classifyFolder` sigue existiendo (Python/Node/etc.); el rol de Rust tiene prioridad y la caché de `classifyFolder` debe incluir `role` en la clave.
- El árbol sigue siendo recursivo (no virtualizado): mantener `memo` en `FileTreeNode`, selectores finos; no pasar objetos nuevos como props en cada render.

### 1.8 Tests A
- Rust (`cargo test`): fixtures en `src-tauri/tests/fixtures/symbols/` leídas con `include_str!` desde `symbols.rs` tests: `UserController.java` (@RestController), `UserService.java` (@Service), `UserRepository.java` (interface extends JpaRepository sin anotación → Interface+Repository), `User.java` (@Entity), `AppConfig.java` (@Configuration), `FinanceTrackerApplication.java` (@SpringBootApplication + main → SpringBootApp), `Main.java` (main → Main), `Shape.java` (abstract class), `Color.java` (enum), `Point.java` (record), `Audited.java` (@interface), `NotFoundException.java`, `Commented.java` (`// @Service` y `"@Entity"` en string → Class sin estereotipo), `UserControllerTest.java` (@SpringBootTest), Kotlin: `Money.kt` (data class), `Result.kt` (sealed), `Registry.kt` (object), `Utils.kt` (solo funciones → KotlinFile), `App.kt` (`fun main` + @SpringBootApplication), `build.gradle.kts` (Script). Más: caché respeta mtime (reescribir archivo y obtener nuevo resultado); ruta fuera del workspace se omite.
- Rust `roles.rs` con `tempfile::tempdir()`: árbol Maven `pom.xml`, `src/main/java/com/example/finance_tracker/{App.java,web/}`, `src/test/java/...`, `src/main/resources/application.yml`, `target/` → roles correctos; compactación `com.example.finance_tracker` y que se detiene ante una carpeta con archivo o con 2 hijos.
- vitest: `fileIconRegistry.test.ts` (pom.xml, Dockerfile, application.yml, `.env.local`, `x.test.ts`, java con/sin símbolo); `classifyFolder.test.ts` actualizado con `role`.

### 1.9 Criterios de aceptación A
- Proyecto Spring abierto: `src/main/java` azul, `src/test/java` verde, `resources` con su icono, `target` atenuado, paquetes compactados `com.example.finance_tracker` con icono de paquete; clases con C/I/E/R/@, Application con hoja+play, controllers/services/repos/entities con overlay Spring y tooltip.
- Expandir una carpeta con 1 000 archivos Java no bloquea la UI (> 0 lecturas solo para visibles; segunda expansión = 0 lecturas, verificable con un contador en tests de caché).
- Git badges, menú contextual, notify, Quick Open y pestañas siguen funcionando.

---

## 2. BLOQUE B — Proyectos, New Project y Spring Microservices

### 2.1 Archivos
Crear (Rust, `src-tauri/src/projects/`):
- `mod.rs` (tipos), `commands.rs`, `recent.rs`, `tools.rs` (detección de herramientas y JDKs), `validate.rs`, `template.rs` (motor `{{var}}` mínimo, sin deps), `local.rs` (generadores con plantillas embebidas), `cli.rs` (generadores por CLI con streaming), `http.rs` (descarga + unzip seguro), `spring/mod.rs`, `spring/catalog.rs` (dependencias, versiones fallback, mapeo a coordenadas Maven/Gradle y a ids de start.spring.io), `spring/generate.rs` (multi-módulo, yml, capas, infra, docker), `spring/offline.rs` (plantillas locales).
- `src-tauri/templates/**` (plantillas embebidas con `include_str!`; listado en `local.rs` como `&[(&str /*ruta relativa con {{vars}}*/, &str /*contenido*/)]`): `java-maven/`, `java-gradle/`, `java-plain/` (IntelliJ build system: `src/Main.java` + `.gitignore`), `kotlin-gradle/`, `kotlin-maven/`, `groovy-gradle/`, `javafx-maven/`, `jakarta-maven/` (war, `jakarta.jakartaee-api` provided), `ktor-gradle/` (offline siempre), `html/`, `express/`, `node/`, `python/` (pyproject + `src/<pkg>/__init__.py` + `main.py`), `go/`, `rust/`, `empty/`, `spring-service-maven/`, `spring-service-gradle/`, `spring-parent/`, `spring-gateway/`, `spring-eureka/`, `spring-config/`, `common/{gitignore-*,README.md,Dockerfile-spring}`.
Crear (frontend):
- `src/store/projects.ts`; `src/components/projects/ProjectsScreen.tsx` (sustituye a `WelcomeView` cuando no hay workspace y accesible desde el header: botón/entrada “Projects”), `RecentList.tsx`, `NewProjectWizard.tsx` (Modal grande: panel izquierdo generadores agrupados — *Java/JVM*: Java, Kotlin, Groovy, Empty Project, Maven Archetype, JavaFX, Jakarta EE; *Frameworks*: Spring Boot, Quarkus, Micronaut, Ktor; *Web*: HTML, React, Vue.js, Vite, Angular CLI, Express, Node; *Otros*: Python, Go, Rust), `forms/CommonFields.tsx` (Name, Location con diálogo de carpeta, Create Git repository, vista previa de ruta final), `forms/JvmFields.tsx` (Build system IntelliJ/Maven/Gradle(Kotlin/Groovy DSL), JDK detectado, Java version, GroupId, ArtifactId, Add sample code), `forms/WebFields.tsx` (template/variant TS/JS), `forms/SpringMicroservicesForm.tsx`, `GenerationLog.tsx` (salida en streaming, estado, botón Cancelar), `ToolWarning.tsx` (herramienta ausente: qué falta y cómo instalarla, botón de “usar plantilla local” cuando exista fallback).
Tocar: `state.rs` (`project_jobs: Mutex<HashMap<String, CancellationHandle>>`), `workspace/commands.rs` (`workspace_open` recibe `app: AppHandle` y llama `projects::recent::touch`), `lib.rs`, `build.rs`, `capabilities/default.json`, `types.ts`, `ipc.ts`, `App.tsx` (pantalla Projects sin workspace; mantener `cc.lastWorkspace` de localStorage como hoy, pero la fuente de recientes es Rust), `AppHeader.tsx` (entrada Projects / New Project), `Cargo.toml`.

### 2.2 Contratos
```rust
#[derive(Serialize, Deserialize, Clone, Debug)] #[serde(rename_all = "camelCase")]
pub struct RecentProject { pub path: String, pub name: String, pub last_opened: u64 /*ms*/, pub kind: Option<String> /*module de tree.rs*/, pub pinned: bool, pub exists: bool /*calculado al listar*/ }

#[derive(Serialize, Clone, Debug)] #[serde(rename_all = "camelCase")]
pub struct ToolInfo { pub id: String /*java|javac|mvn|gradle|node|npm|npx|ng|cargo|go|python3|git*/, pub path: Option<String>, pub version: Option<String> }
#[derive(Serialize, Clone, Debug)] #[serde(rename_all = "camelCase")]
pub struct JdkInfo { pub home: String, pub version: String /*"21.0.4"*/, pub major: u32, pub vendor: Option<String> }
#[derive(Serialize, Clone, Debug)] #[serde(rename_all = "camelCase")]
pub struct ToolsReport { pub tools: Vec<ToolInfo>, pub jdks: Vec<JdkInfo>, pub online: Option<bool> /*None = no comprobado*/ }

#[derive(Serialize, Clone, Debug)] #[serde(rename_all = "camelCase")]
pub struct GeneratorInfo { pub id: String, pub label: String, pub group: String, pub strategy: Strategy /*local|cli|http|httpWithFallback*/, pub requires: Vec<String> /*tool ids*/, pub available: bool, pub missing: Vec<String>, pub offline_fallback: bool }

#[derive(Deserialize, Clone, Debug)] #[serde(rename_all = "camelCase")]
pub struct CommonOpts { pub name: String, pub location: String /*carpeta padre existente*/, pub git: bool }

#[derive(Deserialize, Clone, Debug)] #[serde(tag = "generator", rename_all = "camelCase")]
pub enum ProjectRequest {
    Java { common: CommonOpts, jvm: JvmOpts }, Kotlin { common: CommonOpts, jvm: JvmOpts }, Groovy { common: CommonOpts, jvm: JvmOpts },
    Empty { common: CommonOpts },
    MavenArchetype { common: CommonOpts, jvm: JvmOpts, archetype: ArchetypeRef /*group, artifact, version*/ },
    JavaFx { common: CommonOpts, jvm: JvmOpts }, JakartaEe { common: CommonOpts, jvm: JvmOpts },
    Quarkus { common: CommonOpts, jvm: JvmOpts, extensions: Vec<String> },
    Micronaut { common: CommonOpts, jvm: JvmOpts, features: Vec<String> },
    Ktor { common: CommonOpts, jvm: JvmOpts },
    SpringMicroservices { common: CommonOpts, spring: SpringRequest },
    Html { common: CommonOpts }, Express { common: CommonOpts, typescript: bool }, Node { common: CommonOpts, typescript: bool },
    React { common: CommonOpts, typescript: bool }, Vue { common: CommonOpts, typescript: bool, router: bool, pinia: bool },
    Vite { common: CommonOpts, template: String /*whitelist: vanilla, vanilla-ts, react, react-ts, vue, vue-ts, svelte, svelte-ts, preact, preact-ts, lit, lit-ts, solid, solid-ts*/ },
    Angular { common: CommonOpts, routing: bool, style: String /*css|scss|sass|less*/ },
    Python { common: CommonOpts, venv: bool }, Go { common: CommonOpts, module: String }, Rust { common: CommonOpts, bin: bool },
}
#[derive(Deserialize, Clone, Debug)] #[serde(rename_all = "camelCase")]
pub struct JvmOpts { pub build: BuildSystem /*intellij|maven|gradleKotlin|gradleGroovy*/, pub java_version: u32, pub jdk_home: Option<String>, pub group_id: String, pub artifact_id: String, pub package_name: String, pub sample_code: bool }

#[derive(Deserialize, Clone, Debug)] #[serde(rename_all = "camelCase")]
pub struct SpringRequest {
    pub group_id: String, pub base_package: String /*com.example.shop*/, pub java_version: u32, pub boot_version: Option<String> /*None = recomendada*/,
    pub build: BuildSystem /*maven|gradleKotlin|gradleGroovy*/, pub services: Vec<MicroserviceSpec>,
    pub gateway: Option<InfraSpec>, pub discovery: Option<InfraSpec>, pub config_server: Option<InfraSpec>,
    pub docker_compose: bool, pub dockerfiles: bool, pub prefer_online: bool,
}
#[derive(Deserialize, Clone, Debug)] #[serde(rename_all = "camelCase")]
pub struct MicroserviceSpec { pub name: String /*kebab: orders-service*/, pub port: u16, pub dependencies: Vec<String> /*ids de start.spring.io*/, pub layers: bool }
#[derive(Deserialize, Clone, Debug)] #[serde(rename_all = "camelCase")]
pub struct InfraSpec { pub port: u16 }

#[derive(Serialize, Clone, Debug)] #[serde(tag = "type", rename_all = "camelCase")]
pub enum GenEvent { Step { label: String }, Output { line: String, stream: String /*stdout|stderr*/ }, Warning { message: String }, Done { root: String, run_configs: usize }, Failed { message: String } }

#[derive(Serialize, Clone, Debug)] #[serde(rename_all = "camelCase")]
pub struct SpringCatalog { pub boot_versions: Vec<String>, pub default_boot: String, pub java_versions: Vec<u32>, pub dependencies: Vec<SpringDepGroup>, pub source: String /*"online"|"offline"*/ }
#[derive(Serialize, Clone, Debug)] #[serde(rename_all = "camelCase")]
pub struct SpringDepGroup { pub name: String, pub items: Vec<SpringDep> }
#[derive(Serialize, Clone, Debug)] #[serde(rename_all = "camelCase")]
pub struct SpringDep { pub id: String, pub name: String, pub description: String, pub offline: bool /*soportada por la plantilla local*/ }
```
Comandos nuevos (TRES sitios):

| Comando | Firma | Notas |
|---|---|---|
| `projects_recent_list` | `() -> Vec<RecentProject>` | ordena pinned, luego `last_opened` desc; marca `exists` |
| `projects_recent_remove` | `(path) -> ()` | solo de la lista, nunca del disco |
| `projects_recent_pin` | `(path, pinned) -> ()` | |
| `projects_tools_detect` | `(check_online: bool) -> ToolsReport` | `which` + `--version` con timeout 3 s en paralelo; online = HEAD a start.spring.io timeout 3 s |
| `project_generators` | `() -> Vec<GeneratorInfo>` | usa la última `ToolsReport` cacheada |
| `spring_catalog` | `() -> SpringCatalog` | `GET https://start.spring.io/metadata/client` con `Accept: application/vnd.initializr.v2.2+json`, timeout 6 s; fallback `catalog.rs` |
| `project_create` | `(request: ProjectRequest, job_id: String, on_event: Channel<GenEvent>) -> String /*root*/` | valida todo antes de tocar disco |
| `project_cancel` | `(job_id) -> ()` | mata el proceso hijo / aborta la descarga; borra la carpeta destino solo si la creó este job |

`workspace_open` añade `projects::recent::touch(&app_data_dir, &root)` (fallo de escritura de recientes = log, nunca error del comando).

### 2.3 Estrategia por generador
| Generador | Estrategia | Detalle |
|---|---|---|
| Java / Kotlin / Groovy | local | `intellij` (solo Java): `src/Main.java`; Maven: `pom.xml` con `maven.compiler.release`; Gradle: wrapper NO se genera (sin binarios); `settings.gradle(.kts)` + `build.gradle(.kts)`; si `gradle` está en PATH, ejecutar `gradle wrapper` opcional con streaming (aviso si falla). Sample code según paquete. |
| Empty | local | carpeta + `.gitignore` + README |
| Maven Archetype | cli (`mvn`) | `mvn -B archetype:generate -DarchetypeGroupId=.. -DarchetypeArtifactId=.. -DarchetypeVersion=.. -DgroupId=.. -DartifactId=.. -Dpackage=.. -DinteractiveMode=false` en `location`; lista corta whitelist (quickstart, webapp, simple) + campo libre validado por regex de coordenadas |
| JavaFX / Jakarta EE / Ktor | local | Maven (JavaFX con `javafx-maven-plugin`; Jakarta war) / Gradle KTS (Ktor) |
| Spring Boot | httpWithFallback | ver §2.4 |
| Quarkus | httpWithFallback | `GET https://code.quarkus.io/d?g=..&a=..&j=<java>&b=MAVEN|GRADLE|GRADLE_KOTLIN_DSL&e=<ext>...` → zip; offline: plantilla Maven mínima con BOM fijo en `catalog.rs` + aviso |
| Micronaut | httpWithFallback | `GET https://launch.micronaut.io/create/default/<group>.<artifact>?lang=JAVA&build=MAVEN|GRADLE|GRADLE_KOTLIN&jdk=<n>&features=a&features=b` → zip; offline igual que Quarkus |
| HTML / Express / Node / Python / Go / Rust | local, CLI si existe | Rust: `cargo new --vcs none [--bin|--lib] <name>` si hay cargo, si no plantilla; Go: plantilla + `go mod init <module>` si hay go (si no, `go.mod` escrito con `go 1.22`); Python: plantilla + `python3 -m venv .venv` si `venv` y python3 |
| React / Vite | cli (`npm`) | `npm create vite@latest <name> -- --template <react-ts|react|...>` con env `npm_config_yes=true`, `CI=1`; sin npm → aviso |
| Vue | cli (`npm`) | `npm create vue@latest <name> -- --default` + flags `--ts --router --pinia` según opciones (verificar con `--help` en el test manual; si la CLI pide input, stdin null hace que falle → mensaje claro) |
| Angular | cli (`ng` o `npx`) | `ng new <name> --defaults --skip-git --routing=<bool> --style=<s> --skip-install=false` o `npx -y @angular/cli@latest new ...`; env `NG_CLI_ANALYTICS=false`, `CI=1` |

Reglas comunes de `project_create`:
1. Validar (§2.5). `target = location/name`; debe no existir o estar vacío. Crear `target` (salvo CLIs que crean la carpeta ellas: ejecutar en `location` y comprobar que no existía).
2. Generar (streaming `GenEvent::Output` línea a línea; stdout y stderr con `BufReader::lines` en dos tareas; timeout total 10 min; cancelación vía `project_jobs`).
3. `.gitignore` adecuado si no lo trajo la plantilla; si `git` → `git init -b main` (vía `git::run_git` adaptado a ruta arbitraria: añadir `run_git_in(cwd, args)` sin `AppState`, reutilizando la misma configuración de `Command`).
4. Auto-run: escribir `.caesar/run-configs.json` con `run::store::save(root, &RunConfigFile{..})` (Spring: una `RunConfig { kind: SpringBoot, cwd: "<svc>", command: "./mvnw spring-boot:run" o "mvn spring-boot:run" si no hay wrapper / "gradle bootRun", port }` por servicio e infra; Vite/Vue/React/Angular: `npm run dev`/`npm start`; Python/Go/Rust/Node equivalentes). Ids slug sin `:` (respetar validación de `run/`).
5. `Done { root }`. El frontend llama `openWorkspace(root)` (que añade a recientes) y abre el README.

### 2.4 Inicializador Spring Microservices
- Formulario: GroupId, base package, Java version (17/21/25 según catálogo y JDKs detectados), Spring Boot version (catálogo), Maven/Gradle(KTS/Groovy), lista editable de N microservicios (nombre, puerto auto-incremental desde 8081 sin colisión, chips de dependencias con buscador y grupos; presets: Web, JPA, PostgreSQL, MySQL, Security, Validation, Lombok, Actuator, Kafka, RabbitMQ (`amqp`), Redis (`data-redis`), OpenFeign (`cloud-feign`), Eureka client (`cloud-eureka`), Config client (`cloud-config-client`), DevTools, WebFlux, MongoDB), checkbox “capas controller/service/repository/model”; infra opcional: API Gateway (`cloud-gateway`, puerto 8080), Eureka Server (`cloud-eureka-server`, 8761), Config Server (`cloud-config-server`, 8888); docker-compose, Dockerfile por servicio; “usar start.spring.io si hay red”.
- Online: por cada servicio/infra `GET https://start.spring.io/starter.zip?type=<maven-project|gradle-project|gradle-project-kotlin>&language=java&bootVersion=&groupId=&artifactId=<svc>&name=<svc>&packageName=<base>.<svc_sin_guiones>&javaVersion=&dependencies=<csv>&baseDir=<svc>` (URL construida con `reqwest::Url::query_pairs_mut`, nunca concatenando). Descomprimir en `target/` con zip-slip guard. Si cualquier descarga falla → **fallback offline para ese servicio** + `GenEvent::Warning`.
- Offline: plantillas `spring-service-*` con `catalog.rs` (mapa id → coordenadas, incluido Spring Cloud BOM y versión de release train compatible con la Boot por defecto del catálogo offline; una sola constante `FALLBACK_BOOT` y `FALLBACK_CLOUD` documentadas). Deps no soportadas offline → se omiten con aviso.
- Post-proceso común (online u offline): parent agregador (Maven `pom.xml` packaging `pom` con `<modules>`; Gradle `settings.gradle(.kts)` con `include(...)`), sustituir `application.properties` por `application.yml` con `server.port`, `spring.application.name`, datasource (Postgres/MySQL → url `jdbc:postgresql://localhost:5432/<svc_db>` usuario/clave de desarrollo que coinciden con docker-compose), `eureka.client.service-url.defaultZone` si hay discovery, `spring.config.import=optional:configserver:http://localhost:8888` si hay config server, rutas del gateway (`spring.cloud.gateway.routes` por servicio, `lb://<svc>` si hay Eureka o `http://localhost:<port>`); paquetes por capas con `<Svc>Controller` (`GET /api/<svc>/health`), `<Svc>Service`, `<Svc>Repository` (solo si JPA), `model/<Svc>Entity` (solo si JPA); clase main `<Svc>Application` (`@EnableDiscoveryClient`/`@EnableFeignClients`/`@EnableEurekaServer`/`@EnableConfigServer` según deps); `docker-compose.yml` con postgres/mysql/redis/kafka(KRaft)/rabbitmq según deps; `Dockerfile` multi-stage (eclipse-temurin:<java>) por servicio; `.gitignore`, `README.md` con tabla de servicios y puertos.
- Run configs auto: una por servicio + infra (orden sugerido en README: config → eureka → servicios → gateway).

### 2.5 Validación (`validate.rs`, autoridad = Rust; el frontend replica para feedback inmediato)
- Nombre de proyecto / microservicio: `^[A-Za-z][A-Za-z0-9._-]{0,63}$`, sin `..`, no nombres reservados Windows (`CON PRN AUX NUL COM1-9 LPT1-9`), no termina en `.` o espacio. Microservicio además kebab `^[a-z][a-z0-9-]{0,40}$`.
- Paquete Java: `^[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)*$`, ningún segmento palabra reservada Java (lista completa en `validate.rs`). `-` en nombres → `_` al derivar paquete (como start.spring.io).
- GroupId igual que paquete; ArtifactId `^[a-z][a-z0-9-]*$`; versión Boot `^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?$`; plantilla Vite/estilos Angular por whitelist; Go module `^[A-Za-z0-9.\-_/~]+$` sin `..`.
- Location: existe, es carpeta, canonicalizada; target = `location.join(name)` y `target.parent() == location` (anti traversal). Puertos 1024–65535, únicos.
- Zip: usar `ZipFile::enclosed_name()`; rechazar entradas absolutas, con `..` o symlinks; tope 200 MB descomprimidos y 20 000 entradas.
- Nunca interpolar input en una línea de shell. Argumentos ya validados van como `arg()` separados.

### 2.6 Trampas B
- `npm create` / `ng new` pueden pedir input → `stdin(null)`, `CI=1`, `npm_config_yes=true`; detectar “prompt” colgado por inactividad 120 s → cancelar con mensaje.
- Windows: `npm`/`ng` son `.cmd`; resolver con `which` y lanzar directo (Rust escapa args de batch desde 1.77.2; los args están validados). PATH correcto ya lo arregla `env_path::fix_path`.
- Sin red: `spring_catalog` y descargas con timeout corto; jamás bloquear el formulario esperando la red (cargar catálogo offline primero y reemplazar si llega el online).
- No generar binarios de wrapper (`mvnw`/`gradlew` jar) desde plantillas; online sí vienen en el zip (respetar permisos de ejecución `unix_mode` al extraer).
- Cancelación: matar el árbol del proceso hijo (reusar `services/process_tree.rs` si el CLI lanza nietos, p. ej. npm → node).
- No reescribir `.caesar/services.json`; solo `run-configs.json`.
- Recientes: la ruta puede haber desaparecido → `exists=false`, se muestra atenuada con acción “Quitar”.

### 2.7 Tests B (headless, `tempfile::tempdir()`)
- `recent.rs`: touch/list/remove/pin, orden, tope 50, escritura atómica, JSON corrupto → lista vacía (y backup `.bak`).
- `validate.rs`: casos válidos/ inválidos (paquete con `class`, `com..x`, `-x`, `../evil`, `CON`, puertos duplicados).
- `template.rs`: sustitución, variable desconocida → error, sin `{{` residual en ninguna plantilla (test que recorre todas las embebidas con vars de ejemplo).
- `local.rs`: cada generador local en tempdir crea los archivos esperados (pom/gradle con groupId/artifactId/paquete correctos, ruta de paquete `com/example/x/Main.java`).
- `spring/offline.rs`: 2 servicios (orders 8081 web+jpa+postgresql+lombok, users 8082 web+security) + gateway + eureka + compose + dockerfiles, Maven y Gradle KTS: parent con `<modules>`/`include`, `application.yml` con puerto y nombre, capas, main por servicio, compose con postgres, run-configs con 4 entradas. No requiere red ni Maven.
- `http.rs`: zip fabricado en test con entrada `../evil` y absoluta → rechazado; zip válido extraído.
- `tools.rs`: `projects_tools_detect` no panic sin herramientas (PATH vacío en el test).
- Red real (start.spring.io, quarkus, micronaut) y CLIs (npm/ng/cargo) → `#[ignore]`.

### 2.8 Criterios de aceptación B
- Sin workspace se ve la pantalla Projects: recientes (buscar, abrir, fijar, quitar), “Open…”, “New Project”. Abrir cualquier carpeta la añade a recientes y persiste tras reiniciar.
- New Project: cada generador muestra su formulario; si falta una herramienta, aviso claro con lo que falta y (si existe) opción de plantilla local; la salida se ve en streaming y se puede cancelar.
- Spring Microservices offline (Wi-Fi apagado) genera proyecto multi-módulo que compila con `mvn -q -DskipTests package` (prueba manual si hay Maven) y abre el workspace con run configs listas en el widget Run.

---

## 3. BLOQUE C1 — Backend Git

### 3.1 Archivos
- `src-tauri/src/git/mod.rs` (mantener `run_git`, `GitStatus`, `parse_porcelain`: **el árbol y `git_status` actuales no cambian**), añadir:
  - `exec.rs`: `GitCmd` builder → `run_git_raw(state, root, args, opts) -> Vec<u8>` (stdout bytes), opción `stdin: Option<Vec<u8>>` (commit `-F -`, `apply`), `env` extra, `timeout`, y `run_git_streaming(…, on_line)` que lee stderr en trozos partiendo por `\r` y `\n` (progreso de fetch/pull/push con `--progress`). Todas reutilizan la configuración de `run_git` (extraer `fn base_command(root) -> Command`).
  - `status2.rs`, `log.rs`, `refs.rs`, `diff.rs` (parser unified → hunks; generador de parche por hunk), `ops.rs`, `stash.rs`, `remote.rs`, `blame.rs`, `validate.rs` (refnames, shas, rutas).
  - `commands.rs`: comandos nuevos.
- `state.rs`: `git_op: tokio::sync::Mutex<()>` (serializa operaciones mutantes), `git_busy: AtomicU32`, `git_jobs: Mutex<HashMap<String, oneshot::Sender<()>>>` (cancelación de remotos).
- `fs/watcher.rs`: si `git_busy > 0` no emitir `GitChanged` (además de la ventana `GIT_QUIET_MS` existente).
- `build.rs`, `lib.rs`, `capabilities/default.json`.

### 3.2 Contratos (Rust; TS espejo en `types.ts` con los mismos nombres camelCase)
```rust
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct RepoInfo { pub is_repo: bool, pub top: Option<String>, pub head: Option<String> /*sha*/, pub branch: Option<String>, pub detached: bool, pub upstream: Option<String>, pub ahead: u32, pub behind: u32, pub state: RepoState, pub git_version: Option<String> }
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub enum RepoState { Clean, Merging, Rebasing, CherryPicking, Reverting, Bisecting }   // por .git/MERGE_HEAD, rebase-merge|rebase-apply, CHERRY_PICK_HEAD, REVERT_HEAD, BISECT_LOG (resolver gitdir con `rev-parse --git-dir`)

#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct StatusEntry { pub path: String /*relativa al top, '/'*/, pub orig_path: Option<String>, pub index: char, pub worktree: char, pub kind: EntryKind /*ordinary|renamed|copied|unmerged|untracked|ignored*/, pub conflict: Option<String> /*"UU","AA","DU"...*/ }
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct StatusV2 { pub info: RepoInfo, pub staged: Vec<StatusEntry>, pub unstaged: Vec<StatusEntry>, pub untracked: Vec<StatusEntry>, pub conflicted: Vec<StatusEntry> }

#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct CommitRow { pub sha: String, pub parents: Vec<String>, pub author: String, pub email: String, pub time: i64, pub subject: String, pub refs: Vec<RefLabel> }
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct RefLabel { pub name: String, pub kind: RefKind /*head|local|remote|tag|stash*/ }
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct LogPage { pub commits: Vec<CommitRow>, pub has_more: bool }
#[derive(Deserialize)] #[serde(rename_all = "camelCase")]
pub struct LogQuery { pub skip: u32, pub limit: u32 /*≤ 2000*/, pub all: bool, pub path: Option<String>, pub search: Option<String> /*--grep, -F, -i*/, pub author: Option<String> }

#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct Refs { pub local: Vec<BranchRef>, pub remote: Vec<BranchRef>, pub tags: Vec<TagRef>, pub stashes: Vec<StashRef>, pub remotes: Vec<RemoteInfo> }
pub struct BranchRef { name, full_name, sha, upstream: Option<String>, ahead: u32, behind: u32, is_head: bool }   // for-each-ref --format con %00
pub struct TagRef { name, sha /*peeled*/, annotated: bool, message: Option<String> }
pub struct StashRef { index: u32, sha, message, time }
pub struct RemoteInfo { name, fetch_url /*sin credenciales: quitar user:pass@*/ }

#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct CommitDetail { pub row: CommitRow, pub body: String, pub committer: String, pub commit_time: i64, pub files: Vec<ChangedFile> }
pub struct ChangedFile { path, orig_path: Option<String>, status: char /*A M D R C T*/, additions: Option<u32>, deletions: Option<u32> /*None = binario*/ }

#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct FileDiff { pub path: String, pub old_path: Option<String>, pub binary: bool, pub hunks: Vec<Hunk>, pub header: String /*diff --git… hasta el primer @@, para reconstruir parches*/, pub too_large: bool }
pub struct Hunk { header: String /*@@ -a,b +c,d @@ ctx*/, old_start: u32, old_lines: u32, new_start: u32, new_lines: u32, lines: Vec<DiffLine> }
pub struct DiffLine { kind: char /*' ' '+' '-' '\\'*/, text: String, old_no: Option<u32>, new_no: Option<u32> }
#[derive(Deserialize)] #[serde(tag = "type", rename_all = "camelCase")]
pub enum DiffTarget { Worktree { path }, Index { path }, Commit { sha, path }, Range { from, to, path } }

pub struct BlameLine { line: u32, sha, author, time: i64, summary }   // blame --porcelain, agrupado por sha (enviar metadatos una vez)
pub struct BlameResult { commits: HashMap<String, BlameCommit>, lines: Vec<(u32 /*line*/, String /*sha*/)> }

#[derive(Serialize, Clone)] #[serde(tag = "type", rename_all = "camelCase")]
pub enum GitProgress { Line { text: String }, Percent { phase: String, pct: u8 }, Done, Failed { message: String } }
```
Comandos nuevos (TRES sitios; permiso `allow-git-…`):

| Comando | Args | Git |
|---|---|---|
| `git_repo_info` | — | `rev-parse --show-toplevel --git-dir`, `status --porcelain=v2 --branch -z` (cabeceras), `--version` (cacheado) |
| `git_status_v2` | — | `status --porcelain=v2 --branch -z --untracked-files=all` |
| `git_log` | `query: LogQuery` | `log --topo-order --format=%H%x00%P%x00%an%x00%ae%x00%at%x00%D%x00%s%x1e --skip --max-count=limit+1 [--all] [-- path]`; `--all` excluye `refs/stash` (`--exclude=refs/stash --all`) |
| `git_refs` | — | `for-each-ref --format=… refs/heads refs/remotes refs/tags`, `stash list --format=%gd%x00%H%x00%gs%x00%ct`, `remote -v` |
| `git_commit_detail` | `sha` | `show -s --format=…%B` + `show --format= --numstat -z -M` + `--name-status -z -M` (merge: contra primer padre `-m --first-parent`) |
| `git_diff` | `target: DiffTarget, context: u32` | `diff --no-color --no-ext-diff -M -U<n>` (`--cached` / `<sha>^!`); untracked: `diff --no-index -- /dev/null <path>` (NUL en Windows); tope 2 MB → `too_large` |
| `git_file_at` | `rev: String /*sha, "INDEX", "HEAD"*/, path` | `show <rev>:<path>` / `show :<path>`; tope 5 MB; binario → error tipado |
| `git_stage` / `git_unstage` | `paths: Vec<String>` | `add -A -- <paths>` / `restore --staged -- <paths>` (repo sin commits: `rm --cached -r --`) |
| `git_discard` | `paths, untracked: bool` | `restore --worktree --source=HEAD? -- ` / untracked → mover a la papelera con crate `trash` (ya dependencia), no `clean -f` |
| `git_apply_patch` | `patch: String, cached: bool, reverse: bool` | `apply [--cached] [-R] --recount --whitespace=nowarn -` por stdin (stage/unstage/discard por hunk) |
| `git_commit` | `message, amend: bool, sign_off: bool` | `commit -F - [--amend] [-s]` (mensaje por stdin; vacío → error salvo amend) |
| `git_branch` | `action: BranchAction` tag `create{name, start?, checkout}` `delete{name, force}` `rename{from, to}` `checkout{name}` `checkoutRemote{remote_branch}` `setUpstream{name, upstream}` | `switch`, `switch -c`, `branch -d/-D/-m`, `switch --track` |
| `git_checkout_commit` | `sha` | `switch --detach <sha>` |
| `git_merge` | `name, no_ff: bool, squash: bool` | `merge --no-edit [--no-ff|--squash]`; conflicto → `Ok(OpOutcome::Conflicts)` no error |
| `git_rebase` | `onto` | `rebase <onto>` con `GIT_EDITOR=true`, `GIT_SEQUENCE_EDITOR=true` |
| `git_cherry_pick` | `shas: Vec<String>` | `cherry-pick -x` |
| `git_revert` | `sha` | `revert --no-edit` (merge: `-m 1`) |
| `git_reset` | `sha, mode: soft|mixed|hard` | `reset --<mode> <sha>` (confirmación en UI; Rust exige `confirm: true` para hard) |
| `git_sequence` | `op: merge|rebase|cherryPick|revert, action: abort|continue|skip` | `<op> --abort|--continue|--skip`, `GIT_EDITOR=true` |
| `git_conflict` | `path, action: ours|theirs|markResolved` | `checkout --ours/--theirs -- p` + `add -- p` |
| `git_stash` | tag `push{message?, include_untracked, keep_index}` `pop{index}` `apply{index}` `drop{index}` | `stash push -m … [-u]`, `stash pop stash@{n}` (n validado u32) |
| `git_tag` | tag `create{name, sha, message?}` `delete{name}` `push{remote, name}` | `tag [-a -m]`, `tag -d`, `push <remote> refs/tags/<name>` |
| `git_remote` | `op: fetch{remote?, prune}|pull{rebase: bool}|push{remote?, branch?, force_with_lease: bool, set_upstream: bool, tags: bool}`, `job_id`, `on_progress: Channel<GitProgress>` | `--progress`, `-c http.lowSpeedLimit=1000 -c http.lowSpeedTime=30`, `GIT_SSH_COMMAND="ssh -o BatchMode=yes -o ConnectTimeout=15"` si el usuario no lo tiene; timeout 10 min; pull por defecto `--ff-only` salvo `rebase` |
| `git_cancel` | `job_id` | mata el proceso remoto |
| `git_blame` | `path, rev?` | `blame --porcelain [rev] -- path` |
| `git_file_history` | `path, skip, limit` | `log --follow --format=… -- path` |
| `git_init` | `path` | `init -b main` (para workspace sin repo) |

Resultado de ops mutantes: `OpOutcome { ok: bool, conflicts: Vec<String>, message: String /*stdout+stderr recortados*/ }`.

### 3.3 Reglas de implementación
- Parseo siempre por bytes con `-z` / `%x00` / `%x1e`; nunca por espacios. Rutas porcelain v2 tipo `1`, `2` (rename con `\0` orig), `u`, `?`, `!`.
- `--no-optional-locks` solo en lecturas (ya lo pone `run_git`; mantener). Mutaciones: crear `run_git_mut` sin esa opción **no es necesario** (es global e inocua) pero sí: tomar `git_op` lock, `git_busy += 1` con guard RAII que decrementa y marca `git_last_run` al soltar.
- Validación (`git/validate.rs`): refname vía regex conservadora `^(?!-)(?!.*\.\.)(?!.*@\{)(?!.*//)[A-Za-z0-9._/\-]+(?<!\.lock)(?<!/)(?<!\.)$` (implementar sin lookaround: comprobaciones manuales) y además `git check-ref-format --branch <name>` antes de crear/renombrar; sha `^[0-9a-fA-F]{4,64}$`; rev especiales permitidos: `HEAD`, `HEAD~n`, `INDEX`; remoto: debe existir en `remote` list; rutas: `ensure_in_workspace` y convertir a relativa al top con `/`. Todo arg de usuario que no sea ruta va **antes** de `--`; las rutas siempre **después** de `--`. Cualquier valor que empiece por `-` se rechaza.
- Mensajes de commit/tag por stdin (`-F -`) para evitar límites de longitud y escapes.
- Credenciales: nunca registrar URLs con `user:pass@`; `GIT_TERMINAL_PROMPT=0` ya está → fallo de auth da error claro (“configura un credential helper o SSH”).
- Sin red: fetch/pull/push fallan rápido (ConnectTimeout) con `GitProgress::Failed`; el resto funciona offline.
- Repos grandes: `git_log` paginado (página 500, tope 2000); `git_status_v2` con `--untracked-files=all` puede ser lento en repos gigantes → si tarda > 3 s, la siguiente llamada usa `--untracked-files=normal` (flag en estado) y se informa en `RepoInfo`.

### 3.4 Tests C1 (`cargo test`, `tempfile::tempdir()`; helper `TestRepo::new()` que hace `git init -b main`, `config user.name/email`, `commit.gpgsign=false`; si `git` no está en PATH el test hace `return` con eprintln)
- `status2`: fixture de bytes porcelain v2 (ordinario, rename con espacios y unicode `ñ`, unmerged `UU`, untracked, submódulo) + repo real con cambios staged/unstaged/untracked.
- `log`: parse fixture; repo real con rama y merge → parents correctos; paginación `has_more`.
- `refs`: ramas, upstream ahead/behind con remoto **bare local** (`git init --bare` en tempdir, `remote add origin <ruta>`), tags anotado/ligero, stash list.
- `diff` + `apply`: archivo con 2 hunks → stage solo el segundo vía `git_apply_patch(cached)`; `diff --cached` contiene solo ese hunk; unstage `-R`; discard hunk de worktree.
- ops: commit por stdin con mensaje multilínea y comillas; amend; branch create/rename/delete (no fusionada sin force → error; con force ok); merge con conflicto → `OpOutcome.conflicts` y `RepoState::Merging`; `git_conflict ours` + `markResolved` + `sequence continue`; abort; rebase simple; cherry-pick; revert; reset soft/mixed/hard (`hard` sin `confirm` → error); stash push -u/pop/apply/drop.
- remote: fetch/pull/push contra bare local con progreso recibido por un `Channel` de test (usar un sink abstracto `FnMut(GitProgress)` para testear sin Tauri).
- validate: rechaza `-x`, `a..b`, `x@{1}`, `foo.lock`, `a//b`, `/a`, `a/`, `HEAD:evil`, `--upload-pack=…`; ruta fuera del workspace.
- blame porcelain fixture; file history con rename (`--follow`).
- Watcher: test unitario del guard `git_busy` (incrementa/decrementa, también en panic/err).

### 3.5 Criterios de aceptación C1
`cargo test` verde con git real; ninguna invocación usa shell; comandos registrados en los tres sitios; `git_status` original y badges del árbol intactos.

---

## 4. BLOQUE C2 — Frontend Git estilo GitKraken

### 4.1 Archivos
- `src/lib/gitGraph.ts` (+ `gitGraph.test.ts`): `layoutGraph(state: GraphState | null, commits: CommitRow[]) -> { state, rows: GraphRow[] }`. `GraphRow = { sha, lane: number, color: number, edges: Edge[] /*fromLane,toLane,color,type: 'pass'|'merge'|'branch'*/, maxLane }`. Algoritmo: array `activeLanes: (sha|null)[]`; el commit ocupa la lane que espera su sha (o la primera libre); primer padre hereda la lane; padres extra ocupan lane existente o nueva; lanes libres se reutilizan (compactación). Color = índice de lane estable mod 8 (paleta acorde al tema: ámbar primero). Incremental entre páginas.
- `src/lib/gitDiff.ts` (+ test): construir parche de un hunk a partir de `FileDiff.header + hunk` (para `gitApplyPatch`), y de un subconjunto de líneas (opcional, solo si viable).
- `src/store/git.ts`: `info, status, refs, log: CommitRow[], graph: GraphRow[], graphState, hasMore, selected: {type:'wip'}|{type:'commit', sha}|null, detail, diff: {target, data, mode: 'unified'|'split'}, busy: string|null, progress: GitProgress[], error`. Acciones mapeadas 1:1 a los comandos; tras cada mutación → `refreshAll()` (status+refs+info; log solo si cambian refs/HEAD: recargar desde 0 conservando scroll al sha seleccionado). Suscribirse al `gitChanged` existente (desde `workspace.ts` llamar también a `useGit.getState().refreshSoft()` debounced 300 ms) solo si la vista Git está montada o el panel de cambios visible.
- `src/components/git/GitView.tsx` (layout: `react-resizable-panels` — izquierda `RefsSidebar`, centro `CommitGraph` con fila WIP arriba, derecha `CommitPanel`/`ChangesPanel`), `RefsSidebar.tsx` (LOCAL/REMOTE/TAGS/STASHES colapsables, filtro, doble clic = checkout, menú contextual), `CommitGraph.tsx` (`@tanstack/react-virtual`, fila 26 px; columna grafo = SVG por fila con `edges` + nodo circular (merge = anillo); chips de refs con color de lane; columnas autor (iniciales en círculo), mensaje, fecha relativa, sha corto; carga de más páginas al llegar al final; búsqueda), `ChangesPanel.tsx` (Unstaged/Staged/Conflicts con stage/unstage/discard por archivo y “Stage all”; clic → diff), `CommitForm.tsx` (resumen + descripción, contador 72, Amend, botón Commit / Commit & Push), `CommitDetail.tsx` (meta, cuerpo, archivos con +/-), `DiffViewer.tsx` (unified propio con botones por hunk *Stage hunk / Unstage hunk / Discard hunk*; split con `@codemirror/merge` `MergeView` read-only usando `git_file_at` old/new y lenguaje de `editor/languages.ts`; tema de `editor/theme.ts`), `ConflictBanner.tsx` (estado Merging/Rebasing… con Continue/Abort/Skip y por archivo Ours/Theirs/Open/Mark resolved), `RemoteBar.tsx` (Fetch/Pull/Push con ahead/behind, progreso en streaming y Cancel), `CommitContextMenu.tsx`, `BranchDialogs.tsx` (crear/renombrar/tag con validación en vivo), `FileHistory.tsx`.
- `src/components/editor/blame.ts`: extensión CM6 de gutter con blame (toggle por comando/menú contextual de la pestaña: “Annotate with Git Blame”), datos de `git_blame`, cache por `(path, mtime)`.
- Tocar: `ui.ts` (`centerView: 'editor' | 'git'`), `AppHeader.tsx` (selector Code | Git + rama actual con ahead/behind; clic en rama abre la vista Git), `AppLayout.tsx` (renderizar `GitView` en lugar de `EditorArea` cuando `centerView === 'git'`, sin desmontar el editor: `hidden` para conservar estado CM), `StatusBar.tsx` (rama/ahead/behind ya existente → clic abre Git), `FileTree` menú contextual (“Git: Show History”, “Git: Annotate”, “Git: Stage/Discard”), `types.ts`, `ipc.ts`, `package.json` (`@codemirror/merge`).

### 4.2 Menú contextual de commit
Checkout (detached) · Create branch here · Create tag here · Cherry-pick · Revert · Reset current branch to here ▸ Soft / Mixed / Hard (Hard con `ConfirmHost` en rojo) · Merge into current (si es punta de rama) · Rebase current onto · Copy SHA · Copy message. Rama (sidebar / chip): Checkout · Merge into current · Rebase current onto · Rename · Delete (force si no fusionada, confirm) · Push · Set upstream · Pull (si es la actual). Stash: Apply · Pop · Drop (confirm). Tag: Push · Delete (confirm).

### 4.3 Trampas C2
- Miles de commits: virtualización + SVG mínimo por fila (sin un único SVG gigante); `layoutGraph` incremental O(n·lanes); memo de filas por sha; no recalcular el grafo en cada `status` refresh.
- Bucle git↔watcher: no llamar `refreshAll` desde `gitChanged` mientras `busy`; el backend ya silencia con `git_busy`.
- Diff de archivos grandes/binarios: respetar `tooLarge`/`binary` con mensaje, no renderizar.
- `MergeView` de `@codemirror/merge` con buffers grandes: limitar a 5 MB (backend ya topa).
- Confirmaciones obligatorias: discard, reset hard, delete branch force, drop stash, push force-with-lease.
- Los paths del backend son relativos al top del repo (que puede ser ancestro del workspace): convertir a absolutos con `info.top` para abrir en el editor.
- Estilo: tokens existentes (Graphite & Amber/imperial), sin `<select>` nativos (usar `ui/Select.tsx`), `lucide-react` para iconos de acción.

### 4.4 Tests C2 (vitest)
- `gitGraph.test.ts`: lineal (1 lane), rama + merge (2 lanes y edge de merge), dos ramas paralelas largas, octopus (3 padres), commit raíz múltiple (historias no relacionadas), continuidad entre páginas (layout de [0..k] + [k..n] == layout de [0..n]), 10 000 commits sintéticos < 200 ms.
- `gitDiff.test.ts`: parche de hunk reconstruido aplica el formato correcto (cabecera `diff --git`, `---/+++`, `@@`), archivo nuevo, “\ No newline at end of file”.

### 4.5 Criterios de aceptación C (C1+C2)
- Vista Git: grafo con colores y merges correcto en un repo real de > 5 000 commits con scroll fluido; ramas/tags/stashes en sidebar; WIP con staged/unstaged/untracked; stage/unstage por archivo y por hunk; diff unified y side-by-side para archivo y para commit; commit y amend; checkout/crear/borrar/renombrar rama; merge, rebase, cherry-pick, revert, reset con confirmación; stash push/pop/apply/drop; fetch/pull/push con progreso y ahead/behind; tags; blame en el editor e historial de archivo; conflictos resolubles (ours/theirs/marcar/continuar/abortar).
- Sin red: todo menos remotos funciona; remotos fallan con mensaje claro en < 20 s.
- Ni bucles de refresco (verificar con contador de llamadas `git_status_v2` en dev: ≤ 2 por operación) ni regresiones en badges del árbol.

---

## 5. Seguridad transversal (todos los bloques)
1. Rutas: `util/paths.rs::ensure_in_workspace` para todo lo que viene del frontend dentro del workspace; para `project_create` validar `location` y `target.parent()==location`.
2. Procesos: vector de args, sin shell, `--` antes de rutas en git, rechazo de valores que empiecen por `-`, whitelist donde haya enumeración (plantillas Vite, estilos Angular, modos reset, ops de secuencia).
3. `.cmd`/`.bat` en Windows solo con args validados por regex de §2.5.
4. Zip-slip y límites de tamaño en extracción.
5. Nunca loguear credenciales (URLs de remotos saneadas).
6. HTTP solo a hosts fijos en código (`start.spring.io`, `code.quarkus.io`, `launch.micronaut.io`), HTTPS, timeout, tamaño máx 50 MB.

---

## 6. BLOQUE D — Icono de macOS (diagnóstico y verificación)
Estado observado: `src-tauri/icons/icon.icns` regenerado hoy (contiene 16…512@2x); `scripts/make-icon.py` genera un rombo ámbar (`app-icon.png`) que puede no ser el diseño actual (`src/assets/brand.png`).
Diagnóstico/fix (en este orden):
1. Fuente única: PNG 1024×1024 del diseño nuevo (con el margen/squircle de macOS ≈ 10 % de padding). Regenerar todo con `npx tauri icon <fuente.png>` (sobrescribe `src-tauri/icons/*`, incluido `icon.icns` e `icon.ico`). Si `make-icon.py` ya no es la fuente, borrarlo o actualizarlo para que nadie regenere el rombo.
2. Comprobar el icns: `iconutil -c iconset src-tauri/icons/icon.icns -o /tmp/x.iconset` y abrir `icon_512x512@2x.png` → debe ser el diseño nuevo.
3. `tauri.conf.json > bundle.icon` incluye `icons/icon.icns` (ya está) y `productName`/`identifier` sin cambios (`CaesarCode`, `com.caesarcode.app`).
4. Build limpio: borrar `src-tauri/target/release/bundle` (y `target/universal-apple-darwin/release/bundle` si aplica) y `npm run tauri build`. Comprobar `CaesarCode.app/Contents/Info.plist` → `CFBundleIconFile = icon.icns` y que `Contents/Resources/icon.icns` tiene el mismo hash que `src-tauri/icons/icon.icns`.
5. Cachés: copias antiguas registradas en Launch Services (`mdfind "kMDItemCFBundleIdentifier == 'com.caesarcode.app'"` → borrar/actualizar las viejas en `/Applications`, `~/Applications`, `target/**/bundle`); `/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f /Applications/CaesarCode.app`; `touch /Applications/CaesarCode.app`; `killall Dock; killall Finder`; si persiste: `sudo rm -rf /Library/Caches/com.apple.iconservices.store` + reinicio. Quitar y volver a anclar en el Dock.
6. `tauri dev`: el Dock usa el icono por defecto de la ventana (`generate_context!` → `icons/`), requiere `cargo clean -p caesar-code` si el icono se incrustó en un build previo.
Criterios de verificación: (a) el iconset extraído del `.icns` muestra el diseño nuevo en todas las resoluciones; (b) hash de `Contents/Resources/icon.icns` == hash del repo; (c) Finder (Obtener información), Dock, Cmd+Tab y Launchpad muestran el nuevo tras los pasos 4-5; (d) `mdfind` devuelve una sola copia de la app; (e) en `tauri dev` el Dock muestra el nuevo.

---

## 7. Prompts para coders (autocontenidos)

### Prompt BLOQUE A (coder Sonnet)
Proyecto Tauri 2 + React en `/Users/cesargarrido/Proyectos/caesarCode`. Lee `PLAN3.md` §0, §1 y §5 (contratos exactos) y, solo lo necesario, `src-tauri/src/fs/{mod,tree,commands}.rs`, `src-tauri/src/state.rs`, `src-tauri/src/util/paths.rs`, `src/components/sidebar/{FileIcon,FileTreeNode,FileTree}.tsx`, `src/lib/classifyFolder.ts`, `src/store/workspace.ts`. Implementa el Bloque A: `fs/symbols.rs` (clasificador Java/Kotlin con pre-lexer, cabecera 8 KiB, `SymbolCache` por mtime+len en `AppState`), comando `fs_classify` (registrar en `build.rs`, `lib.rs` y `capabilities/default.json` como `allow-fs-classify`), `fs/roles.rs` (`FsEntry.role` y `FsEntry.compact` con compactación de paquetes), iconos SVG propios inline estilo IntelliJ (símbolos C/I/E/R/@ con overlays Spring/main/test, tipos de archivo, carpetas por rol) con registro puro `fileIconKey`, store `symbols.ts` con lotes perezosos e invalidación por watcher, y `FileTreeNode` con nombres compactados. Conserva badges Git, módulo, menú contextual, notify, Quick Open y pestañas. Añade `tempfile = "3"` a dev-dependencies. Tests: fixtures de §1.8 en `src-tauri/tests/fixtures/symbols/`, roles con tempdir, vitest del registro y de `classifyFolder`. No toques Git, run ni services. Termina con `cargo check && cargo test` en `src-tauri` y `npm run build && npm test` en verde; nada de stubs ni TODOs. Nunca añadas Co-Authored-By; no hagas commit salvo que te lo pidan.

### Prompt BLOQUE B (coder Sonnet)
Proyecto `/Users/cesargarrido/Proyectos/caesarCode` (Tauri 2 + React). Lee `PLAN3.md` §0, §2 y §5 (contratos exactos) y, solo lo necesario, `src-tauri/src/workspace/commands.rs`, `src-tauri/src/run/{mod,store}.rs`, `src-tauri/src/git/mod.rs` (`run_git`), `src-tauri/src/services/process_tree.rs`, `src-tauri/src/util/{paths,proc}.rs`, `src-tauri/src/lsp/commands.rs` (uso de `app_data_dir`), `src/App.tsx`, `src/components/layout/{WelcomeView,AppHeader}.tsx`, `src/components/ui/{Modal,Select,TextInput}.tsx`, `src/lib/ipc.ts`. Implementa el Bloque B: módulo `src-tauri/src/projects/` (recientes en app data con escritura atómica y `touch` desde `workspace_open`, detección de herramientas y JDKs, validación, motor de plantillas `{{var}}`, plantillas embebidas en `src-tauri/templates/`, generadores locales, generadores CLI con streaming por `Channel<GenEvent>` y cancelación, descarga HTTP con `reqwest` rustls + `zip` con protección zip-slip, inicializador Spring Microservices online con start.spring.io y fallback offline completo: multi-módulo Maven/Gradle, application.yml, capas, main por servicio, gateway/eureka/config, docker-compose, Dockerfiles, README, `.gitignore`, run-configs vía `run::store::save`). Comandos de la tabla §2.2 registrados en los TRES sitios. Frontend: `ProjectsScreen` (sustituye a `WelcomeView` sin workspace), `NewProjectWizard` con panel de generadores + formularios, `SpringMicroservicesForm`, `GenerationLog`, avisos de herramienta ausente; tras crear, `openWorkspace(root)`. Sin shell: args en vector y validados (§2.5). Tests §2.7 con `tempfile` (red y CLIs `#[ignore]`). Termina con `cargo check && cargo test` y `npm run build && npm test` en verde, sin stubs. No toques el árbol (Bloque A) ni Git (C). Nunca añadas Co-Authored-By; no hagas commit salvo que te lo pidan.

### Prompt BLOQUE C1 (coder Sonnet)
Proyecto `/Users/cesargarrido/Proyectos/caesarCode`. Lee `PLAN3.md` §0, §3 y §5 (contratos y tabla de comandos exactos) y `src-tauri/src/git/{mod,commands}.rs`, `src-tauri/src/fs/watcher.rs`, `src-tauri/src/state.rs`, `src-tauri/src/util/paths.rs`, `src-tauri/src/error.rs`. Implementa el backend Git: builder común extraído de `run_git` (misma config), variantes bytes/stdin/streaming con progreso por `Channel<GitProgress>` y cancelación, parsers NUL (status porcelain v2, log `%x00/%x1e`, for-each-ref, numstat/name-status -z, blame --porcelain, diff unified → hunks), ops mutantes serializadas con `git_op` y guard RAII `git_busy` que el watcher respeta para no emitir `gitChanged`, validación estricta de refnames/shas/rutas (`--` antes de rutas, rechazo de `-…`), commit/tag por stdin, discard de untracked a la papelera (`trash`). Registra todos los comandos de §3.2 en `build.rs`, `lib.rs` y `capabilities/default.json`. Añade tipos espejo en `src/lib/types.ts` y wrappers en `src/lib/ipc.ts` (sin UI). No cambies `git_status` ni `parse_porcelain` existentes. Tests §3.4 con repos reales en `tempfile::tempdir()` y remoto bare local (si no hay git, el test retorna). Termina con `cargo check && cargo test` y `npm run build` en verde. Nunca añadas Co-Authored-By; no hagas commit salvo que te lo pidan.

### Prompt BLOQUE C2 (coder Sonnet)
Proyecto `/Users/cesargarrido/Proyectos/caesarCode`. C1 ya está hecho: los tipos y wrappers Git están en `src/lib/types.ts` / `src/lib/ipc.ts`. Lee `PLAN3.md` §4 y §3.2 (contratos), y `src/store/{ui,workspace}.ts`, `src/components/layout/{AppLayout,AppHeader,StatusBar}.tsx`, `src/components/editor/{theme,languages,extensions}.ts`, `src/components/bottom/LogViewer.tsx` (patrón de `@tanstack/react-virtual`), `src/components/ui/{ContextMenu,ConfirmHost,Modal,Select}.tsx`. Implementa la vista Git estilo GitKraken: `lib/gitGraph.ts` (lanes incrementales y puras) y `lib/gitDiff.ts` con tests vitest de §4.4, `store/git.ts`, `components/git/*` (refs sidebar, grafo virtualizado con SVG por fila y fila WIP, panel de cambios con stage/unstage/discard por archivo y por hunk, formulario de commit con amend, detalle de commit, diff unified propio + split con `@codemirror/merge`, banner de conflictos, barra de remotos con progreso y cancelación, menús contextuales de §4.2 con confirmaciones, diálogos de rama/tag, historial de archivo), gutter de blame CM6 en el editor, selector Code | Git en el header (el editor no se desmonta), entradas Git en el menú contextual del árbol. Añade `@codemirror/merge` a `package.json`. Estilo Graphite & Amber con los tokens existentes, sin `<select>` nativos. Sin bucles de refresco (§4.3). Termina con `npm run build && npm test` y `cargo check` en verde, sin stubs. Nunca añadas Co-Authored-By; no hagas commit salvo que te lo pidan.

---

## 8. Orquestación (agente principal)
1. A → revisar (`cargo test`, `npm test`, abrir un proyecto Spring y comparar con IntelliJ) → B → revisar (generar Spring offline y uno por CLI) → C1 → revisar tests → C2 → revisar UI con un repo grande.
2. Tras cada bloque: comprobar los tres registros de comandos (`grep` de cada comando nuevo en `build.rs`, `lib.rs`, `capabilities/default.json`).
3. Bloque D lo cierra el usuario con los criterios de §6.
