import { useState } from 'react';
import { ArrowDown, ArrowUp, Archive, ChevronDown, GitBranch, RefreshCw, X } from 'lucide-react';
import { useGit } from '../../store/git';
import { Button } from '../ui/Button';
import { IconButton } from '../ui/IconButton';
import { ContextMenu, type MenuItem } from '../ui/ContextMenu';
import { confirmAction } from './gitActions';
import { openGitDialog } from './BranchDialogs';

export function RemoteBar() {
  const info = useGit((s) => s.info);
  const refs = useGit((s) => s.refs);
  const job = useGit((s) => s.remoteJob);
  const busy = useGit((s) => s.busy);
  const progress = useGit((s) => s.progress);
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const g = useGit.getState;
  if (!info) return null;

  const firstRemote = refs?.remotes[0]?.name ?? null;
  const noRemote = !firstRemote;
  const noUp = !info.upstream;
  const disabled = !!job || noRemote;

  const lastPct = [...progress].reverse().find((p) => p.type === 'percent');
  const lastLine = [...progress].reverse().find((p) => p.type === 'line');
  const text = lastPct && lastPct.type === 'percent' ? `${lastPct.phase} ${lastPct.pct}%` : lastLine && lastLine.type === 'line' ? lastLine.text : 'Trabajando…';
  const pct = lastPct && lastPct.type === 'percent' ? lastPct.pct : null;

  const open = (e: React.MouseEvent, items: MenuItem[]) => {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setMenu({ x: r.left, y: r.bottom + 2, items });
  };
  const push = (opts: { force?: boolean; tags?: boolean; setUp?: boolean }) =>
    g().remote({
      type: 'push', remote: opts.setUp || noUp ? firstRemote : null, branch: opts.setUp || noUp ? info.branch : null,
      forceWithLease: !!opts.force, setUpstream: !!opts.setUp || noUp, tags: !!opts.tags,
    });

  return (
    <div className="shrink-0">
      <div className="flex h-9 items-center gap-2 px-3">
        <GitBranch size={13} className="shrink-0 text-accent" />
        <span className="max-w-[220px] truncate font-mono text-ui-lg text-fg">{info.branch ?? (info.detached ? `HEAD ${info.head?.slice(0, 7) ?? ''}` : '—')}</span>
        {info.upstream && <span className="max-w-[160px] truncate font-mono text-[11px] text-muted">→ {info.upstream}</span>}
        {info.ahead > 0 && <span className="font-mono text-[11px] text-run-text">↑{info.ahead}</span>}
        {info.behind > 0 && <span className="font-mono text-[11px] text-accent">↓{info.behind}</span>}
        <div className="flex-1" />
        <Button disabled={disabled} onClick={() => void g().remote({ type: 'fetch', remote: null, prune: true })} title="Fetch --prune">
          <RefreshCw size={12} /> Fetch
        </Button>
        <div className="flex">
          <Button className="rounded-r-none" disabled={disabled || noUp} onClick={() => void g().remote({ type: 'pull', rebase: false })} title="Pull (solo fast-forward)">
            <ArrowDown size={12} /> Pull{info.behind > 0 ? ` ${info.behind}` : ''}
          </Button>
          <Button
            className="rounded-l-none border-l-0 px-1.5" disabled={disabled || noUp} aria-label="Opciones de pull"
            onClick={(e) => open(e, [
              { label: 'Pull (fast-forward only)', onSelect: () => void g().remote({ type: 'pull', rebase: false }) },
              { label: 'Pull --rebase', onSelect: () => void g().remote({ type: 'pull', rebase: true }) },
            ])}
          >
            <ChevronDown size={11} />
          </Button>
        </div>
        <div className="flex">
          <Button className="rounded-r-none" disabled={disabled || !info.branch} onClick={() => void push({})} title={noUp ? 'Push y fijar upstream' : 'Push'}>
            <ArrowUp size={12} /> Push{info.ahead > 0 ? ` ${info.ahead}` : ''}
          </Button>
          <Button
            className="rounded-l-none border-l-0 px-1.5" disabled={disabled || !info.branch} aria-label="Opciones de push"
            onClick={(e) => open(e, [
              { label: 'Push', onSelect: () => void push({}) },
              { label: 'Push y fijar upstream', onSelect: () => void push({ setUp: true }) },
              { label: 'Push --tags', onSelect: () => void push({ tags: true }) },
              {
                label: 'Push --force-with-lease…', danger: true, separator: true,
                onSelect: async () => {
                  if (await confirmAction('Force push', 'Se reescribirá la rama remota (--force-with-lease). Puede perder commits de otros.', 'Force push')) void push({ force: true });
                },
              },
            ])}
          >
            <ChevronDown size={11} />
          </Button>
        </div>
        <IconButton label="Stash" onClick={() => openGitDialog({ type: 'stashPush' })}><Archive size={13} /></IconButton>
        <IconButton label="Refrescar" disabled={!!busy} onClick={() => void g().refreshAll()}><RefreshCw size={13} /></IconButton>
      </div>
      {noRemote && <div className="px-3 pb-1.5 text-[11px] text-muted">Sin remotos configurados: fetch/pull/push desactivados.</div>}
      {job && (
        <div className="flex h-7 items-center gap-2 px-3">
          <div className="h-1 w-32 shrink-0 overflow-hidden rounded bg-border">
            <div className={`h-full bg-accent ${pct === null ? 'w-1/3 animate-pulse' : ''}`} style={pct === null ? undefined : { width: `${pct}%` }} />
          </div>
          <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-muted">{text}</span>
          <Button className="h-5 px-2" onClick={() => g().cancelRemote()}><X size={11} /> Cancel</Button>
        </div>
      )}
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
    </div>
  );
}
