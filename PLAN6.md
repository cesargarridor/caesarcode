# PLAN6 — Implementación de SPEC6

Opus planifica, Sonnet programa. Cada bloque termina en verde con: `npx tsc --noEmit && npm run build && npm test` y, si toca Rust, `cd src-tauri && cargo check && cargo test && cargo clippy -- -D warnings`.

## 0. Decisiones transversales

### 0.1 Temas (A1)
- **Tokens como canales RGB**: `--cc-canvas: 30 31 34;` y en Tailwind `canvas: 'rgb(var(--cc-canvas) / <alpha-value>)'` → `bg-accent/10` sigue funcionando. Tokens que ya llevan alfa (`border`, `border-strong`, `hover`, `hover-strong`, `selection`, `scroll*`) se definen como color completo (`--cc-border: rgb(255 255 255 / .06)`) y en Tailwind como `'var(--cc-border)'` (sin modificador alfa). En el tema claro esos overlays pasan a negro.
- **Registro de temas** `src/theme/themes.ts`: `interface Theme { id; name; dark: boolean; ui: {...tokens}; syntax: {keyword, annotation, type, string, func, field, fieldItalic, variable, comment, number, operator, heading, link, invalid}; term: {...16 ANSI} }`. `src/theme/applyTheme.ts` escribe las vars en `document.documentElement` y `color-scheme`.
- **CodeMirror sin reconfigurar**: `theme.ts` usa `var(--cc-*)` y `HighlightStyle` con `color: 'var(--cc-syn-keyword)'`, `fontStyle: 'var(--cc-syn-field-style)'` → el cambio de tema es solo CSS. Tras cambiar fuente/tamaño: `registry.view?.requestMeasure()`.
- **xterm** necesita hex reales: `src/theme/xterm.ts` → `xtermTheme(theme)`; los terminales se suscriben a `useSettings` y hacen `term.options.theme = …` (con WebGL: `clearTextureAtlas()`). `background` = color de la island.
- `palette.ts` se queda solo con datos no temáticos (`serviceColors`, `laneColors`) + reexport `currentTheme()`; el resto de usos de `palette.*` se migra.
- **Tema por defecto: «Caesar Islands»** (New UI dark): canvas `#1e1f22`, island `#2b2d30`, elevated `#393b40`, borde `#ffffff12`, fg `#dfe1e5`, muted `#868a91`, acento ámbar `#f59e0b` (marca), sintaxis Darcula (A.2). «Caesar Obsidian» = valores actuales exactos.
- Limpieza: los 10 archivos con hex (`blame.ts`, `CommitGraph.tsx`, `DiffViewer.tsx`, `JvmSymbolIcon.tsx`, `iconColors.ts`, `classifyFolder.ts`(+test)) y los 9 con colores Tailwind crudos (`zinc|amber|sky|…`: `RootNode`, `Badge`, `Kbd`, `Modal`, `FileTreeNode`, `ServicesPanel`, `ServiceRow`, `RepoTabs`, `QuickOpen`) pasan a tokens. Excepción permitida: colores semánticos fijos de iconos y las pestañas externas naranjas (C2).

### 0.2 Ajustes (A1 es dueño de `store/settings.ts` y `SettingsModal.tsx`)
A1 define **todos** los campos de una vez para que A2/A3 no toquen esos archivos:
`theme: ThemeId` (def. `caesar-islands`), `iconPack: 'intellij'|'material'|'symbols'` (def. `intellij`), `uiFont: 'inter'|'system'`, `uiSize` (12–15, def. 13), `editorFont: 'jetbrains'|'system-mono'|'menlo'`, `editorSize` (11–20, def. 13.5), `editorLineHeight` (1.3–2.0, def. 1.6), `ligatures: boolean` (def. true), + los 3 del árbol. Clave nueva `cc.settings.v2` con migración desde v1. Aplicación vía vars `--cc-ui-font`, `--cc-ui-size`, `--cc-editor-font`, `--cc-editor-size`, `--cc-editor-lh`, `--cc-ligatures` (`normal`/`none` → `font-variant-ligatures`). Modal con secciones **Apariencia** (tema: lista con muestra de 4 colores; pack de iconos) · **Tipografía** · **Árbol de archivos**.

