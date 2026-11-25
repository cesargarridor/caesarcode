import { useState } from 'react';
import { ChevronDown, ChevronRight, Minus, Plus, Undo2 } from 'lucide-react';
import { useGit, type DiffSource } from '../../store/git';
import type { StatusEntry } from '../../lib/types';
import { IconButton } from '../ui/IconButton';
import { ContextMenu, type MenuItem } from '../ui/ContextMenu';
import { absPath, splitPath, STATUS_COLOR } from './gitUtils';
import { discardWithConfirm, openInEditor } from './gitActions';
import { useUi } from '../../store/ui';
import { toggleBlame } from '../editor/blame';
import { useEditor } from '../../store/editor';

function letter(e: StatusEntry, source: DiffSource): string {
  if (source === 'untracked') return '?';
  const c = source === 'staged' ? e.index : e.worktree;
  return c.trim() || 'M';
}

function FileRow({
  e, source, active, onMenu,
}: { e: StatusEntry; source: DiffSource; active: boolean; onMenu: (ev: React.MouseEvent, e: StatusEntry, s: DiffSource) => void }) {
  const g = useGit.getState;
  const { name, dir } = splitPath(e.path);
  const l = letter(e, source);
  return (
    <div
      onClick={() => void g().openDiff(source, e.path, { oldPath: e.origPath })}
      onDoubleClick={() => source !== 'staged' && l !== 'D' && openInEditor(e.path)}
      onContextMenu={(ev) => onMenu(ev, e, source)}
      title={e.origPath ? `${e.origPath} → ${e.path}` : e.path}
      className={`group flex h-6 cursor-pointer items-center gap-1.5 pl-3 pr-1 text-ui-lg ${active ? 'bg-accent/12' : 'hover:bg-hover'}`}
    >
      <span className={`w-3 shrink-0 text-center font-mono text-[11px] font-semibold ${STATUS_COLOR[l] ?? 'text-muted'}`}>{l}</span>
      <span className="shrink-0 truncate text-fg">{name}</span>
      <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-muted">{dir}</span>
      <span className="hidden shrink-0 items-center group-hover:flex">
        {source === 'staged' ? (
          <IconButton label="Unstage" onClick={(ev) => { ev.stopPropagation(); void g().unstage([e.path]); }}><Minus size={12} /></IconButton>
        ) : (
          <>
            <IconButton label="Descartar cambios" onClick={(ev) => { ev.stopPropagation(); void discardWithConfirm([e.path], source === 'untracked'); }}><Undo2 size={12} /></IconButton>
            <IconButton label="Stage" onClick={(ev) => { ev.stopPropagation(); void g().stage([e.path]); }}><Plus size={12} /></IconButton>
          </>
        )}
      </span>
    </div>
  );
}

