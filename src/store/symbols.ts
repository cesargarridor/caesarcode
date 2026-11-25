import { create } from 'zustand';
import { fsClassify } from '../lib/ipc';
import { extOf } from '../lib/paths';
import { JVM_EXTS } from '../lib/fileIconRegistry';
import type { SymbolInfo } from '../lib/types';

const BATCH = 256;
const DEBOUNCE_MS = 60;

interface SymbolsStore {
  symbols: Record<string, SymbolInfo>;
  /** Encola rutas Java/Kotlin visibles (las ya conocidas o en vuelo se ignoran). */
  request: (paths: readonly string[]) => void;
  /** Olvida símbolos de rutas cambiadas (el llamador decide si re-encolar). */
  forget: (paths: readonly string[]) => void;
  reset: () => void;
}

let queue = new Set<string>();
const inflight = new Set<string>();
let timer: ReturnType<typeof setTimeout> | null = null;
/** Invalida respuestas de lotes lanzados antes de un reset. */
let epoch = 0;
/** Rutas olvidadas mientras su lote estaba en vuelo: se descarta esa respuesta y se re-encolan. */
const stale = new Set<string>();

export function isClassifiable(path: string): boolean {
  const ext = extOf(path);
  return !!ext && JVM_EXTS.has(ext);
}

async function flush(): Promise<void> {
  timer = null;
  const all = [...queue];
  queue = new Set();
  for (let i = 0; i < all.length; i += BATCH) {
    const batch = all.slice(i, i + BATCH);
    const myEpoch = epoch;
    for (const p of batch) inflight.add(p);
    try {
      const res = await fsClassify(batch);
      if (myEpoch !== epoch) continue;
      const add: Record<string, SymbolInfo> = {};
      for (const [p, info] of res) if (!stale.has(p)) add[p] = info;
      useSymbols.setState((s) => ({ symbols: { ...s.symbols, ...add } }));
    } catch {
      /* sin clasificación se queda el icono por extensión */
    } finally {
      const redo: string[] = [];
      for (const p of batch) {
        inflight.delete(p);
        if (stale.delete(p)) redo.push(p);
      }
      if (redo.length && myEpoch === epoch) useSymbols.getState().request(redo);
    }
  }
}

export const useSymbols = create<SymbolsStore>((set, get) => ({
  symbols: {},

  request: (paths) => {
    const known = get().symbols;
    let added = false;
    for (const p of paths) {
      if (!isClassifiable(p) || known[p] || inflight.has(p) || queue.has(p)) continue;
      queue.add(p);
      added = true;
    }
    if (added && !timer) timer = setTimeout(() => void flush(), DEBOUNCE_MS);
  },

  forget: (paths) => {
    const drop = new Set(paths);
    for (const p of drop) {
      if (inflight.has(p)) stale.add(p);
      queue.delete(p);
    }
    set((s) => {
      if (!paths.some((p) => p in s.symbols)) return s;
      const symbols = { ...s.symbols };
      for (const p of drop) delete symbols[p];
      return { symbols };
    });
  },

  reset: () => {
    epoch++;
    queue = new Set();
    inflight.clear();
    stale.clear();
    if (timer) clearTimeout(timer);
    timer = null;
    set({ symbols: {} });
  },
}));
