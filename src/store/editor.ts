import { create } from 'zustand';
import { EditorState, Transaction } from '@codemirror/state';
import type { ViewUpdate } from '@codemirror/view';
import { fsReadFile, fsWriteFile } from '../lib/ipc';
import { toMessage } from '../lib/errors';
import { basename, extOf, isSameOrInside } from '../lib/paths';
import { createEditorState, languageCompartment, type EditorHandlers } from '../components/editor/extensions';
import { languageFor } from '../components/editor/languages';
import { lspClose, lspOpen, lspResetSync, lspSave, scheduleDidChange } from '../components/editor/lsp/sync';
import { forgetPath, registry, rekeyPath } from './editorRegistry';
import { useUi } from './ui';

export type TabKind = 'text' | 'binary' | 'tooLarge' | 'tabular' | 'media';
export interface Tab { path: string; name: string; dirty: boolean; kind: TabKind; isExternal: boolean }

const TABULAR_EXT = new Set(['csv', 'tsv', 'xlsx', 'xls', 'ods']);
const MEDIA_EXT = new Set([
  'png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'ico', 'avif', 'svg',
  'mp3', 'wav', 'ogg', 'oga', 'flac', 'm4a', 'aac', 'opus',
  'mp4', 'webm', 'mov', 'm4v', 'ogv', 'mkv',
  'pdf',
]);

function kindByExt(path: string): 'tabular' | 'media' | null {
  const ext = (extOf(path) ?? '').toLowerCase();
  if (TABULAR_EXT.has(ext)) return 'tabular';
  if (MEDIA_EXT.has(ext)) return 'media';
  return null;
}

/** Externo = fuera de todas las raíces del workspace (cualquier raíz cuenta como interno). */
function externalFor(roots: string[], path: string): boolean {
  return !roots.some((r) => isSameOrInside(r, path));
}
export interface Cursor { line: number; col: number }
export interface RevealRequest { path: string; line: number; column: number; nonce: number }

interface EditorStore {
  tabs: Tab[];
  active: string | null;
  cursor: Cursor | null;
  reveal: RevealRequest | null;
  /** Raíces del workspace (las fija App.tsx); recalcula `isExternal`. */
  rootPaths: string[];
  setRoots: (roots: string[]) => void;
  openFile: (path: string) => Promise<void>;
  /** `line` 1-based, `column` 0-based UTF-16. */
  openFileAt: (path: string, line: number, column: number) => Promise<void>;
  setActive: (path: string) => void;
  save: (path: string) => Promise<void>;
  /** Cierra pidiendo confirmación si hay cambios sin guardar. */
  requestClose: (path: string) => Promise<void>;
  close: (path: string) => void;
  closeAll: () => void;
  /** Pestaña siguiente (+1) o anterior (-1), circular. */
  cycleTab: (delta: 1 | -1) => void;
  clearReveal: (nonce: number) => void;
  /** El archivo cambió en disco: recarga el contenido si no hay cambios sin guardar. */
  reloadFromDisk: (path: string) => Promise<void>;
  onPathRenamed: (oldPath: string, newPath: string) => void;
  onPathDeleted: (path: string) => void;
  refreshCursor: () => void;
}

const inflight = new Map<string, Promise<void>>();
let revealSeq = 0;

function cursorOf(state: EditorState): Cursor {
  const head = state.selection.main.head;
  const line = state.doc.lineAt(head);
  return { line: line.number, col: head - line.from + 1 };
}

function sameCursor(a: Cursor | null, b: Cursor | null) {
  return a === b || (!!a && !!b && a.line === b.line && a.col === b.col);
}

