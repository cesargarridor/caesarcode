import { useEffect, useRef } from 'react';
import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { registry } from '../../store/editorRegistry';
import { useEditor } from '../../store/editor';
import { useLsp } from '../../store/lsp';
import { graphiteTheme } from './theme';
import { applyDiagnostics } from './lsp/diagnostics';

/**
 * UN solo EditorView para todas las pestañas: al cambiar de pestaña se intercambia el
 * EditorState con view.setState(). El estado vive en `registry`, no en zustand.
 */
export function CodeEditor() {
  const hostRef = useRef<HTMLDivElement>(null);
  const active = useEditor((s) => s.active);
  const activeKind = useEditor((s) => s.tabs.find((t) => t.path === s.active)?.kind ?? null);
  const readOnlyView = useEditor((s) => {
    const t = s.tabs.find((x) => x.path === s.active);
    return t?.kind === 'text' && !!registry.states.get(t.path)?.readOnly;
  });
  const reveal = useEditor((s) => s.reveal);
  const clearReveal = useEditor((s) => s.clearReveal);

  useEffect(() => {
    const view = new EditorView({
      parent: hostRef.current!,
      state: EditorState.create({ doc: '', extensions: [graphiteTheme, EditorState.readOnly.of(true)] }),
    });
    registry.view = view;
    return () => {
      registry.view = null;
      registry.shownPath = null;
      view.destroy();
    };
  }, []);

  // Cambio de pestaña: guarda scroll de la saliente y monta el estado de la entrante.
  useEffect(() => {
    const view = registry.view;
    if (!view) return;
    const prev = registry.shownPath;
    const target = active && activeKind === 'text' ? active : null;
    if (prev === target) return;
    if (prev && registry.states.has(prev)) registry.scroll.set(prev, view.scrollSnapshot());
    if (!target) {
      registry.shownPath = null;
      return;
    }
    const state = registry.states.get(target);
    if (!state) return;
    view.setState(state);
    registry.shownPath = target;
    const snap = registry.scroll.get(target);
    if (snap) view.dispatch({ effects: snap });
    // Diagnósticos que llegaron mientras la pestaña no estaba a la vista.
    applyDiagnostics(view, useLsp.getState().diagnostics[target] ?? []);
    useEditor.getState().refreshCursor();
    view.focus();
  }, [active, activeKind]);

  // Salto a línea/columna (resultados de búsqueda). Va después del efecto anterior.
  useEffect(() => {
    const view = registry.view;
    if (!reveal || !view || registry.shownPath !== reveal.path) return;
    const doc = view.state.doc;
    const line = doc.line(Math.min(Math.max(reveal.line, 1), doc.lines));
    const pos = line.from + Math.min(Math.max(reveal.column, 0), line.length);
    view.dispatch({ selection: { anchor: pos }, effects: EditorView.scrollIntoView(pos, { y: 'center' }) });
    view.focus();
    clearReveal(reveal.nonce);
  }, [reveal, active, activeKind, clearReveal]);

  return (
    <div className="flex h-full w-full flex-col overflow-hidden">
      {readOnlyView && (
        <div className="shrink-0 border-b border-amber-500/40 bg-amber-500/10 px-3 py-1 text-xs text-amber-300">
          Archivo &gt; 5 MB: vista parcial de solo lectura
        </div>
      )}
      <div ref={hostRef} className="min-h-0 flex-1 overflow-hidden" />
    </div>
  );
}
