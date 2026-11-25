import {
  Folder,
  FolderCheck,
  FolderCode,
  FolderCog,
  FolderGit2,
  FolderKanban,
  FolderOpen,
  Package,
  type LucideIcon,
} from 'lucide-react';
import { palette } from './palette';
import { ICON_COLORS } from './iconColors';
import type { FolderRole } from './types';

export type FolderClass = 'module' | 'source' | 'test' | 'config' | 'infra' | 'excluded' | 'package' | 'generated' | 'plain';

export interface FolderStyle {
  cls: FolderClass;
  color: string;
  icon: LucideIcon;
  iconOpen: LucideIcon;
  badge?: string;
  /** Rol calculado por Rust (gana a la heurística por nombre). */
  role?: FolderRole;
  dimName: boolean;
  collapsedByDefault: boolean;
}

export interface ClassifyOpts { isRoot?: boolean; module?: string | null; role?: FolderRole | null }

const EXCLUDED = new Set([
  'node_modules', 'target', 'dist', 'build', 'out', '.venv', 'venv', '__pycache__', '.next', '.git', '.idea', 'coverage',
]);
const SOURCE = new Set(['src', 'app', 'lib', 'pkg', 'cmd', 'internal', 'core']);
const TEST = new Set(['test', 'tests', '__tests__', 'spec', 'e2e', 'integration']);
const CONFIG = new Set(['config', 'resources', 'assets', '.caesar', '.vscode']);
const INFRA = new Set(['.github', 'docker', 'k8s', 'helm', 'infra', 'terraform', 'scripts', 'migrations']);

/** Archivo marcador -> clave de módulo. */
const MARKERS: Record<string, string> = {
  'pom.xml': 'maven',
  'build.gradle': 'gradle',
  'build.gradle.kts': 'gradle',
  'package.json': 'node',
  'cargo.toml': 'cargo',
  'go.mod': 'go',
  'pyproject.toml': 'python',
  'requirements.txt': 'python',
  'setup.py': 'python',
};
const BADGES: Record<string, string> = {
  maven: 'mvn', gradle: 'gradle', node: 'npm', cargo: 'cargo', go: 'go', python: 'py',
};

function moduleFromChildren(children?: readonly string[] | null): string | null {
  if (!children) return null;
  for (const c of children) {
    const m = MARKERS[c.toLowerCase()];
    if (m) return m;
  }
  return null;
}

const cache = new Map<string, FolderStyle>();

function compute(name: string, children: readonly string[] | null | undefined, opts: ClassifyOpts): FolderStyle {
  const n = name.toLowerCase();
  const mk = (
    cls: FolderClass, color: string, icon: LucideIcon, extra: Partial<FolderStyle> = {},
  ): FolderStyle => ({ cls, color, icon, iconOpen: icon, dimName: false, collapsedByDefault: false, ...extra });

  const role = opts.role ?? undefined;
  if (role === 'excluded') {
    return mk('excluded', palette.excluded, Folder, { iconOpen: FolderOpen, dimName: true, collapsedByDefault: true, role });
  }
  if (role && role !== 'module') {
    const byRole: Record<Exclude<FolderRole, 'excluded' | 'module'>, [FolderClass, string]> = {
      sourceRoot: ['source', ICON_COLORS.sourceRoot],
      testRoot: ['test', ICON_COLORS.testRoot],
      resourcesRoot: ['config', ICON_COLORS.resources],
      testResourcesRoot: ['config', ICON_COLORS.testResources],
      generatedRoot: ['generated', ICON_COLORS.generated],
      package: ['package', ICON_COLORS.package],
    };
    const [cls, color] = byRole[role];
    return mk(cls, color, cls === 'package' ? Package : Folder, { iconOpen: FolderOpen, role });
  }
  if (EXCLUDED.has(n)) {
    return mk('excluded', palette.excluded, Folder, { iconOpen: FolderOpen, dimName: true, collapsedByDefault: true });
  }
  const mod = opts.module || moduleFromChildren(children);
  if (mod) {
    return mk('module', palette.accent, opts.isRoot ? FolderGit2 : Package, { badge: BADGES[mod] ?? mod, role });
  }
  if (TEST.has(n)) return mk('test', palette.green, FolderCheck);
  if (SOURCE.has(n)) return mk('source', palette.sky, FolderCode);
  if (CONFIG.has(n)) return mk('config', palette.warm, FolderCog);
  if (INFRA.has(n)) return mk('infra', palette.violet, FolderKanban);
  return mk('plain', '#c8932e', Folder, { iconOpen: FolderOpen });
}

/** Clasifica una carpeta. Pura y memoizada por path + entradas que influyen. */
export function classifyFolder(
  name: string,
  path: string,
  children?: readonly string[] | null,
  opts: ClassifyOpts = {},
): FolderStyle {
  const mod = opts.module || moduleFromChildren(children) || '';
  const key = `${path}\u0000${name}\u0000${mod}\u0000${opts.isRoot ? 1 : 0}\u0000${opts.role ?? ''}`;
  const hit = cache.get(key);
  if (hit) return hit;
  const style = compute(name, children, opts);
  if (cache.size > 5000) cache.clear();
  cache.set(key, style);
  return style;
}
