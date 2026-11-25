# SPEC6 — Temas + iconos estilo IntelliJ/Atom + tipografía · Panel de agentes IA · Live Templates + visor universal

Resumen fiel de la petición del usuario. Tres bloques: **A** look & feel, **B** panel derecho de agentes IA, **C** Live Templates + archivos externos + resaltado + visores (tablas y multimedia).

---

## Bloque A — Look & feel

### A.1 Árbol de carpetas estilo IntelliJ (captura del usuario: árbol de IntelliJ New UI)
- Carpetas normales: icono de carpeta **en contorno (outline)**, claro.
- Carpetas especiales con glifo/color propio:
  - `.idea` → cian/teal · `.mvn` → carmesí/rojo · `src` → azul con glifo de código · `target` → naranja/ámbar; las carpetas **excluidas** tienen el **texto del nombre teñido de naranja** · `docs` → azul · `infra` → gris con badge · `colecciones` / `.caesar` → contorno simple.
- Archivos:
  - `.gitattributes` / `.gitignore` → icono git rojo.
  - `HELP.md` → icono markdown naranja «M↓»; `README.md` → documento simple.
  - `mvnw` → icono shell; `mvnw.cmd` → azul; `pom.xml` → icono Maven «m» rosa/rojo.
- Texto del árbol: sans limpia (no mono), 13 px, filas holgadas.

### A.2 Editor estilo IntelliJ Darcula (captura del usuario)
- Anotaciones amarillo-oliva (`#bbb529`), keywords naranja (`#cc7832`), strings verde (`#6a8759`), campos morado en cursiva, números azul, comentarios gris, resaltado de línea activa sutil.
- Subrayado ondulado de sugerencias ortográficas: **opcional**.

### A.3 Sistema de temas
- El aspecto actual es demasiado negro y «muerto»: se quiere algo más vivo, menos negro puro.
- Ajustes → **Tema** (persistido). Temas basados en variables CSS `--cc-*` + un tema de CodeMirror a juego por tema.
- Temas: **Caesar Obsidian** (el actual), **Monokai**, **Darcula (IntelliJ)**, **One Dark**, **Dracula**, **Nord**, **Solarized Dark**, **GitHub Dark**, **Tokyo Night** y, a ser posible, **un tema claro**.
- Cada tema define: canvas, island, elevated, bordes, texto, acento y colores de sintaxis.
- El **tema por defecto** debe ser más vivo y menos negro (p. ej. grises tipo Darcula/New UI `#1e1f22` canvas / `#2b2d30` island, o un gris azulado).
- Todos los colores de Tailwind pasan por variables CSS: el cambio de tema es **en vivo, sin recargar**.
- El terminal xterm también sigue el tema.

### A.4 Pack de iconos estilo Atom Material
- El usuario usa «Atom Material Icons» en IntelliJ y quiere ese estilo (iconos material de colores).
- Empaquetado offline, sin red en runtime, **solo el subconjunto necesario** (bundle pequeño).
- Ajuste **Pack de iconos**: `Symbols` (actual, `@react-symbols`) | `Material (Atom)` | `IntelliJ (carpetas outline, A.1)`.
- `SymbolIcon.tsx` sigue siendo el **único resolver**; `FileIcon` / `FolderIcon` / `TechIcon` mantienen su firma.

### A.5 Tipografía estilo IntelliJ
- Fuente UI (Inter / sistema) con pesos y tamaños correctos.
- Editor: JetBrains Mono (ya incluida vía `@fontsource`), **ligaduras on/off**, tamaño, interlineado y elección de familia.
- Ajustes: tamaño UI + tamaño editor + fuente editor + ligaduras.

### A.6 Modal de Ajustes limpio, por secciones
- **Apariencia** (tema, iconos) · **Tipografía** · **Árbol de archivos** (los sliders ya existentes).

---

