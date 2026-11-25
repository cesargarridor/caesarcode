# CaesarCode

Entorno de desarrollo nativo, ligero y local-first (Tauri 2 + React + Rust): editor CodeMirror 6 con LSP,
microservicios locales con logs y métricas, terminal integrada. Ver `SPEC.md` y `PLAN.md`.

## Requisitos

- Node 20+ y Rust estable (`rustup`).
- `git` en el PATH (badges y rama en la barra de estado).
- Servidores LSP, todos opcionales (sin ellos el editor funciona igual y la barra de estado muestra `○ LSP no instalado`):

```
npm i -g pyright typescript-language-server typescript   # Python y TypeScript/JavaScript
rustup component add rust-analyzer                       # Rust
brew install jdtls                                       # Java (requiere JDK 21+)
```

La app hereda el PATH de tu shell de login, así que basta con que el servidor esté instalado.

## Desarrollo

```
npm install
npm run tauri dev      # app de escritorio
npm run tauri build    # bundle de producción
npm run build          # tsc -b && vite build
(cd src-tauri && cargo check && cargo test && cargo clippy --all-targets -- -D warnings)
```

## Atajos

| Atajo | Acción |
|---|---|
| Cmd/Ctrl+P | Abrir archivo (fuzzy) |
| Cmd/Ctrl+Shift+F | Búsqueda global |
| Cmd/Ctrl+S | Guardar |
| Cmd/Ctrl+W | Cerrar pestaña |
| Cmd/Ctrl+Shift+[ / ] | Pestaña anterior / siguiente |
| Cmd/Ctrl+F | Buscar y reemplazar en el archivo |
| Cmd/Ctrl+D | Siguiente coincidencia (multicursor); Alt+clic añade cursor |
| F12, Cmd/Ctrl+clic | Ir a la definición (LSP) |
| Ctrl+Espacio | Autocompletado |
