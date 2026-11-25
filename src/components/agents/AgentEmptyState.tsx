import { Button } from '../ui/Button';
import { AGENT_LABELS } from '../../store/agents';
import type { AgentInfo } from '../../lib/ipcAgents';

/** Agente no encontrado en el PATH: comando oficial de instalación + Reintentar. */
export function AgentEmptyState({ info, busy, onRetry }: { info: AgentInfo; busy: boolean; onRetry: () => void }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-5 text-center">
      <div className="text-ui-lg font-medium text-fg">{AGENT_LABELS[info.id]} no está instalado</div>
      <div className="text-ui text-muted">
        No se encontró <code className="font-mono text-fg">{info.bin}</code> en el PATH. Instálalo con:
      </div>
      <code className="max-w-full select-all break-all rounded-md border border-border-strong bg-elevated px-2.5 py-1.5 font-mono text-ui text-fg">
        {info.installCmd}
      </code>
      <Button variant="primary" onClick={onRetry} disabled={busy}>
        {busy ? 'Comprobando…' : 'Reintentar'}
      </Button>
    </div>
  );
}