## Bloque B — Panel derecho de agentes IA (requisitos textuales)
- Panel derecho **plegable** dentro del layout de Islands; atajo **Cmd+Shift+A**.
- Pastillas de agente: **Claude Code** (`claude`), **Codex** (`codex`), **Gemini CLI** (`gemini`).
- Rust comprueba en el `PATH` si existe cada binario; si no, **estado vacío** con el comando oficial de instalación + botón **Reintentar**.
- **NUNCA** pasar flags de autoaprobación (`--dangerously-skip-permissions`, `--full-auto`, `--yolo`).
- Badge verde con candado: **«Permisos activos»**.
- El `cwd` del PTY = `workspaceRoot`; si el proyecto cambia, **banner para reiniciar** la sesión.
- Barra de contexto: archivo activo + rango de líneas seleccionado (p. ej. `IngresoController.java · L25-42`) y botón **«Enviar contexto (⌘L)»** que inyecta `@ruta/relativa` (+rango) en el stdin del PTY.
- Fondo de xterm **exactamente** el color de la island (ahora debe seguir el tema); scrollback 3000.
- Mantener el hot-reload por `notify` para que CM6 se actualice cuando el agente edita archivos.
- Reutilizar `portable-pty`, `tauri::ipc::Channel` y la infraestructura de xterm.
- Nuevos comandos Rust: registrar en `build.rs`, `generate_handler!` y `capabilities/default.json`.
- Existe workspace multi-raíz (`Workspace{roots}`).

---

## Bloque C — Live Templates + visor universal (requisitos textuales)

### C1 Live Templates
- `src/editor/liveTemplates.ts` con `snippetCompletion`:
  - Java/Spring: `getm`, `postm`, `putm`, `delm`, `restc`, `serv`, `logi`, `loge`, `tryc`, `testm`.
  - AWS Lambda: `lambdah` (Java `RequestHandler<APIGatewayProxyRequestEvent, APIGatewayProxyResponseEvent>`), `lambdap` (Python `lambda_handler` con try/except y `statusCode`/`body` JSON).
  - Extras: `main` (Python), `tryc` (Python/TS), `springapp` (bloque `application.yml`).
- Registrados por extensión de archivo **junto** a la completion LSP (conviven, tipo `snippet`); Tab / Shift+Tab navegan campos, Esc sale.
- Sin librerías nuevas.

### C2 Archivos externos
- **Cmd+O** diálogo nativo, **drag & drop** sobre la ventana, **«Abrir con CaesarCode»** / argumentos CLI en macOS (asociación de archivos / evento `Opened`).
- En el store de pestañas, `isExternal` se calcula por **ruta canónica** frente a las raíces del workspace (estar dentro de **cualquier** raíz cuenta como interno).
- Pestañas externas en naranja: `border-orange-500/50`, `bg-orange-500/12`, `text-orange-300`, punto, tooltip `Archivo externo al proyecto: <ruta>`.
- Editables y guardables.

### C3 Resaltado de sintaxis
- yml/yaml, xml, html, css, scss, json, sql, md, toml, properties, env, sh, dockerfile, log, txt + Java/Python/TS/JS/Rust/Go.
- Instalar `@codemirror/lang-*` ligeros **solo si faltan**.
- Archivos > 5 MB: abrir en solo lectura o truncados, con aviso.

### C4 Visor tabular + multimedia
- Rust: crates `calamine` + `csv`; comando `read_tabular_file(path, sheet?, limit?)` → hojas, cabeceras, filas (límite inicial 5000 + total real), tamaño de archivo.
- `TabularViewer.tsx` con `@tanstack/react-virtual`, selector de hoja abajo, barra de filtro/búsqueda, clic en celda = copiar, conmutador csv/tsv **Tabla | Texto plano**.
- **Nuevo (usuario):** ver vídeos, PDFs y todo tipo de multimedia (imagen/audio/vídeo/pdf) vía asset protocol o blob; planificar scope del asset protocol y CSP; RAM baja: **streaming, no leer archivos enteros**.

---

## Verificación común
`npx tsc --noEmit`, `npm run build`, `npm test`, `cargo check`, `cargo test`, `cargo clippy` (en `src-tauri`).
