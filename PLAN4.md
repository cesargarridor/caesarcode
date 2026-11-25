# PLAN4 — Implementación de SPEC4 "Modern Islands UI"

Solo capa visual. Prohibido tocar `src/store/**`, `src/lib/ipc*`, lógica de handlers, atajos de `App.tsx` y Rust (salvo `tauri.conf.json` + `capabilities/default.json`).
Orden: **A → B → C**, en secuencia (B y C dependen de los tokens de A). Cada bloque termina con `npx tsc --noEmit && npm run build && npm test` en verde.

---

## 0. Decisiones transversales

### 0.1 Tokens (fuente única)
Estrategia: **remapear los tokens existentes** (así ~90 % de componentes heredan el nuevo look sin tocarlos) y **añadir tokens semánticos nuevos**. Hoy `bg-base` se usa en 27 sitios como "fondo interior de panel/input", por eso `base` pasa a ser el fondo de isla, NO el canvas.

| Token (Tailwind / palette.ts / CSS var) | Antes | Después | Uso |
|---|---|---|---|
| `canvas` / `palette.canvas` / `--cc-canvas` (NUEVO) | — | `#09090b` | fondo app, header, statusbar, huecos entre islas |
| `base` / `--cc-base` | `#0f0f0f` | `#111114` | interior de isla (editor, logs, terminal) |
| `panel` / `--cc-panel` | `#161616` | `#111114` | isla (Sidebar, Editor, Bottom, Git) |
| `elevated` / `--cc-elevated` (NUEVO) | — | `#18181c` | cards, dropdowns, inputs, modales, pills |
| `elevated-hover` (NUEVO) | — | `#222228` | hover de pills/selector proyecto |
| `card` (NUEVO) | — | `#16161b` | ServiceRow; `card-hover` `#1c1c22` |
| `tab-active` (NUEVO) | — | `#1b1b20` | pestaña activa |
| `quick` (NUEVO) | — | `#141418` | trigger Quick Open |
| `border` | `#262626` | `rgb(255 255 255 / 0.06)` | borde de isla |
| `border-strong` (NUEVO) | — | `rgb(255 255 255 / 0.08)` | borde de elevadas / pestaña activa |
| `hover` | `#1f1f1f` | `rgb(255 255 255 / 0.05)` | hover sutil |
| `fg` | `#ededed` | `#ececef` | texto |
| `fg-strong` (NUEVO) | — | `#f4f4f6` | texto pestaña activa |
| `muted` | `#888888` | `#8e8e98` | secundario |
| `faint` (NUEVO) | — | `#71717a` | breadcrumbs, statusbar (zinc-500) |
| `run` (NUEVO) | — | `#10b981` | Run/Running |
| `stop` (NUEVO) | — | `#e11d48` | Stop/Error/marca (`crimson` pasa a `#e11d48`) |
| `accent` | `#f59e0b` | igual | oro |
| `syntax.*` (NUEVO, solo palette.ts) | — | ver §B.4 | tema CM6 |

- `tailwind.config.js`: para los tokens con alfa usar la forma `rgb(255 255 255 / <alpha-value>)` NO; usar string literal `'rgb(255 255 255 / 0.06)'`. Ojo: hay 5 usos de `border-border/xx`, `bg-base/xx`, `bg-panel/xx` (grep `'border-border/\|bg-base/\|bg-panel/'`): con color rgba el modificador `/xx` deja de funcionar → sustituirlos por `border-white/[0.0x]` o el token sólido equivalente.
- Los fondos suaves de acento del SPEC (`bg-amber-500/12`, `bg-emerald-500/15`, `bg-rose-500/15`, `bg-white/[0.05]`) se escriben como utilidades Tailwind sobre tokens: `bg-accent/12`, `bg-run/15`, `bg-stop/15` (hex → el modificador sí funciona). Añadir `opacity: { 12: '0.12', 15: '0.15' }` en `theme.extend` si hace falta. `text-amber-200/300`, `text-emerald-400`, `text-rose-400` → añadir tokens `accent-soft #fcd34d`, `accent-text #fde68a`, `run-text #34d399`, `stop-text #fb7185` (no hex en componentes).
- `palette.ts`: añadir las mismas claves (camelCase) y bloque `syntax`. `index.css :root`: mismas vars `--cc-*`. Mantener comentario "espejo".
- `borderRadius`: el SPEC usa `rounded-xl`(12)/`rounded-lg`(8)/`rounded-md`(6) de Tailwind por defecto: no hace falta extender.
- Tipografía (`tailwind.fontFamily.sans` + `index.css body`): `-apple-system, BlinkMacSystemFont, "Inter", system-ui, sans-serif`; `font-size: 12.5px`; `fontSize.ui` → `['12.5px','17px']`, `ui-lg` → `['13px','18px']`.

