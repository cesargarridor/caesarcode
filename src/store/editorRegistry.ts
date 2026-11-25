// Estado de CodeMirror FUERA de zustand: los EditorState son grandes y mutables por transacción.
// Hay UN solo EditorView; cada pestaña guarda su EditorState y se intercambia con view.setState().
import type { EditorState, StateEffect, Text } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';

export const registry = {
  /** EditorState por ruta (siempre el último conocido). */
  states: new Map<string, EditorState>(),
  /** Documento tal como está en disco, para calcular `dirty` (deshacer hasta el original limpia el punto). */
  saved: new Map<string, Text>(),
  /** Posición de scroll por pestaña. */
  scroll: new Map<string, StateEffect<unknown>>(),
  /** El único EditorView (lo crea CodeEditor). */
  view: null as EditorView | null,
  /** Ruta cuyo estado está mostrando ahora el view. */
  shownPath: null as string | null,
};

export function forgetPath(path: string) {
  registry.states.delete(path);
  registry.saved.delete(path);
  registry.scroll.delete(path);
  if (registry.shownPath === path) registry.shownPath = null;
}

/** Re-asigna las entradas de `oldPath` a `newPath` (renombrado de archivo o carpeta). */
export function rekeyPath(oldPath: string, newPath: string) {
  for (const map of [registry.states, registry.saved, registry.scroll] as Map<string, unknown>[]) {
    const v = map.get(oldPath);
    if (v !== undefined) {
      map.delete(oldPath);
      map.set(newPath, v);
    }
  }
  if (registry.shownPath === oldPath) registry.shownPath = newPath;
}
