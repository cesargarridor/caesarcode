import { useEffect, useState } from 'react';
import { useEditor } from '../../store/editor';
import { formatRange, readContext, useAgents, type ContextInfo } from '../../store/agents';
import { Button } from '../ui/Button';
import { modLabel } from '../../lib/keys';

/** Archivo activo + rango seleccionado y botón para inyectarlo en el PTY del agente. */
export function ContextBar() {
  const active = useEditor((s) => s.active);
  const cursor = useEditor((s) => s.cursor);
  const hasTerm = useAgents((s) => !!s.termIds[s.selected]);
  const [ctx, setCtx] = useState<ContextInfo | null>(null);

  useEffect(() => {
    const next = readContext();
    setCtx((prev) =>
      prev?.rel === next?.rel && prev?.range?.from === next?.range?.from && prev?.range?.to === next?.range?.to ? prev : next,
    );
  }, [active, cursor]);

  const label = ctx ? `${ctx.name}${ctx.range ? ` · ${formatRange(ctx.range)}` : ''}` : 'Sin archivo activo';
  return (
    <div className="flex items-center gap-2 border-t border-border px-2 py-1.5">
      <span className="min-w-0 flex-1 truncate text-ui text-muted" title={ctx?.rel}>
        {label}
      </span>
      <Button
        className="shrink-0"
        disabled={!ctx || !hasTerm}
        onClick={() => void useAgents.getState().sendContext()}
        title="Escribe la referencia al archivo en el agente (sin pulsar Enter)"
      >
        Enviar contexto ({modLabel === '⌘' ? '⌘L' : 'Ctrl+L'})
      </Button>
    </div>
  );
}