### 0.2 Ventana macOS (Tauri v2, `@tauri-apps/api ~2.12`)
- `src-tauri/tauri.conf.json` ventana `main`: añadir `"titleBarStyle": "Overlay"`, `"hiddenTitle": true`, `"backgroundColor": "#09090b"`. Mantener `decorations` por defecto (true). Overlay solo aplica en macOS; en Windows/Linux se ignora y la barra nativa sigue → no hace falta lógica extra allí.
- `trafficLightPosition` (`{ "x": 14, "y": 22 }` para centrar en 44 px) es opcional: añadir solo si `src-tauri/gen/schemas/*` / el build lo acepta; si `cargo tauri build` lo rechaza, quitarlo (por defecto los semáforos quedan a y≈ 14 px, aceptable).
- Padding izquierdo: en `AppHeader`, si macOS (`navigator.userAgent.includes('Mac')`), `pl-[78px]` en el contenedor izquierdo; en otros SO `pl-3`. Constante `TRAFFIC_LIGHTS_PAD = 78` en `src/lib/platform.ts` (NUEVO, 5 líneas: `export const isMac = ...`).
- En pantalla completa macOS los semáforos desaparecen: aceptable dejar el hueco (opcional: `getCurrentWindow().onResized` + `isFullscreen()` para quitar el padding; requiere `core:window:allow-is-fullscreen`, ya en `core:default`).
- Arrastre: `data-tauri-drag-region` en el `<header>` y en los huecos/spacers (no en botones: los hijos interactivos no arrastran, pero los `<div>` contenedores intermedios sí deben llevar el atributo o el arrastre solo funciona en el fondo directo). Doble clic = maximizar.
- `src-tauri/capabilities/default.json` añadir: `"core:window:allow-start-dragging"`, `"core:window:allow-internal-toggle-maximize"` (ambos existen en `gen/schemas/desktop-schema.json`). Sin ellos `data-tauri-drag-region` no hace nada en v2.
- `ProjectsScreen` tiene su propio `<header>`: debe aplicar el mismo padding mac + drag region (si no, con Overlay los semáforos tapan el logo).

### 0.3 Qué ya existe y se reutiliza
- `FsEntry.compact` (Rust) + `FileTreeNode` ya pinta `entry.compact.name` y usa `compact.path` como clave: **solo estilo**, no lógica.
- `useUi.openModal('quickOpen')` abre el QuickOpen real (App.tsx línea ~138) → el trigger del header llama a eso.
- `RunToolbar` / `RunDropdown` / `TechIcon` ya tienen la lógica run/stop/config activa → reestilar dentro de un contenedor pill.
- `BranchChip` y `CenterSwitch` (Code/Git) en `AppHeader` → se conservan, reestilados como pills.
- `ui/Select.tsx` ya es un select custom (no hay `<select>` nativo en el repo): solo reestilar → LogViewer lo hereda.
- `@tanstack/react-virtual` ya virtualiza `LogViewer` y `CommitGraph`: no tocar el virtualizer ni `estimateSize`.
- `graphiteTheme` se importa en `extensions.ts`, `CodeEditor.tsx`, `DiffViewer.tsx` → renombrar a `obsidianTheme` y mantener `export const graphiteTheme = obsidianTheme` como alias (o actualizar los 3 imports).