// El view solo muestra un estado a la vez (registry.shownPath), así que los handlers no
// necesitan capturar la ruta: se resuelve en el momento y sobreviven a renombrados.
const handlers: EditorHandlers = {
  onSave: () => {
    const p = registry.shownPath;
    if (p) void useEditor.getState().save(p);
  },
  onUpdate: (u: ViewUpdate) => {
    const path = registry.shownPath;
    if (!path) return;
    registry.states.set(path, u.state);
    const st = useEditor.getState();
    if (u.docChanged) {
      scheduleDidChange(path);
      const saved = registry.saved.get(path);
      const dirty = saved ? !u.state.doc.eq(saved) : true;
      const tab = st.tabs.find((t) => t.path === path);
      if (tab && tab.dirty !== dirty) {
        useEditor.setState({ tabs: st.tabs.map((t) => (t.path === path ? { ...t, dirty } : t)) });
      }
    }
    if (u.docChanged || u.selectionSet) {
      const c = cursorOf(u.state);
      if (!sameCursor(useEditor.getState().cursor, c)) useEditor.setState({ cursor: c });
    }
  },
};

function pickNeighbor(tabs: Tab[], closing: string): string | null {
  const i = tabs.findIndex((t) => t.path === closing);
  const rest = tabs.filter((t) => t.path !== closing);
  if (rest.length === 0) return null;
  return rest[Math.min(i, rest.length - 1)].path;
}

