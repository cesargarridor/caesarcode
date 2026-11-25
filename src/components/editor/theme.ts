import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from '@codemirror/view';
import { HighlightStyle, syntaxHighlighting, syntaxTree } from '@codemirror/language';
import { RangeSetBuilder } from '@codemirror/state';
import { tags as t } from '@lezer/highlight';

// Todo el color/tipografía sale de variables CSS (--cc-*, --cc-syn-*, --cc-editor-*): el cambio de
// tema o de fuente es solo CSS, sin reconfigurar CodeMirror.
const MONO = 'var(--cc-editor-font)';
const rgb = (name: string, a?: number) => (a === undefined ? `rgb(var(--cc-${name}))` : `rgb(var(--cc-${name}) / ${a})`);

const PANEL = rgb('panel');
const FG = rgb('fg');
const MUTED = rgb('muted');
const ACCENT = rgb('accent');
const ELEVATED = rgb('elevated');
const BORDER_STRONG = 'var(--cc-border-strong)';
const GUTTER = 'var(--cc-gutter)';

const SYN = {
  keyword: 'var(--cc-syn-keyword)',
  annotation: 'var(--cc-syn-annotation)',
  type: 'var(--cc-syn-type)',
  string: 'var(--cc-syn-string)',
  func: 'var(--cc-syn-func)',
  field: 'var(--cc-syn-field)',
  variable: 'var(--cc-syn-variable)',
  comment: 'var(--cc-syn-comment)',
  number: 'var(--cc-syn-number)',
  operator: 'var(--cc-syn-operator)',
  heading: 'var(--cc-syn-heading)',
  link: 'var(--cc-syn-link)',
  invalid: 'var(--cc-syn-invalid)',
};

/**
 * Tema CM6 dirigido por variables CSS (SPEC6 A.3).
 * Los grises de comentarios de algunos temas tienen contraste < 4.5:1: es el estilo propio de
 * cada tema (p. ej. Darcula/Obsidian) y se acepta a propósito.
 */
const baseTheme = EditorView.theme(
  {
    '&': { height: '100%', backgroundColor: PANEL, color: FG, fontSize: 'var(--cc-editor-size)' },
    '&.cm-focused': { outline: 'none' },
    '.cm-scroller': { fontFamily: MONO, lineHeight: 'var(--cc-editor-lh)', fontVariantLigatures: 'var(--cc-ligatures)' },
    '.cm-content': { caretColor: ACCENT, padding: '6px 0' },
    '.cm-cursor, .cm-dropCursor': { borderLeftColor: ACCENT, borderLeftWidth: '2px' },
    '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, ::selection': {
      backgroundColor: 'var(--cc-selection)',
    },
    '.cm-gutters': { backgroundColor: PANEL, color: GUTTER, border: 'none' },
    '.cm-lineNumbers .cm-gutterElement': { padding: '0 10px 0 14px' },
    '.cm-activeLine': { backgroundColor: 'rgb(var(--cc-ink) / 0.04)' },
    '.cm-activeLineGutter': { backgroundColor: 'transparent', color: ACCENT },
    '.cm-foldGutter .cm-gutterElement': { cursor: 'pointer', color: GUTTER, padding: '0 4px' },
    '.cm-foldGutter .cm-gutterElement:hover': { color: ACCENT },
    '.cm-foldPlaceholder': {
      backgroundColor: ELEVATED,
      border: `1px solid ${BORDER_STRONG}`,
      color: MUTED,
      borderRadius: '4px',
      padding: '0 4px',
    },
    // Búsqueda
    '.cm-searchMatch': { backgroundColor: rgb('accent', 0.25), outline: `1px solid ${rgb('accent', 0.4)}` },
    '.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: rgb('accent-soft', 0.45) },
    '.cm-selectionMatch': { backgroundColor: rgb('accent', 0.15) },
    '&.cm-focused .cm-matchingBracket': { backgroundColor: rgb('accent', 0.2), outline: `1px solid ${rgb('accent', 0.4)}` },
    '&.cm-focused .cm-nonmatchingBracket': { backgroundColor: rgb('danger', 0.2) },
    '.cm-panels': { backgroundColor: ELEVATED, color: FG, fontFamily: MONO },
    '.cm-panels.cm-panels-top': { borderBottom: `1px solid ${BORDER_STRONG}` },
    '.cm-panels.cm-panels-bottom': { borderTop: `1px solid ${BORDER_STRONG}` },
    '.cm-search': { fontSize: '12px', padding: '6px 8px' },
    '.cm-search input, .cm-search button, .cm-search label': { fontSize: '12px' },
    '.cm-search .cm-textfield': {
      backgroundColor: PANEL,
      color: FG,
      border: `1px solid ${BORDER_STRONG}`,
      borderRadius: '6px',
      padding: '2px 6px',
      outline: 'none',
    },
    '.cm-search .cm-textfield:focus': { borderColor: ACCENT },
    '.cm-search .cm-button': {
      backgroundImage: 'none',
      backgroundColor: rgb('elevated-hover'),
      color: FG,
      border: `1px solid ${BORDER_STRONG}`,
      borderRadius: '6px',
      padding: '2px 8px',
      cursor: 'pointer',
    },
    '.cm-search .cm-button:hover': { backgroundColor: rgb('elevated-hover') },
    '.cm-search label': { color: MUTED },
    '.cm-search [name=close]': { color: MUTED, cursor: 'pointer' },
    // Tooltips y autocompletado
    '.cm-tooltip': { backgroundColor: ELEVATED, color: FG, border: `1px solid ${BORDER_STRONG}`, borderRadius: '8px', overflow: 'hidden', userSelect: 'text' },
    '.cm-tooltip.cm-tooltip-autocomplete > ul': { fontFamily: MONO, fontSize: '12px' },
    '.cm-tooltip-autocomplete ul li[aria-selected]': { backgroundColor: rgb('accent', 0.15), color: FG },
    '.cm-completionDetail': { color: MUTED, fontStyle: 'normal', marginLeft: '8px' },
    '.cm-completionMatchedText': { color: ACCENT, textDecoration: 'none', fontWeight: '600' },
    '.cm-tooltip.cm-completionInfo': { padding: '6px 8px', maxWidth: '420px', whiteSpace: 'pre-wrap' },
    '.cm-tooltip-lint': { fontFamily: MONO, fontSize: '12px' },
    '.cm-diagnostic': { padding: '3px 8px' },
    '.cm-diagnostic-error': { borderLeft: `3px solid ${rgb('danger')}` },
    '.cm-diagnostic-warning': { borderLeft: `3px solid ${ACCENT}` },
    '.cm-diagnostic-info': { borderLeft: `3px solid ${rgb('tree-src')}` },
    '.cm-lsp-link': { textDecoration: 'underline', textDecorationColor: ACCENT, cursor: 'pointer', color: ACCENT },
    '.cm-lsp-hover': { maxWidth: '480px', maxHeight: '300px', overflow: 'auto', padding: '6px 8px' },
    '.cm-lsp-hover pre': {
      margin: '4px 0',
      padding: '4px 6px',
      backgroundColor: PANEL,
      border: `1px solid ${BORDER_STRONG}`,
      borderRadius: '6px',
      fontFamily: MONO,
      fontSize: '12px',
      whiteSpace: 'pre-wrap',
    },
    '.cm-lsp-hover .cm-lsp-text': { whiteSpace: 'pre-wrap', fontSize: '12px', color: FG },
  },
  { dark: true },
);