### 0.4 Trampas
1. **No romper lógica/stores**: solo `className`, markup envolvente y componentes visuales nuevos. No cambiar props públicas de componentes usadas por otros (salvo añadir opcionales). No tocar `panelRefs`, `autoSaveId`, `id/order` de `Panel`.
2. **react-resizable-panels**: las islas van DENTRO de cada `Panel` (wrapper `h-full rounded-xl border bg-panel overflow-hidden`); el gap lo pone el `ResizeHandle` (`w-1.5`/`h-1.5` transparente). No meter `gap` en `PanelGroup` (rompe el cálculo de tamaños). Panel colapsado (size 0) no debe dejar borde visible: el wrapper con `overflow-hidden` y Panel a 0 lo oculta; verificar colapsando sidebar y bottom.
3. **Editor oculto en vista Git**: mantener el patrón `hidden` vs desmontaje en `AppLayout` (estado CM).
4. **WCAG**: `muted #8e8e98` sobre `#111114` ≈ 5.9:1 OK; `faint #71717a` sobre `#09090b` ≈ 4.4:1 (límite) → solo para texto no esencial (breadcrumbs, statusbar secundaria); info esencial de statusbar (errores, rama) en `muted`/color. Comentarios `#52525b` sobre `#111114` ≈ 2.5:1: es valor del SPEC; aceptado como excepción documentada (comentario en theme.ts), no subir otros colores de texto por debajo de 4.5:1. `text-accent` sobre `bg-accent/12` ≈ ok (>6:1).
5. **Rendimiento**: en filas de `LogViewer`/`CommitGraph` nada de `transition`, `backdrop-blur`, `shadow` ni `animate-*` por fila; hover solo `hover:bg-hover`. El punto pulsante (`animate-pulse`) solo en badges de servicios (pocos). No cambiar altura de fila del virtualizer sin actualizar `estimateSize` (mejor: no cambiarla).
6. **Scrollbars**: `index.css` global a 6 px; xterm y CodeMirror usan su propio scroller → regla `.cm-scroller::-webkit-scrollbar` hereda de `*`, verificar; xterm `.xterm-viewport` también.
7. **Coherencia**: vista Git, ProjectsScreen, NewProjectWizard, Modal, ContextMenu, Toasts, QuickOpen/GlobalSearch deben usar `elevated`/`border-strong`/`rounded-lg`; buscar restos con `grep -rn 'bg-\[#\|#[0-9a-fA-F]\{6\}' src/components` y migrarlos a tokens (excepto `laneColors` y `TechIcon` de marca).
8. `crimson-dark` en bordes (`border-crimson-dark` del header actual) desaparece: sin bordes duros.

### 0.5 Criterios de aceptación globales
- `npx tsc --noEmit` = 0 errores; `npm run build` OK; `npm test` (vitest, 7 ficheros: RecentList, classifyFolder, fileIconRegistry, gitDiff, gitGraph, projectRequest, projectValidate) en verde.
- `grep -rnE '#[0-9a-fA-F]{6}' src/components` solo devuelve: `TechIcon`/iconos de marca y `FileIcon`/registry. Ningún hex nuevo en componentes.
- `git diff --stat src/store src/lib/ipc* src-tauri/src` vacío.
- Visual manual (`npm run tauri dev`): canvas visible entre islas; semáforos sin tapar contenido; arrastrar ventana desde el header; doble clic maximiza; handles invisibles y ámbar al hover/arrastre; colapsar sidebar/bottom sin bordes fantasma.

---

## Bloque A — Tokens + layout islas + AppHeader + handles + scrollbars + ventana

