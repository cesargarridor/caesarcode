import { useRef, useState } from 'react';
import { GitBranch } from 'lucide-react';
import { RepoPicker, summaryBranch } from './RepoPicker';
import { useGit } from '../../store/git';
import { useUi } from '../../store/ui';

/** Chip de la cabecera: `repo · rama · ●N ↑a ↓b` (sin nombre de repo si solo hay uno). Clic abre el selector de repos. */
export function RepoBranchChip() {
  const repos = useGit((s) => s.repos);
  const activeRepo = useGit((s) => s.activeRepo);
  const info = useGit((s) => s.info);
  const summary = useGit((s) => (s.activeRepo ? s.summaries[s.activeRepo] : undefined));
  const setView = useUi((s) => s.setCenterView);
  const [anchor, setAnchor] = useState<DOMRect | null>(null);
  const ref = useRef<HTMLButtonElement>(null);
  const repo = repos.find((r) => r.top === activeRepo);
  if (!repo && !info?.isRepo) return null;

  const branch = info?.isRepo
    ? (info.branch ?? (info.detached ? `HEAD ${info.head?.slice(0, 7) ?? ''}` : '—'))
    : summaryBranch(summary);
  const ahead = info?.isRepo ? info.ahead : (summary?.ahead ?? 0);
  const behind = info?.isRepo ? info.behind : (summary?.behind ?? 0);
  const multi = repos.length > 1;

  const open = () => {
    if (multi) setAnchor(anchor ? null : (ref.current?.getBoundingClientRect() ?? null));
    else setView('git');
  };

  return (
    <>
      <button
        ref={ref}
        type="button"
        aria-haspopup={multi ? 'listbox' : undefined}
        onClick={open}
        title={info?.upstream ? `${repo?.name ?? ''} ${branch} → ${info.upstream}`.trim() : `${repo?.name ?? ''} ${branch}`.trim()}
        className="flex max-w-[340px] items-center gap-1 rounded-lg bg-elevated px-2.5 py-1 font-mono text-ui text-muted outline-none hover:bg-elevated-hover hover:text-fg"
      >
        <GitBranch size={12} className="shrink-0 text-accent" />
        {multi && repo && <span className="max-w-[120px] truncate text-fg">{repo.name}</span>}
        {multi && repo && <span className="text-faint">·</span>}
        <span className="truncate">{branch}</span>
        {summary && summary.changed > 0 && <span className="shrink-0 text-accent">●{summary.changed}</span>}
        {ahead > 0 && <span className="text-ok">↑{ahead}</span>}
        {behind > 0 && <span className="text-accent">↓{behind}</span>}
      </button>
      {anchor && <RepoPicker anchor={anchor} onClose={() => setAnchor(null)} onPicked={() => setView('git')} />}
    </>
  );
}
