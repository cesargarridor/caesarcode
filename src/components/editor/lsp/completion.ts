import {
  snippet,
  type Completion,
  type CompletionContext,
  type CompletionResult,
  type CompletionSource,
} from '@codemirror/autocomplete';
import type { EditorView } from '@codemirror/view';
import { lspCompletion } from '../../../lib/ipc';
import type { CompletionDto } from '../../../lib/types';
import { registry } from '../../../store/editorRegistry';
import { lspToOffset, offsetToLsp } from './diagnostics';
import { lspSnippetToCm } from './snippet';
import { flushChange, lspAvailable } from './sync';

const WORD = /[\w$]+$/;
const TRIGGER = /(?:\.|::|->)$/;
const VALID_FOR = /^[\w$]*$/;

function toCompletion(item: CompletionDto, rank: number): Completion {
  const base: Completion = {
    label: item.label,
    type: item.kind ?? undefined,
    detail: item.detail ?? undefined,
    info: item.documentation ?? undefined,
    boost: Math.max(-90, 50 - rank),
  };
  const edit = item.textEdit;
  if (edit) {
    base.apply = (view: EditorView, completion: Completion, from: number, to: number) => {
      const doc = view.state.doc;
      const ef = lspToOffset(doc, edit.fromLine, edit.fromChar);
      // El usuario pudo seguir escribiendo tras pedir la lista: el final nunca queda por detrás del cursor.
      const et = Math.max(lspToOffset(doc, edit.toLine, edit.toChar), to);
      if (item.isSnippet) {
        snippet(lspSnippetToCm(edit.newText))(view, completion, Math.min(ef, from), et);
      } else {
        const start = Math.min(ef, from);
        view.dispatch({
          changes: { from: start, to: et, insert: edit.newText },
          selection: { anchor: start + edit.newText.length },
          userEvent: 'input.complete',
        });
      }
    };
  } else if (item.isSnippet) {
    base.apply = snippet(lspSnippetToCm(item.insertText));
  } else if (item.insertText !== item.label) {
    base.apply = item.insertText;
  }
  return base;
}

function sortByServer(items: CompletionDto[]): CompletionDto[] {
  return items
    .map((it, i) => ({ it, i }))
    .sort((a, b) => {
      const ka = a.it.sortText ?? a.it.label;
      const kb = b.it.sortText ?? b.it.label;
      return ka < kb ? -1 : ka > kb ? 1 : a.i - b.i;
    })
    .map((x) => x.it);
}

/** Completion del lenguaje (palabras locales, palabras clave...) cuando el servidor no aporta nada. */
async function languageFallback(ctx: CompletionContext): Promise<CompletionResult | null> {
  for (const src of ctx.state.languageDataAt<CompletionSource>('autocomplete', ctx.pos)) {
    if (typeof src !== 'function') continue;
    const r = await src(ctx);
    if (r && r.options.length > 0) return r;
  }
  return null;
}

/** Fuente única: LSP primero; si no hay servidor o no devuelve nada, la del lenguaje. */
export const lspCompletionSource: CompletionSource = async (ctx) => {
  const word = ctx.matchBefore(WORD);
  const before = ctx.state.sliceDoc(Math.max(0, ctx.pos - 2), ctx.pos);
  if (!ctx.explicit && !word && !TRIGGER.test(before)) return null;

  const path = registry.shownPath;
  if (lspAvailable(path)) {
    try {
      await flushChange(path);
      if (ctx.aborted) return null;
      const { line, character } = offsetToLsp(ctx.state.doc, ctx.pos);
      const items = await lspCompletion(path, line, character);
      if (ctx.aborted) return null;
      if (items.length > 0) {
        return {
          from: word ? word.from : ctx.pos,
          options: sortByServer(items).map(toCompletion),
          validFor: VALID_FOR,
        };
      }
    } catch {
      /* un fallo del servidor nunca debe romper el editor: se usa la del lenguaje */
    }
  }
  return languageFallback(ctx);
};