| Archivo | Cambio |
|---|---|
| `tailwind.config.js` | tokens §0.1, fontFamily.sans, fontSize, opacity 12/15 |
| `src/lib/palette.ts` | mismas claves + `syntax` (valores §B.4, para que B no toque palette) + `canvas`, `elevated`, `card`, etc. `laneColors` sin cambios |
| `src/index.css` | `:root` vars `--cc-*` nuevas; `body` bg `var(--cc-canvas)`, color `#ececef`, fuente/tamaño SPEC; scrollbars 6 px, thumb `rgb(255 255 255 / .10)`, hover `.18`, `border-radius: 999px`, `background-clip: padding-box` con borde transparente 1px; `*::-webkit-scrollbar-corner {transparent}`; `:focus-visible` con `outline-offset: 1px` + `border-radius` heredado |
| `src/lib/platform.ts` (NUEVO) | `isMac`, `TRAFFIC_LIGHTS_PAD = 78` |
| `src/components/layout/AppLayout.tsx` | root `bg-canvas`; zona central `min-h-0 flex-1 px-1.5` (sin padding arriba: el header ya separa; abajo lo da StatusBar); cada `<Panel>` envuelve su contenido en `<Island>` (`h-full overflow-hidden rounded-xl border border-border bg-panel`); el Panel center NO es isla (contiene el PanelGroup vertical con dos islas: editor/git y bottom) |
| `src/components/layout/Island.tsx` (NUEVO) | wrapper trivial con `className` extra opcional |
| `src/components/layout/ResizeHandle.tsx` | `w-1.5`/`h-1.5` `bg-transparent`; línea interior 1px centrada (`absolute` `w-px`/`h-px`) `bg-transparent group-hover:bg-accent group-data-[resize-handle-state=drag]:bg-accent transition-colors`; zona de agarre mantener |
| `src/components/layout/AppHeader.tsx` | `h-11 bg-canvas` sin border-b, `data-tauri-drag-region` en header y contenedores; grid 3 columnas (`grid-cols-[1fr_auto_1fr]`) para centrar de verdad el trigger. Izquierda: padding mac, BrandMark + pill proyecto (`bg-elevated hover:bg-elevated-hover rounded-lg px-2.5 py-1` con nombre; onClick → `useProjects.getState().openScreen()`, sustituye al IconButton Projects; `Plus` New Project queda como IconButton pequeño), `BranchChip` como pill igual + `CenterSwitch` como segmented pill (`bg-elevated rounded-lg p-0.5`, activo `bg-white/[0.08] text-fg`). Centro: `<QuickOpenTrigger/>`. Derecha: `RunToolbar` |
| `src/components/layout/QuickOpenTrigger.tsx` (NUEVO) | `<button>` `w-72 h-7 bg-quick border border-border rounded-lg text-xs text-muted` con icono Search + texto "Buscar archivos, símbolos..." + `<Kbd>⌘P</Kbd>` (en no-mac `Ctrl P`); onClick `useUi.getState().openModal('quickOpen')`; oculto si no hay `root` |
| `src/components/run/RunToolbar.tsx` | contenedor `bg-elevated border border-border-strong rounded-lg p-0.5 flex items-center gap-0.5`; dropdown (RunDropdown trigger) sin borde propio, `rounded-md px-2 h-7 hover:bg-hover` con TechIcon + nombre; Run `text-run-text hover:bg-run/15 rounded-md p-1.5`; Stop `text-stop-text hover:bg-stop/15 rounded-md p-1.5`, `disabled:opacity-40` cuando no corre. Quitar `bg-[#dc2626]`/`bg-accent` sólidos. Lógica intacta |
| `src/components/run/RunDropdown.tsx` | solo clases del trigger y del menú (menú: `bg-elevated border-border-strong rounded-lg shadow-xl p-1`, items `rounded-md`) |
| `src/components/layout/StatusBar.tsx` | (mínimo en A) `bg-canvas` sin `border-t`; resto en C |
| `src-tauri/tauri.conf.json` | §0.2 |
| `src-tauri/capabilities/default.json` | §0.2 |
| `src/components/projects/ProjectsScreen.tsx` | header: padding mac + drag region (solo eso en A para no tapar semáforos) |

Criterios A: tsc/build/test verdes; header 44 px; trigger abre QuickOpen; Run/Stop funcionan igual; handles invisibles; capabilities válidas (`npm run tauri dev` arranca sin error de ACL).

---

## Bloque B — Sidebar/árbol + pestañas + breadcrumbs + tema CM6

