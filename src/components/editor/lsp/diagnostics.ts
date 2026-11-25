import type { Text } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { setDiagnostics, type Diagnostic } from '@codemirror/lint';
import type { DiagnosticDto } from '../../../lib/types';

/** (línea 0-based, carácter UTF-16) → offset del documento, con clamp a rangos válidos. */
export function lspToOffset(doc: Text, line: number, character: number): number {
  if (line < 0) return 0;
  if (line + 1 > doc.lines) return doc.length;
  const l = doc.line(line + 1);
  return l.from + Math.min(Math.max(character, 0), l.length);
}

export function offsetToLsp(doc: Text, pos: number): { line: number; character: number } {
  const l = doc.lineAt(pos);
  return { line: l.number - 1, character: pos - l.from };
}

export function toCmDiagnostics(doc: Text, diags: DiagnosticDto[]): Diagnostic[] {
  return diags.map((d) => {
    const from = lspToOffset(doc, d.fromLine, d.fromChar);
    const to = Math.max(from, lspToOffset(doc, d.toLine, d.toChar));
    return {
      from,
      to,
      severity: d.severity === 'hint' ? 'info' : d.severity,
      message: d.message,
      source: d.source ?? undefined,
    };
  });
}

/** Sustituye los diagnósticos del estado que muestra el view. */
export function applyDiagnostics(view: EditorView, diags: DiagnosticDto[]) {
  view.dispatch(setDiagnostics(view.state, toCmDiagnostics(view.state.doc, diags)));
}
