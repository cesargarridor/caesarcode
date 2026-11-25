import { Compartment, StateEffect, StateField, type Extension } from '@codemirror/state';
import { EditorView, GutterMarker, gutter } from '@codemirror/view';
import { gitBlame } from '../../lib/ipc';
import { toMessage } from '../../lib/errors';
import { laneColors } from '../../lib/palette';
import { registry } from '../../store/editorRegistry';
import { useUi } from '../../store/ui';
import { useGit } from '../../store/git';
import { repoFor } from '../../lib/paths';
import type { BlameResult } from '../../lib/types';

interface BlameData {
  /** sha por línea (índice = línea − 1). */
  shas: string[];
  result: BlameResult;
}

const setBlame = StateEffect.define<BlameData | null>();

const blameField = StateField.define<BlameData | null>({
  create: () => null,
  update(v, tr) {
    for (const e of tr.effects) if (e.is(setBlame)) return e.value;
    return v;
  },
});

/** Compartment por estado: vacío = apagado; con contenido = gutter activo. */
export const blameCompartment = new Compartment();

const COLORS = laneColors;
function colorOf(sha: string): string {
  let h = 0;
  for (let i = 0; i < sha.length; i++) h = (h * 31 + sha.charCodeAt(i)) >>> 0;
  return COLORS[h % COLORS.length];
}

function ago(sec: number): string {
  const d = Math.max(0, Date.now() / 1000 - sec);
  if (d < 3600) return `${Math.max(1, Math.floor(d / 60))}m`;
  if (d < 86400) return `${Math.floor(d / 3600)}h`;
  if (d < 86400 * 30) return `${Math.floor(d / 86400)}d`;
  if (d < 86400 * 365) return `${Math.floor(d / 86400 / 30)}mo`;
  return `${Math.floor(d / 86400 / 365)}y`;
}

class BlameMarker extends GutterMarker {
  constructor(readonly sha: string, readonly label: string, readonly title: string, readonly first: boolean) {
    super();
  }
  eq(o: BlameMarker) {
    return o.sha === this.sha && o.label === this.label && o.first === this.first;
  }
  toDOM() {
    const el = document.createElement('div');
    el.className = 'cm-blame-cell';
    el.style.borderLeft = `2px solid ${colorOf(this.sha)}`;
    el.title = this.title;
    el.textContent = this.first ? this.label : '';
    el.addEventListener('click', () => {
      useGit.getState().selectCommit(this.sha).catch(() => undefined);
    });
    return el;
  }
}

const blameGutter = gutter({
  class: 'cm-blame-gutter',
  lineMarker(view, line) {
    const data = view.state.field(blameField, false);
    if (!data) return null;
    const n = view.state.doc.lineAt(line.from).number;
    const sha = data.shas[n - 1];
    if (!sha) return null;
    const c = data.result.commits[sha];
    const prev = n > 1 ? data.shas[n - 2] : null;
    const first = prev !== sha;
    const zero = /^0+$/.test(sha);
    const label = zero ? 'No confirmado' : `${(c?.author ?? '?').slice(0, 14)} · ${c ? ago(c.time) : ''}`;
    const title = zero ? 'Cambio sin confirmar' : `${sha.slice(0, 8)} ${c?.summary ?? ''}\n${c?.author ?? ''}`;
    return new BlameMarker(sha, label, title, first);
  },
  lineMarkerChange: (u) => u.transactions.some((t) => t.effects.some((e) => e.is(setBlame))) || u.docChanged,
  initialSpacer: () => new BlameMarker('0', 'xxxxxxxxxxxxxx · 00mo', '', true),
});

const blameTheme = EditorView.theme({
  '.cm-blame-gutter': { minWidth: '150px', backgroundColor: 'rgb(var(--cc-canvas))', borderRight: '1px solid var(--cc-border)' },
  '.cm-blame-cell': {
    padding: '0 8px',
    fontSize: '11px',
    color: 'rgb(var(--cc-muted))',
    whiteSpace: 'nowrap',
    overflow: 'hidden',
    cursor: 'pointer',
  },
  '.cm-blame-cell:hover': { color: 'rgb(var(--cc-fg-strong))' },
});

/** Se incluye en `buildExtensions`: apagado por defecto. */
export const blameBase: Extension = blameCompartment.of([]);

export function blameActive(view: EditorView): boolean {
  return view.state.field(blameField, false) !== undefined;
}

/** Activa/desactiva la anotación de blame en la pestaña mostrada de `path`. */
export async function toggleBlame(path: string): Promise<void> {
  const view = registry.view;
  const toast = useUi.getState().pushToast;
  if (!view || registry.shownPath !== path) {
    toast('Abre el archivo en el editor para anotarlo', 'info');
    return;
  }
  if (blameActive(view)) {
    view.dispatch({ effects: blameCompartment.reconfigure([]) });
    return;
  }
  const saved = registry.saved.get(path);
  if (saved && !view.state.doc.eq(saved)) {
    toast('Guarda el archivo antes de anotarlo con Git Blame', 'info');
    return;
  }
  try {
    const repo = repoFor(useGit.getState().repos, path)?.top;
    if (!repo) {
      toast('El archivo no pertenece a ningún repositorio Git', 'info');
      return;
    }
    const result = await gitBlame(repo, path);
    if (registry.view !== view || registry.shownPath !== path) return;
    const shas: string[] = [];
    for (const [line, sha] of result.lines) shas[line - 1] = sha;
    view.dispatch({ effects: blameCompartment.reconfigure([blameField, blameGutter, blameTheme]) });
    view.dispatch({ effects: setBlame.of({ shas, result }) });
  } catch (e) {
    toast(toMessage(e));
  }
}
