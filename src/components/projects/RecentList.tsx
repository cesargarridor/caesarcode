import { useState } from 'react';
import { FolderOpen, Pin, PinOff, Plus, X } from 'lucide-react';
import { ContextMenu, type MenuItem } from '../ui/ContextMenu';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { IconButton } from '../ui/IconButton';
import type { RecentProject } from '../../lib/types';

export function timeAgo(ms: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - ms) / 1000));
  if (s < 60) return 'ahora';
  const m = Math.round(s / 60);
  if (m < 60) return `hace ${m} min`;
  const h = Math.round(m / 60);
  if (h < 24) return `hace ${h} h`;
  const d = Math.round(h / 24);
  if (d < 30) return `hace ${d} d`;
  return new Date(ms).toLocaleDateString();
}

interface Props {
  items: RecentProject[];
  query: string;
  onOpen: (p: RecentProject) => void;
  onRemove: (p: RecentProject) => void;
  onPin: (p: RecentProject) => void;
  /** Con un workspace abierto: añadir el proyecto como raíz adicional (Attach). */
  onAttach?: (p: RecentProject) => void;
}

/** Filtro de recientes por nombre o ruta (sin distinguir mayúsculas). */
export function filterRecent(items: RecentProject[], query: string): RecentProject[] {
  const q = query.trim().toLowerCase();
  if (!q) return items;
  return items.filter((p) => p.name.toLowerCase().includes(q) || p.path.toLowerCase().includes(q));
}

/** Lista de proyectos recientes: abrir, fijar y quitar de recientes (nunca del disco). */
export function RecentList({ items, query, onOpen, onRemove, onPin, onAttach }: Props) {
  const shown = filterRecent(items, query);
  const [menu, setMenu] = useState<{ x: number; y: number; p: RecentProject } | null>(null);
  const menuItems = (p: RecentProject): MenuItem[] => [
    { label: onAttach ? 'Abrir aquí (reemplaza)' : 'Abrir', disabled: !p.exists, onSelect: () => onOpen(p) },
    ...(onAttach ? [{ label: 'Añadir al workspace', disabled: !p.exists, onSelect: () => onAttach(p) }] : []),
    { label: p.pinned ? 'Desfijar' : 'Fijar', separator: true, onSelect: () => onPin(p) },
    { label: 'Quitar de recientes', danger: true, onSelect: () => onRemove(p) },
  ];
  if (shown.length === 0) {
    return <div className="px-3 py-8 text-center text-ui-lg text-muted">{items.length === 0 ? 'Aún no hay proyectos recientes' : 'Sin resultados'}</div>;
  }
  return (
    <ul className="space-y-1.5 p-1">
      {shown.map((p) => (
        <li
          key={p.path}
          onContextMenu={(e) => {
            e.preventDefault();
            setMenu({ x: e.clientX, y: e.clientY, p });
          }}
          className={`group flex items-center gap-2 rounded-lg border border-border bg-card px-3 py-2 hover:bg-card-hover ${p.exists ? '' : 'opacity-60'}`}
        >
          <button type="button" onClick={() => onOpen(p)} className="flex min-w-0 flex-1 items-center gap-3 text-left outline-none focus-visible:ring-1 focus-visible:ring-accent">
            <FolderOpen size={15} className={p.exists ? 'shrink-0 text-accent' : 'shrink-0 text-muted'} />
            <span className="min-w-0 flex-1">
              <span className="flex items-center gap-2">
                <span className="truncate text-ui-lg text-fg">{p.name}</span>
                {p.kind && <Badge>{p.kind}</Badge>}
                {(p.extraRoots ?? 0) > 0 && <Badge tone="accent">+{p.extraRoots}</Badge>}
                {!p.exists && <Badge tone="danger">no encontrado</Badge>}
              </span>
              <span className="block truncate font-mono text-[11px] text-muted" title={p.path}>{p.path}</span>
            </span>
            <span className="shrink-0 text-[11px] text-muted">{timeAgo(p.lastOpened)}</span>
          </button>
          {onAttach && p.exists && (
            <IconButton label="Añadir al workspace" onClick={() => onAttach(p)} className="shrink-0">
              <Plus size={13} />
            </IconButton>
          )}
          {!p.exists && <Button className="!h-6" onClick={() => onRemove(p)}>Quitar</Button>}
          <IconButton label={p.pinned ? 'Desfijar' : 'Fijar'} active={p.pinned} onClick={() => onPin(p)} className="shrink-0">
            {p.pinned ? <PinOff size={13} /> : <Pin size={13} />}
          </IconButton>
          {p.exists && (
            <IconButton label="Quitar de recientes" onClick={() => onRemove(p)} className="shrink-0 opacity-0 group-hover:opacity-100 focus-visible:opacity-100">
              <X size={13} />
            </IconButton>
          )}
        </li>
      ))}
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.p)} onClose={() => setMenu(null)} />}
    </ul>
  );
}