/**
 * Decoración extra: lezer-java no emite un tag propio para anotaciones (@GetMapping queda como
 * variableName) y lezer-python solo marca el `@` del decorador. Marcamos los nodos completos
 * con una clase para colorearlos como anotación (color de anotación del tema). Solo recorre rangos visibles.
 */
const ANNOTATION_NODES = new Set(['MarkerAnnotation', 'Annotation', 'Decorator']);
const annotationMark = Decoration.mark({ class: 'cm-annotation' });

function buildAnnotations(view: EditorView): DecorationSet {
  const b = new RangeSetBuilder<Decoration>();
  const tree = syntaxTree(view.state);
  let last = -1;
  for (const { from, to } of view.visibleRanges) {
    tree.iterate({
      from,
      to,
      enter: (n) => {
        if (!ANNOTATION_NODES.has(n.name) || n.from < last) return;
        // Java: nombre de la anotación sin los argumentos; Python: la línea del decorador.
        let end = n.to;
        if (n.name === 'Annotation') {
          const args = n.node.getChild('AnnotationArgumentList');
          if (args) end = args.from;
        }
        if (end > n.from) {
          b.add(n.from, end, annotationMark);
          last = end;
        }
        return false;
      },
    });
  }
  return b.finish();
}

const annotationPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildAnnotations(view);
    }
    update(u: ViewUpdate) {
      if (u.docChanged || u.viewportChanged || syntaxTree(u.startState) !== syntaxTree(u.state)) {
        this.decorations = buildAnnotations(u.view);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

const annotationTheme = EditorView.theme({ '.cm-annotation, .cm-annotation *': { color: `${SYN.annotation} !important` } });

// Tags reales (verificados en node_modules): JSON emite las claves como `propertyName` a secas;
// YAML como `definition(propertyName)`; en Java/TS/Python `propertyName` es acceso a miembro.
// Solo `definition(propertyName)` (YAML) va a "tipo"; las claves JSON quedan como variable (color de variable del tema).
const highlight = HighlightStyle.define([
  { tag: [t.keyword, t.controlKeyword, t.definitionKeyword, t.moduleKeyword, t.operatorKeyword, t.modifier, t.self], color: SYN.keyword },
  { tag: [t.annotation, t.meta, t.attributeName, t.labelName], color: SYN.annotation },
  { tag: [t.typeName, t.className, t.namespace, t.definition(t.typeName), t.standard(t.typeName), t.definition(t.propertyName)], color: SYN.type },
  { tag: [t.string, t.special(t.string), t.character, t.regexp, t.escape, t.monospace], color: SYN.string },
  {
    tag: [t.function(t.variableName), t.function(t.propertyName), t.function(t.definition(t.variableName)), t.macroName],
    color: SYN.func,
  },
  { tag: [t.variableName, t.definition(t.variableName), t.local(t.variableName)], color: SYN.variable },
  { tag: t.propertyName, color: SYN.field, fontStyle: 'var(--cc-syn-field-style)' },
  { tag: [t.comment, t.lineComment, t.blockComment, t.docComment, t.quote], color: SYN.comment, fontStyle: 'italic' },
  { tag: [t.number, t.integer, t.float, t.atom, t.bool, t.null], color: SYN.number },
  { tag: [t.operator, t.punctuation, t.separator, t.bracket, t.derefOperator], color: SYN.operator },
  { tag: t.heading, color: SYN.heading, fontWeight: '700' },
  { tag: t.strong, fontWeight: '700' },
  { tag: t.emphasis, fontStyle: 'italic' },
  { tag: [t.link, t.url], color: SYN.link, textDecoration: 'underline' },
  { tag: t.list, color: SYN.keyword },
  { tag: t.invalid, color: SYN.invalid },
]);

export const obsidianTheme = [baseTheme, syntaxHighlighting(highlight), annotationPlugin, annotationTheme];
/** Alias retro-compatible (CodeEditor, extensions, DiffViewer). */
export const graphiteTheme = obsidianTheme;
