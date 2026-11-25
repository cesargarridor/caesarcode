import type { FolderStyle } from './classifyFolder';
import type { Stereotype, SymbolInfo, SymbolKind } from './types';

/** Extensiones cuyo contenido clasifica Rust (`fs_classify`). */
export const JVM_EXTS: ReadonlySet<string> = new Set(['java', 'kt', 'kts', 'groovy', 'scala']);

export type FileIconName =
  | 'pom' | 'gradle' | 'mvnw' | 'mvnwCmd' | 'helpMd' | 'springApp' | 'java' | 'kotlin' | 'jar' | 'properties' | 'springConfig'
  | 'pyproject' | 'requirements' | 'pipfile' | 'poetry' | 'uv' | 'djangoManage' | 'python' | 'notebook'
  | 'packageJson' | 'tsconfig' | 'vite' | 'next' | 'tailwind' | 'pnpm' | 'yarn' | 'bun'
  | 'typescript' | 'react' | 'reactTs' | 'javascript' | 'html' | 'css' | 'scss' | 'svg'
  | 'cargo' | 'rust' | 'gomod' | 'go'
  | 'docker' | 'dockerignore' | 'git' | 'env' | 'readme' | 'license' | 'makefile' | 'caesar' | 'editorconfig'
  | 'json' | 'yaml' | 'xml' | 'toml' | 'ini' | 'sql' | 'graphql' | 'proto' | 'shell' | 'markdown' | 'text'
  | 'c' | 'cpp' | 'h' | 'csharp' | 'ruby' | 'php' | 'swift' | 'vue' | 'dart'
  | 'image' | 'lock' | 'testFile' | 'generic';
export type IconKey = FileIconName | `sym:${SymbolKind}`;

const EXT_ICON: Record<string, FileIconName> = {
  java: 'java', groovy: 'java', scala: 'java', kt: 'kotlin', kts: 'kotlin', jar: 'jar', war: 'jar',
  properties: 'properties', gradle: 'gradle',
  py: 'python', pyi: 'python', ipynb: 'notebook',
  ts: 'typescript', mts: 'typescript', cts: 'typescript', tsx: 'reactTs', jsx: 'react',
  js: 'javascript', mjs: 'javascript', cjs: 'javascript',
  html: 'html', htm: 'html', xhtml: 'html', css: 'css', scss: 'scss', sass: 'scss', less: 'css', svg: 'svg',
  rs: 'rust', go: 'go',
  json: 'json', jsonc: 'json', yml: 'yaml', yaml: 'yaml', xml: 'xml', xsd: 'xml', xsl: 'xml', toml: 'toml',
  ini: 'ini', conf: 'ini', cfg: 'ini', sql: 'sql', graphql: 'graphql', gql: 'graphql', proto: 'proto',
  sh: 'shell', bash: 'shell', zsh: 'shell', md: 'markdown', mdx: 'markdown', txt: 'text', log: 'text',
  c: 'c', cpp: 'cpp', cc: 'cpp', hpp: 'h', h: 'h', cs: 'csharp', rb: 'ruby', php: 'php', swift: 'swift',
  vue: 'vue', dart: 'dart', lock: 'lock',
  png: 'image', jpg: 'image', jpeg: 'image', gif: 'image', webp: 'image', ico: 'image', bmp: 'image', icns: 'image',
};

