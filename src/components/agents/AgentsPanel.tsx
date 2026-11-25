import { useEffect } from 'react';
import { AGENT_IDS, AGENT_LABELS, useAgents } from '../../store/agents';
import { AgentEmptyState } from './AgentEmptyState';
import { AgentTerminal } from './AgentTerminal';
import { ContextBar } from './ContextBar';

/** Panel derecho de agentes IA: pastillas, terminal del agente y barra de contexto. */
export function AgentsPanel() {
  const infos = useAgents((s) => s.infos);
  const detecting = useAgents((s) => s.detecting);
  const selected = useAgents((s) => s.selected);
  const started = useAgents((s) => s.started);

  useEffect(() => {
    void useAgents.getState().detect().then(() => {
      const st = useAgents.getState();
      st.select(st.selected);
    });
  }, []);

  const info = infos.find((i) => i.id === selected);
  const retry = () =>
    void useAgents.getState().detect().then(() => {
      const st = useAgents.getState();
      st.select(st.selected);
    });

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex items-center gap-1.5 border-b border-border px-2 py-1.5">
        {AGENT_IDS.map((id) => (
          <button
            key={id}
            type="button"
            onClick={() => useAgents.getState().select(id)}
            className={`rounded-full border px-2.5 py-0.5 text-ui outline-none transition-colors focus-visible:ring-1 focus-visible:ring-accent ${
              selected === id
                ? 'border-transparent bg-accent/15 text-accent-text'
                : 'border-border-strong text-muted hover:bg-hover hover:text-fg'
            }`}
          >
            {AGENT_LABELS[id]}
          </button>
        ))}
        <span
          className="ml-auto inline-flex shrink-0 items-center gap-1 rounded-full bg-ok/15 px-2 py-0.5 text-ui text-ok"
          title="El agente pide permiso antes de actuar: nunca se lanza con autoaprobación"
        >
          <svg width="11" height="11" viewBox="0 0 16 16" fill="currentColor" aria-hidden>
            <path d="M8 1a3.5 3.5 0 0 0-3.5 3.5V6H4a1 1 0 0 0-1 1v7a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V7a1 1 0 0 0-1-1h-.5V4.5A3.5 3.5 0 0 0 8 1Zm2 5H6V4.5a2 2 0 1 1 4 0V6Z" />
          </svg>
          Permisos activos
        </span>
      </div>
      <div className="relative min-h-0 flex-1">
        {AGENT_IDS.filter((id) => started[id]).map((id) => (
          <div key={id} className={id === selected ? 'h-full' : 'hidden'}>
            <AgentTerminal agent={id} visible={id === selected} />
          </div>
        ))}
        {info && !info.found && <AgentEmptyState info={info} busy={detecting} onRetry={retry} />}
        {!info && detecting && <div className="p-3 text-ui text-muted">Buscando agentes…</div>}
      </div>
      <ContextBar />
    </div>
  );
}
