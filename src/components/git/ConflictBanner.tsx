import { useState } from 'react';
import { AlertTriangle } from 'lucide-react';
import { useGit } from '../../store/git';
import { Button } from '../ui/Button';
import type { GitRepoState, GitSeqOp } from '../../lib/types';
import { confirmAction, openInEditor } from './gitActions';
import { splitPath } from './gitUtils';

const NONE: never[] = [];
const OP: Partial<Record<GitRepoState, { op: GitSeqOp; label: string; skip: boolean }>> = {
  merging: { op: 'merge', label: 'Merge en curso', skip: false },
  rebasing: { op: 'rebase', label: 'Rebase en curso', skip: true },
  cherryPicking: { op: 'cherryPick', label: 'Cherry-pick en curso', skip: true },
  reverting: { op: 'revert', label: 'Revert en curso', skip: true },
};

/** Banner de operación en curso (Merging/Rebasing…) con resolución básica de conflictos. */
export function ConflictBanner() {
  const info = useGit((s) => s.info);
  const conflicted = useGit((s) => s.status?.conflicted ?? NONE);
  const busy = useGit((s) => s.busy);
  const [open, setOpen] = useState(true);
  const meta = info ? OP[info.state] : undefined;
  if (!info || !meta) return null;
  const g = useGit.getState;
  const pending = conflicted.length;

  return (
    <div className="mx-2 mb-1.5 shrink-0 overflow-hidden rounded-lg border border-stop/30 bg-stop/10">
      <div className="flex h-9 items-center gap-2 px-3">
        <AlertTriangle size={14} className="shrink-0 text-stop-text" />
        <button type="button" onClick={() => setOpen(!open)} className="min-w-0 flex-1 truncate text-left text-ui-lg text-fg outline-none">
          <span className="font-medium">{meta.label}</span>
          <span className="ml-2 text-muted">{pending > 0 ? `${pending} conflicto${pending === 1 ? '' : 's'} sin resolver` : 'Sin conflictos pendientes: puedes continuar'}</span>
        </button>
        <Button variant="primary" disabled={pending > 0 || !!busy} onClick={() => void g().sequence(meta.op, 'continue')}>Continue</Button>
        {meta.skip && <Button disabled={!!busy} onClick={() => void g().sequence(meta.op, 'skip')}>Skip</Button>}
        <Button
          variant="danger"
          disabled={!!busy}
          onClick={async () => {
            if (await confirmAction('Abortar operación', `${meta.label}: se descartará el progreso y se volverá al estado anterior.`, 'Abortar'))
              void g().sequence(meta.op, 'abort');
          }}
        >
          Abort
        </Button>
      </div>
      {open && pending > 0 && (
        <div className="max-h-40 overflow-auto border-t border-stop/30 py-1">
          {conflicted.map((e) => {
            const { name, dir } = splitPath(e.path);
            return (
              <div key={e.path} className="flex h-7 items-center gap-2 px-3 text-ui-lg">
                <span className="w-7 shrink-0 font-mono text-[11px] font-semibold text-stop-text">{e.conflict ?? 'UU'}</span>
                <span className="shrink-0 text-fg">{name}</span>
                <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-muted">{dir}</span>
                <Button className="h-6 px-2" onClick={() => openInEditor(e.path)}>Open</Button>
                <Button className="h-6 px-2" title="Quedarse con nuestra versión" onClick={() => void g().conflict(e.path, 'ours')}>Ours</Button>
                <Button className="h-6 px-2" title="Quedarse con la suya" onClick={() => void g().conflict(e.path, 'theirs')}>Theirs</Button>
                <Button className="h-6 px-2" variant="primary" onClick={() => void g().conflict(e.path, 'markResolved')}>Mark resolved</Button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