/** Nombre exacto (minúsculas) -> clave. */
const NAME_ICON: Record<string, FileIconName> = {
  'pom.xml': 'pom', mvnw: 'mvnw', 'mvnw.cmd': 'mvnwCmd', 'help.md': 'helpMd', gradlew: 'mvnw', 'gradlew.bat': 'mvnwCmd',
  'build.gradle': 'gradle', 'build.gradle.kts': 'gradle', 'settings.gradle': 'gradle', 'settings.gradle.kts': 'gradle',
  'pyproject.toml': 'pyproject', 'requirements.txt': 'requirements', pipfile: 'pipfile', 'poetry.lock': 'poetry',
  'uv.lock': 'uv', 'manage.py': 'djangoManage',
  'package.json': 'packageJson', 'package-lock.json': 'packageJson', 'tsconfig.json': 'tsconfig',
  'pnpm-lock.yaml': 'pnpm', 'yarn.lock': 'yarn', 'bun.lockb': 'bun', 'bun.lock': 'bun',
  'cargo.toml': 'cargo', 'cargo.lock': 'cargo', 'go.mod': 'gomod', 'go.sum': 'gomod',
  dockerfile: 'docker', '.dockerignore': 'dockerignore',
  '.gitignore': 'git', '.gitattributes': 'git', '.gitmodules': 'git', '.env': 'env',
  'readme.md': 'readme', readme: 'readme', license: 'license', 'license.md': 'license', 'license.txt': 'license',
  makefile: 'makefile', 'caesar.json': 'caesar', '.editorconfig': 'editorconfig',
};

const TEST_FILE = /(?:[._-](?:test|spec)\.[a-z0-9]+$)|(?:^test_.+\.py$)|(?:_test\.(?:go|py|rs)$)/;

/**
 * Clave del icono de un archivo. Pura.
 * Orden: símbolo JVM > nombre exacto > patrón > extensión > `generic`.
 */
export function fileIconKey(name: string, ext: string | null, symbol?: SymbolInfo | null): IconKey {
  if (symbol && symbol.kind !== 'unknown') return `sym:${symbol.kind}`;
  const n = name.toLowerCase();
  const exact = NAME_ICON[n];
  if (exact) return exact;
  // patrones
  if (/^dockerfile\./.test(n) || /^(docker-)?compose(\.[\w-]+)?\.ya?ml$/.test(n)) return 'docker';
  if (/^vite\.config\./.test(n)) return 'vite';
  if (/^next\.config\./.test(n)) return 'next';
  if (/^tailwind\.config\./.test(n)) return 'tailwind';
  if (/^(application|bootstrap)([-.][\w-]+)?\.(ya?ml|properties)$/.test(n)) return 'springConfig';
  if (n === '.env' || n.startsWith('.env.')) return 'env';
  if (/^readme\./.test(n)) return 'readme';
  if (/^licen[sc]e[.-]/.test(n)) return 'license';
  if (/application\.java$/.test(n)) return 'springApp';
  if (TEST_FILE.test(n) && !(ext && JVM_EXTS.has(ext))) return 'testFile';
  return (ext && EXT_ICON[ext]) || 'generic';
}

export type SymbolOverlay = 'springRun' | 'run' | 'spring' | 'test';

/** Overlay (esquina inferior derecha) según el estereotipo. */
export function stereotypeOverlay(st: Stereotype | null | undefined): SymbolOverlay | null {
  switch (st) {
    case 'springBootApp': return 'springRun';
    case 'main': return 'run';
    case 'test': return 'test';
    case 'controller': case 'service': case 'repository': case 'configuration': case 'entity': case 'component':
      return 'spring';
    default: return null;
  }
}

const KIND_LABEL: Record<SymbolKind, string> = {
  class: 'Clase', abstractClass: 'Clase abstracta', interface: 'Interfaz', enum: 'Enum', record: 'Record',
  annotation: 'Anotación', exception: 'Excepción', object: 'Objeto', dataClass: 'Data class',
  sealedClass: 'Sealed class', kotlinFile: 'Archivo Kotlin', script: 'Script', unknown: '',
};
const STEREOTYPE_LABEL: Record<Stereotype, string> = {
  springBootApp: '@SpringBootApplication', main: 'main()', controller: '@RestController', service: '@Service',
  repository: '@Repository', configuration: '@Configuration', entity: '@Entity', component: '@Component', test: 'test',
};

/** Tooltip: `Clase · @RestController`. Vacío si no hay símbolo. */
export function symbolLabel(symbol?: SymbolInfo | null): string {
  if (!symbol) return '';
  return [KIND_LABEL[symbol.kind], symbol.stereotype ? STEREOTYPE_LABEL[symbol.stereotype] : ''].filter(Boolean).join(' · ');
}

