import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { ChevronDown, FileCode, Settings2 } from 'lucide-react';
import { TechIcon } from './TechIcon';
import { FileIcon } from '../icons/SymbolIcon';
import { selectionKey, selId, techForRunKind, useRunConfigStore, type RunSelection } from '../../store/runConfigs';
import { useServices } from '../../store/services';
import { useEditor } from '../../store/editor';
import { useWorkspace } from '../../store/workspace';
import { basename } from '../../lib/paths';
import { TECH_LABEL, TECH_ORDER, techOf } from '../../lib/tech';
import type { ResolvedRun, ServiceState, Tech } from '../../lib/types';

export const STATE_DOT: Record<ServiceState, string> = {
  running: 'bg-run',
  stopped: 'bg-muted',
  error: 'bg-stop',
  starting: 'bg-accent animate-pulse',
  stopping: 'bg-accent animate-pulse',
};

const KIND_LABEL = { springBoot: 'Spring Boot', python: 'Python', node: 'Node/TS', go: 'Go', rust: 'Rust', shell: 'Shell' } as const;

type Row =
  | { type: 'header'; label: string }
  | { type: 'sub'; label: string }
  | { type: 'sep' }
  | {
      type: 'item';
      key: string;
      label: string;
      meta?: string;
      tag?: string;
      icon: ReactNode;
      disabled?: boolean;
      current?: boolean;
      onSelect: () => void;
    };

interface Props {
  /** Selección vigente (ya saneada). */
  sel: RunSelection;
  label: string;
  state: ServiceState;
  resolved: ResolvedRun | null;
}

