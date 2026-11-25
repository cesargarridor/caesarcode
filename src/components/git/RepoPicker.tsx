import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { GitBranch, Layers, Loader2 } from 'lucide-react';
import { FolderIcon } from '../icons/SymbolIcon';
import { useGit } from '../../store/git';
import { useWorkspace } from '../../store/workspace';
import { basename } from '../../lib/paths';
import type { RepoEntry, RepoSummary } from '../../lib/types';

const SEARCH_THRESHOLD = 8;

/** Rama de un resumen: nombre, o `HEAD <sha>` desacoplado. */
export function summaryBranch(s: RepoSummary | undefined): string {
  if (!s) return '…';
  if (s.error) return 'error';
  return s.branch ?? (s.detached ? 'HEAD' : '—');
}

/** `●N ↑a ↓b` de un repo. */
export function RepoStats({ s }: { s: RepoSummary | undefined }) {
  if (!s || s.error) return null;
  return (
    <>
      {s.changed > 0 && <span className="shrink-0 font-mono text-[11px] text-accent">●{s.changed}</span>}
      {s.ahead > 0 && <span className="shrink-0 font-mono text-[11px] text-run-text">↑{s.ahead}</span>}
      {s.behind > 0 && <span className="shrink-0 font-mono text-[11px] text-accent">↓{s.behind}</span>}
    </>
  );
}

interface PickerProps {
  anchor: DOMRect;
  onClose: () => void;
  /** Tras elegir un repo (o "All repos"); p. ej. cambiar a la vista Git. */
  onPicked?: () => void;
}

type Item = { type: 'all' } | { type: 'repo'; repo: RepoEntry };

/** Popover con todos los repos del workspace: nombre + rama + cambios + ahead/behind, buscador si hay muchos. */
export function RepoPicker({ anchor, onClose, onPicked }: PickerProps) {
  const repos = useGit((s) => s.repos);
  const summaries = useGit((s) => s.summaries);
  const busyByRepo = useGit((s) => s.busyByRepo);
  const activeRepo = useGit((s) => s.activeRepo);
  const view = useGit((s) => s.view);
  const roots = useWorkspace((s) => s.roots);
  const [query, setQuery] = useState('');
  const [cursor, setCursor] = useState(0);
  const popRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const showSearch = repos.length > SEARCH_THRESHOLD;
  const multiRoot = roots.length > 1;

  const items = useMemo<Item[]>(() => {
    const q = query.trim().toLowerCase();
    const list = repos.filter((r) => !q || r.name.toLowerCase().includes(q) || r.top.toLowerCase().includes(q));
    const out: Item[] = [];
    if (!q && repos.length > 1) out.push({ type: 'all' });
    for (const repo of list) out.push({ type: 'repo', repo });
    return out;
  }, [repos, query]);

  useEffect(() => {
    void useGit.getState().loadSummaries();
  }, []);

  useEffect(() => {
    const i = items.findIndex((it) => (it.type === 'repo' ? it.repo.top === activeRepo && view === 'repo' : view === 'all'));
    setCursor(i >= 0 ? i : 0);
    (showSearch ? inputRef.current : popRef.current)?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    setCursor((c) => Math.min(c, Math.max(0, items.length - 1)));
  }, [items.length]);

  useLayoutEffect(() => {
    popRef.current?.querySelector(`[data-row="${cursor}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [cursor]);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (!popRef.current?.contains(e.target as Node)) onClose();
    };
    window.addEventListener('mousedown', onDown);
    window.addEventListener('blur', onClose);
    window.addEventListener('resize', onClose);
    return () => {
      window.removeEventListener('mousedown', onDown);
      window.removeEventListener('blur', onClose);
      window.removeEventListener('resize', onClose);
    };
  }, [onClose]);

  const pick = (it: Item) => {
    const g = useGit.getState();
    if (it.type === 'all') g.setView('all');
    else if (it.repo.top !== g.activeRepo) void g.setActiveRepo(it.repo.top);
    else g.setView('repo');
    onClose();
    onPicked?.();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      setCursor((c) => (items.length ? (c + 1) % items.length : 0));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setCursor((c) => (items.length ? (c - 1 + items.length) % items.length : 0));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const it = items[cursor];
      if (it) pick(it);
    } else if (e.key === 'Tab') {
      e.preventDefault();
    }
  };

  const width = 380;
  const left = Math.max(4, Math.min(anchor.left, window.innerWidth - width - 4));

  return createPortal(
    <div
      ref={popRef}
      role="listbox"
      tabIndex={-1}
      aria-label="Repositorios Git"
      onKeyDown={onKeyDown}
      style={{ left, top: anchor.bottom + 4, width }}
      className="fixed z-[100] flex max-h-[60vh] flex-col rounded-lg border border-border-strong bg-elevated p-1 shadow-xl outline-none"
    >
      {showSearch && (
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Buscar repositorio…"
          aria-label="Buscar repositorio"
          className="mb-1 h-7 w-full shrink-0 rounded-md border border-border-strong bg-canvas px-2 text-ui-lg text-fg outline-none placeholder:text-faint focus:border-accent"
        />
      )}
      <div className="min-h-0 flex-1 overflow-auto">
        {items.map((it, i) => {
          const active = i === cursor;
          const base = `flex cursor-pointer items-center gap-2 rounded-md px-2 py-1 text-ui-lg ${active ? 'bg-hover ring-1 ring-accent' : ''}`;
          if (it.type === 'all') {
            return (
              <div
                key="all"
                data-row={i}
                role="option"
                aria-selected={view === 'all'}
                onMouseEnter={() => setCursor(i)}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => pick(it)}
                className={`${base} ${view === 'all' ? 'text-accent' : 'text-fg'}`}
              >
                <Layers size={14} className="shrink-0 text-muted" />
                <span className="flex-1">All repos</span>
                <span className="font-mono text-[11px] text-muted">{repos.length}</span>
              </div>
            );
          }
          const { repo } = it;
          const s = summaries[repo.top];
          const rootName = multiRoot ? roots.find((r) => r.path === repo.root)?.name : undefined;
          const current = view === 'repo' && repo.top === activeRepo;
          return (
            <div
              key={repo.top}
              data-row={i}
              role="option"
              aria-selected={current}
              title={repo.top}
              onMouseEnter={() => setCursor(i)}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => pick(it)}
              className={`${base} ${current ? 'text-accent' : 'text-fg'}`}
            >
              <FolderIcon name={repo.name} isModule />
              <span className="min-w-0 shrink truncate font-medium">{repo.name}</span>
              {(repo.nested || (rootName && rootName !== repo.name)) && (
                <span className="min-w-0 shrink-[2] truncate text-[11px] text-faint">
                  {repo.nested ? `en ${basename(repo.root)}` : rootName}
                </span>
              )}
              <span className="flex-1" />
              {busyByRepo[repo.top] && <Loader2 size={12} className="shrink-0 animate-spin text-accent" aria-label="Trabajando" />}
              <span className="flex min-w-0 max-w-[110px] shrink-0 items-center gap-1 font-mono text-[11px] text-muted">
                <GitBranch size={11} className="shrink-0" />
                <span className="truncate">{summaryBranch(s)}</span>
              </span>
              <RepoStats s={s} />
            </div>
          );
        })}
        {items.length === 0 && <div className="px-3 py-2 text-ui text-muted">Sin repositorios{query ? ' que coincidan' : ''}.</div>}
      </div>
    </div>,
    document.body,
  );
}
