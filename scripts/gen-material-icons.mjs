// Copia el subconjunto curado de iconos de `material-icon-theme` (devDependency) a src/assets/icons/material/
// y genera src/components/icons/materialMap.ts. Uso: node scripts/gen-material-icons.mjs
import { copyFileSync, existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const pkg = join(root, 'node_modules', 'material-icon-theme');
const src = join(pkg, 'icons');
const dst = join(root, 'src', 'assets', 'icons', 'material');

/** FileIconName -> icono de archivo. */
const FILES = {
  pom: 'maven', gradle: 'gradle', mvnw: 'console', mvnwCmd: 'console', helpMd: 'markdown', springApp: 'java',
  java: 'java', kotlin: 'kotlin', jar: 'jar', properties: 'settings', springConfig: 'yaml',
  pyproject: 'python-misc', requirements: 'python-misc', pipfile: 'python-misc', poetry: 'poetry', uv: 'uv',
  djangoManage: 'python', python: 'python', notebook: 'jupyter',
  packageJson: 'npm', tsconfig: 'tsconfig', vite: 'vite', next: 'next', tailwind: 'tailwindcss', pnpm: 'pnpm',
  yarn: 'yarn', bun: 'bun', typescript: 'typescript', react: 'react', reactTs: 'react_ts', javascript: 'javascript',
  html: 'html', css: 'css', scss: 'sass', svg: 'svg',
  cargo: 'rust', rust: 'rust', gomod: 'go-mod', go: 'go',
  docker: 'docker', dockerignore: 'docker', git: 'git', env: 'tune', readme: 'readme', license: 'license',
  makefile: 'makefile', caesar: 'settings', editorconfig: 'editorconfig',
  json: 'json', yaml: 'yaml', xml: 'xml', toml: 'toml', ini: 'settings', sql: 'database', graphql: 'graphql',
  proto: 'proto', shell: 'console', markdown: 'markdown', text: 'document',
  c: 'c', cpp: 'cpp', h: 'h', csharp: 'csharp', ruby: 'ruby', php: 'php', swift: 'swift', vue: 'vue', dart: 'dart',
  image: 'image', lock: 'lock', testFile: 'test-ts', generic: 'file',
};

/** FolderIconName -> base de carpeta (`folder-<base>` y `folder-<base>-open`). */
const FOLDERS = {
  src: 'src', app: 'app', lib: 'lib', core: 'core', java: 'java', controllers: 'controller', services: 'server',
  models: 'class', entity: 'class', repositories: 'repository', dto: 'interface', auth: 'secure', shared: 'shared',
  utils: 'utils', hooks: 'hook', components: 'components', store: 'store', pages: 'views', routes: 'routes', api: 'api',
  tests: 'test', collections: 'other', caesar: 'project', mvn: 'tools', github: 'github', vscode: 'vscode',
  config: 'config', resources: 'resource', assets: 'images', public: 'public', infra: 'cluster', docker: 'docker',
  k8s: 'kubernetes', terraform: 'terraform', scripts: 'scripts', migrations: 'migrations', prisma: 'prisma',
  docs: 'docs', target: 'target', nodeModules: 'node', ide: 'intellij', gitDir: 'git', build: 'dist', venv: 'python',
  plain: 'base', sourceRoot: 'src', testRoot: 'test', resourcesRoot: 'resource', testResourcesRoot: 'test',
  generatedRoot: 'generator', excluded: 'temp', module: 'project', package: 'base',
};

const needed = new Set();
for (const s of Object.values(FILES)) needed.add(s);
for (const b of Object.values(FOLDERS)) { needed.add(`folder-${b}`); needed.add(`folder-${b}-open`); }

if (!existsSync(src)) { console.error('Falta material-icon-theme: npm install'); process.exit(1); }
const missing = [...needed].filter((n) => !existsSync(join(src, `${n}.svg`)));
if (missing.length) { console.error('Iconos inexistentes:', missing.join(', ')); process.exit(1); }

rmSync(dst, { recursive: true, force: true });
mkdirSync(dst, { recursive: true });
for (const n of [...needed].sort()) copyFileSync(join(src, `${n}.svg`), join(dst, `${n}.svg`));
copyFileSync(join(pkg, 'LICENSE'), join(dst, 'LICENSE'));

const version = JSON.parse((await import('node:fs')).readFileSync(join(pkg, 'package.json'), 'utf8')).version;
const obj = (o) => Object.entries(o).map(([k, v]) => `  ${k}: '${v}',`).join('\n');
const out = `// GENERADO por scripts/gen-material-icons.mjs (material-icon-theme ${version}, MIT). No editar a mano.
import type { FileIconName, FolderIconName } from '../../lib/fileIconRegistry';

/** Archivo -> nombre del SVG en src/assets/icons/material (sin extensión). */
export const MATERIAL_FILE: Record<FileIconName, string> = {
${obj(FILES)}
};

/** Carpeta -> base; los SVG son \`folder-<base>\` y \`folder-<base>-open\`. */
export const MATERIAL_FOLDER: Record<FolderIconName, string> = {
${obj(FOLDERS)}
};
`;
writeFileSync(join(root, 'src', 'components', 'icons', 'materialMap.ts'), out);
console.log(`${needed.size} iconos copiados a ${dst}`);
