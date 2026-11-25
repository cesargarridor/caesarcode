import { useGit } from './git';
import { create } from 'zustand';
import { open } from '@tauri-apps/plugin-dialog';
import {
  fsCreateDir,
  fsCreateFile,
  fsDelete,
  fsReadDir,
  fsRename,
  fsWatchSubscribe,
  gitStatus,
  workspaceAddRoot,
  workspaceOpen,
  workspaceRemoveRoot,
  workspaceRoots,
} from '../lib/ipc';
import { toMessage } from '../lib/errors';
import { basename, dirname, isSameOrInside, repoFor } from '../lib/paths';
import type { FsEntry, FsEvent, GitStatus, RootInfo } from '../lib/types';
import { useEditor } from './editor';
import { useLsp } from './lsp';
import { useRunConfigStore } from './runConfigs';
import { useServices } from './services';
import { isClassifiable, useSymbols } from './symbols';
import { useUi } from './ui';

const LAST_WORKSPACE_KEY = 'cc.lastWorkspace';
const NO_GIT: GitStatus = { isRepo: false, branch: null, files: {} };
/** Con más raíces que esto, solo la primaria arranca expandida. */
const MANY_ROOTS = 5;

export type InlineEdit =
  | { type: 'new-file' | 'new-dir'; parent: string }
  | { type: 'rename'; path: string };

interface WorkspaceStore {
  /** Raíz PRIMARIA (identidad del workspace). */
  root: string | null;
  name: string | null;
  /** Todas las raíces (la primera es la primaria). */
  roots: RootInfo[];
  /** Estado git por repo (toplevel). */
  gitByRepo: Record<string, GitStatus>;
  children: Record<string, FsEntry[]>;
  expanded: Record<string, true>;
  selected: string | null;
  /** Agregado (unión de `files` de todos los repos) para badges; `branch` = la del repo de la primaria. */
  git: GitStatus;
  inlineEdit: InlineEdit | null;

  openWorkspace: (path: string) => Promise<void>;
  addRoot: (path: string) => Promise<boolean>;
  removeRoot: (path: string) => Promise<void>;
  refreshRoots: () => Promise<void>;
  toggleDir: (path: string) => void;
  expandDir: (path: string) => void;
  collapseAll: () => void;
  select: (path: string | null) => void;
  refreshDir: (path: string, silent?: boolean) => Promise<void>;
  /** `repo` = solo ese repo; sin argumento = todos. */
  refreshGit: (repo?: string | null) => void;
  handleFsEvent: (ev: FsEvent) => void;
  startInlineEdit: (edit: InlineEdit | null) => void;
  createEntry: (parent: string, name: string, dir: boolean) => Promise<void>;
  renameEntry: (path: string, newName: string) => Promise<void>;
  deleteEntry: (entry: { path: string; name: string; isDir: boolean }) => Promise<void>;
}

function sameListing(a: FsEntry[] | undefined, b: FsEntry[]): boolean {
  if (!a || a.length !== b.length) return false;
  return a.every(
    (e, i) =>
      e.path === b[i].path &&
      e.isDir === b[i].isDir &&
      e.size === b[i].size &&
      e.isSymlink === b[i].isSymlink &&
      (e.role ?? null) === (b[i].role ?? null) &&
      (e.compact?.path ?? null) === (b[i].compact?.path ?? null),
  );
}

/** Directorio cuyo listado contiene `dir`: él mismo, o (si está dentro de una cadena compactada) el padre del nodo compacto. */
function owningDir(children: Record<string, FsEntry[]>, dir: string, compacts: Array<[string, FsEntry]>): string | null {
  if (children[dir]) return dir;
  for (const [parent, e] of compacts) if (isSameOrInside(e.path, dir)) return parent;
  return null;
}

function collectCompacts(children: Record<string, FsEntry[]>): Array<[string, FsEntry]> {
  const out: Array<[string, FsEntry]> = [];
  for (const [parent, list] of Object.entries(children)) for (const e of list) if (e.compact) out.push([parent, e]);
  return out;
}

let gitTimer: ReturnType<typeof setTimeout> | null = null;
let gitToken = 0;
let openToken = 0;
/** Repos pendientes de refrescar; `null` = todos. */
let gitPending: Set<string> | null = new Set();

/** Estado agregado para badges: unión de `files`; rama del repo de la primaria. */
function aggregateGit(by: Record<string, GitStatus>, primary: string | null, repos: { top: string; root: string }[]): GitStatus {
  const tops = Object.keys(by);
  if (tops.length === 0) return NO_GIT;
  const files: GitStatus['files'] = {};
  for (const t of tops) Object.assign(files, by[t].files);
  const own = primary ? (repoFor(repos, primary) ?? repos.find((r) => r.root === primary) ?? null) : null;
  const main = (own && by[own.top]) ?? by[tops[0]];
  return { isRepo: tops.some((t) => by[t].isRepo), branch: main.branch, files };
}

