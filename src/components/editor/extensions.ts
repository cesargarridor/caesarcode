import { Compartment, EditorState, type Extension } from '@codemirror/state';
import {
  EditorView,
  crosshairCursor,
  drawSelection,
  dropCursor,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
  rectangularSelection,
  type ViewUpdate,
} from '@codemirror/view';
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
import { bracketMatching, foldGutter, foldKeymap, indentOnInput } from '@codemirror/language';
import { autocompletion, closeBrackets, closeBracketsKeymap, completionKeymap } from '@codemirror/autocomplete';
import { highlightSelectionMatches, search, searchKeymap, selectNextOccurrence } from '@codemirror/search';
import { lintGutter, lintKeymap } from '@codemirror/lint';
import { graphiteTheme } from './theme';
import { languageFor } from './languages';
import { lspCompletionSource } from './lsp/completion';
import { goToDefinition, definitionExtension } from './lsp/definition';
import { lspHoverTooltip } from './lsp/hover';
import { blameBase } from './blame';
import { liveTemplateSource } from '../../editor/liveTemplates';

export interface EditorHandlers {
  onSave: () => void;
  onUpdate: (update: ViewUpdate) => void;
}

/** Permite cambiar el lenguaje de sintaxis de un estado (p. ej. al renombrar el archivo). */
export const languageCompartment = new Compartment();

/** Extensiones del editor completo. Los datos por pestaña (ruta, lenguaje) son lo único que varía. */
export function buildExtensions(path: string, text: string, h: EditorHandlers, opts: { readOnly?: boolean } = {}): Extension[] {
  return [
    ...(opts.readOnly ? [EditorState.readOnly.of(true)] : []),
    // Respeta el fin de línea del archivo para no convertir CRLF a LF al guardar.
    EditorState.lineSeparator.of(text.includes('\r\n') ? '\r\n' : '\n'),
    EditorState.allowMultipleSelections.of(true),
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightActiveLine(),
    foldGutter(),
    blameBase,
    history(),
    drawSelection(),
    dropCursor(),
    rectangularSelection(),
    crosshairCursor(),
    indentOnInput(),
    bracketMatching(),
    closeBrackets(),
    highlightSelectionMatches(),
    autocompletion({ override: [lspCompletionSource, liveTemplateSource(path)] }),
    lintGutter(),
    search({ top: true }),
    languageCompartment.of(languageFor(path)),
    lspHoverTooltip,
    definitionExtension(),
    keymap.of([
      // stopPropagation evita que el atajo global de App guarde dos veces.
      { key: 'Mod-s', preventDefault: true, stopPropagation: true, run: () => { h.onSave(); return true; } },
      { key: 'F12', preventDefault: true, run: goToDefinition },
      { key: 'Mod-d', preventDefault: true, run: selectNextOccurrence },
      ...closeBracketsKeymap,
      ...completionKeymap,
      ...searchKeymap,
      ...foldKeymap,
      ...lintKeymap,
      ...historyKeymap,
      indentWithTab,
      ...defaultKeymap,
    ]),
    graphiteTheme,
    EditorView.updateListener.of(h.onUpdate),
  ];
}

export function createEditorState(path: string, text: string, h: EditorHandlers, opts: { readOnly?: boolean } = {}): EditorState {
  return EditorState.create({ doc: text, extensions: buildExtensions(path, text, h, opts) });
}
