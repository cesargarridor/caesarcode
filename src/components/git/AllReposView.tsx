import { useEffect } from 'react';
import { ArrowDownToLine, GitPullRequestArrow, Loader2, RefreshCw } from 'lucide-react';
import { Button } from '../ui/Button';
import { FolderIcon } from '../icons/SymbolIcon';
import { summaryBranch } from './RepoPicker';
import { useGit } from '../../store/git';
import { useWorkspace } from '../../store/workspace';
import { summarizeJobs, type RepoJobResult } from '../../lib/repoQueue';
import type { RepoEntry, RepoSummary } from '../../lib/types';

const STATUS_TEXT: Record<RepoJobResult['status'], string> = { ok: 'ok', skipped: 'saltado', error: 'error' };
const STATUS_COLOR: Record<RepoJobResult['status'], string> = { ok: 'text-ok', skipped: 'text-muted', error: 'text-stop-text' };

function stateText(s: RepoSummary | undefined): string {
  if (!s) return '…';
  if (s.error) return s.error;
  if (s.state) return s.state;
  if (s.conflicted > 0) return `${s.conflicted} conflicto(s)`;
  if (s.changed === 0 && s.ahead === 0 && s.behind === 0) return 'limpio';
  const parts: string[] = [];
  if (s.staged > 0) parts.push(`${s.staged} staged`);
  if (s.untracked > 0) parts.push(`${s.untracked} sin seguir`);
  return parts.join(' · ') || '—';
}

/** Vista "All repos": estado resumido de cada repo del workspace + Fetch all / Pull all (ff-only). */
export function AllReposView() {
  const repos = useGit((s) => s.repos);
  const summaries = useGit((s) => s.summaries);
  const busyByRepo = useGit((s) => s.busyByRepo);
  const bulk = useGit((s) => s.bulk);
  const roots = useWorkspace((s) => s.roots);
  const running = !!bulk?.running;

  useEffect(() => {
    void useGit.getState().loadSummaries(true);
  }, []);

  const resultOf = (r: RepoEntry) => bulk?.results.find((x) => x.top === r.top);
  const rootName = (r: RepoEntry) => roots.find((x) => x.path === r.root)?.name ?? '';
  const g = useGit.getState;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex h-10 shrink-0 items-center gap-2 px-3">
        <span className="text-ui-lg font-medium text-fg">{repos.length} repositorios</span>
        <span className="flex-1" />
        <Button disabled={running} onClick={() => void g().fetchAll()} title="git fetch --prune en todos los repos">
          <RefreshCw size={12} /> Fetch all
        </Button>
        <Button disabled={running} onClick={() => void g().pullAll()} title="git pull --ff-only; salta repos con cambios, sin upstream o en merge/rebase">
          <ArrowDownToLine size={12} /> Pull all (ff-only)
        </Button>
        <Button disabled={running} onClick={() => void g().loadSummaries(true)} title="Releer el estado de todos los repos">
          <GitPullRequestArrow size={12} /> Refresh
        </Button>
      </div>

      {bulk && (
        <div className="mx-3 mb-2 shrink-0 rounded-lg border border-border-strong bg-elevated px-3 py-2 text-ui">
          <div className="flex items-center gap-2 text-fg">
            {running && <Loader2 size={12} className="animate-spin text-accent" />}
            <span className="font-medium">{bulk.kind === 'fetch' ? 'Fetch all' : 'Pull all'}</span>
            <span className="font-mono text-[11px] text-muted">
              {bulk.done}/{bulk.total}
            </span>
            {!running && <span className="text-muted">{summarizeJobs(bulk.results)}</span>}
          </div>
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-auto px-3 pb-4">
        <table className="w-full table-fixed border-collapse text-ui-lg">
          <thead>
            <tr className="text-left text-[11px] uppercase tracking-wider text-muted">
              <th className="w-[24%] px-2 py-1.5 font-medium">Repositorio</th>
              <th className="w-[14%] px-2 py-1.5 font-medium">Raíz</th>
              <th className="w-[16%] px-2 py-1.5 font-medium">Rama</th>
              <th className="w-[8%] px-2 py-1.5 font-medium">Cambios</th>
              <th className="w-[9%] px-2 py-1.5 font-medium">↑ ↓</th>
              <th className="px-2 py-1.5 font-medium">Estado</th>
            </tr>
          </thead>
          <tbody>
            {repos.map((r) => {
              const s = summaries[r.top];
              const res = resultOf(r);
              return (
                <tr
                  key={r.top}
                  tabIndex={0}
                  title={r.top}
                  onClick={() => void g().setActiveRepo(r.top)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') void g().setActiveRepo(r.top);
                  }}
                  className="cursor-pointer border-t border-border outline-none hover:bg-hover focus-visible:ring-1 focus-visible:ring-accent"
                >
                  <td className="px-2 py-1.5">
                    <span className="flex items-center gap-1.5">
                      <FolderIcon name={r.name} isModule />
                      <span className="truncate font-medium text-fg">{r.name}</span>
                      {busyByRepo[r.top] && <Loader2 size={12} className="shrink-0 animate-spin text-accent" aria-label="Trabajando" />}
                    </span>
                  </td>
                  <td className="truncate px-2 py-1.5 text-muted">{rootName(r)}</td>
                  <td className="truncate px-2 py-1.5 font-mono text-[12px] text-fg">{summaryBranch(s)}</td>
                  <td className="px-2 py-1.5 font-mono text-[12px] text-accent">{s && !s.error && s.changed > 0 ? `●${s.changed}` : ''}</td>
                  <td className="px-2 py-1.5 font-mono text-[12px]">
                    {s && !s.error && s.ahead > 0 && <span className="text-run-text">↑{s.ahead} </span>}
                    {s && !s.error && s.behind > 0 && <span className="text-accent">↓{s.behind}</span>}
                  </td>
                  <td className="truncate px-2 py-1.5 text-muted">
                    {res ? (
                      <span className={STATUS_COLOR[res.status]} title={res.message}>
                        {STATUS_TEXT[res.status]}
                        {res.message ? ` · ${res.message}` : ''}
                      </span>
                    ) : (
                      <span className={s?.error ? 'text-stop-text' : ''}>{stateText(s)}</span>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {repos.length === 0 && <div className="px-2 py-4 text-ui text-muted">No hay repositorios Git en el workspace.</div>}
      </div>
    </div>
  );
}
