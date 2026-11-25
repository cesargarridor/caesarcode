import { ChevronDown, Plus } from 'lucide-react';
import { BrandMark } from './BrandMark';
import { IconButton } from '../ui/IconButton';
import { RunToolbar } from '../run/RunToolbar';
import { QuickOpenTrigger } from './QuickOpenTrigger';
import { isMac, TRAFFIC_LIGHTS_PAD } from '../../lib/platform';
import { useProjects } from '../../store/projects';
import { useWorkspace } from '../../store/workspace';
import { useUi } from '../../store/ui';
import { RepoBranchChip } from '../git/RepoBranchChip';

/** Barra superior unificada de 44 px sobre el canvas: proyecto/rama a la izquierda, Quick Open al centro, Run a la derecha. */
function CenterSwitch() {
  const view = useUi((s) => s.centerView);
  const setView = useUi((s) => s.setCenterView);
  return (
    <div role="tablist" aria-label="Vista" className="flex rounded-lg bg-elevated p-0.5 text-ui">
      {(['editor', 'git'] as const).map((v) => (
        <button
          key={v}
          type="button"
          role="tab"
          aria-selected={view === v}
          onClick={() => setView(v)}
          className={`rounded-md px-2.5 py-0.5 outline-none ${view === v ? 'bg-hover-strong text-fg' : 'text-muted hover:bg-hover hover:text-fg'}`}
        >
          {v === 'editor' ? 'Code' : 'Git'}
        </button>
      ))}
    </div>
  );
}

export function AppHeader() {
  const root = useWorkspace((s) => s.root);
  const name = useWorkspace((s) => s.name);
  return (
    <header data-tauri-drag-region className="grid h-11 shrink-0 grid-cols-[1fr_auto_1fr] items-center gap-3 bg-canvas pr-3">
      <div
        data-tauri-drag-region
        style={{ paddingLeft: isMac ? TRAFFIC_LIGHTS_PAD : 12 }}
        className="flex min-w-0 items-center gap-2 text-ui"
      >
        <BrandMark size={16} />
        <button
          type="button"
          onClick={() => useProjects.getState().openScreen()}
          title="Projects"
          className="flex min-w-0 items-center gap-1.5 rounded-lg bg-elevated px-2.5 py-1 text-fg outline-none hover:bg-elevated-hover"
        >
          <span className="truncate font-medium">{name ?? 'CaesarCode'}</span>
          <ChevronDown size={12} className="shrink-0 text-muted" />
        </button>
        {root && <RepoBranchChip />}
        {root && <CenterSwitch />}
      </div>
      <div data-tauri-drag-region className="flex justify-center">
        <QuickOpenTrigger />
      </div>
      <div data-tauri-drag-region className="flex items-center justify-end gap-1.5">
        <IconButton label="New Project" onClick={() => useProjects.getState().openWizard()}>
          <Plus size={14} />
        </IconButton>
        {root && <RunToolbar />}
      </div>
    </header>
  );
}
