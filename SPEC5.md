# SPEC5 — Multi-proyecto en una ventana + selector Git + iconos @react-symbols/icons

## Parte 1 (petición del usuario, textual resumida)
En una sola ventana puede haber MUCHOS proyectos/microservicios (estilo IntelliJ: varios módulos/proyectos adjuntos a un mismo workspace). El código es fácil de abrir (cualquier archivo), pero en Git el usuario quiere:
- Un **selector de repositorio Git** entre todos los repos presentes en el workspace (cada microservicio/proyecto suele ser su propio repo) para elegir sobre cuál operar (grafo, cambios, commit, ramas, push/pull…).
- Poder **juntar varios microservicios o proyectos en el IDE** como en IntelliJ: añadir carpetas/proyectos al workspace (attach/add module), verlos todos juntos en el árbol (cada raíz como nodo de nivel superior), quitarlos, y que servicios/run configs/búsqueda/Quick Open/LSP funcionen sobre todos ellos. Persistir el conjunto de raíces del workspace (p. ej. `.caesar/workspace.json` o app data) y recordarlo entre sesiones.
- Los repos Git deben detectarse automáticamente (raíz de cada proyecto y subcarpetas con `.git`, profundidad razonable, monorepos) y mostrar en el selector nombre + rama + estado (cambios, ahead/behind). Opcional: vista "All repos" con estado resumido y acciones masivas (fetch all, pull all).
- Badges Git del árbol deben funcionar por repo (cada raíz con su propio estado).

## Parte 2 — Integración de `@react-symbols/icons` en toda la app
Instalar `@react-symbols/icons` y sustituir TODOS los iconos de carpetas, archivos, lenguajes y ecosistemas de microservicios (manteniendo `lucide-react` solo para controles de UI: Play, Stop, Chevrons, Close, Search, Trash, etc.).

### Paso 1: instalación e inspección real de exports (OBLIGATORIO)
`npm install @react-symbols/icons`. ANTES de escribir código inspeccionar los `.d.ts` en `node_modules/@react-symbols/icons` y listar los nombres exactos de TODOS los componentes exportados (archivos, carpetas, variantes abiertas/coloreadas). Usar solo exports verificados.

### Paso 2: resolver centralizado `src/components/icons/SymbolIcon.tsx`
1. `<FileIcon filename path? className? />`: mapea por nombre exacto primero, luego patrones, luego extensión. Cubre: Java/Spring/JVM (`pom.xml`, `build.gradle(.kts)`, `mvnw`, `gradlew`, `*Application.java`, `.java`, `.kt`, `.kts`, `.jar`, `.properties`); Python (`pyproject.toml`, `requirements.txt`, `Pipfile`, `poetry.lock`, `uv.lock`, `manage.py`, `.py`, `.pyi`, `.ipynb`); Node/TS/Web (`package.json`, `tsconfig.json`, `vite.config.*`, `next.config.*`, `tailwind.config.*`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lockb`, `.ts`, `.tsx`, `.mts`, `.js`, `.jsx`, `.mjs`, `.html`, `.css`, `.scss`, `.svg`); Rust (`Cargo.toml`, `Cargo.lock`, `.rs`); Go (`go.mod`, `go.sum`, `.go`); Config/Infra/Docs/Git (`Dockerfile`, `docker-compose.yml`, `.dockerignore`, `.gitignore`, `.gitattributes`, `.env`, `.env.*`, `README.md`, `LICENSE`, `Makefile`, `caesar.json`, `.json`, `.yml`, `.yaml`, `.xml`, `.toml`, `.ini`, `.sql`, `.graphql`, `.proto`, `.sh`, `.zsh`, `.md`, `.txt`, `.c`, `.cpp`, `.h`). Fallback: icono genérico de archivo del paquete.
   (Mantener además la distinción de símbolos Java/Kotlin ya existente — class/interface/enum/record/annotation vía `fs_classify` — si el paquete no la cubre: superponer o elegir el icono del paquete por símbolo cuando exista, y conservar el SVG propio como fallback para esos casos.)
2. `<FolderIcon name path? isOpen isModule? className? />`: carpetas semánticas con los iconos de carpeta específicos del paquete (variantes abiertas si existen, si no ajustar opacidad/brillo): código fuente (`src`, `app`, `lib`, `pkg`, `cmd`, `internal`, `core`, `main`, `java`); paquetes/arquitectura (`controller(s)`, `service(s)`, `model(s)`, `entity`, `repository/repositories`, `dto`, `auth`, `shared`, `utils`, `hooks`, `components`, `store`, `pages`, `routes`, `api`); tests (`test`, `tests`, `__tests__`, `spec`, `e2e`); config/infra/recursos (`.caesar`, `.mvn`, `.github`, `.vscode`, `config`, `resources`, `assets`, `public`, `infra`, `docker`, `k8s`, `terraform`, `scripts`, `migrations`, `prisma`); docs (`docs`, `colecciones`); excluidas (`target`, `node_modules`, `.idea`, `.git`, `dist`, `build`, `out`, `.venv`, `venv`, `__pycache__`, `.next`, `coverage`) → `opacity-55` y nombre `text-zinc-500`; módulos/raíz de microservicio (`isModule`) destacados en ámbar/oro.
3. `<TechIcon tech={'spring'|'java'|'python'|'node'|'typescript'|'go'|'rust'|'docker'|'shell'|'other'} className? />`.

### Paso 3: aplicar en TODA la app
1. File Tree (todas las carpetas incluidas las compactadas `com.example.finance_tracker`, y archivos) `w-4 h-4 shrink-0`.
2. Pestañas del editor y breadcrumbs.
3. Quick Open y Búsqueda Global (icono por resultado).
4. Run Widget y modal Run Configurations (TechIcon o FileIcon en modo Current File).
5. Panel Services: TechIcon en cabeceras de grupo y cada card.

### Paso 4: ajuste visual
- File Tree: selección activa → píldora neutra `bg-white/[0.08] text-white rounded-md` con indicador lateral de 2px ámbar `#f59e0b` (no marrón).
- Panel Services: botón `All Services` → pill limpio `bg-white/[0.06] text-zinc-200 rounded-lg` (no barra marrón sólida).

Verificar al terminar: `tsc --noEmit` y build de Vite sin errores ni imports rotos.
