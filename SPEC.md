# CaesarCode — SPEC (fuente de verdad)

Entorno de desarrollo nativo, ultraligero y local-first: edita código con inteligencia real y orquesta microservicios locales sin el consumo de RAM de IntelliJ/PyCharm ni Electron.

## 1. Identidad visual "Graphite & Amber"
- Minimalista estilo Linear.app / Zed. Paleta oscura estricta: base `#0f0f0f`, paneles `#161616`, bordes `#262626`, texto `#ededed`, secundario `#888888`, acento ámbar `#f59e0b` (hover `#d97706`), verde `#22c55e` running, rojo `#ef4444` error/stopped.
- Monoespaciada (`JetBrains Mono`, `Geist Mono` o fallback monospace) en editor, logs, rutas, métricas.
- Cero `<select>` nativos: componentes custom minimalistas con Tailwind.

## 2. Stack obligatorio
### Backend (Rust + Tauri 2)
- `tauri` v2 + `tokio` (rt-multi-thread, full) + `serde`/`serde_json` + `thiserror`.
- Streaming alto rendimiento: `tauri::ipc::Channel` para logs y terminales (NO `emit` global para flujos masivos).
- Microservicios y PTY: `portable-pty` + `sysinfo` (CPU, RAM, árbol de procesos hijos; matar árboles completos sin puertos zombies).
- Archivos/búsqueda: `notify` + `notify-debouncer-mini`, `ignore` + `grep-searcher` (velocidad ripgrep, respeta .gitignore).
- LSP: `lsp-types` + cliente JSON-RPC sobre stdin/stdout en Rust (pyright-langserver, typescript-language-server, rust-analyzer, jdtls).
- Git: binario `git` del sistema vía `tokio::process::Command` (sin libgit2).
### Frontend (React + TS + Tailwind)
- `zustand` (workspace, pestañas, buffers sucios, servicios, diagnósticos LSP).
- `react-resizable-panels` (Sidebar izq. estilo IntelliJ, Editor central con pestañas, Panel inferior Services/Logs/Terminal, StatusBar).
- CodeMirror 6 (state, view, commands, language, autocomplete, lint, search + Python, JS/TS, Rust, Java, JSON, YAML, Markdown). NO Monaco.
- `@xterm/xterm` + `@xterm/addon-webgl` + `@xterm/addon-fit`; `@tanstack/react-virtual` para logs.
- `lucide-react`.

## 3. Funcionalidades (todo funcional, cero stubs)
### A. Archivos/proyecto (Sidebar)
- Selector de carpeta raíz (diálogo nativo Tauri).
- Árbol expandible, carga perezosa, iconos por tipo, badges Git (`M` ámbar, `U` verde, `D` rojo) en vivo.
- Menú contextual: New File, New Folder, Rename, Delete, Copy Path.
- Sincronización con disco vía `notify`.
- Command Palette `Cmd/Ctrl+P`: búsqueda fuzzy de archivos.
- Búsqueda global `Cmd/Ctrl+Shift+F` con `ignore`+`grep-searcher`; clic salta a archivo y línea en CodeMirror.
### B. Editor (CodeMirror 6 + LSP en Rust)
- Pestañas con indicador `•` de sin guardar, cierre rápido, `Cmd/Ctrl+S`.
- Tema custom Graphite & Amber, números de línea, línea activa, buscar/reemplazar, plegado, multicursor.
- Puente LSP `src-tauri/src/lsp/`: detecta lenguaje; si el servidor está en PATH lo arranca una vez por workspace/lenguaje; `didOpen`/`didChange`/`didSave`; expone autocompletado, diagnósticos en vivo (`@codemirror/lint`), hover, Go to Definition (`Cmd/Ctrl+Click`/`F12`).
- Sin LSP instalado: el editor funciona sin errores y la StatusBar muestra indicador discreto.
### C. Microservicios (panel Services estilo IntelliJ)
- Config en `.caesar/services.json` (o `caesar.json`); modal UI para añadir/editar/eliminar (nombre, comando, cwd, env/.env, puerto esperado).
- Auto-detect: escanea subcarpetas buscando `package.json`, `pyproject.toml`/`requirements.txt`, `Cargo.toml`, `pom.xml`/`build.gradle`, `docker-compose.yml`.
- Start/Stop/Restart por servicio; Start All / Stop All.
- Anti-zombies: Stop y `RunEvent::Exit` matan todo el árbol de procesos hijos.
- Métricas vivas: badge Running/Stopped/Error, RAM (MB), CPU (%) con `sysinfo`, puerto activo con botón "Kill Port".
- Logs: visor por servicio y vista "All Services" con filtro por texto y nivel ERROR/WARN/INFO, `@tanstack/react-virtual`, chunks por `Channel`.
- Terminal interactiva real (`portable-pty` + xterm.js WebGL).

## 4. Reglas de ingeniería
1. `src-tauri/capabilities/default.json` estricto (ventana main, dialog, comandos propios).
2. Shutdown limpio: al cerrar la app terminar servicios, PTYs y servidores LSP.
3. Bloques verificables; cada uno debe pasar `cargo check` y `npm run build` antes del siguiente:
   - Bloque 1: scaffolding Tauri 2 + React + Tailwind + Cargo.toml optimizado + `AppState` + `AppError` + layout base (react-resizable-panels, paleta).
   - Bloque 2: sistema de archivos (árbol, CRUD, menú contextual, notify, Git, Quick Open, Global Search).
   - Bloque 3: motor de microservicios (portable-pty, sysinfo, caesar.json, auto-detect, kill tree, Channel, panel Services + logs virtualizados + terminal xterm WebGL).
   - Bloque 4: editor CodeMirror 6 multipestaña + LSP Rust (completion, lint, hover, definition) + limpieza final y verificación completa.
4. Sin funciones vacías ni mocks: todo conectado a Rust real.
