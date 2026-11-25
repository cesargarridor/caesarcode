import { useRef, useState } from 'react';
import { ChevronDown, GitBranch, Layers, Loader2 } from 'lucide-react';
import { RepoPicker, RepoStats, summaryBranch } from './RepoPicker';
import { FolderIcon } from '../icons/SymbolIcon';
import { useGit } from '../../store/git';

/** Selector del repo sobre el que opera la vista Git (nombre + rama + cambios + ahead/behind). */
export function RepoSelector() {
  const repos = useGit((s) => s.repos);
  const activeRepo = useGit((s) => s.activeRepo);
  const view = useGit((s) => s.view);
  const summaries = useGit((s) => s.summaries);
  const busy = useGit((s) => (s.activeRepo ? !!s.busyByRepo[s.activeRepo] : false));
  const [anchor, setAnchor] = useState<DOMRect | null>(null);
  const ref = useRef<HTMLButtonElement>(null);
  const repo = repos.find((r) => r.top === activeRepo);
  const s = repo ? summaries[repo.top] : undefined;

  return (
    <>
      <button
        ref={ref}
        type="button"
        aria-haspopup="listbox"
        aria-expanded={!!anchor}
        aria-label="Repositorio Git"
        title={view === 'all' ? 'Todos los repositorios' : repo?.top}
        onClick={() => setAnchor(anchor ? null : (ref.current?.getBoundingClientRect() ?? null))}
        className="flex h-7 max-w-[420px] items-center gap-1.5 rounded-lg bg-elevated px-2.5 text-ui-lg text-fg outline-none hover:bg-elevated-hover focus-visible:ring-1 focus-visible:ring-accent"
      >
        {view === 'all' ? (
          <>
            <Layers size={14} className="shrink-0 text-muted" />
            <span className="font-medium">All repos</span>
            <span className="font-mono text-[11px] text-muted">{repos.length}</span>
          </>
        ) : repo ? (
          <>
            <FolderIcon name={repo.name} isModule />
            <span className="min-w-0 truncate font-medium">{repo.name}</span>
            <GitBranch size={11} className="shrink-0 text-accent" />
            <span className="min-w-0 truncate font-mono text-[11px] text-muted">{summaryBranch(s)}</span>
            <RepoStats s={s} />
            {busy && <Loader2 size={12} className="shrink-0 animate-spin text-accent" aria-label="Trabajando" />}
          </>
        ) : (
          <span className="text-muted">Sin repositorio</span>
        )}
        <ChevronDown size={12} className="shrink-0 text-muted" />
      </button>
      {anchor && <RepoPicker anchor={anchor} onClose={() => setAnchor(null)} />}
    </>
  );
}
