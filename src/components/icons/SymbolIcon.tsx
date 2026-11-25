/**
 * Resolver centralizado de iconos (paquete `@react-symbols/icons@1.4.1`, componentes `FC<SVGProps<SVGSVGElement>>`).
 *
 * Exports verificados en los `.d.ts` de node_modules/@react-symbols/icons/build:
 *  - `@react-symbols/icons/files`:
 *      Bun, CLang, Cplus, Csharp, Dart, Database, Docker, Document, EditorConfig, Gear, Git,
 *      Go, GoMod, Gradle, Graphql, H, Ignore, Image, Java, Js, Kotlin, License, Lock, Markdown, NPM, Next, Notebook,
 *      PHP, PNPM, Proto, Python, Reactjs, Reactts, Ruby, Rust, SVG, Shell, Sass, Swift, Tailwind, Text, Tsconfig,
 *      TsTest, TypeScript, Vite, Vue, XML, Yaml, Yarn, BracketsYellow, CodeOrange, CodeSky, Compressed.
 *  - `@react-symbols/icons/folders`:
 *      Folder, FolderOpen, FolderApp, FolderAssets, FolderAuth, FolderBlue, FolderBlueCode, FolderBlueOutline,
 *      FolderBuild, FolderConfig, FolderCore, FolderDatabase, FolderDocker, FolderDocuments, FolderGithub,
 *      FolderGray, FolderGrayCode, FolderGrayOutline, FolderGreenCode, FolderGreenOutline, FolderHooks,
 *      FolderInterfaces, FolderLayout, FolderModels, FolderNodeModules, FolderOrange, FolderOrangeOutline, FolderPipes,
 *      FolderPrisma, FolderPurple, FolderPurpleCode, FolderPurpleOutline, FolderReducer, FolderRouter, FolderServices,
 *      FolderShared, FolderSky, FolderSkyCode, FolderSkyOutline, FolderSrc, FolderTarget, FolderUtils, FolderVSCode,
 *      FolderYellowCode.
 *  - `@react-symbols/icons/utils`: DefaultFileIcon (fallback de archivo).
 *
 * Sin icono específico en el paquete (se usa el más cercano): pom/jar/Spring (sin icono de Maven/Spring/jar:
 * XML / Compressed / Java), html (CodeOrange), css (CodeSky), json (BracketsYellow), toml/ini/properties/env/caesar
 * (Gear), makefile (Gear), requirements (Text), poetry/uv (Lock), k8s (FolderBlueCode).
 * Tecnologías: Spring/Java -> Java; Python; Node -> Js (Node del paquete no tiene verde distinguible en 16px);
 * TypeScript; Go; Rust; Docker; Shell; other -> Gear.
 */
import type { ComponentType, SVGProps } from 'react';
import {
  Bun, BracketsYellow, CLang, Cplus, Csharp, CodeOrange, CodeSky, Compressed, Dart, Database, Docker, EditorConfig,
  Gear, Git, Go, GoMod, Gradle, Graphql, H, Ignore, Image, Java, Js, Kotlin, License, Lock, Markdown, NPM, Next,
  Notebook, PHP, PNPM, Proto, Python, Reactjs, Reactts, Ruby, Rust, SVG, Sass, Shell, Swift, Tailwind, Text,
  Tsconfig, TsTest, TypeScript, Vite, Vue, XML, Yaml, Yarn,
} from '@react-symbols/icons/files';
import {
  Folder, FolderApp, FolderAssets, FolderAuth, FolderBlue, FolderBlueCode, FolderBlueOutline, FolderBuild,
  FolderConfig, FolderCore, FolderDatabase, FolderDocker, FolderDocuments, FolderGithub, FolderGray,
  FolderGrayCode, FolderGrayOutline, FolderGreenCode, FolderGreenOutline, FolderHooks,
  FolderInterfaces, FolderLayout, FolderModels, FolderNodeModules, FolderOpen, FolderOrange, FolderOrangeOutline,
  FolderPipes, FolderPrisma, FolderPurple, FolderPurpleCode, FolderPurpleOutline, FolderReducer, FolderRouter,
  FolderServices, FolderShared, FolderSky, FolderSkyCode, FolderSkyOutline, FolderSrc, FolderTarget, FolderUtils,
  FolderVSCode, FolderYellowCode,
} from '@react-symbols/icons/folders';
import { DefaultFileIcon } from '@react-symbols/icons/utils';
import {
  fileIconKey, folderIconName, isExcludedFolderName, type FileIconName, type FolderIconName,
} from '../../lib/fileIconRegistry';
import { extOf } from '../../lib/paths';
import type { FolderStyle } from '../../lib/classifyFolder';
import type { SymbolInfo } from '../../lib/types';
import { useSettings } from '../../store/settings';
import { JvmSymbolIcon } from './JvmSymbolIcon';
import { MATERIAL_FILE, MATERIAL_FOLDER } from './materialMap';
import { INTELLIJ_FILE } from './intellij/files';
import { intellijFolder } from './intellij/folders';