function expandedFor(roots: RootInfo[], primary: string): Record<string, true> {
  const out: Record<string, true> = {};
  for (const r of roots) if (r.exists && (roots.length <= MANY_ROOTS || r.path === primary)) out[r.path] = true;
  return out;
}

/** Diálogo nativo de carpeta + abrir workspace (descarta o confirma cambios sin guardar). */
export async function openFolderDialog(): Promise<void> {
  try {
    const sel = await open({ directory: true, multiple: false, title: 'Abrir carpeta' });
    if (typeof sel === 'string') await useWorkspace.getState().openWorkspace(sel);
  } catch (e) {
    useUi.getState().pushToast(toMessage(e));
  }
}

/** Diálogo nativo (multi-selección) para añadir carpetas al workspace actual (estilo "Attach" de IntelliJ). */
export async function addRootDialog(): Promise<number> {
  let added = 0;
  try {
    const sel = await open({ directory: true, multiple: true, title: 'Añadir proyecto al workspace' });
    const paths = Array.isArray(sel) ? sel : typeof sel === 'string' ? [sel] : [];
    for (const p of paths) if (await useWorkspace.getState().addRoot(p)) added++;
  } catch (e) {
    useUi.getState().pushToast(toMessage(e));
  }
  return added;
}

export const useWorkspace = create<WorkspaceStore>((set, get) => ({
  root: null,
  name: null,
  roots: [],
  gitByRepo: {},
  children: {},
  expanded: {},
  selected: null,
  git: NO_GIT,
  inlineEdit: null,

  openWorkspace: async (path) => {
    const editor = useEditor.getState();
    if (editor.tabs.some((t) => t.dirty)) {
      const choice = await useUi.getState().ask(
        'Cambios sin guardar',
        'Hay archivos con cambios sin guardar. Si abres otra carpeta se perderán.',
        [
          { label: 'Cancelar', value: 'cancel' },
          { label: 'Descartar y abrir', value: 'discard', variant: 'danger' },
        ],
      );
      if (choice !== 'discard') return;
    }
    const token = ++openToken;
    const info = await workspaceOpen(path);
    if (token !== openToken) return; // llegó otra apertura después
    useEditor.getState().closeAll();
    useServices.getState().reset();
    useRunConfigStore.getState().reset();
    useLsp.getState().reset();
    useSymbols.getState().reset();
    useGit.getState().reset();
    gitPending = new Set();
    set({
      root: info.root,
      name: info.name,
      roots: info.roots,
      gitByRepo: {},
      children: {},
      expanded: expandedFor(info.roots, info.root),
      selected: null,
      git: NO_GIT,
      inlineEdit: null,
    });
    try {
      localStorage.setItem(LAST_WORKSPACE_KEY, info.root);
    } catch {
      /* almacenamiento no disponible */
    }
    // Raíces colapsadas no piden `fs_read_dir`; se cargan al expandirlas.
    await Promise.all(info.roots.filter((r) => r.exists && get().expanded[r.path]).map((r) => get().refreshDir(r.path)));
    try {
      await fsWatchSubscribe((ev) => get().handleFsEvent(ev));
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    }
    void useGit.getState().init(); // detecta repos y luego pide el estado de cada uno (`refreshGit`)
    void useServices.getState().init();
    void useRunConfigStore.getState().init();
  },

  addRoot: async (path) => {
    try {
      const info = await workspaceAddRoot(path);
      const before = new Set(get().roots.map((r) => r.path));
      const added = info.roots.find((r) => !before.has(r.path));
      set((s) => ({
        roots: info.roots,
        expanded: added && info.roots.length <= MANY_ROOTS ? { ...s.expanded, [added.path]: true } : s.expanded,
      }));
      if (added) {
        if (get().expanded[added.path]) await get().refreshDir(added.path);
        void useServices.getState().loadRoot(added.path);
        void useRunConfigStore.getState().loadRoot(added.path);
      }
      void useGit.getState().loadRepos();
      return true;
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
      return false;
    }
  },

  removeRoot: async (path) => {
    const { roots } = get();
    if (roots[0]?.path === path) {
      useUi.getState().pushToast('La raíz principal no se puede quitar; abre otra carpeta.', 'info');
      return;
    }
    const dirty = useEditor.getState().tabs.filter((t) => t.dirty && isSameOrInside(path, t.path));
    if (dirty.length > 0) {
      const choice = await useUi.getState().ask(
        'Cambios sin guardar',
        `Hay ${dirty.length} archivo(s) con cambios sin guardar en esta raíz. Si la quitas se perderán.`,
        [
          { label: 'Cancelar', value: 'cancel' },
          { label: 'Descartar y quitar', value: 'discard', variant: 'danger' },
        ],
      );
      if (choice !== 'discard') return;
    }
    try {
      const info = await workspaceRemoveRoot(path);
      for (const t of useEditor.getState().tabs.filter((x) => isSameOrInside(path, x.path))) {
        useEditor.getState().onPathDeleted(t.path);
      }
      set((s) => {
        const children = { ...s.children };
        const expanded = { ...s.expanded };
        for (const k of Object.keys(children)) if (isSameOrInside(path, k)) delete children[k];
        for (const k of Object.keys(expanded)) if (isSameOrInside(path, k)) delete expanded[k];
        const gitByRepo = { ...s.gitByRepo };
        for (const k of Object.keys(gitByRepo)) if (isSameOrInside(path, k)) delete gitByRepo[k];
        return {
          roots: info.roots,
          children,
          expanded,
          gitByRepo,
          selected: s.selected && isSameOrInside(path, s.selected) ? null : s.selected,
          git: aggregateGit(gitByRepo, s.root, useGit.getState().repos),
        };
      });
      useServices.getState().dropRoot(path);
      useRunConfigStore.getState().dropRoot(path);
      void useGit.getState().loadRepos();
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    }
  },

  refreshRoots: async () => {
    try {
      const roots = await workspaceRoots();
      const prev = get().roots;
      const same =
        prev.length === roots.length &&
        prev.every((r, i) => r.path === roots[i].path && r.exists === roots[i].exists && r.name === roots[i].name);
      if (!same) {
        set({ roots });
        void useGit.getState().loadRepos();
      }
    } catch {
      /* sin workspace */
    }
  },

  toggleDir: (path) =>
    set((s) => {
      const expanded = { ...s.expanded };
      if (expanded[path]) delete expanded[path];
      else expanded[path] = true;
      return { expanded };
    }),

  expandDir: (path) => set((s) => (s.expanded[path] ? s : { expanded: { ...s.expanded, [path]: true } })),

  collapseAll: () =>
    set((s) => {
      const expanded: Record<string, true> = {};
      for (const r of s.roots) if (s.expanded[r.path]) expanded[r.path] = true;
      return { expanded };
    }),

  select: (path) => set({ selected: path }),

  refreshDir: async (path, silent = false) => {
    let entries: FsEntry[];
    try {
      entries = await fsReadDir(path);
    } catch (e) {
      // Directorio borrado/ilegible: se olvida su listado (y el de sus descendientes).
      set((s) => {
        const children = { ...s.children };
        for (const k of Object.keys(children)) if (isSameOrInside(path, k)) delete children[k];
        return { children };
      });
      if (!silent) useUi.getState().pushToast(toMessage(e));
      return;
    }
    const prev = get().children[path];
    if (sameListing(prev, entries)) return;
    set((s) => {
      const children = { ...s.children, [path]: entries };
      if (prev) {
        const alive = new Set(entries.filter((e) => e.isDir).map((e) => e.path));
        for (const old of prev) {
          if (old.isDir && !alive.has(old.path)) {
            for (const k of Object.keys(children)) if (isSameOrInside(old.path, k)) delete children[k];
          }
        }
      }
      return { children };
    });
    // Iconos de símbolo: solo para los archivos Java/Kotlin de carpetas visibles.
    if (get().expanded[path] || get().roots.some((r) => r.path === path)) {
      useSymbols.getState().request(entries.filter((e) => !e.isDir && isClassifiable(e.path)).map((e) => e.path));
    }
  },

  refreshGit: (repo) => {
    if (repo) gitPending?.add(repo);
    else gitPending = null;
    if (gitTimer) clearTimeout(gitTimer);
    gitTimer = setTimeout(async () => {
      gitTimer = null;
      if (!get().root) return;
      const pending = gitPending;
      gitPending = new Set();
      const repos = useGit.getState().repos;
      const targets = pending === null ? repos.map((r) => r.top) : [...pending];
      const token = pending === null ? ++gitToken : gitToken;
      const results = await Promise.all(
        targets.map(async (top) => {
          try {
            return [top, await gitStatus(top)] as const;
          } catch {
            return [top, NO_GIT] as const;
          }
        }),
      );
      if (token !== gitToken) return; // llegó un refresco completo más nuevo
      set((s) => {
        const live = useGit.getState().repos;
        const alive = new Set(live.map((r) => r.top));
        const gitByRepo: Record<string, GitStatus> = {};
        for (const [k, v] of Object.entries(s.gitByRepo)) if (alive.has(k)) gitByRepo[k] = v;
        for (const [top, st] of results) if (alive.has(top)) gitByRepo[top] = st;
        return { gitByRepo, git: aggregateGit(gitByRepo, s.root, live) };
      });
    }, 300);
  },

  handleFsEvent: (ev) => {
    if (ev.type === 'error') {
      useUi.getState().pushToast(ev.message);
      return;
    }
    if (ev.type === 'gitChanged') {
      get().refreshGit(ev.repo ?? undefined);
      useGit.getState().refreshSoft(ev.repo ?? undefined);
      return;
    }
    if (ev.type === 'rootsChanged') {
      void get().refreshRoots();
      return;
    }
    const { children, expanded } = get();
    const compacts = collectCompacts(children);
    const dirs = new Set<string>();
    const changedJvm: string[] = [];
    for (const p of ev.paths) {
      const parent = dirname(p);
      const owner = owningDir(children, parent, compacts);
      if (owner) dirs.add(owner);
      if (children[p]) dirs.add(p); // el propio directorio pudo cambiar o desaparecer
      if (isClassifiable(p)) changedJvm.push(p);
    }
    for (const d of dirs) void get().refreshDir(d, true);
    if (changedJvm.length) {
      // El contenido pudo cambiar sin cambiar el listado: se olvida y se re-pide si está visible.
      const sym = useSymbols.getState();
      sym.forget(changedJvm);
      const visible = changedJvm.filter((p) => {
        const parent = dirname(p);
        return children[parent] && (expanded[parent] || get().roots.some((r) => r.path === parent));
      });
      if (visible.length) sym.request(visible);
    }
    const editor = useEditor.getState();
    for (const t of editor.tabs) {
      if (!t.dirty && ev.paths.includes(t.path)) void editor.reloadFromDisk(t.path);
    }
    get().refreshGit();
  },

  startInlineEdit: (edit) => {
    if (edit && edit.type !== 'rename') get().expandDir(edit.parent);
    set({ inlineEdit: edit });
  },

  createEntry: async (parent, name, dir) => {
    try {
      const created = dir ? await fsCreateDir(parent, name) : await fsCreateFile(parent, name);
      set({ inlineEdit: null, selected: created });
      get().expandDir(parent);
      await get().refreshDir(parent);
      get().refreshGit();
      if (!dir) await useEditor.getState().openFile(created);
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    }
  },

  renameEntry: async (path, newName) => {
    if (newName === basename(path)) {
      set({ inlineEdit: null });
      return;
    }
    try {
      const newPath = await fsRename(path, newName);
      set((s) => {
        const remap = (p: string) => (isSameOrInside(path, p) ? newPath + p.slice(path.length) : p);
        const expanded: Record<string, true> = {};
        for (const k of Object.keys(s.expanded)) expanded[remap(k)] = true;
        const children = { ...s.children };
        for (const k of Object.keys(children)) if (isSameOrInside(path, k)) delete children[k];
        return { inlineEdit: null, expanded, children, selected: s.selected ? remap(s.selected) : null };
      });
      useEditor.getState().onPathRenamed(path, newPath);
      await get().refreshDir(owningDir(get().children, dirname(path), collectCompacts(get().children)) ?? dirname(path));
      get().refreshGit();
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    }
  },

  deleteEntry: async ({ path, name, isDir }) => {
    const choice = await useUi.getState().ask(
      'Eliminar',
      `Se moverá a la papelera: ${name}${isDir ? ' (y todo su contenido)' : ''}.`,
      [
        { label: 'Cancelar', value: 'cancel' },
        { label: 'Mover a la papelera', value: 'delete', variant: 'danger' },
      ],
    );
    if (choice !== 'delete') return;
    try {
      await fsDelete(path);
      useEditor.getState().onPathDeleted(path);
      set((s) => {
        const children = { ...s.children };
        const expanded = { ...s.expanded };
        for (const k of Object.keys(children)) if (isSameOrInside(path, k)) delete children[k];
        for (const k of Object.keys(expanded)) if (isSameOrInside(path, k)) delete expanded[k];
        return { children, expanded, selected: s.selected && isSameOrInside(path, s.selected) ? null : s.selected };
      });
      await get().refreshDir(owningDir(get().children, dirname(path), collectCompacts(get().children)) ?? dirname(path));
      get().refreshGit();
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    }
  },
}));
