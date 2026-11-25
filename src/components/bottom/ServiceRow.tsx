import { Pencil, Play, RotateCw, Square, Trash2, Zap } from 'lucide-react';
import { TechIcon } from '../icons/SymbolIcon';
import { techOf } from '../../lib/tech';
import type { ServiceConfig, ServiceState, ServiceStatus } from '../../lib/types';

const DOT: Record<ServiceState, string> = {
  running: 'bg-run animate-pulse',
  stopped: 'bg-muted',
  error: 'bg-stop',
  starting: 'bg-accent animate-pulse',
  stopping: 'bg-accent animate-pulse',
};

const PILL: Record<ServiceState, string> = {
  running: 'bg-run/15 text-run-text',
  stopped: 'bg-elevated-hover text-muted',
  error: 'bg-stop/15 text-stop-text',
  starting: 'bg-accent/12 text-accent-text',
  stopping: 'bg-accent/12 text-accent-text',
};

const BTN =
  'inline-flex h-7 w-7 items-center justify-center rounded-md text-muted outline-none transition-colors focus-visible:ring-1 focus-visible:ring-accent disabled:opacity-40';

const LABEL: Record<ServiceState, string> = {
  running: 'Running',
  stopped: 'Stopped',
  error: 'Error',
  starting: 'Starting',
  stopping: 'Stopping',
};

interface Props {
  config: ServiceConfig;
  status: ServiceStatus | undefined;
  selected: boolean;
  onSelect: () => void;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
  onEdit?: () => void;
  onDelete?: () => void;
  onKillPort: (port: number) => void;
}

export function ServiceRow({ config, status, selected, onSelect, onStart, onStop, onRestart, onEdit, onDelete, onKillPort }: Props) {
  const state: ServiceState = status?.state ?? 'stopped';
  const active = state === 'running' || state === 'starting';
  const busy = state === 'starting' || state === 'stopping';
  const port = config.port;
  const portOpen = !!status?.portOpen && port != null;

  return (
    <div
      role="option"
      aria-selected={selected}
      onClick={onSelect}
      className={`group mx-2 mb-1.5 cursor-pointer rounded-lg border p-2.5 ${
        selected ? 'border-accent/30 bg-card-hover' : 'border-border bg-card hover:bg-card-hover'
      }`}
    >
      <div className="flex items-center gap-2">
        <TechIcon tech={techOf(config)} />
        <span className="min-w-0 flex-1 truncate text-ui-lg text-fg" title={`${config.id}\n${config.command}`}>
          {config.name}
        </span>
        <span className={`inline-flex shrink-0 items-center gap-1.5 rounded-full px-2 py-0.5 text-[11px] ${PILL[state]}`}>
          <span className={`h-1.5 w-1.5 rounded-full ${DOT[state]}`} />
          {LABEL[state]}
        </span>
      </div>
      <div className="mt-1.5 flex items-center gap-2 font-mono text-[11px] text-muted">
        {port != null && (
          <span
            className={`rounded bg-ink/[0.05] px-1.5 py-0.5 ${portOpen ? 'text-run-text' : ''}`}
            title={portOpen ? 'Puerto activo' : 'Puerto esperado'}
          >
            :{port}
          </span>
        )}
        {state === 'running' && status && (
          <>
            <span>RAM {status.memoryMb.toFixed(1)} MB</span>
            <span>CPU {status.cpu.toFixed(1)}%</span>
          </>
        )}
        {state === 'error' && status?.message && <span className="truncate text-stop-text">{status.message}</span>}
        {portOpen && port != null && (
          <button
            type="button"
            title="Kill Port"
            onClick={(e) => {
              e.stopPropagation();
              onKillPort(port);
            }}
            className="inline-flex items-center gap-1 rounded-md bg-ink/[0.05] px-1.5 py-0.5 text-[10px] text-fg outline-none hover:bg-stop/15 hover:text-stop-text focus-visible:ring-1 focus-visible:ring-accent"
          >
            <Zap size={10} /> Kill Port
          </button>
        )}
        <span className="ml-auto flex items-center gap-0.5" onClick={(e) => e.stopPropagation()}>
          {active || state === 'stopping' ? (
            <button type="button" title="Stop" aria-label="Stop" onClick={onStop} disabled={state === 'stopping'} className={`${BTN} hover:bg-stop/15 hover:text-stop-text`}>
              <Square size={13} />
            </button>
          ) : (
            <button type="button" title="Start" aria-label="Start" onClick={onStart} className={`${BTN} hover:bg-run/15 hover:text-run-text`}>
              <Play size={13} />
            </button>
          )}
          <button type="button" title="Restart" aria-label="Restart" onClick={onRestart} disabled={busy} className={`${BTN} hover:bg-accent/12 hover:text-accent-text`}>
            <RotateCw size={13} />
          </button>
          {onEdit && (
            <button type="button" title="Editar" aria-label="Editar" onClick={onEdit} className={`${BTN} hover:bg-hover hover:text-fg`}>
              <Pencil size={13} />
            </button>
          )}
          {onDelete && (
            <button type="button" title="Eliminar" aria-label="Eliminar" onClick={onDelete} className={`${BTN} hover:bg-stop/15 hover:text-stop-text`}>
              <Trash2 size={13} />
            </button>
          )}
        </span>
      </div>
    </div>
  );
}
