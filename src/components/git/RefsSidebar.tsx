import { useMemo, useState } from 'react';
import { Archive, ChevronDown, ChevronRight, Cloud, GitBranch, Plus, Tag } from 'lucide-react';
import { useGit } from '../../store/git';
import { TextInput } from '../ui/TextInput';
import { ContextMenu, type MenuItem } from '../ui/ContextMenu';
import { IconButton } from '../ui/IconButton';
import type { BranchRef } from '../../lib/types';
import { branchMenu, stashMenu, tagMenu } from './gitActions';
import { openGitDialog } from './BranchDialogs';

function Section({
  title, count, open, onToggle, action, children,
}: { title: string; count: number; open: boolean; onToggle: () => void; action?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="mb-1">
      <div className="flex h-6 items-center pr-1 text-[10.5px] font-semibold uppercase tracking-wider text-muted">
        <button type="button" onClick={onToggle} className="flex min-w-0 flex-1 items-center gap-1 px-2 text-left outline-none hover:text-fg">
          {open ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
          {title}
          <span className="font-mono font-normal">{count}</span>
        </button>
        {action}
      </div>
      {open && children}
    </div>
  );
}

function Item({
  icon, label, sub, active, onClick, onDoubleClick, onContextMenu, title,
}: {
  icon: React.ReactNode; label: string; sub?: React.ReactNode; active?: boolean; title?: string;
  onClick?: () => void; onDoubleClick?: () => void; onContextMenu?: (e: React.MouseEvent) => void;
}) {
  return (
    <div
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
      title={title ?? label}
      className={`flex h-6 cursor-pointer items-center gap-1.5 pl-3.5 pr-2 text-ui-lg mx-1.5 rounded-md hover:bg-hover ${active ? 'bg-accent/12 text-accent-text' : 'text-fg'}`}
    >
      <span className="shrink-0 text-muted">{icon}</span>
      <span className={`min-w-0 flex-1 truncate ${active ? 'font-medium' : ''}`}>{label}</span>
      {sub}
    </div>
  );
}

function AheadBehind({ b }: { b: BranchRef }) {
  if (!b.ahead && !b.behind) return null;
  return (
    <span className="shrink-0 font-mono text-[10.5px] text-muted">
      {b.ahead > 0 && <span className="text-run-text">↑{b.ahead}</span>}
      {b.behind > 0 && <span className="ml-1 text-accent">↓{b.behind}</span>}
    </span>
  );
}

export function RefsSidebar() {
  const refs = useGit((s) => s.refs);
  const [filter, setFilter] = useState('');
  const [open, setOpen] = useState({ local: true, remote: true, tags: false, stashes: true });
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const q = filter.trim().toLowerCase();
  const match = (s: string) => !q || s.toLowerCase().includes(q);

  const local = useMemo(() => (refs?.local ?? []).filter((b) => match(b.name)), [refs, q]); // eslint-disable-line react-hooks/exhaustive-deps
  const remote = useMemo(() => (refs?.remote ?? []).filter((b) => !b.name.endsWith('/HEAD') && match(b.name)), [refs, q]); // eslint-disable-line react-hooks/exhaustive-deps
  const tags = useMemo(() => (refs?.tags ?? []).filter((t) => match(t.name)), [refs, q]); // eslint-disable-line react-hooks/exhaustive-deps
  const stashes = refs?.stashes ?? [];

  const ctx = (items: MenuItem[]) => (e: React.MouseEvent) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY, items });
  };
  const toggle = (k: keyof typeof open) => setOpen((o) => ({ ...o, [k]: !o[k] }));
  const g = useGit.getState;
  const showOpen = (k: keyof typeof open) => open[k] || !!q;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="p-2">
        <TextInput value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Filtrar ramas, tags…" className="h-6" />
      </div>
      <div className="min-h-0 flex-1 overflow-auto py-1">
        <Section title="Local" count={local.length} open={showOpen('local')} onToggle={() => toggle('local')}
          action={<IconButton label="Nueva rama" onClick={() => openGitDialog({ type: 'createBranch', start: 'HEAD' })}><Plus size={12} /></IconButton>}>
          {local.map((b) => (
            <Item
              key={b.fullName} icon={<GitBranch size={12} />} label={b.name} active={b.isHead}
              sub={<AheadBehind b={b} />}
              title={b.upstream ? `${b.name} → ${b.upstream}` : b.name}
              onClick={() => void g().selectCommit(b.sha)}
              onDoubleClick={() => !b.isHead && void g().branch({ type: 'checkout', name: b.name })}
              onContextMenu={ctx(branchMenu(b, false))}
            />
          ))}
        </Section>
        <Section title="Remote" count={remote.length} open={showOpen('remote')} onToggle={() => toggle('remote')}>
          {remote.map((b) => (
            <Item
              key={b.fullName} icon={<Cloud size={12} />} label={b.name}
              onClick={() => void g().selectCommit(b.sha)}
              onDoubleClick={() => void g().branch({ type: 'checkoutRemote', remoteBranch: b.name })}
              onContextMenu={ctx(branchMenu(b, true))}
            />
          ))}
        </Section>
        <Section title="Tags" count={tags.length} open={showOpen('tags')} onToggle={() => toggle('tags')}>
          {tags.map((t) => (
            <Item
              key={t.name} icon={<Tag size={12} />} label={t.name} title={t.message ?? t.name}
              onClick={() => void g().selectCommit(t.sha)}
              onContextMenu={ctx(tagMenu(t))}
            />
          ))}
        </Section>
        <Section title="Stashes" count={stashes.length} open={showOpen('stashes')} onToggle={() => toggle('stashes')}
          action={<IconButton label="Guardar en stash" onClick={() => openGitDialog({ type: 'stashPush' })}><Plus size={12} /></IconButton>}>
          {stashes.map((s) => (
            <Item
              key={s.index} icon={<Archive size={12} />} label={s.message || `stash@{${s.index}}`}
              onClick={() => void g().selectCommit(s.sha)}
              onDoubleClick={() => void g().stash({ type: 'apply', index: s.index })}
              onContextMenu={ctx(stashMenu(s))}
            />
          ))}
        </Section>
      </div>
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
    </div>
  );
}