| Archivo | Cambio |
|---|---|
| `src/components/sidebar/Sidebar.tsx` | quitar `bg-panel` (lo da la isla) y `border-b`; header `h-9 px-3` título `text-[11px] uppercase tracking-wider text-muted`; botones Nuevo archivo / Nueva carpeta / Colapsar todo como `IconButton` `rounded-md p-1 hover:bg-white/[0.06]` (si "Colapsar todo" no existe en el store, buscar acción existente con `grep -rn collapse src/store`; si no hay, NO crearla en el store: omitir botón y anotarlo) |
| `src/components/sidebar/FileTree.tsx` | contenedor `py-1`; nada de lógica |
| `src/components/sidebar/FileTreeNode.tsx` | fila: `mx-1.5 h-7 rounded-md px-2` (el indent sigue con `paddingLeft: depth*INDENT + 8`); seleccionado `bg-accent/12 text-accent-text font-medium` + indicador `before:` absoluto `left-0.5 top-1.5 bottom-1.5 w-[3px] rounded-full bg-accent` (fila `relative`); hover `hover:bg-hover`; guías: los hijos se renderizan dentro de un `<div className="relative">` con una línea `absolute top-0 bottom-0 border-l border-white/[0.05]` en `left: depth*INDENT + 14` (un div por nivel, NO por fila, para no multiplicar nodos). `NewEntryRow` mismas medidas. Nombre compacto `entry.compact?.name` ya muestra `com.example.app`: mantener, solo asegurar `truncate` y `title` |
| `src/components/sidebar/InlineNameInput.tsx` | `bg-elevated border-border-strong rounded-md` |
| `src/components/editor/TabBar.tsx` | `h-10 items-center px-1 bg-panel` sin `border-b` dura (opcional `border-b border-border`); pestaña `rounded-lg px-3 py-1.5 my-1 mx-0.5 border border-transparent text-muted hover:bg-hover`; activa `bg-tab-active text-fg-strong border-border-strong` + punto `h-1.5 w-1.5 rounded-full bg-accent` (si la pestaña está dirty, el punto dirty existente toma ese rol; no duplicar). Scroll horizontal fino (hereda scrollbar global). Lógica de menú/cierre intacta |
| `src/components/editor/Breadcrumbs.tsx` (NUEVO) | lee `useEditor(s=>s.active)` y `useWorkspace(s=>s.root)`; ruta relativa con `src/lib/paths` (reutilizar helpers existentes); segmentos `text-[11px] text-faint` separados por `›` (`ChevronRight size=10`), último `text-muted`; `px-4 py-1 h-6 truncate`. Clic: si `src/store/editor.ts` expone ya una función de reveal (grep `reveal`), usarla para revelar la carpeta en el árbol; si no, segmentos no clicables (no tocar stores). Oculto si no hay pestaña activa |
| `src/components/editor/EditorArea.tsx` | `TabBar` → `Breadcrumbs` → editor; `bg-base` (=`#111114`) |
| `src/components/editor/theme.ts` | renombrar a "Caesar Obsidian" (`obsidianTheme`, alias `graphiteTheme`); ver B.4 |
| `src/components/editor/CodeEditor.tsx`, `extensions.ts`, `git/DiffViewer.tsx` | solo si se cambia el nombre del export |