type Icon = ComponentType<SVGProps<SVGSVGElement>>;

const DEFAULT_SIZE = 'h-4 w-4 shrink-0';
const MATERIAL_URLS = import.meta.glob('../../assets/icons/material/*.svg', {
  eager: true, query: '?url', import: 'default',
}) as Record<string, string>;

/** URL del SVG Material por nombre (sin extensión). */
function materialUrl(stem: string): string | undefined {
  return MATERIAL_URLS[`../../assets/icons/material/${stem}.svg`];
}

function MaterialImg({ stem, className, extra }: { stem: string; className?: string; extra?: string }) {
  return <img src={materialUrl(stem)} alt="" aria-hidden draggable={false} className={cx('shrink-0', className ?? 'h-4 w-4', extra)} />;
}

const cx = (...p: (string | false | null | undefined)[]) => p.filter(Boolean).join(' ');

const FILE_ICONS: Record<FileIconName, Icon> = {
  pom: XML, gradle: Gradle, mvnw: Shell, mvnwCmd: Shell, helpMd: Markdown, springApp: Java, java: Java, kotlin: Kotlin, jar: Compressed,
  properties: Gear, springConfig: Yaml,
  pyproject: Python, requirements: Text, pipfile: Python, poetry: Lock, uv: Lock, djangoManage: Python,
  python: Python, notebook: Notebook,
  packageJson: NPM, tsconfig: Tsconfig, vite: Vite, next: Next, tailwind: Tailwind, pnpm: PNPM, yarn: Yarn, bun: Bun,
  typescript: TypeScript, react: Reactjs, reactTs: Reactts, javascript: Js, html: CodeOrange, css: CodeSky,
  scss: Sass, svg: SVG,
  cargo: Rust, rust: Rust, gomod: GoMod, go: Go,
  docker: Docker, dockerignore: Ignore, git: Git, env: Gear, readme: Markdown, license: License, makefile: Gear,
  caesar: Gear, editorconfig: EditorConfig,
  json: BracketsYellow, yaml: Yaml, xml: XML, toml: Gear, ini: Gear, sql: Database, graphql: Graphql, proto: Proto,
  shell: Shell, markdown: Markdown, text: Text,
  c: CLang, cpp: Cplus, h: H, csharp: Csharp, ruby: Ruby, php: PHP, swift: Swift, vue: Vue, dart: Dart,
  image: Image, lock: Lock, testFile: TsTest, generic: DefaultFileIcon,
};

const FOLDER_ICONS: Record<FolderIconName, Icon> = {
  src: FolderSrc, app: FolderApp, lib: FolderSkyCode, core: FolderCore, java: FolderSkyCode,
  controllers: FolderRouter, services: FolderServices, models: FolderModels, entity: FolderModels,
  repositories: FolderDatabase, dto: FolderInterfaces, auth: FolderAuth, shared: FolderShared, utils: FolderUtils,
  hooks: FolderHooks, components: FolderPurpleCode, store: FolderReducer, pages: FolderLayout, routes: FolderRouter,
  api: FolderPipes,
  tests: FolderGreenCode, collections: FolderGrayOutline, caesar: FolderPurple, mvn: FolderOrangeOutline, github: FolderGithub, vscode: FolderVSCode,
  config: FolderConfig, resources: FolderOrange, assets: FolderAssets, public: FolderSky,
  infra: FolderSkyOutline, docker: FolderDocker, k8s: FolderBlueCode, terraform: FolderPurpleOutline,
  scripts: FolderYellowCode, migrations: FolderDatabase, prisma: FolderPrisma, docs: FolderDocuments,
  target: FolderTarget, nodeModules: FolderNodeModules, ide: FolderGray, gitDir: FolderGrayOutline,
  build: FolderBuild, venv: FolderGrayCode,
  // Roles de classifyFolder.
  plain: Folder, sourceRoot: FolderSrc, testRoot: FolderGreenCode, resourcesRoot: FolderOrange,
  testResourcesRoot: FolderGreenOutline, generatedRoot: FolderBlueOutline, excluded: FolderGray, module: FolderOrange,
  package: FolderBlue,
};

