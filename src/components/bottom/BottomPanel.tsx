import { X } from 'lucide-react';
import { LogViewer } from './LogViewer';
import { ServicesPanel } from './ServicesPanel';
import { TerminalPanel } from './TerminalPanel';
import { useWorkspace } from '../../store/workspace';
import { useServices } from '../../store/services';
import { useUi, type BottomTab, type RunTab } from '../../store/ui';
import { STATE_DOT } from '../run/RunDropdown';

const TABS: { id: BottomTab; label: string }[] = [
  { id: 'services', label: 'Services' },
  { id: 'logs', label: 'Logs' },
  { id: 'terminal', label: 'Terminal' },
];

function RunTabButton({ run, active }: { run: RunTab; active: boolean }) {
  const setTab = useUi((s) => s.setBottomTab);
  const close = useUi((s) => s.closeRunTab);
  const state = useServices((s) => s.statuses[run.id]?.state ?? 'stopped');
  return (
    <div
      className={`group flex h-6 items-center rounded-md transition-colors hover:bg-hover ${active ? 'bg-hover-strong text-fg-strong' : 'text-muted'}`}
    >
      <button
        type="button"
        role="tab"
        aria-selected={active}
        onClick={() => setTab(`run:${run.id}`)}
        className="flex h-6 items-center gap-1.5 rounded-md pl-2.5 pr-1 text-xs outline-none focus-visible:ring-1 focus-visible:ring-accent"
      >
        <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${STATE_DOT[state]}`} />
        <span className="max-w-[160px] truncate">{run.label}</span>
      </button>
      <button
        type="button"
        aria-label={`Cerrar ${run.label}`}
        title="Cerrar pestaña (no detiene el proceso)"
        onClick={() => close(run.id)}
        className="mr-1 inline-flex h-4 w-4 items-center justify-center rounded-md text-muted outline-none hover:text-fg focus-visible:ring-1 focus-visible:ring-accent"
      >
        <X size={11} />
      </button>
    </div>
  );
}

export function BottomPanel() {
  const tab = useUi((s) => s.bottomTab);
  const setTab = useUi((s) => s.setBottomTab);
  const root = useWorkspace((s) => s.root);
  const runTabs = useUi((s) => s.runTabs);
  return (
    <div className="flex h-full flex-col">
      <div className="flex h-9 shrink-0 items-center gap-1 px-2" role="tablist">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={tab === t.id}
            onClick={() => setTab(t.id)}
            className={`h-6 rounded-md px-2.5 py-1 text-xs outline-none transition-colors hover:bg-hover focus-visible:ring-1 focus-visible:ring-accent ${
              tab === t.id ? 'bg-hover-strong text-fg-strong' : 'text-muted'
            }`}
          >
            {t.label}
          </button>
        ))}
        {runTabs.map((r) => (
          <RunTabButton key={r.id} run={r} active={tab === `run:${r.id}`} />
        ))}
      </div>
      <div className="relative min-h-0 flex-1">
        {tab === 'services' && <ServicesPanel />}
        {tab === 'logs' &&
          (root ? (
            <LogViewer serviceId="all" />
          ) : (
            <div className="flex h-full items-center justify-center text-ui text-muted">Abre una carpeta</div>
          ))}
        {runTabs.map((r) => tab === `run:${r.id}` && <LogViewer key={r.id} serviceId={r.id} />)}
        {/* Las terminales no se desmontan al cambiar de pestaña */}
        <div className="absolute inset-0" style={{ display: tab === 'terminal' ? 'block' : 'none' }}>
          <TerminalPanel visible={tab === 'terminal'} />
        </div>
      </div>
    </div>
  );
}
