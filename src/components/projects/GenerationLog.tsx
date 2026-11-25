import { useEffect, useRef } from 'react';
import { AlertTriangle, CheckCircle2, Loader2, XCircle } from 'lucide-react';
import { Button } from '../ui/Button';
import type { GenEvent } from '../../lib/types';

export type GenStatus = 'running' | 'done' | 'failed' | 'cancelled';

interface Props {
  events: GenEvent[];
  status: GenStatus;
  onCancel: () => void;
  onBack: () => void;
}

function Line({ ev }: { ev: GenEvent }) {
  switch (ev.type) {
    case 'step':
      return <div className="mt-1.5 font-medium text-accent">▸ {ev.label}</div>;
    case 'warning':
      return <div className="flex items-start gap-1 text-gold"><AlertTriangle size={11} className="mt-[3px] shrink-0" />{ev.message}</div>;
    case 'output':
      return <div className={`whitespace-pre-wrap break-all ${ev.stream === 'stderr' ? 'text-stop-text/70' : 'text-muted'}`}>{ev.line}</div>;
    case 'done':
      return <div className="mt-1.5 text-run-text">✔ Proyecto creado en {ev.root}{ev.runConfigs > 0 ? ` · ${ev.runConfigs} configuraciones de ejecución` : ''}</div>;
    case 'failed':
      return <div className="mt-1.5 text-stop-text">✖ {ev.message}</div>;
  }
}

/** Salida en streaming de la generación + estado + botón Cancelar. */
export function GenerationLog({ events, status, onCancel, onBack }: Props) {
  const endRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    endRef.current?.scrollIntoView({ block: 'end' });
  }, [events.length]);
  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <div className="flex items-center gap-2 text-ui-lg">
        {status === 'running' && <><Loader2 size={14} className="animate-spin text-accent" /><span>Generando proyecto…</span></>}
        {status === 'done' && <><CheckCircle2 size={14} className="text-run-text" /><span>Proyecto creado</span></>}
        {status === 'failed' && <><XCircle size={14} className="text-stop-text" /><span>La generación falló</span></>}
        {status === 'cancelled' && <><XCircle size={14} className="text-muted" /><span>Cancelado</span></>}
      </div>
      <div role="log" aria-live="polite" className="min-h-0 flex-1 overflow-auto rounded-lg border border-border bg-canvas p-2 font-mono text-[11px] leading-[16px]">
        {events.map((ev, i) => <Line key={i} ev={ev} />)}
        <div ref={endRef} />
      </div>
      <div className="flex justify-end gap-2">
        {status === 'running' ? (
          <Button variant="danger" onClick={onCancel}>Cancelar</Button>
        ) : (
          status !== 'done' && <Button onClick={onBack}>Volver al formulario</Button>
        )}
      </div>
    </div>
  );
}
