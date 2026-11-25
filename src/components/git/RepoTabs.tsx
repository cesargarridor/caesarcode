import { Layers } from 'lucide-react';
import { FolderIcon } from '../icons/SymbolIcon';
import { RepoStats, summaryBranch } from './RepoPicker';
import { useGit } from '../../store/git';

/** Pestañas por repositorio (+ "All repos"): elige sobre qué proyecto opera Git. */
export function RepoTabs() {
  const repos = useGit((s) => s.repos);
  const activeRepo = useGit((s) => s.activeRepo);
  const view = useGit((s) => s.view);
  const summaries = useGit((s) => s.summaries);

  const pick = (top: string) => {
    const g = useGit.getState();
    if (g.activeRepo === top) g.setView('repo');
    else void g.setActiveRepo(top);
  };

  return (
    <div role="tablist" aria-label="Repositorios Git" className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto py-1">
      {repos.length > 1 && (
        <button
          type="button"
          role="tab"
          aria-selected={view === 'all'}
          onClick={() => useGit.getState().setView('all')}
          className={`flex h-7 shrink-0 items-center gap-1.5 rounded-lg px-2.5 text-ui-lg outline-none focus-visible:ring-1 focus-visible:ring-accent ${
            view === 'all' ? 'bg-ink/[0.08] text-fg-strong' : 'text-muted hover:bg-hover hover:text-fg'
          }`}
        >
          <Layers size={14} />
          <span className="font-medium">All repos</span>
        </button>
      )}
      {repos.map((r) => {
        const on = view !== 'all' && r.top === activeRepo;
        const s = summaries[r.top];
        return (
          <button
            key={r.top}
            type="button"
            role="tab"
            aria-selected={on}
            title={r.top}
            onClick={() => pick(r.top)}
            className={`flex h-7 shrink-0 items-center gap-1.5 rounded-lg px-2.5 text-ui-lg outline-none focus-visible:ring-1 focus-visible:ring-accent ${
              on ? 'border border-ink/[0.08] bg-ink/[0.08] text-fg-strong' : 'border border-transparent text-muted hover:bg-hover hover:text-fg'
            }`}
          >
            <FolderIcon name={r.name} isModule className="h-4 w-4" />
            <span className="max-w-[160px] truncate font-medium">{r.name}</span>
            <span className="font-mono text-[11px] text-faint">{summaryBranch(s)}</span>
            <RepoStats s={s} />
          </button>
        );
      })}
    </div>
  );
}
