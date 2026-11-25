import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { quickOpen } from '../../lib/ipc';
import { toMessage } from '../../lib/errors';
import { useEditor } from '../../store/editor';
import { useUi } from '../../store/ui';
import { useWorkspace } from '../../store/workspace';
import type { QuickOpenItem } from '../../lib/types';
import { FileIcon } from '../icons/SymbolIcon';

const LIMIT = 50;

/** Pinta `text` (chars desde `offset` dentro de relPath) resaltando los índices de coincidencia. */
function Highlighted({ text, offset, indices }: { text: string; offset: number; indices: Set<number> }) {
  const chars = Array.from(text);
  return (
    <>
      {chars.map((c, i) =>
        indices.has(offset + i) ? (
          <span key={i} className="text-accent">
            {c}
          </span>
        ) : (
          <span key={i}>{c}</span>
        ),
      )}
    </>
  );
}

function Row({ item, selected, showRoot, onPick, onHover }: { item: QuickOpenItem; selected: boolean; showRoot: boolean; onPick: () => void; onHover: () => void }) {
  const chars = Array.from(item.relPath);
  const slash = chars.lastIndexOf('/');
  const dir = chars.slice(0, slash + 1).join('');
  const base = chars.slice(slash + 1).join('');
  const idx = new Set(item.indices);
  return (
    <div
      role="option"
      aria-selected={selected}
      data-selected={selected}
      onMouseMove={onHover}
      onClick={onPick}
      className={`mx-1.5 flex h-7 cursor-pointer items-center gap-2 rounded-md px-2 ${selected ? 'bg-accent/12' : ''}`}
    >
      <FileIcon filename={base} path={item.relPath} />
      <span className="shrink-0 font-mono text-[12px] text-fg">
        <Highlighted text={base} offset={slash + 1} indices={idx} />
      </span>
      <span className="min-w-0 truncate font-mono text-[11px] text-muted">
        <Highlighted text={dir} offset={0} indices={idx} />
      </span>
      {showRoot && item.root && (
        <span className="ml-auto shrink-0 rounded bg-hover px-1.5 font-mono text-[10px] text-muted" title="Proyecto">{item.root}</span>
      )}
    </div>
  );
}

export function QuickOpen() {
  const visible = useUi((s) => !!s.modals.quickOpen);
  const close = useUi((s) => s.closeModal);
  const root = useWorkspace((s) => s.root);
  const multiRoot = useWorkspace((s) => s.roots.length > 1);
  const openFile = useEditor((s) => s.openFile);
  const pushToast = useUi((s) => s.pushToast);
  const [query, setQuery] = useState('');
  const [items, setItems] = useState<QuickOpenItem[]>([]);
  const [sel, setSel] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const seq = useRef(0);

  useEffect(() => {
    if (visible) {
      setQuery('');
      setSel(0);
      setItems([]);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [visible]);

  useEffect(() => {
    if (!visible || !root) return;
    const id = ++seq.current;
    const t = setTimeout(async () => {
      try {
        const res = await quickOpen(query, LIMIT);
        if (id !== seq.current) return;
        setItems(res);
        setSel(0);
      } catch (e) {
        if (id === seq.current) pushToast(toMessage(e));
      }
    }, query ? 60 : 0);
    return () => clearTimeout(t);
  }, [query, visible, root, pushToast]);

  useEffect(() => {
    listRef.current?.querySelector('[data-selected="true"]')?.scrollIntoView({ block: 'nearest' });
  }, [sel, items]);

  if (!visible) return null;

  const pick = (item: QuickOpenItem | undefined) => {
    if (!item) return;
    close('quickOpen');
    void openFile(item.path);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      close('quickOpen');
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSel((s) => (items.length ? (s + 1) % items.length : 0));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSel((s) => (items.length ? (s - 1 + items.length) % items.length : 0));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      pick(items[sel]);
    }
  };

  return createPortal(
    <div
      className="fixed inset-0 z-[90] flex items-start justify-center bg-black/50 pt-[12vh] backdrop-blur-sm"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) close('quickOpen');
      }}
    >
      <div
        role="dialog"
        aria-label="Abrir archivo"
        onKeyDown={onKeyDown}
        className="w-[600px] max-w-[92vw] overflow-hidden rounded-xl border border-border-strong bg-elevated shadow-2xl"
      >
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={root ? 'Buscar archivo por nombre…' : 'Abre una carpeta primero'}
          disabled={!root}
          spellCheck={false}
          autoComplete="off"
          className="h-10 w-full border-b border-border bg-transparent px-3 font-mono text-[13px] text-fg outline-none placeholder:text-muted"
        />
        <div ref={listRef} role="listbox" className="max-h-[50vh] overflow-auto py-1">
          {items.map((it, i) => (
            <Row key={it.path} item={it} showRoot={multiRoot} selected={i === sel} onPick={() => pick(it)} onHover={() => setSel(i)} />
          ))}
          {root && items.length === 0 && <div className="px-3 py-2 text-ui text-muted">Sin coincidencias.</div>}
        </div>
      </div>
    </div>,
    document.body,
  );
}
