# SPEC4 — Rediseño visual "Modern Islands UI" (IntelliJ New UI + Zed + Linear)

Rediseñar la capa visual (CSS/Tailwind + componentes React + tema CodeMirror 6) SIN romper la lógica ni los stores Zustand. De layout plano de cajas cuadradas a interfaz moderna, redondeada, con aire.

## 1. Superficies "Islands" y paleta
- Canvas base (espacio entre paneles): `#09090b`, con `p-1.5` y `gap-1.5` entre paneles.
- Paneles/Islas (Sidebar, Editor, Services): `#111114`, `rounded-xl` (12px), borde `1px solid rgba(255,255,255,0.06)`.
- Superficies elevadas (cards, dropdowns, inputs, modales): `#18181c`, `rounded-lg` (8px), borde `1px solid rgba(255,255,255,0.08)`.
- Hover sutil: `rgba(255,255,255,0.05)`.
- Acentos: principal oro/ámbar `#f59e0b` con moderación y fondos suaves (`bg-amber-500/12 text-amber-300`, nunca bloques sólidos marrones); Run/Running esmeralda `#10b981` (`bg-emerald-500/15 text-emerald-400`); Stop/Error/marca carmesí `#e11d48`.
- Tipografía UI: `-apple-system, BlinkMacSystemFont, "Inter", sans-serif`, antialiased, 12.5–13px, texto `#ececef`, secundario `#8e8e98`. Código: `"JetBrains Mono","Geist Mono",ui-monospace,monospace` 13.5px, line-height 1.65.

## 2. Unified Toolbar (`h-11`, 44px), sin borde inferior duro, sobre el canvas `#09090b`
- Izquierda: icono CaesarCode + selector de proyecto en pill (`bg-[#18181c] hover:bg-[#222228] rounded-lg px-2.5 py-1`) con nombre del workspace; pill de rama Git al lado (icono rama + `main`).
- Centro: trigger de Quick Open centrado (`⌘P  Buscar archivos, símbolos...`), `w-72 bg-[#141418] border border-white/[0.06] rounded-lg text-xs text-zinc-400` (abre el Quick Open real).
- Derecha (Run Widget): contenedor pill `bg-[#18181c] border border-white/[0.07] rounded-lg p-0.5 flex items-center gap-0.5`: dropdown con icono del lenguaje + nombre de la config activa; botón Run (Play esmeralda `text-emerald-400 hover:bg-emerald-500/15 rounded-md p-1.5`); botón Stop (visible/activo cuando corre) `text-rose-400 hover:bg-rose-500/15 rounded-md p-1.5`.
- Debe seguir siendo zona de arrastre de ventana (data-tauri-drag-region) y respetar los botones de semáforo de macOS si la ventana usa overlay.

## 3. Sidebar y árbol
- Compact Middle Packages (CRÍTICO Java/Spring): carpeta sin archivos con una única subcarpeta se compacta en un nodo (`com.example.finance_tracker`). (Ya existe `FsEntry.compact` en Rust: reutilizar y asegurar que se ve así.)
- Filas tipo pill: `mx-1.5 px-2 h-7 rounded-md` (28px). Seleccionado: `bg-amber-500/12 text-amber-200 font-medium` con indicador lateral redondeado sutil; nunca franja rectangular de lado a lado.
- Guías de indentación `border-l border-white/[0.05]`.
- Header del panel limpio con botones (Nuevo archivo, Nueva carpeta, Colapsar todo) `rounded-md p-1 hover:bg-white/[0.06]`.

## 4. Editor y pestañas
- Barra de pestañas `h-10`: pestañas pill `rounded-lg px-3 py-1.5 my-1 mx-0.5`; activa: fondo `#1b1b20`, texto `#f4f4f6`, borde `border-white/[0.08]` y punto/acento oro `#f59e0b`.
- Breadcrumbs discretos bajo las pestañas (`src › main › java › ... › UserController.java`, `text-[11px] text-zinc-500 px-4 py-1`), clicables si es viable.
- Tema CodeMirror 6 "Caesar Obsidian": fondo `#111114`; línea activa `rgba(255,255,255,0.03)`; keywords `#f59e0b` (o coral `#fb7185`); anotaciones/decoradores `#fcd34d`; tipos y clases `#7dd3fc`; strings `#6ee7b7`; métodos/funciones `#c4b5fd`; variables/propiedades `#e4e4e7`; comentarios `#52525b` cursiva.

## 5. Panel inferior
- Pestañas (Services, Logs, Terminal) como segmented control/pills `rounded-md px-2.5 py-1 text-xs`, activa `bg-white/[0.08] text-white`.
- Cards de servicio: `rounded-lg bg-[#16161b] hover:bg-[#1c1c22] border border-white/[0.06] p-2.5 mx-2 mb-1.5`; badge estado pill: Running `bg-emerald-500/15 text-emerald-400` con punto pulsante / Stopped `bg-zinc-800 text-zinc-400`; puerto en badge mono (`:8030`, `bg-white/[0.05] px-1.5 py-0.5 rounded text-[11px]`); botones Play/Restart/Trash con buen tamaño táctil `rounded-md`.
- Filtros de logs: inputs/selectores custom (nada de `<select>` nativo) `rounded-lg bg-[#18181c] border border-white/[0.07] h-7 px-2.5 text-xs`.
- StatusBar: `h-6 px-3 text-[11px] text-zinc-500`, integrada sobre el canvas `#09090b` sin bordes pesados.

## Reglas
- Handles de `react-resizable-panels`: invisibles en reposo (transparentes, `w-1.5`/`h-1.5` sobre el canvas) y solo línea fina ámbar `#f59e0b` en hover/arrastre.
- Sin scrollbars nativos gruesos: finos (6px), redondeados y translúcidos.
- Al terminar: `tsc --noEmit` (y `npm run build`) sin errores; tests vitest siguen pasando; no romper lógica ni stores.
- Los colores deben vivir en `src/lib/palette.ts` / tokens Tailwind / vars `--cc-*` (fuente única), no hardcodeados por componente.