function Section({
  title, list, source, open, onToggle, actions, activePath, onMenu,
}: {
  title: string; list: StatusEntry[]; source: DiffSource; open: boolean; onToggle: () => void;
  actions: React.ReactNode; activePath: string | null; onMenu: (ev: React.MouseEvent, e: StatusEntry, s: DiffSource) => void;
}) {
  return (
    <div>
      <div className="flex h-7 items-center pr-1 text-[10.5px] font-semibold uppercase tracking-wider text-muted">
        <button type="button" onClick={onToggle} className="flex min-w-0 flex-1 items-center gap-1 px-2 text-left outline-none hover:text-fg">
          {open ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
          {title}
          <span className="font-mono font-normal">{list.length}</span>
        </button>
        {list.length > 0 && <div className="flex items-center gap-0.5 normal-case">{actions}</div>}
      </div>
      {open && list.map((e) => <FileRow key={source + e.path} e={e} source={source} active={activePath === e.path} onMenu={onMenu} />)}
    </div>
  );
}

function linkBtn(label: string, onClick: () => void) {
  return (
    <button type="button" onClick={onClick} className="rounded-md px-1.5 py-0.5 text-[11px] font-normal tracking-normal text-muted outline-none hover:bg-hover hover:text-fg">
      {label}
    </button>
  );
}

export function ChangesPanel() {
  const status = useGit((s) => s.status);
  const diff = useGit((s) => s.diff);
  const info = useGit((s) => s.info);
  const [open, setOpen] = useState({ staged: true, unstaged: true, untracked: true });
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const g = useGit.getState;
  if (!status) return <div className="p-3 text-ui text-muted">Cargando…</div>;
  const toggle = (k: keyof typeof open) => setOpen((o) => ({ ...o, [k]: !o[k] }));
  const activePath = diff && diff.source !== 'commit' ? diff.path : null;
  const total = status.staged.length + status.unstaged.length + status.untracked.length;

  const onMenu = (ev: React.MouseEvent, e: StatusEntry, s: DiffSource) => {
    ev.preventDefault();
    const items: MenuItem[] = [];
    if (s === 'staged') items.push({ label: 'Unstage', onSelect: () => void g().unstage([e.path]) });
    else {
      items.push({ label: 'Stage', onSelect: () => void g().stage([e.path]) });
      items.push({ label: 'Discard changes…', danger: true, onSelect: () => void discardWithConfirm([e.path], s === 'untracked') });
    }
    items.push({ label: 'Open file', separator: true, onSelect: () => openInEditor(e.path) });
    items.push({ label: 'Show history', onSelect: () => g().openFileHistory(e.path) });
    items.push({
      label: 'Annotate with Git Blame',
      onSelect: async () => {
        const p = absPath(info?.top, e.path);
        await useEditor.getState().openFile(p);
        useUi.getState().setCenterView('editor');
        setTimeout(() => void toggleBlame(p), 60);
      },
    });
    setMenu({ x: ev.clientX, y: ev.clientY, items });
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-8 shrink-0 items-center justify-between px-3 text-ui-lg text-fg">
        <span className="font-medium">Cambios</span>
        <span className="font-mono text-[11px] text-muted">{total} archivo{total === 1 ? '' : 's'}</span>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        {status.conflicted.length > 0 && (
          <div className="mx-2 my-1 rounded-lg border border-stop/30 bg-stop/10 px-3 py-1.5 text-ui text-stop-text">
            {status.conflicted.length} archivo(s) en conflicto: resuélvelos en la barra superior.
          </div>
        )}
        <Section
          title="Staged" list={status.staged} source="staged" open={open.staged} onToggle={() => toggle('staged')} activePath={activePath} onMenu={onMenu}
          actions={linkBtn('Unstage all', () => void g().unstage(status.staged.map((e) => e.path)))}
        />
        <Section
          title="Unstaged" list={status.unstaged} source="unstaged" open={open.unstaged} onToggle={() => toggle('unstaged')} activePath={activePath} onMenu={onMenu}
          actions={<>
            {linkBtn('Discard all', () => void discardWithConfirm(status.unstaged.map((e) => e.path), false))}
            {linkBtn('Stage all', () => void g().stage(status.unstaged.map((e) => e.path)))}
          </>}
        />
        <Section
          title="Untracked" list={status.untracked} source="untracked" open={open.untracked} onToggle={() => toggle('untracked')} activePath={activePath} onMenu={onMenu}
          actions={<>
            {linkBtn('Trash all', () => void discardWithConfirm(status.untracked.map((e) => e.path), true))}
            {linkBtn('Stage all', () => void g().stage(status.untracked.map((e) => e.path)))}
          </>}
        />
        {total === 0 && status.conflicted.length === 0 && <div className="px-3 py-4 text-center text-ui text-muted">Árbol de trabajo limpio</div>}
        {info?.untrackedLimited && <div className="px-3 py-2 text-[11px] text-muted">Repo grande: solo se listan carpetas sin seguimiento (no archivos individuales).</div>}
      </div>
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
    </div>
  );
}
