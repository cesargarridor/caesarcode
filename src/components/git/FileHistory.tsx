import { useEffect, useState } from 'react';
import { X } from 'lucide-react';
import { gitFileHistory } from '../../lib/ipc';
import { toMessage } from '../../lib/errors';
import { useGit } from '../../store/git';
import { useUi } from '../../store/ui';
import { IconButton } from '../ui/IconButton';
import type { CommitRow } from '../../lib/types';
import { hashColor, initials, relTime, shortSha, fullDate } from './gitUtils';

const PAGE = 100;

/** Historial de un archivo (`--follow`); clic = detalle del commit + diff del archivo en él. */
export function FileHistory({ path }: { path: string }) {
  const [rows, setRows] = useState<CommitRow[]>([]);
  const [more, setMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const selected = useGit((s) => s.selected);

  const load = async (skip: number) => {
    setLoading(true);
    try {
      const repo = useGit.getState().activeRepo;
      if (!repo) return;
      const page = await gitFileHistory(repo, path, skip, PAGE);
      setRows((r) => (skip === 0 ? page.commits : r.concat(page.commits)));
      setMore(page.hasMore);
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => {
    setRows([]);
    void load(0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path]);

  const pick = async (c: CommitRow) => {
    const g = useGit.getState();
    await g.selectCommit(c.sha);
    // Ruta relativa al top: si llegó absoluta, el backend también la acepta.
    await g.openDiff('commit', path, { sha: c.sha, parent: c.parents[0] ?? null });
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex h-9 shrink-0 items-center gap-2 px-3">
        <span className="text-ui text-muted">Historial</span>
        <span className="min-w-0 flex-1 truncate font-mono text-ui-lg text-fg" title={path}>{path}</span>
        <IconButton label="Cerrar historial" onClick={() => useGit.getState().openFileHistory(null)}><X size={13} /></IconButton>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        {rows.map((c) => (
          <div
            key={c.sha}
            onClick={() => void pick(c)}
            className={`flex h-7 cursor-pointer items-center gap-2 px-3 text-ui-lg ${selected?.type === 'commit' && selected.sha === c.sha ? 'bg-accent/12' : 'hover:bg-hover'}`}
          >
            <span className="font-mono text-[11px] text-muted">{shortSha(c.sha)}</span>
            <span className="min-w-0 flex-1 truncate text-fg">{c.subject}</span>
            <span className="flex h-[18px] w-[18px] shrink-0 items-center justify-center rounded-full text-[9px] font-semibold text-base" style={{ backgroundColor: hashColor(c.email || c.author) }} title={c.author}>
              {initials(c.author)}
            </span>
            <span className="w-24 shrink-0 text-muted" title={fullDate(c.time)}>{relTime(c.time)}</span>
          </div>
        ))}
        {loading && <div className="p-3 text-ui text-muted">Cargando…</div>}
        {!loading && rows.length === 0 && <div className="p-3 text-ui text-muted">Sin historial para este archivo.</div>}
        {!loading && more && (
          <button type="button" onClick={() => void load(rows.length)} className="m-2 rounded-lg border border-border-strong bg-elevated px-3 py-1 text-ui text-muted outline-none hover:bg-hover hover:text-fg">
            Cargar más
          </button>
        )}
      </div>
    </div>
  );
}