### B.4 Tema CM6 "Caesar Obsidian" (`theme.ts`, colores desde `palette.syntax`)
- Base: `&` bg `palette.panel` (#111114), `fontSize 13.5px`; `.cm-scroller` `lineHeight 1.65`, fuente MONO; gutters bg `#111114`, color `#3f3f46`, sin borderRight; `.cm-activeLine` `rgba(255,255,255,0.03)`; `.cm-activeLineGutter` transparente color accent; panels/search/tooltip/autocomplete → `palette.elevated` + borde `rgba(255,255,255,0.08)` + `borderRadius 8px` (inputs del search `6px`); `.cm-tooltip-autocomplete li[aria-selected]` `rgba(245,158,11,0.12)`.
- `palette.syntax = { keyword:'#f59e0b', annotation:'#fcd34d', type:'#7dd3fc', string:'#6ee7b7', func:'#c4b5fd', variable:'#e4e4e7', comment:'#52525b', number:'#fdba74', operator:'#8e8e98', heading:'#f4f4f6', link:'#f59e0b', invalid:'#e11d48' }`.
- HighlightStyle (tags `@lezer/highlight` que emiten los parsers de lang-java/python/javascript(TS)/rust/json/yaml/markdown):
  - keyword: `t.keyword, t.controlKeyword, t.definitionKeyword, t.moduleKeyword, t.operatorKeyword, t.modifier, t.self, t.bool, t.null` (bool/null opcional en number)
  - annotation/decorator: `t.annotation, t.meta, t.attributeName` (Java `@GetMapping` = `t.annotation`; Python decorator = `t.meta`; Rust `#[derive]` = `t.attributeName`/`t.meta`; YAML anchors/tags `t.labelName`)
  - type/class: `t.typeName, t.className, t.namespace, t.definition(t.typeName), t.standard(t.typeName)`
  - claves JSON/YAML: `t.definition(t.propertyName)` = type (#7dd3fc). No mapear `t.propertyName` a type (en Java/TS es acceso a miembro → variable). Si lang-json/lang-yaml emiten las claves como `propertyName` a secas y quedan grises, aceptarlo o comprobar en el navegador el tag real antes de ajustar.
  - string: `t.string, t.special(t.string), t.character, t.regexp, t.escape`
  - function/method: `t.function(t.variableName), t.function(t.propertyName), t.function(t.definition(t.variableName)), t.macroName`
  - variable/property: `t.variableName, t.definition(t.variableName), t.propertyName, t.local(t.variableName)`
  - comment: `t.comment, t.lineComment, t.blockComment, t.docComment` italic
  - number: `t.number, t.integer, t.float, t.atom`
  - operator/punct: `t.operator, t.punctuation, t.separator, t.bracket, t.derefOperator`
  - markdown: `t.heading` heading bold, `t.strong` bold, `t.emphasis` italic, `t.link, t.url` link underline, `t.monospace` string, `t.quote` comment, `t.list` keyword
  - `t.invalid` invalid
  - Orden: tags más específicos (`t.function(...)`, `t.definition(...)`) ANTES de los genéricos no importa en HighlightStyle (gana la regla más específica del árbol de tags), pero no repetir el mismo tag en dos reglas.

Criterios B: tsc/build/test; carpeta Java `src/main/java/com/example/app` sin archivos intermedios se ve como una fila `com.example.app`; selección pill sin franja de lado a lado; breadcrumbs cambian al cambiar de pestaña; abrir un .java/.py/.ts/.rs/.json/.yaml/.md y comprobar colores de keyword/anotación/tipo/string/método/comentario.

---

## Bloque C — Panel inferior + logs + statusbar + Git/modales/proyectos

| Archivo | Cambio |
|---|---|
| `src/components/bottom/BottomPanel.tsx` | barra `h-9 px-2` sin `border-b`; tabs (Services/Logs/Terminal + run tabs) como segmented: contenedor `bg-elevated/60 rounded-lg p-0.5` o pills sueltas `rounded-md px-2.5 py-1 text-xs`, activa `bg-white/[0.08] text-white` (token: `bg-hover-strong` = `rgb(255 255 255/0.08)` y `text-fg-strong`); `STATE_DOT` running → `bg-run`, error → `bg-stop` |
| `src/components/bottom/ServicesPanel.tsx` | lista `py-1.5`; quitar `bg-base` (isla) ; toolbar superior mismos botones `rounded-md` |
| `src/components/bottom/ServiceRow.tsx` | card `mx-2 mb-1.5 p-2.5 rounded-lg bg-card hover:bg-card-hover border border-border`; badge estado pill `rounded-full px-2 py-0.5 text-[11px]`: running `bg-run/15 text-run-text` + punto `animate-pulse bg-run`, stopped `bg-zinc-800 text-muted` (token `bg-elevated-hover`), error `bg-stop/15 text-stop-text`; puerto `font-mono text-[11px] bg-white/[0.05] px-1.5 py-0.5 rounded`; botones Play/Restart/Trash `h-7 w-7 rounded-md` (área táctil ≥28 px) con hover tintado (run/accent/stop) |
| `src/components/bottom/ServiceModal.tsx` | hereda de Modal; inputs → TextInput/Select ya reestilados |
| `src/components/bottom/LogViewer.tsx` | barra filtros `h-10 px-2 gap-2` sin `border-b` dura; `bg-base`→ sin fondo propio (isla); filtro texto: `TextInput` con clases `h-7 rounded-lg bg-elevated border-border-strong px-2.5 text-xs`; nivel/servicio con `Select`. Filas virtualizadas: SOLO cambiar colores (`hover:bg-hover`), sin tocar altura/estimateSize ni añadir transiciones |
| `src/components/bottom/TerminalPanel.tsx`, `TerminalView.tsx` | fondo xterm `palette.base` (#111114) — ya importan palette: actualizar `theme.background`/`selection` |
| `src/components/ui/Select.tsx` | trigger `h-7 rounded-lg bg-elevated border border-border-strong px-2.5 text-xs hover:bg-elevated-hover focus:border-accent/60`; menú `bg-elevated border-border-strong rounded-lg shadow-xl p-1`, opción `rounded-md px-2 h-7`, seleccionada `bg-accent/12 text-accent-text` |
| `src/components/ui/TextInput.tsx` | `h-7 rounded-lg bg-elevated border border-border-strong px-2.5 focus:border-accent/60` |
| `src/components/ui/Button.tsx`, `IconButton.tsx`, `Badge.tsx`, `Kbd.tsx` | `rounded-md`, hover `bg-hover`; variant primary = `bg-accent/15 text-accent-text hover:bg-accent/25` (no bloques sólidos marrones); danger = `bg-stop/15 text-stop-text` |
| `src/components/ui/Modal.tsx` | overlay `bg-black/60 backdrop-blur-sm`; caja `rounded-xl bg-elevated border border-border-strong shadow-2xl`; header sin borde duro (`px-4 pt-3 pb-2`) |
| `src/components/ui/ContextMenu.tsx`, `Toasts.tsx`, `ConfirmHost.tsx`, `EnvEditor.tsx` | superficies `elevated` + `rounded-lg` + items `rounded-md` |
| `src/components/palette/QuickOpen.tsx`, `GlobalSearch.tsx` | caja `rounded-xl bg-elevated border-border-strong`, input sin borde interno, item activo `bg-accent/12` |
| `src/components/layout/StatusBar.tsx` | `h-6 px-3 text-[11px] text-faint bg-canvas` sin border; separadores `gap-3`; errores `text-stop-text`, warnings `text-accent` |
| `src/components/git/*` (GitView, RefsSidebar, ChangesPanel, CommitGraph, CommitDetail, CommitForm, DiffViewer, FileHistory, RemoteBar, ConflictBanner, BranchDialogs) | GitView ocupa la isla editor: quitar `bg-base`/`bg-panel` exteriores; sus subpaneles internos con separadores `border-border` (no islas anidadas, salvo si GitView usa su propio PanelGroup: entonces usar `ResizeHandle` ya reestilado y fondos `bg-panel`); filas seleccionadas `bg-accent/12`; inputs/commit textarea `bg-elevated rounded-lg`; ConflictBanner `bg-stop/10 border-stop/30 rounded-lg mx-2`. CommitGraph: NO tocar SVG/virtualizer, solo colores de fila y `laneColors` intactos |
| `src/components/projects/ProjectsScreen.tsx`, `RecentList.tsx`, `NewProjectWizard.tsx`, `GenerationLog.tsx`, `ToolWarning.tsx`, `forms/*` | fondo `bg-canvas`; contenido central como isla (`rounded-xl bg-panel border-border p-6`); cards de recientes `bg-card hover:bg-card-hover rounded-lg`; wizard en Modal/isla elevada; quitar `border-crimson-dark`. RecentList.test.ts no depende de clases: no romper exports |
| `src/components/layout/WelcomeView.tsx` | tokens nuevos |
| barrido final | `grep -rnE 'bg-\[#|text-\[#|border-\[#|#[0-9a-fA-F]{6}' src/components` → migrar a tokens (excepciones §0.5) |

Criterios C: tsc/build/test; filtros de logs sin `<select>` nativo; logs con 10k líneas fluidos (sin transiciones por fila); vista Git, Projects y Wizard con mismas superficies que el resto; barrido de hex limpio.

---

## Prompts autocontenidos para coders (Sonnet, en secuencia)

### Prompt A
> Proyecto Tauri v2 + React + Tailwind en `/Users/cesargarrido/Proyectos/caesarCode`. Lee `SPEC4.md` y `PLAN4.md` (§0 completo y "Bloque A"). Implementa SOLO el Bloque A: tokens en `tailwind.config.js`, `src/lib/palette.ts` (incluye ya `syntax` de §B.4) y vars `--cc-*` en `src/index.css` según la tabla §0.1 (remapea `base/panel/border/hover/fg/muted/crimson`, añade `canvas, elevated, elevated-hover, card, card-hover, tab-active, quick, border-strong, fg-strong, faint, run, stop, accent-soft, accent-text, run-text, stop-text`); scrollbars 6 px; `src/lib/platform.ts`; `Island.tsx`; `AppLayout` con canvas `#09090b` + islas `rounded-xl` dentro de cada Panel (no `gap` en PanelGroup); `ResizeHandle` transparente con línea ámbar en hover/drag; `AppHeader` h-11 unificado (pill proyecto, pill rama, segmented Code/Git, `QuickOpenTrigger.tsx` que llama `useUi.getState().openModal('quickOpen')`, RunToolbar en pill); reestilo de `RunToolbar`/`RunDropdown`; `StatusBar` solo `bg-canvas` sin borde; ventana: `tauri.conf.json` `titleBarStyle: "Overlay"`, `hiddenTitle: true`, `backgroundColor: "#09090b"`, capabilities `core:window:allow-start-dragging` y `core:window:allow-internal-toggle-maximize`; `data-tauri-drag-region` en header y contenedores no interactivos; padding izquierdo 78 px en macOS (también en el header de `ProjectsScreen`). Sustituye los 5 usos `border-border/xx|bg-base/xx|bg-panel/xx` que se rompen con colores rgba. No toques `src/store/**`, `src/lib/ipc*`, ni Rust (excepto los dos JSON). No hex nuevos en componentes. Al terminar: `npx tsc --noEmit`, `npm run build`, `npm test` en verde. Devuelve lista de archivos tocados y cualquier desviación del plan.

### Prompt B
> Proyecto `/Users/cesargarrido/Proyectos/caesarCode`. El Bloque A de `PLAN4.md` ya está hecho (tokens nuevos en tailwind/palette/index.css, incluido `palette.syntax`). Lee `SPEC4.md` §3–4 y `PLAN4.md` §0 + "Bloque B". Implementa SOLO el Bloque B: `Sidebar.tsx` header limpio con IconButtons `rounded-md p-1` (Colapsar todo solo si ya existe acción en el store; no crear lógica nueva en stores); `FileTreeNode.tsx` filas pill `mx-1.5 h-7 rounded-md px-2`, seleccionado `bg-accent/12 text-accent-text font-medium` con indicador lateral redondeado (pseudo-elemento), guías de indentación con UNA línea `border-white/[0.05]` por contenedor de hijos (no por fila); nodos compactos (`entry.compact`) ya funcionan: solo estilo; `InlineNameInput` elevated; `TabBar.tsx` h-10 con pestañas pill y activa `bg-tab-active text-fg-strong border-border-strong` + punto ámbar (sin duplicar el indicador dirty); nuevo `src/components/editor/Breadcrumbs.tsx` (ruta relativa al root, `text-[11px] text-faint px-4 py-1`, separador `›`, clicable solo si existe una función de reveal ya expuesta en `src/store/editor.ts`), montado en `EditorArea.tsx` bajo las pestañas; `theme.ts` → tema "Caesar Obsidian" (`obsidianTheme`, mantener alias `graphiteTheme` exportado) con colores de `palette.syntax` y el mapeo de tags `@lezer/highlight` de PLAN4 §B.4 (Java anotaciones `t.annotation`, Python decorators `t.meta`, Rust atributos, JSON/YAML keys, Markdown), fondo #111114, línea activa rgba(255,255,255,0.03), 13.5px / line-height 1.65, tooltips/search/autocomplete en superficie elevated rounded. Comentario en theme.ts documentando que el gris de comentarios (#52525b) es excepción WCAG pedida por el SPEC. No toques stores, IPC ni Rust. Verifica `npx tsc --noEmit`, `npm run build`, `npm test`. Devuelve archivos tocados y desviaciones.

### Prompt C
> Proyecto `/Users/cesargarrido/Proyectos/caesarCode`. Bloques A y B de `PLAN4.md` ya hechos. Lee `SPEC4.md` §5 y `PLAN4.md` §0 + "Bloque C". Implementa SOLO el Bloque C: `BottomPanel` con pestañas segmented/pills (`rounded-md px-2.5 py-1 text-xs`, activa `bg-white/[0.08]` vía token); `ServicesPanel` + `ServiceRow` como cards (`mx-2 mb-1.5 p-2.5 rounded-lg bg-card hover:bg-card-hover border-border`), badge de estado pill (running con punto `animate-pulse`), puerto en badge mono, botones `h-7 w-7 rounded-md`; `LogViewer` filtros con `TextInput`/`Select` reestilados (`h-7 rounded-lg bg-elevated border-border-strong text-xs`), filas virtualizadas solo cambio de color (no tocar altura/estimateSize ni añadir transiciones/sombras por fila); xterm con fondo `palette.base`; `ui/Select`, `TextInput`, `Button`, `IconButton`, `Badge`, `Kbd`, `Modal`, `ContextMenu`, `Toasts`, `ConfirmHost`, `EnvEditor`, `QuickOpen`, `GlobalSearch` sobre superficies `elevated` `rounded-lg/xl` (primario = `bg-accent/15 text-accent-text`, nunca bloques sólidos); `StatusBar` `h-6 px-3 text-[11px] text-faint bg-canvas` sin borde; vista Git (`src/components/git/*`) dentro de la isla sin fondos/bordes duros propios, selección `bg-accent/12`, inputs elevated, sin tocar SVG ni virtualizer de `CommitGraph` ni `laneColors`; `ProjectsScreen`/`RecentList`/`NewProjectWizard`/`GenerationLog`/`ToolWarning`/`forms/*`/`WelcomeView` con canvas + isla + cards (sin `border-crimson-dark`). Barrido final: `grep -rnE 'bg-\[#|text-\[#|border-\[#|#[0-9a-fA-F]{6}' src/components` debe quedar solo en iconos de marca (`TechIcon`, `FileIcon`/registry). No toques stores, IPC ni Rust. Verifica `npx tsc --noEmit`, `npm run build`, `npm test` y `git diff --stat src/store src/lib/ipc* src-tauri/src` vacío. Devuelve archivos tocados y desviaciones.
