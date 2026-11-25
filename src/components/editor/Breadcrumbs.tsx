import { Fragment } from 'react';
import { ChevronRight } from 'lucide-react';
import { useEditor } from '../../store/editor';
import { useWorkspace } from '../../store/workspace';
import { useSymbols } from '../../store/symbols';
import { FileIcon } from '../icons/SymbolIcon';
import { relativeTo, rootFor } from '../../lib/paths';

/**
 * Ruta del archivo activo relativa al root del workspace.
 * Segmentos no clicables: `useEditor.reveal` solo salta a línea/columna dentro de un archivo,
 * no existe una acción de "revelar carpeta" en los stores (y no se crean aquí).
 */
export function Breadcrumbs() {
  const active = useEditor((s) => s.active);
  const roots = useWorkspace((s) => s.roots);
  const symbol = useSymbols((s) => (active ? s.symbols[active] : undefined));
  if (!active) return null;

  const owner = rootFor(roots, active);
  const rel = owner ? relativeTo(owner.path, active) : active;
  const parts = rel.replace(/\\/g, '/').split('/').filter(Boolean);
  // Con varias raíces, el nombre de la raíz es el primer segmento.
  if (owner && roots.length > 1) parts.unshift(owner.name);
  const fileIdx = parts.length - 1;
  if (parts.length === 0) return null;

  return (
    <div
      aria-label="Ruta del archivo"
      title={active}
      className="flex h-6 shrink-0 items-center gap-1 overflow-hidden whitespace-nowrap px-4 py-1 text-[11px] text-faint"
    >
      {parts.map((seg, i) => (
        <Fragment key={i}>
          {i > 0 && <ChevronRight size={10} className="shrink-0" aria-hidden />}
          {i === fileIdx && <FileIcon filename={seg} path={active} symbol={symbol} className="h-3.5 w-3.5" />}
          <span className={`truncate ${i === parts.length - 1 ? 'text-muted' : ''}`}>{seg}</span>
        </Fragment>
      ))}
    </div>
  );
}