export function RunDropdown({ sel, label, state, resolved }: Props) {
  const [open, setOpen] = useState(false);
  const [cursor, setCursor] = useState(0);
  const [rect, setRect] = useState<DOMRect | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popRef = useRef<HTMLDivElement>(null);

  const files = useRunConfigStore((s) => s.files);
  const roots = useWorkspace((s) => s.roots);
  const detected = useRunConfigStore((s) => s.detected);
  const select = useRunConfigStore((s) => s.select);
  const refreshDetected = useRunConfigStore((s) => s.refreshDetected);
  const openEditor = useRunConfigStore((s) => s.openEditor);
  const services = useServices((s) => s.configs);
  const activePath = useEditor((s) => (s.active && s.tabs.some((t) => t.path === s.active) ? s.active : null));

  const close = (refocus = true) => {
    setOpen(false);
    if (refocus) triggerRef.current?.focus();
  };

  const rows = useMemo<Row[]>(() => {
    const out: Row[] = [];
    const pick = (s: RunSelection) => () => {
      select(s);
      close();
    };
    const cur = selectionKey(sel);
    out.push({ type: 'header', label: 'Current File' });
    out.push({
      type: 'item',
      key: 'file',
      label: activePath ? basename(activePath) : 'Sin archivo activo',
      meta: activePath ? (resolved?.runner ?? undefined) : undefined,
      icon: activePath ? <FileIcon filename={basename(activePath)} path={activePath} /> : <FileCode size={13} className="shrink-0 text-accent" />,
      disabled: !activePath,
      current: cur === 'file',
      onSelect: pick({ kind: 'file' }),
    });

    const multi = roots.length > 1;
    const nameOf = (r: string | undefined) => roots.find((x) => x.path === r)?.name ?? '';

    if (detected.length > 0) {
      out.push({ type: 'header', label: 'Auto-detected' });
      for (const rt of roots) {
        const inRoot = detected.filter((d) => d.root === rt.path);
        if (inRoot.length === 0) continue;
        const byTech = new Map<Tech, typeof detected>();
        for (const d of inRoot) {
          const t = techOf(d);
          byTech.set(t, [...(byTech.get(t) ?? []), d]);
        }
        for (const t of TECH_ORDER) {
          const list = byTech.get(t);
          if (!list) continue;
          out.push({ type: 'sub', label: multi ? `${rt.name} · ${TECH_LABEL[t]}` : TECH_LABEL[t] });
          for (const d of list) {
            const s: RunSelection = { kind: 'detected', id: selId(rt.path, d.id) };
            out.push({
              type: 'item',
              key: selectionKey(s),
              label: d.name,
              meta: [d.framework, d.port != null ? `:${d.port}` : null].filter(Boolean).join(' · ') || undefined,
              icon: <TechIcon tech={t} />,
              current: cur === selectionKey(s),
              onSelect: pick(s),
            });
          }
        }
      }
    }

    const cfgCount = Object.values(files).reduce((n, f) => n + f.configs.length, 0);
    if (cfgCount > 0 || services.length > 0) {
      out.push({ type: 'header', label: 'Custom Configurations' });
      for (const rt of roots) {
        const list = files[rt.path]?.configs ?? [];
        if (list.length === 0) continue;
        if (multi) out.push({ type: 'sub', label: rt.name });
        for (const c of list) {
          const s: RunSelection = { kind: 'config', id: selId(rt.path, c.id) };
          out.push({
            type: 'item',
            key: selectionKey(s),
            label: c.name,
            meta: [KIND_LABEL[c.kind], c.port != null ? `:${c.port}` : null].filter(Boolean).join(' · '),
            icon: <TechIcon tech={techForRunKind(c.kind)} />,
            current: cur === selectionKey(s),
            onSelect: pick(s),
          });
        }
      }
      let lastRoot: string | undefined;
      const rootIdx = (r?: string) => Math.max(0, roots.findIndex((x) => x.path === r));
      for (const c of [...services].sort((a, b) => rootIdx(a.root) - rootIdx(b.root))) {
        if (multi && c.root !== lastRoot) {
          lastRoot = c.root;
          out.push({ type: 'sub', label: `${nameOf(c.root)} · Services` });
        }
        const s: RunSelection = { kind: 'service', id: c.id };
        out.push({
          type: 'item',
          key: selectionKey(s),
          label: c.name,
          meta: c.port != null ? `:${c.port}` : undefined,
          tag: 'Services',
          icon: <TechIcon tech={techOf(c)} />,
          current: cur === selectionKey(s),
          onSelect: pick(s),
        });
      }
    }

    out.push({ type: 'sep' });
    out.push({
      type: 'item',
      key: 'edit',
      label: '+ Edit / New Configuration…',
      icon: <Settings2 size={13} className="shrink-0 text-muted" />,
      onSelect: () => {
        close(false);
        openEditor(sel.kind === 'config' ? sel.id : null);
      },
    });
    return out;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sel, files, detected, services, roots, activePath, resolved, select, openEditor]);

  const selectable = (i: number) => {
    const r = rows[i];
    return !!r && r.type === 'item' && !r.disabled;
  };
  const move = (from: number, dir: 1 | -1) => {
    for (let n = 1; n <= rows.length; n++) {
      const i = (from + dir * n + rows.length * 2) % rows.length;
      if (selectable(i)) return i;
    }
    return from;
  };

  const openList = () => {
    setRect(triggerRef.current?.getBoundingClientRect() ?? null);
    setOpen(true);
    void refreshDetected();
  };

  // Cursor inicial: la selección actual.
  useEffect(() => {
    if (!open) return;
    const i = rows.findIndex((r) => r.type === 'item' && r.current);
    setCursor(i >= 0 ? i : rows.findIndex((_, j) => selectable(j)));
    popRef.current?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  useLayoutEffect(() => {
    if (!open) return;
    popRef.current?.querySelector(`[data-row="${cursor}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [open, cursor]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (popRef.current?.contains(t) || triggerRef.current?.contains(t)) return;
      setOpen(false);
    };
    const onClose = () => setOpen(false);
    window.addEventListener('mousedown', onDown);
    window.addEventListener('blur', onClose);
    window.addEventListener('resize', onClose);
    return () => {
      window.removeEventListener('mousedown', onDown);
      window.removeEventListener('blur', onClose);
      window.removeEventListener('resize', onClose);
    };
  }, [open]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      setCursor((c) => move(c, 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setCursor((c) => move(c, -1));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const r = rows[cursor];
      if (r?.type === 'item' && !r.disabled) r.onSelect();
    } else if (e.key === 'Tab') {
      e.preventDefault();
    }
  };

  const left = rect ? Math.max(4, Math.min(rect.left, window.innerWidth - 360 - 4)) : 0;

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label="Configuración de ejecución"
        title={resolved && sel.kind === 'file' ? `${resolved.runner}: ${resolved.command}` : 'Configuración de ejecución'}
        onClick={() => (open ? close(false) : openList())}
        onKeyDown={(e) => {
          if (!open && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
            e.preventDefault();
            openList();
          }
        }}
        className="flex h-7 max-w-[240px] items-center gap-1.5 rounded-md px-2 text-ui text-fg outline-none transition-colors hover:bg-hover"
      >
        <span className={`h-2 w-2 shrink-0 rounded-full ${STATE_DOT[state]}`} />
        <span className="truncate">{label}</span>
        <ChevronDown size={12} className="shrink-0 text-muted" />
      </button>
      {open &&
        rect &&
        createPortal(
          <div
            ref={popRef}
            role="listbox"
            tabIndex={-1}
            aria-label="Configuraciones de ejecución"
            onKeyDown={onKeyDown}
            style={{ left, top: rect.bottom + 4, width: 360 }}
            className="fixed z-[100] max-h-[60vh] overflow-auto rounded-lg border border-border-strong bg-elevated p-1 shadow-xl outline-none"
          >
            {rows.map((r, i) => {
              if (r.type === 'header') {
                return (
                  <div key={`h${i}`} className="px-3 pb-0.5 pt-2 text-[10px] font-medium uppercase tracking-wider text-muted">
                    {r.label}
                  </div>
                );
              }
              if (r.type === 'sub') {
                return (
                  <div key={`s${i}`} className="px-3 pt-1 text-[11px] text-muted/80">
                    {r.label}
                  </div>
                );
              }
              if (r.type === 'sep') return <div key={`p${i}`} className="my-1 h-px bg-border" />;
              const active = i === cursor;
              return (
                <div
                  key={r.key}
                  data-row={i}
                  role="option"
                  aria-selected={!!r.current}
                  aria-disabled={r.disabled}
                  onMouseEnter={() => !r.disabled && setCursor(i)}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => !r.disabled && r.onSelect()}
                  className={`flex cursor-pointer items-center gap-2 rounded-md px-2 py-1 text-ui-lg ${
                    active ? 'bg-hover ring-1 ring-accent' : ''
                  } ${r.disabled ? 'cursor-not-allowed opacity-40' : ''} ${r.current ? 'text-accent' : 'text-fg'}`}
                >
                  {r.icon}
                  <span className="min-w-0 flex-1 truncate">{r.label}</span>
                  {r.meta && <span className="max-w-[45%] shrink-0 truncate font-mono text-[11px] text-muted">{r.meta}</span>}
                  {r.tag && (
                    <span className="shrink-0 rounded border border-border px-1 font-mono text-[10px] leading-4 text-muted">{r.tag}</span>
                  )}
                </div>
              );
            })}
          </div>,
          document.body,
        )}
    </>
  );
}
