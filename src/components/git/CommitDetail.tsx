import { Copy } from 'lucide-react';
import { useGit } from '../../store/git';
import { IconButton } from '../ui/IconButton';
import { copyToClipboard } from './gitActions';
import { fullDate, hashColor, initials, shortSha, splitPath, STATUS_COLOR } from './gitUtils';

export function CommitDetail() {
  const detail = useGit((s) => s.detail);
  const diff = useGit((s) => s.diff);
  if (!detail) return <div className="p-3 text-ui text-muted">Cargando commit…</div>;
  const { row } = detail;
  const g = useGit.getState;
  const adds = detail.files.reduce((a, f) => a + (f.additions ?? 0), 0);
  const dels = detail.files.reduce((a, f) => a + (f.deletions ?? 0), 0);
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="shrink-0 p-3">
        <div className="mb-2 break-words text-ui-lg font-medium text-fg">{row.subject}</div>
        {detail.body.replace(row.subject, '').trim() && (
          <pre className="mb-2 max-h-32 overflow-auto whitespace-pre-wrap break-words font-sans text-ui text-muted">
            {detail.body.replace(row.subject, '').trim()}
          </pre>
        )}
        <div className="flex items-center gap-2 text-ui">
          <span
            className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-[10px] font-semibold text-base"
            style={{ backgroundColor: hashColor(row.email || row.author) }}
          >
            {initials(row.author)}
          </span>
          <div className="min-w-0">
            <div className="truncate text-fg">{row.author} <span className="text-muted">&lt;{row.email}&gt;</span></div>
            <div className="text-muted">{fullDate(row.time)}{detail.committer !== row.author ? ` · committer ${detail.committer}` : ''}</div>
          </div>
        </div>
        <div className="mt-2 flex items-center gap-1 font-mono text-[11px] text-muted">
          <span className="text-fg">{shortSha(row.sha)}</span>
          <IconButton label="Copiar SHA" onClick={() => void copyToClipboard(row.sha)}><Copy size={11} /></IconButton>
          {row.parents.length > 0 && <span>padres:</span>}
          {row.parents.map((p) => (
            <button key={p} type="button" onClick={() => void g().selectCommit(p)} className="text-accent outline-none hover:underline">{shortSha(p)}</button>
          ))}
        </div>
      </div>
      <div className="flex h-7 shrink-0 items-center justify-between px-3 text-[10.5px] font-semibold uppercase tracking-wider text-muted">
        <span>{detail.files.length} archivo{detail.files.length === 1 ? '' : 's'}</span>
        <span className="font-mono font-normal"><span className="text-run-text">+{adds}</span> <span className="text-stop-text">−{dels}</span></span>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        {detail.files.map((f) => {
          const { name, dir } = splitPath(f.path);
          const active = diff?.source === 'commit' && diff.path === f.path;
          return (
            <div
              key={f.path}
              onClick={() => void g().openDiff('commit', f.path, { sha: row.sha, oldPath: f.origPath, parent: row.parents[0] ?? null })}
              onContextMenu={(e) => { e.preventDefault(); g().openFileHistory(f.path); }}
              title={`${f.path} (clic derecho: historial)`}
              className={`flex h-6 cursor-pointer items-center gap-1.5 px-3 text-ui-lg ${active ? 'bg-accent/12' : 'hover:bg-hover'}`}
            >
              <span className={`w-3 shrink-0 text-center font-mono text-[11px] font-semibold ${STATUS_COLOR[f.status] ?? 'text-muted'}`}>{f.status}</span>
              <span className="shrink-0 truncate text-fg">{name}</span>
              <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-muted">{dir}</span>
              <span className="shrink-0 font-mono text-[10.5px]">
                {f.additions === null ? <span className="text-muted">bin</span> : (
                  <><span className="text-run-text">+{f.additions}</span> <span className="text-stop-text">−{f.deletions}</span></>
                )}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