### 0.3 Iconos (A2)
- **Material**: `material-icon-theme` (MIT, misma familia que Atom Material Icons) como **devDependency fijada**; script `scripts/gen-material-icons.mjs` copia una **lista curada (~90 SVG)** a `src/assets/icons/material/` + `LICENSE`, y genera `src/components/icons/materialMap.ts`. Los SVG se **commitean** (build offline, sin depender del paquete). Carga con `import.meta.glob('../../assets/icons/material/*.svg', { eager: true, query: '?url', import: 'default' })` → `<img>` (CSP `img-src 'self'` ya vale). Solo el subconjunto entra en el bundle.
- **IntelliJ**: componentes SVG propios 16×16 dibujados a mano en `src/components/icons/intellij/` (carpeta outline + overlays: código, badge, etc.) y los archivos de A.1 (git rojo, M↓ naranja, doc, shell, cmd azul, Maven «m»). Lo que no cubra → Material.
- Matriz: `intellij` = carpetas IntelliJ + archivos IntelliJ→Material · `material` = todo Material · `symbols` = actual. `JvmSymbolIcon` (C/I/E/R/@) se mantiene en los tres. Carpeta excluida → nombre en naranja (`text-[var(--cc-excluded-text)]`) en `FileTreeNode`.

### 0.4 Agentes (B)
- Rust `src-tauri/src/agents/{mod,commands}.rs`: `agents_detect() -> Vec<AgentInfo{id, bin, found, path, install_cmd}>` con `which` + PATH de `env_path.rs`. `agent_open(agent: AgentId, cols, rows, on_event)` → **whitelist** `claude|codex|gemini`, **argv vacío** (el frontend no puede pasar args), cwd = raíz primaria. Se registra en el mismo gestor de terminales (`terminal/mod.rs`: nuevo `open_program(dir, program, cols, rows, cb)`), así **`terminal_write/resize/close` se reutilizan**.
- Test Rust: el argv construido no contiene `--dangerously-skip-permissions`, `--full-auto`, `--yolo`, ni nada.
- Comandos de instalación: `npm install -g @anthropic-ai/claude-code`, `npm install -g @openai/codex`, `npm install -g @google/gemini-cli`.
- ⌘L: listener en `App.tsx` en **fase de captura** con `preventDefault()+stopPropagation()` (evita `selectLine` de CM6) — así B no toca `extensions.ts`. Selección leída de `registry.view.state.selection.main` (sin tocar `store/editor.ts`). Comprobar que Cmd+Shift+A y Cmd+L no colisionan con atajos existentes en `App.tsx`.

### 0.5 Archivos externos (C2) — seguridad
`fs_read_file/fs_write_file` usan `ensure_in_roots`. Se añade en `AppState` `external_allow: Mutex<HashSet<PathBuf>>` (canónicas) que **solo concede Rust**: comando `external_open_dialog` (diálogo vía `tauri-plugin-dialog` en Rust), evento `DragDrop` de la ventana, `RunEvent::Opened` (macOS) y args CLI. Rust emite `external-open { path }`. `ensure_in_roots_or_granted` para read/write. `isExternal` = ruta canónica fuera de todas las raíces; se recalcula al cambiar raíces. `tauri.conf.json` → `bundle.fileAssociations` (txt, md, json, yml, xml, csv, xlsx, log, java, py, ts, …).

