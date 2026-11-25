import { Search } from 'lucide-react';
import { Kbd } from '../ui/Kbd';
import { useUi } from '../../store/ui';
import { useWorkspace } from '../../store/workspace';
import { isMac } from '../../lib/platform';

/** Trigger centrado del Quick Open en el header. */
export function QuickOpenTrigger() {
  const root = useWorkspace((s) => s.root);
  if (!root) return null;
  return (
    <button
      type="button"
      onClick={() => useUi.getState().openModal('quickOpen')}
      className="flex h-7 w-72 items-center gap-2 rounded-lg border border-border bg-quick px-2.5 text-left text-xs text-muted outline-none hover:bg-hover"
    >
      <Search size={12} className="shrink-0" />
      <span className="min-w-0 flex-1 truncate">Buscar archivos, símbolos...</span>
      <Kbd>{isMac ? '⌘P' : 'Ctrl P'}</Kbd>
    </button>
  );
}
