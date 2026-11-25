import { useEffect, useRef, useState } from 'react';
import { X } from 'lucide-react';
import { FileIcon } from '../icons/SymbolIcon';
import { isClassifiable, useSymbols } from '../../store/symbols';
import { useEditor } from '../../store/editor';
import { countsFor, useLsp } from '../../store/lsp';
import { ContextMenu, type MenuItem } from '../ui/ContextMenu';
import { toggleBlame } from './blame';
import { useGit } from '../../store/git';
import { useWorkspace } from '../../store/workspace';
import { relativeTo, rootFor } from '../../lib/paths';

/** Icono de pestaña: pide la clasificación Java/Kotlin si aún no está en caché. */
function TabIcon({ path, name }: { path: string; name: string }) {
  const symbol = useSymbols((s) => s.symbols[path]);
  const request = useSymbols((s) => s.request);
  useEffect(() => {
    if (isClassifiable(path)) request([path]);
  }, [path, request]);
  return <FileIcon filename={name} path={path} symbol={symbol} />;
}

/** Tooltip de pestaña: con varias raíces, `raíz/ruta/relativa`; si no, la ruta completa. */
function tabTitle(roots: { path: string; name: string }[], path: string): string {
  const r = roots.length > 1 ? rootFor(roots, path) : null;
  return r ? `${r.name}/${relativeTo(r.path, path)}\n${path}` : path;
}

export function TabBar() {
  const tabs = useEditor((s) => s.tabs);
  const active = useEditor((s) => s.active);
  const setActive = useEditor((s) => s.setActive);
  const requestClose = useEditor((s) => s.requestClose);
  const counts = useLsp((s) => s.counts);
  const roots = useWorkspace((s) => s.roots);
  const activeRef = useRef<HTMLDivElement>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);

  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [active]);

  return (
    <div role="tablist" className="flex h-10 shrink-0 items-center overflow-x-auto px-1">
      {tabs.map((t) => {
        const isActive = t.path === active;
        const diag = countsFor(counts, t.path);
        return (
          <div
            key={t.path}
            ref={isActive ? activeRef : undefined}
            role="tab"
            aria-selected={isActive}
            title={t.isExternal ? `Archivo externo al proyecto: ${t.path}` : tabTitle(roots, t.path)}
            onClick={() => setActive(t.path)}
            onContextMenu={(e) => {
              e.preventDefault();
              setMenu({
                x: e.clientX,
                y: e.clientY,
                items: [
                  {
                    label: 'Annotate with Git Blame',
                    onSelect: () => {
                      setActive(t.path);
                      setTimeout(() => void toggleBlame(t.path), 60);
                    },
                  },
                  {
                    label: 'Git: Show History',
                    onSelect: () => {
                      useGit.getState().showFileHistory(t.path);
                    },
                  },
                  { label: 'Close', separator: true, onSelect: () => void requestClose(t.path) },
                ],
              });
            }}
            // Clic medio cierra (mousedown evita el autoscroll del navegador).
            onMouseDown={(e) => {
              if (e.button === 1) e.preventDefault();
            }}
            onAuxClick={(e) => {
              if (e.button === 1) {
                e.preventDefault();
                void requestClose(t.path);
              }
            }}
            className={`group mx-0.5 my-1 flex shrink-0 cursor-pointer items-center gap-1.5 rounded-lg border px-3 py-1.5 text-ui-lg ${
              t.isExternal
                ? 'border-orange-500/50 bg-orange-500/12 text-orange-300'
                : isActive
                  ? 'border-border-strong bg-tab-active text-fg-strong'
                  : 'border-transparent text-muted hover:bg-hover'
            }`}
          >
            {t.isExternal && <span aria-hidden className="h-1.5 w-1.5 shrink-0 rounded-full bg-orange-400" />}
            {isActive && !t.dirty && !t.isExternal && <span aria-hidden className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent" />}
            <TabIcon path={t.path} name={t.name} />
            <span className="max-w-[180px] truncate font-mono text-[12px]">{t.name}</span>
            {(diag.errors > 0 || diag.warnings > 0) && (
              <span
                aria-hidden
                title={`${diag.errors} errores, ${diag.warnings} avisos`}
                className={`h-1.5 w-1.5 shrink-0 rounded-full ${diag.errors > 0 ? 'bg-danger' : 'bg-accent'}`}
              />
            )}
            <button
              type="button"
              aria-label={t.dirty ? 'Cerrar (cambios sin guardar)' : 'Cerrar'}
              onClick={(e) => {
                e.stopPropagation();
                void requestClose(t.path);
              }}
              className="relative flex h-4 w-4 items-center justify-center rounded text-muted outline-none hover:bg-hover-strong hover:text-fg"
            >
              {t.dirty && <span className="absolute text-[14px] leading-none text-accent group-hover:hidden">•</span>}
              <X size={11} className={t.dirty ? 'hidden group-hover:block' : isActive ? '' : 'opacity-0 group-hover:opacity-100'} />
            </button>
          </div>
        );
      })}
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
    </div>
  );
}