### 0.6 Visores (C3/C4)
- Lenguajes: añadir `@codemirror/lang-xml`, `lang-html`, `lang-css`, `lang-sass`, `lang-sql`, `lang-go`, `@codemirror/legacy-modes` (toml, properties, shell, dockerfile) vía `StreamLanguage`; `log` = StreamLanguage propio (niveles ERROR/WARN/INFO/DEBUG, timestamps); `env` → properties; `txt` → nada.
- \> 5 MB: `ops::read_file` devuelve los primeros 5 MB con `truncated: true` (umbral total sigue en 10 MB → `tooLarge`); la pestaña se abre `EditorState.readOnly` + banner.
- Tabular: `src-tauri/src/viewer/{mod,tabular}.rs` (`calamine`, `csv`), `read_tabular_file(path, sheet?, limit?) -> {sheets, sheet, headers, rows: Vec<Vec<String>>, total_rows, file_size, delimiter}`; CSV en streaming (cuenta total sin guardar filas > limit). Misma regla de roots/granted.
- Multimedia: feature `protocol-asset` en `tauri` (Cargo.toml), `app.security.assetProtocol: { enable: true, scope: [] }` y **scope dinámico**: comando `media_allow(path)` que valida roots/granted y llama `app.asset_protocol_scope().allow_file(&p)`. Frontend `convertFileSrc(path)`. El asset protocol sirve **Range requests** → vídeo/audio en streaming, RAM baja. CSP: añadir `media-src 'self' asset: http://asset.localhost; frame-src 'self' asset: http://asset.localhost; object-src 'none'`. PDF en `<iframe>` (WKWebView y WebView2 lo renderizan; WebKitGTK/Linux no → aviso + botón «Revelar en Finder/Explorador»).
- `TabKind` += `'tabular' | 'media'`; detección por extensión en el frontend antes de leer (no se llama a `fs_read_file` para binarios multimedia).

## 1. Archivos por bloque (propiedad)

| Bloque | Archivos (N = nuevo) |
|---|---|
| **A1** Temas+tipografía+ajustes | `tailwind.config.js`, `src/index.css`, `src/lib/palette.ts`, N `src/theme/{themes,applyTheme,xterm}.ts`, `src/components/editor/theme.ts`, `src/components/editor/blame.ts`, `src/components/bottom/TerminalView.tsx`, `src/store/settings.ts`, `src/components/ui/SettingsModal.tsx`, `src/main.tsx`, `package.json` (`@fontsource-variable/inter`), los 10 archivos con hex y los 9 con colores crudos (§0.1) |
| **A2** Iconos | `package.json` (devDep), N `scripts/gen-material-icons.mjs`, N `src/assets/icons/material/*`, N `src/components/icons/{materialMap.ts,intellij/*}`, `src/components/icons/SymbolIcon.tsx`, `src/lib/fileIconRegistry.ts`(+test), `src/components/sidebar/FileTreeNode.tsx` (nombre excluido naranja) |
| **B1** Rust agentes | N `src-tauri/src/agents/*`, `src-tauri/src/terminal/mod.rs`, *compartidos:* `lib.rs`, `build.rs`, `capabilities/default.json` |
| **B2** Panel agentes | N `src/components/agents/*`, N `src/store/agents.ts`, N `src/lib/ipcAgents.ts`, `src/components/layout/AppLayout.tsx`, `src/components/layout/panelRefs.ts`, `src/App.tsx` (Cmd+Shift+A, ⌘L captura) |
| **C-Rust** (C2r+C3r+C4r) | `src-tauri/src/state.rs`, `src-tauri/src/util/paths.rs`, `src-tauri/src/fs/{ops,commands}.rs`, N `src-tauri/src/viewer/*`, N `src-tauri/src/external.rs`, `Cargo.toml`, `tauri.conf.json`, *compartidos:* `lib.rs`, `build.rs`, `capabilities/default.json` |
| **C1** Live Templates | N `src/editor/liveTemplates.ts`(+test), `src/components/editor/extensions.ts` |
| **C3f** Lenguajes | `src/components/editor/languages.ts`, `package.json`, `src/components/editor/CodeEditor.tsx` (readOnly+banner) |
| **C24f** Externos+visores | `src/store/editor.ts`, `src/components/editor/{EditorArea,TabBar}.tsx`, `src/lib/{ipc,types}.ts`, N `src/components/viewers/{TabularViewer,MediaViewer}.tsx`, `src/App.tsx` (Cmd+O, listener `external-open`), `package.json`? (no: react-virtual ya está) |

## 2. Orden y paralelismo