export const useEditor = create<EditorStore>((set, get) => ({
  tabs: [],
  active: null,
  cursor: null,
  reveal: null,
  rootPaths: [],

  setRoots: (roots) => {
    set((s) => ({
      rootPaths: roots,
      tabs: s.tabs.map((t) => {
        const isExternal = externalFor(roots, t.path);
        return t.isExternal === isExternal ? t : { ...t, isExternal };
      }),
    }));
  },

  openFile: async (path) => {
    if (get().tabs.some((t) => t.path === path)) {
      set({ active: path });
      get().refreshCursor();
      return;
    }
    const pending = inflight.get(path);
    if (pending) return pending.then(() => set({ active: path }));
    const job = (async () => {
      try {
        const byExt = kindByExt(path);
        if (byExt) {
          // Tabular y multimedia: el visor carga el contenido; aquí no se lee el archivo.
          set((s) =>
            s.tabs.some((t) => t.path === path)
              ? { active: path }
              : {
                  tabs: [...s.tabs, { path, name: basename(path), dirty: false, kind: byExt, isExternal: externalFor(s.rootPaths, path) }],
                  active: path,
                },
          );
          return;
        }
        const fc = await fsReadFile(path);
        const kind: TabKind = fc.isBinary ? 'binary' : fc.tooLarge ? 'tooLarge' : 'text';
        if (kind === 'text') {
          const state = createEditorState(path, fc.text, handlers, { readOnly: fc.truncated });
          registry.states.set(path, state);
          registry.saved.set(path, state.doc);
          lspOpen(path, fc.text);
        }
        set((s) =>
          s.tabs.some((t) => t.path === path)
            ? { active: path }
            : { tabs: [...s.tabs, { path, name: basename(path), dirty: false, kind, isExternal: externalFor(s.rootPaths, path) }], active: path },
        );
      } catch (e) {
        useUi.getState().pushToast(toMessage(e));
      } finally {
        inflight.delete(path);
      }
    })();
    inflight.set(path, job);
    return job;
  },

  openFileAt: async (path, line, column) => {
    await get().openFile(path);
    if (get().active !== path) return; // falló la apertura
    set({ reveal: { path, line, column, nonce: ++revealSeq } });
  },

  setActive: (path) => {
    set({ active: path });
    get().refreshCursor();
  },

  save: async (path) => {
    const state = registry.states.get(path);
    if (!state) return;
    const doc = state.doc;
    try {
      await fsWriteFile(path, state.sliceDoc());
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
      return;
    }
    registry.saved.set(path, doc);
    lspSave(path, state.sliceDoc());
    const current = registry.states.get(path)?.doc ?? doc;
    const dirty = !current.eq(doc);
    set((s) => ({ tabs: s.tabs.map((t) => (t.path === path ? { ...t, dirty } : t)) }));
  },

  requestClose: async (path) => {
    const tab = get().tabs.find((t) => t.path === path);
    if (!tab) return;
    if (tab.dirty) {
      const choice = await useUi.getState().ask(
        'Cambios sin guardar',
        `${tab.name} tiene cambios sin guardar. ¿Quieres guardarlos antes de cerrar?`,
        [
          { label: 'Cancelar', value: 'cancel' },
          { label: 'No guardar', value: 'discard', variant: 'danger' },
          { label: 'Guardar', value: 'save', variant: 'primary' },
        ],
      );
      if (choice === null || choice === 'cancel') return;
      if (choice === 'save') {
        await get().save(path);
        if (get().tabs.find((t) => t.path === path)?.dirty) return; // el guardado falló
      }
    }
    get().close(path);
  },

  close: (path) => {
    const { tabs, active } = get();
    if (!tabs.some((t) => t.path === path)) return;
    const next = active === path ? pickNeighbor(tabs, path) : active;
    lspClose(path);
    forgetPath(path);
    set({ tabs: tabs.filter((t) => t.path !== path), active: next, cursor: next ? get().cursor : null });
    get().refreshCursor();
  },

  closeAll: () => {
    for (const t of get().tabs) forgetPath(t.path);
    lspResetSync();
    set({ tabs: [], active: null, cursor: null, reveal: null });
  },

  cycleTab: (delta) => {
    const { tabs, active } = get();
    if (tabs.length < 2) return;
    const i = tabs.findIndex((t) => t.path === active);
    const next = tabs[(i + delta + tabs.length) % tabs.length];
    get().setActive(next.path);
  },

  clearReveal: (nonce) => {
    if (get().reveal?.nonce === nonce) set({ reveal: null });
  },

  reloadFromDisk: async (path) => {
    const tab = get().tabs.find((t) => t.path === path);
    if (!tab || tab.dirty || tab.kind !== 'text') return;
    let text: string;
    try {
      const fc = await fsReadFile(path);
      if (fc.isBinary || fc.tooLarge) return;
      text = fc.text;
    } catch {
      return; // borrado o ilegible: se deja la pestaña tal cual
    }
    // Releer el estado tras el await: el usuario pudo editar mientras tanto.
    if (get().tabs.find((t) => t.path === path)?.dirty) return;
    const shown = registry.shownPath === path && registry.view;
    const st = shown ? registry.view!.state : registry.states.get(path);
    if (!st) return;
    if (st.sliceDoc() === text) return; // eco de nuestro propio guardado
    const tr = st.update({
      changes: { from: 0, to: st.doc.length, insert: text },
      annotations: Transaction.addToHistory.of(false),
    });
    registry.saved.set(path, tr.newDoc);
    if (shown) registry.view!.dispatch(tr);
    else registry.states.set(path, tr.state);
    scheduleDidChange(path);
  },

  onPathRenamed: (oldPath, newPath) => {
    const map = (p: string) => (isSameOrInside(oldPath, p) ? newPath + p.slice(oldPath.length) : p);
    for (const t of get().tabs) {
      if (!isSameOrInside(oldPath, t.path)) continue;
      const to = map(t.path);
      // El servidor conoce el documento por su URI: se cierra el viejo y se abre el nuevo.
      lspClose(t.path);
      rekeyPath(t.path, to);
      const st = registry.states.get(to);
      if (st) {
        // El lenguaje puede cambiar con la extensión.
        registry.states.set(to, st.update({ effects: languageCompartment.reconfigure(languageFor(to)) }).state);
        lspOpen(to, st.sliceDoc());
      }
    }
    // Con el view mostrando el estado renombrado hay que reflejar el nuevo estado.
    const shown = registry.shownPath;
    const shownState = shown ? registry.states.get(shown) : undefined;
    if (registry.view && shownState && registry.view.state !== shownState) {
      const snap = registry.view.scrollSnapshot();
      registry.view.setState(shownState);
      registry.view.dispatch({ effects: snap });
    }
    set((s) => ({
      tabs: s.tabs.map((t) => (isSameOrInside(oldPath, t.path) ? { ...t, path: map(t.path), name: basename(map(t.path)) } : t)),
      active: s.active ? map(s.active) : null,
      reveal: null,
    }));
  },

  onPathDeleted: (path) => {
    for (const t of get().tabs.filter((x) => isSameOrInside(path, x.path))) get().close(t.path);
  },

  refreshCursor: () => {
    const { active } = get();
    const st = active ? registry.states.get(active) : undefined;
    const c = st ? cursorOf(st) : null;
    if (!sameCursor(get().cursor, c)) set({ cursor: c });
  },
}));