export interface FileIconProps {
  filename: string;
  path?: string;
  className?: string;
  /** Símbolo Java/Kotlin clasificado por Rust: si existe, se usan los círculos C/I/E/R/@ propios. */
  symbol?: SymbolInfo | null;
}

/** Icono de archivo: símbolo JVM > nombre exacto > patrón > extensión > genérico. */
export function FileIcon({ filename, className, symbol }: FileIconProps) {
  const key = fileIconKey(filename, extOf(filename), symbol);
  const pack = useSettings((s) => s.iconPack);
  if (key.startsWith('sym:') && symbol) return <JvmSymbolIcon symbol={symbol} className={className ?? DEFAULT_SIZE} />;
  if (pack === 'intellij') {
    const Own = INTELLIJ_FILE[key as FileIconName];
    if (Own) return <Own aria-hidden focusable="false" className={cx('shrink-0', className ?? 'h-4 w-4')} />;
  }
  if (pack !== 'symbols') return <MaterialImg stem={MATERIAL_FILE[key as FileIconName] ?? 'file'} className={className} />;
  const Icon = FILE_ICONS[key as FileIconName] ?? DefaultFileIcon;
  return <Icon aria-hidden focusable="false" className={cx('shrink-0', className ?? 'h-4 w-4')} />;
}

export interface FolderIconProps {
  name: string;
  path?: string;
  isOpen?: boolean;
  /** Módulo / raíz de microservicio: resaltado ámbar. */
  isModule?: boolean;
  /** Estilo de `classifyFolder` (rol/clase) para carpetas sin nombre conocido. */
  style?: Pick<FolderStyle, 'cls' | 'role'> | null;
  className?: string;
}

/** Icono de carpeta semántica. El paquete solo trae variante abierta genérica: el resto sube brillo al abrirse. */
export function FolderIcon({ name, isOpen = false, isModule = false, style, className }: FolderIconProps) {
  const key = folderIconName(name, style);
  const excluded = isExcludedFolderName(key);
  const pack = useSettings((s) => s.iconPack);
  const extra = cx(isOpen && key !== 'plain' && pack === 'symbols' && 'brightness-125', excluded && 'opacity-55');
  let icon: JSX.Element;
  if (pack === 'material') {
    const base = MATERIAL_FOLDER[key] ?? 'base';
    icon = <MaterialImg stem={`folder-${base}${isOpen ? '-open' : ''}`} className={className} extra={excluded ? 'opacity-55' : undefined} />;
  } else if (pack === 'intellij') {
    const Own = intellijFolder(key, isOpen);
    icon = <Own aria-hidden focusable="false" className={cx('shrink-0', className ?? 'h-4 w-4', excluded && 'opacity-55')} />;
  } else {
      const Base = FOLDER_ICONS[key] ?? Folder;
    const Icon = isOpen && key === 'plain' ? FolderOpen : Base;
    icon = <Icon aria-hidden focusable="false" className={cx('shrink-0', className ?? 'h-4 w-4', extra)} />;
  }
  if (!isModule) return icon;
  return <span className="inline-flex shrink-0 rounded-sm text-accent drop-shadow-[0_0_3px_currentColor]">{icon}</span>;
}

export type TechName = 'spring' | 'java' | 'python' | 'node' | 'typescript' | 'go' | 'rust' | 'docker' | 'shell' | 'other';

const TECH_ICONS: Record<TechName, Icon> = {
  spring: Java, java: Java, python: Python, node: Js, typescript: TypeScript, go: Go, rust: Rust, docker: Docker,
  shell: Shell, other: Gear,
};

const TECH_MATERIAL: Record<TechName, string> = {
  spring: 'java', java: 'java', python: 'python', node: 'npm', typescript: 'typescript', go: 'go', rust: 'rust',
  docker: 'docker', shell: 'console', other: 'settings',
};

/** Icono de ecosistema (Services, Run). */
export function TechIcon({ tech, className }: { tech: TechName; className?: string }) {
  const pack = useSettings((s) => s.iconPack);
  if (pack !== 'symbols') return <MaterialImg stem={TECH_MATERIAL[tech] ?? 'settings'} className={className} />;
  const Icon = TECH_ICONS[tech] ?? Gear;
  return <Icon aria-hidden focusable="false" className={cx('shrink-0', className ?? 'h-4 w-4')} />;
}