```
Ola 1:  A1 ─────────────┐        ‖  B1 → C-Rust   (un solo coder Rust, secuencial)
Ola 2:  A2 ‖ B2 ‖ C1 ‖ C3f       (tras A1 y tras su Rust correspondiente)
Ola 3:  C24f                     (tras C-Rust y C3f)
```
- **A1 va solo en frontend** en la ola 1: toca ~25 archivos de estilo; nadie más edita TSX a la vez.
- **Rust**: B1 y C-Rust comparten `lib.rs`/`build.rs`/`capabilities/default.json` → mismo coder en secuencia (alternativa: worktrees en paralelo con regla **append-only al final de cada lista**; el merge = conservar ambas líneas).
- **Ola 2** es disjunta: A2 (icons/, FileTreeNode), B2 (agents/, AppLayout, App.tsx, panelRefs), C1 (extensions.ts), C3f (languages.ts, CodeEditor.tsx). `package.json`: A2 y C3f añaden deps → instalar con `npm i` en serie (primero A2, luego C3f) o resolver el conflicto conservando ambas.
- **Ola 3**: C24f toca `App.tsx` (también B2) → va después de B2. Regla: cada uno añade su `useEffect`/handler en bloque propio, sin reordenar.
- `SettingsModal.tsx`/`settings.ts`: **solo A1**. `AppLayout.tsx`: **solo B2**.

## 3. Riesgos
1. Tailwind + vars: clases con alfa sobre tokens que ya llevan alfa (`border/50`) dejan de funcionar → grep `-(border|hover|selection)/` y corregir.
2. Tema claro: overlays `rgb(255 255 255/…)` hardcodeados en TSX → grep `255 255 255` / `rgba(255`.
3. xterm WebGL no lee CSS vars → re-aplicar `options.theme` al cambiar.
4. CM6 cachea alturas: `requestMeasure()` tras cambiar fuente/tamaño/interlineado.
5. `material-icon-theme` es grande (miles de SVG): solo devDep + subconjunto commiteado; revisar licencias (MIT) y añadir `LICENSE`.
6. ⌘L choca con `selectLine` de CM6; Cmd+O con lo que exista → captura en `App.tsx`.
7. Externos: `ensure_in_roots` los bloquea → lista de concesiones **solo desde Rust**.
8. PDF inline no funciona en Linux (WebKitGTK).
9. PATH en app GUI: usar `env_path.rs` para detectar `claude/codex/gemini` (nvm/homebrew).
10. Asset protocol: scope dinámico + CSP `media-src/frame-src`; sin esto, vídeo/PDF fallan en silencio.

## 4. Prompts listos para Sonnet

