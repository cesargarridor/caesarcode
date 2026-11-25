import { useEffect, useMemo, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { EditorState } from '@codemirror/state';
import { EditorView, lineNumbers } from '@codemirror/view';
import { MergeView } from '@codemirror/merge';
import { X } from 'lucide-react';
import { useGit, type DiffState } from '../../store/git';
import { gitFileAt, fsReadFile } from '../../lib/ipc';
import { toMessage } from '../../lib/errors';
import { graphiteTheme } from '../editor/theme';
import { languageFor } from '../editor/languages';
import { IconButton } from '../ui/IconButton';
import { absPath } from './gitUtils';
import { confirmAction } from './gitActions';
import type { DiffLine, DiffHunk } from '../../lib/types';

const ROW_H = 18;
const CHAR_W = 7.4;

type Item = { t: 'hunk'; i: number; h: DiffHunk } | { t: 'line'; l: DiffLine };

function Notice({ children }: { children: React.ReactNode }) {
  return <div className="flex h-full items-center justify-center p-6 text-center text-ui-lg text-muted">{children}</div>;
}

function HunkButton({ label, danger, onClick }: { label: string; danger?: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-md border px-1.5 text-[11px] leading-[16px] outline-none ${danger ? 'border-stop/40 text-stop-text hover:bg-stop/10' : 'border-border-strong text-fg hover:bg-hover'}`}
    >
      {label}
    </button>
  );
}

function UnifiedView({ d }: { d: DiffState }) {
  const data = d.data!;
  const scrollRef = useRef<HTMLDivElement>(null);
  const items = useMemo<Item[]>(() => {
    const out: Item[] = [];
    data.hunks.forEach((h, i) => {
      out.push({ t: 'hunk', i, h });
      for (const l of h.lines) out.push({ t: 'line', l });
    });
    return out;
  }, [data]);
  const width = useMemo(() => {
    let m = 0;
    for (const it of items) if (it.t === 'line' && it.l.text.length > m) m = it.l.text.length;
    return Math.max(m * CHAR_W + 120, 400);
  }, [items]);
  const virt = useVirtualizer({ count: items.length, getScrollElement: () => scrollRef.current, estimateSize: () => ROW_H, overscan: 30 });
  const g = useGit.getState;
  const canHunk = d.source === 'unstaged' || d.source === 'staged';

  if (items.length === 0) return <Notice>Sin cambios de contenido (permisos, modo o archivo vacío).</Notice>;

  return (
    <div ref={scrollRef} className="h-full overflow-auto font-mono text-[12px]">
      <div style={{ height: virt.getTotalSize(), width, minWidth: '100%', position: 'relative' }}>
        {virt.getVirtualItems().map((v) => {
          const it = items[v.index];
          const style: React.CSSProperties = { position: 'absolute', top: 0, left: 0, right: 0, height: ROW_H, transform: `translateY(${v.start}px)` };
          if (it.t === 'hunk') {
            return (
              <div key={v.index} style={style} className="flex items-center border-y border-border bg-panel text-muted">
                <div className="sticky left-0 flex items-center gap-2 pl-2">
                  <span className="truncate">{it.h.header}</span>
                  {canHunk && (
                    <span className="flex gap-1">
                      {d.source === 'unstaged' && (
                        <>
                          <HunkButton label="Stage hunk" onClick={() => void g().applyHunk('stage', it.i)} />
                          <HunkButton
                            label="Discard hunk"
                            danger
                            onClick={async () => {
                              if (await confirmAction('Descartar hunk', 'Se perderán estos cambios del working tree. No se puede deshacer.', 'Descartar'))
                                void g().applyHunk('discard', it.i);
                            }}
                          />
                        </>
                      )}
                      {d.source === 'staged' && <HunkButton label="Unstage hunk" onClick={() => void g().applyHunk('unstage', it.i)} />}
                    </span>
                  )}
                </div>
              </div>
            );
          }
          const l = it.l;
          const bg = l.kind === '+' ? 'bg-ok/10' : l.kind === '-' ? 'bg-danger/10' : '';
          const fg = l.kind === '+' ? 'text-run-text' : l.kind === '-' ? 'text-stop-text' : l.kind === '\\' ? 'text-muted italic' : 'text-fg';
          return (
            <div key={v.index} style={style} className={`flex whitespace-pre ${bg}`}>
              <span className="sticky left-0 flex shrink-0 select-none bg-base text-tree-excluded">
                <span className="w-10 pr-2 text-right">{l.oldNo ?? ''}</span>
                <span className="w-10 pr-2 text-right">{l.newNo ?? ''}</span>
              </span>
              <span className={`w-4 shrink-0 select-none text-center ${l.kind === '+' ? 'text-ok' : l.kind === '-' ? 'text-danger' : 'text-muted'}`}>{l.kind === ' ' || l.kind === '\\' ? '' : l.kind}</span>
              <span className={fg}>{l.text}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

/** Contenido de un lado; `null` = el archivo no existe en esa revisión. Lanza si es binario/enorme. */
async function sideText(repo: string, rev: string, path: string): Promise<string> {
  try {
    return await gitFileAt(repo, rev, path);
  } catch (e) {
    const m = toMessage(e);
    if (/binario|grande/i.test(m)) throw e;
    return ''; // no existe en esa revisión (archivo nuevo/borrado)
  }
}

async function loadSides(d: DiffState, top: string | null): Promise<[string, string]> {
  if (!top) throw new Error('No hay repositorio activo');
  const oldPath = d.oldPath ?? d.path;
  if (d.source === 'commit') {
    const sha = d.target.type === 'commit' ? d.target.sha : '';
    const old = d.parent ? await sideText(top, d.parent, oldPath) : '';
    return [old, await sideText(top, sha, d.path)];
  }
  if (d.source === 'staged') return [await sideText(top, 'HEAD', oldPath), await sideText(top, 'INDEX', d.path)];
  // unstaged / untracked / conflict: índice (o vacío) vs disco.
  const old = d.source === 'untracked' ? '' : await sideText(top, 'INDEX', oldPath);
  let cur = '';
  try {
    const f = await fsReadFile(absPath(top, d.path));
    if (f.isBinary) throw new Error('archivo binario');
    if (f.tooLarge) throw new Error('archivo demasiado grande');
    cur = f.text;
  } catch (e) {
    if (/binario|grande/i.test(toMessage(e))) throw e;
  }
  return [old, cur];
}

const mergeTheme = EditorView.theme(
  {
    '&': { height: 'auto' },
    '.cm-scroller': { overflow: 'visible' },
    '.cm-mergeView': { height: 'auto' },
    '.cm-gutters': { minWidth: '36px' },
    '.cm-changedLine': { backgroundColor: 'rgb(var(--cc-accent) / 0.08)' },
    '.cm-deletedChunk': { backgroundColor: 'rgb(var(--cc-danger) / 0.1)' },
    '.cm-collapsedLines': { backgroundColor: 'rgb(var(--cc-card))', color: 'rgb(var(--cc-muted))', cursor: 'pointer' },
  },
  { dark: true },
);

function SplitView({ d }: { d: DiffState }) {
  const host = useRef<HTMLDivElement>(null);
  const top = useGit((s) => s.info?.top ?? null);
  const [state, setState] = useState<{ loading: boolean; error: string | null }>({ loading: true, error: null });

  useEffect(() => {
    let alive = true;
    let mv: MergeView | null = null;
    setState({ loading: true, error: null });
    loadSides(d, top)
      .then(([a, b]) => {
        if (!alive || !host.current) return;
        const ext = [
          graphiteTheme,
          mergeTheme,
          lineNumbers(),
          languageFor(d.path),
          EditorState.readOnly.of(true),
          EditorView.editable.of(false),
        ];
        mv = new MergeView({
          a: { doc: a, extensions: ext },
          b: { doc: b, extensions: ext },
          parent: host.current,
          highlightChanges: true,
          gutter: true,
          collapseUnchanged: { margin: 3, minSize: 6 },
        });
        setState({ loading: false, error: null });
      })
      .catch((e) => alive && setState({ loading: false, error: toMessage(e) }));
    return () => {
      alive = false;
      mv?.destroy();
    };
    // `d.data` cambia cuando el diff se recarga (p. ej. tras una operación).
  }, [d.path, d.source, d.parent, d.oldPath, d.data, top]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="relative h-full overflow-auto">
      <div ref={host} />
      {state.loading && <div className="absolute inset-0 flex items-center justify-center text-ui text-muted">Cargando…</div>}
      {state.error && <Notice>{state.error}</Notice>}
    </div>
  );
}

export function DiffViewer() {
  const d = useGit((s) => s.diff);
  const mode = useGit((s) => s.diffMode);
  if (!d) return null;
  const label = d.source === 'staged' ? 'staged' : d.source === 'commit' ? 'commit' : d.source === 'untracked' ? 'nuevo' : 'working tree';
  let body: React.ReactNode;
  if (d.loading) body = <Notice>Cargando diff…</Notice>;
  else if (d.error) body = <Notice>{d.error}</Notice>;
  else if (!d.data) body = null;
  else if (d.data.binary) body = <Notice>Archivo binario: no se muestra el diff.</Notice>;
  else if (d.data.tooLarge) body = <Notice>Diff demasiado grande (&gt; 2 MB): ábrelo en el editor.</Notice>;
  else body = mode === 'split' ? <SplitView d={d} /> : <UnifiedView d={d} />;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex h-9 shrink-0 items-center gap-2 px-3">
        <span className="min-w-0 flex-1 truncate font-mono text-ui-lg text-fg" title={d.path}>
          {d.oldPath && d.oldPath !== d.path ? `${d.oldPath} → ${d.path}` : d.path}
        </span>
        <span className="shrink-0 text-ui text-muted">{label}</span>
        <div className="flex shrink-0 overflow-hidden rounded-lg border border-border-strong bg-elevated text-ui">
          {(['unified', 'split'] as const).map((m) => (
            <button
              key={m}
              type="button"
              onClick={() => useGit.getState().setDiffMode(m)}
              className={`px-2 py-0.5 outline-none ${mode === m ? 'bg-accent/12 text-accent-text' : 'text-muted hover:bg-hover'}`}
            >
              {m === 'unified' ? 'Unified' : 'Split'}
            </button>
          ))}
        </div>
        <IconButton label="Cerrar diff" onClick={() => useGit.getState().closeDiff()}><X size={13} /></IconButton>
      </div>
      <div className="min-h-0 flex-1">{body}</div>
    </div>
  );
}