export type FolderIconKey =
  | 'plain' | 'sourceRoot' | 'testRoot' | 'resourcesRoot' | 'testResourcesRoot' | 'generatedRoot' | 'excluded' | 'module'
  | 'package' | 'infra';

/** Clave por rol/clase de `classifyFolder` (el rol de Rust gana a la heurística). */
export function folderIconKey(folder?: Pick<FolderStyle, 'cls' | 'role'> | null): FolderIconKey {
  if (!folder) return 'plain';
  if (folder.role) return folder.role === 'package' ? 'package' : folder.role;
  switch (folder.cls) {
    case 'source': return 'sourceRoot';
    case 'test': return 'testRoot';
    case 'config': return 'resourcesRoot';
    case 'excluded': return 'excluded';
    case 'module': return 'module';
    case 'infra': return 'infra';
    default: return 'plain';
  }
}

/** Claves de carpeta semántica (por nombre) más las de rol (`FolderIconKey`). */
export type FolderIconName =
  | 'src' | 'app' | 'lib' | 'core' | 'java'
  | 'controllers' | 'services' | 'models' | 'entity' | 'repositories' | 'dto' | 'auth' | 'shared' | 'utils'
  | 'hooks' | 'components' | 'store' | 'pages' | 'routes' | 'api'
  | 'tests' | 'collections' | 'caesar' | 'mvn' | 'github' | 'vscode' | 'config' | 'resources' | 'assets' | 'public'
  | 'infra' | 'docker' | 'k8s' | 'terraform' | 'scripts' | 'migrations' | 'prisma' | 'docs'
  | 'target' | 'nodeModules' | 'ide' | 'gitDir' | 'build' | 'venv'
  | FolderIconKey;

const FOLDER_NAME: Record<string, FolderIconName> = {
  src: 'src', app: 'app', lib: 'lib', pkg: 'lib', cmd: 'lib', internal: 'lib', core: 'core', main: 'src', java: 'java',
  controller: 'controllers', controllers: 'controllers', service: 'services', services: 'services',
  model: 'models', models: 'models', entity: 'entity', entities: 'entity', repository: 'repositories',
  repositories: 'repositories', dto: 'dto', auth: 'auth', shared: 'shared', utils: 'utils', util: 'utils',
  hooks: 'hooks', components: 'components', store: 'store', pages: 'pages', routes: 'routes', api: 'api',
  test: 'tests', tests: 'tests', __tests__: 'tests', spec: 'tests', e2e: 'tests',
  '.caesar': 'caesar', '.mvn': 'mvn', '.github': 'github', '.vscode': 'vscode', config: 'config',
  resources: 'resources', assets: 'assets', public: 'public', infra: 'infra', docker: 'docker', k8s: 'k8s',
  terraform: 'terraform', scripts: 'scripts', migrations: 'migrations', prisma: 'prisma',
  docs: 'docs', colecciones: 'collections',
  target: 'target', node_modules: 'nodeModules', '.idea': 'ide', '.git': 'gitDir', dist: 'build', build: 'build',
  out: 'build', '.venv': 'venv', venv: 'venv', __pycache__: 'venv', '.next': 'build', coverage: 'build',
};

/** Clave del icono de carpeta. Pura. El nombre gana al rol; sin nombre conocido, el rol/clase de `classifyFolder`. */
export function folderIconName(name: string, style?: Pick<FolderStyle, 'cls' | 'role'> | null): FolderIconName {
  const byName = FOLDER_NAME[name.toLowerCase()];
  if (byName) return byName;
  return folderIconKey(style);
}

/** Carpetas excluidas (se atenúan). */
export function isExcludedFolderName(name: FolderIconName): boolean {
  return name === 'excluded' || name === 'target' || name === 'nodeModules' || name === 'ide' || name === 'gitDir'
    || name === 'build' || name === 'venv';
}