### A1 — Temas + tipografía + ajustes
> Proyecto CaesarCode (`/Users/cesargarrido/Proyectos/caesarCode`). Lee `SPEC6.md` §A y `PLAN6.md` §0.1, §0.2, §1(A1). Implementa el sistema de temas: tokens `--cc-*` en canales RGB, Tailwind `rgb(var(--x) / <alpha-value>)` (tokens con alfa propio como `var(--x)`), `src/theme/themes.ts` con 11 temas (Caesar Islands [por defecto], Caesar Obsidian [= valores actuales exactos de palette.ts], Darcula, Monokai, One Dark, Dracula, Nord, Solarized Dark, GitHub Dark, Tokyo Night, IntelliJ Light) incluyendo ui, syntax (Darcula: annotation #bbb529, keyword #cc7832, string #6a8759, field #9876aa cursiva, number #6897bb, comment #808080) y ANSI; `applyTheme.ts`; `xterm.ts`; reescribe `editor/theme.ts` y `blame.ts` con `var(--cc-*)` (incluida tipografía vía `--cc-editor-*`, ligaduras con `font-variant-ligatures`), y `TerminalView.tsx` para re-aplicar el tema xterm al cambiar (`clearTextureAtlas` si hay WebGL). Amplía `store/settings.ts` con TODOS los campos de §0.2 (clave v2 con migración de v1) y rehaz `SettingsModal.tsx` en secciones Apariencia/Tipografía/Árbol (selector de pack de iconos ya visible aunque A2 lo consuma después). Instala `@fontsource-variable/inter` e impórtala en `main.tsx`. Árbol: fuente sans 13 px. Migra a tokens los 10 archivos con hex y los 9 con colores Tailwind crudos (lista en §0.1). Tras cambiar fuente llama `registry.view?.requestMeasure()`. No toques AppLayout, icons/, extensions.ts, languages.ts ni Rust. Verifica `npx tsc --noEmit && npm run build && npm test`. No hagas commit.

### A2 — Iconos
> CaesarCode. Lee `SPEC6.md` §A.1 y §A.4 y `PLAN6.md` §0.3, §1(A2). A1 ya añadió `iconPack` a `useSettings`. Instala `material-icon-theme` como devDependency con versión exacta; inspecciona `node_modules/material-icon-theme/icons/` (solo listados de nombres, no leas fuentes) y crea `scripts/gen-material-icons.mjs` que copia una lista curada (~90: todos los `FileIconName`/`FolderIconName` de `lib/fileIconRegistry.ts`, abiertas y cerradas) a `src/assets/icons/material/` + LICENSE y genera `src/components/icons/materialMap.ts`. Crea `src/components/icons/intellij/` con SVG 16×16 propios: carpeta outline y variantes (.idea teal, .mvn carmesí, src azul con glifo código, target ámbar, docs azul, infra gris con badge, resources, test verde) y archivos (git rojo, markdown M↓ naranja para HELP.md, doc para README.md, shell para mvnw, cmd azul para mvnw.cmd, Maven «m» rosa para pom.xml); añade a `fileIconRegistry` las claves nuevas necesarias (+tests). En `SymbolIcon.tsx` resuelve según `iconPack` (matriz en §0.3) sin cambiar las firmas de `FileIcon/FolderIcon/TechIcon`; Material se pinta con `<img>` del mapa. En `FileTreeNode.tsx` nombre de carpeta excluida en naranja. Ejecuta el script (no hagas commit). Verifica tsc/build/test. No toques settings.ts, SettingsModal.tsx ni otros archivos.

### B1 — Rust agentes
> CaesarCode, `src-tauri`. Lee `SPEC6.md` §B y `PLAN6.md` §0.4. Crea `src/agents/{mod,commands}.rs`: `agents_detect` (which + PATH de `env_path.rs`) y `agent_open(agent, cols, rows, on_event: Channel<TerminalEvent>)` con whitelist claude/codex/gemini, argv vacío, cwd raíz primaria. Añade a `terminal/mod.rs` `open_program(...)` reutilizando el gestor para que `terminal_write/resize/close` sirvan. Test: argv nunca contiene `--dangerously-skip-permissions`, `--full-auto`, `--yolo`. Registra los 2 comandos **al final** de `build.rs` COMMANDS, `generate_handler!` en `lib.rs` y `capabilities/default.json` (`allow-agents-detect`, `allow-agent-open`). Verifica `cargo check && cargo test && cargo clippy -- -D warnings`.

### C-Rust — externos, truncado, tabular, multimedia
> CaesarCode, `src-tauri`. Lee `SPEC6.md` §C2–C4 y `PLAN6.md` §0.5, §0.6. (1) `AppState.external_allow` + `ensure_in_roots_or_granted` en `util/paths.rs`, usado por `fs_read_file/fs_write_file`; `src/external.rs`: comando `external_open_dialog` (dialog en Rust, múltiple), manejo de `WindowEvent::DragDrop` y `RunEvent::Opened` + args CLI en `lib.rs`, todos conceden la ruta canónica y emiten `external-open {path}`. (2) `ops::read_file`: > 5 MB devuelve los 5 primeros MB con `truncated: true` (cortado en límite UTF-8). (3) `src/viewer/tabular.rs` con `calamine` y `csv` (`read_tabular_file`, CSV en streaming contando total, límite por defecto 5000, detección `,`/`;`/`\t`) + tests con tempfiles. (4) Feature `protocol-asset`, `assetProtocol {enable:true, scope:[]}`, comando `media_allow(path)` que valida y llama `asset_protocol_scope().allow_file`, CSP con `media-src` y `frame-src` asset, `bundle.fileAssociations`. Registra todo **al final** de build.rs/generate_handler!/capabilities. Verifica cargo check/test/clippy.

### B2 — Panel de agentes
> CaesarCode. Lee `SPEC6.md` §B y `PLAN6.md` §0.4, §1(B2). B1 y A1 ya están. Crea `src/lib/ipcAgents.ts`, `src/store/agents.ts` y `src/components/agents/{AgentsPanel,AgentTerminal,ContextBar,AgentEmptyState}.tsx`: pastillas Claude Code/Codex/Gemini CLI, estado vacío con comando oficial + Reintentar, badge verde candado «Permisos activos», xterm con `xtermTheme()` de `src/theme/xterm.ts` (fondo = island, sigue el tema), scrollback 3000, `terminal_write/resize/close` reutilizados, banner «Reiniciar» si cambia la raíz primaria, barra de contexto `Archivo · L25-42` desde `registry.view.state.selection.main` y botón «Enviar contexto (⌘L)» que escribe `@ruta/relativa` (+`#L25-42` o formato del agente) en el PTY. `AppLayout.tsx`: tercera `Panel id="agents" order={3} collapsible` en una Island, ref en `panelRefs.ts`. `App.tsx`: Cmd+Shift+A alterna, ⌘L en fase de captura con preventDefault+stopPropagation. No toques extensions.ts, store/editor.ts, ipc.ts, types.ts. Verifica tsc/build/test.

### C1 — Live Templates
> CaesarCode. Lee `SPEC6.md` §C1. Crea `src/editor/liveTemplates.ts` con `snippetCompletion` de `@codemirror/autocomplete` (todas las plantillas listadas, por lenguaje: java, python, ts/js, yaml) y `liveTemplateSource(path)`; en `src/components/editor/extensions.ts` añádela a `autocompletion({ override: [lspCompletionSource, liveTemplateSource] })` con `type: 'snippet'`; Tab/Shift+Tab/Esc ya los da `snippetKeymap` (verifícalo). Test vitest de que cada extensión ofrece sus plantillas. Sin libs nuevas. Solo esos archivos. Verifica tsc/build/test.

### C3f — Lenguajes
> CaesarCode. Lee `SPEC6.md` §C3 y `PLAN6.md` §0.6. Instala solo los paquetes que falten (lista en §0.6) y amplía `languages.ts` (+ `Dockerfile`, `.env*`, `*.properties`, `*.log` por nombre). En `CodeEditor.tsx`, si el archivo vino `truncated`, `EditorState.readOnly.of(true)` y banner «Archivo > 5 MB: vista parcial de solo lectura». Verifica tsc/build/test.

### C24f — Externos + visores
> CaesarCode. Lee `SPEC6.md` §C2, §C4 y `PLAN6.md` §0.5, §0.6. C-Rust, B2 y C3f ya están. `lib/ipc.ts`/`types.ts`: wrappers de `external_open_dialog`, `read_tabular_file`, `media_allow`, `truncated`. `store/editor.ts`: `Tab.isExternal` (canónica fuera de todas las raíces; recalcular al cambiar raíces) y `TabKind += 'tabular'|'media'` por extensión (csv/tsv/xlsx/xls/ods → tabular; imagen/audio/vídeo/pdf → media, sin leer el archivo). `TabBar.tsx`: estilo naranja y tooltip exactos de SPEC. `App.tsx`: Cmd+O y listener `external-open` (bloque nuevo propio). `viewers/TabularViewer.tsx` (react-virtual, selector de hoja abajo, filtro, clic copia celda, Tabla|Texto plano para csv/tsv) y `viewers/MediaViewer.tsx` (`media_allow` + `convertFileSrc`; `<img>`, `<video controls preload="metadata">`, `<audio>`, `<iframe>` PDF; fallback en Linux). `EditorArea.tsx` enruta por `kind`. Verifica tsc/build/test.
