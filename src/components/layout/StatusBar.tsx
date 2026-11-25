import { useEffect, useState } from 'react';
import { GitBranch } from 'lucide-react';
import { appInfo } from '../../lib/ipc';
import { toMessage } from '../../lib/errors';
import type { AppInfo } from '../../lib/types';
import { useUi } from '../../store/ui';
import { useGit } from '../../store/git';
import { useEditor } from '../../store/editor';
import { runningCount, useServices } from '../../store/services';
import { countsFor, useLsp } from '../../store/lsp';
import { lspLanguage } from '../editor/languages';

const SERVER_BIN: Record<string, string> = {
  python: 'pyright-langserver',
  typescript: 'typescript-language-server',
  rust: 'rust-analyzer',
  java: 'jdtls',
};

/** Indicador discreto del servidor LSP del archivo activo. Sin lenguaje LSP no muestra nada. */
function LspIndicator() {
  const active = useEditor((s) => s.active);
  const doc = useLsp((s) => (active ? s.docs[active] : undefined));
  const lang = active ? lspLanguage(active) : null;
  const status = useLsp((s) => (lang ? s.statuses[lang] : undefined));
  const counts = useLsp((s) => countsFor(s.counts, active));
  if (!lang) return null;

  const bin = SERVER_BIN[lang];
  let dot = '●';
  let color = 'text-accent';
  let label = status?.server ?? doc?.server ?? bin;
  let title = `Language server (${lang}): iniciando…`;
  if (doc && !doc.available) {
    dot = '○';
    color = 'text-muted';
    label = `LSP no instalado (${status?.server ?? doc.server ?? bin})`;
    title = `Instala ${bin} para tener autocompletado, diagnósticos y definición. Ver README.${status?.message ? `\n${status.message}` : ''}`;
  } else if (status?.state === 'ready') {
    color = 'text-run-text';
    title = `Language server (${lang}) listo`;
  } else if (status?.state === 'crashed') {
    color = 'text-stop-text';
    title = `El servidor (${lang}) se cerró${status.message ? `: ${status.message}` : ''}`;
  } else if (status?.state === 'unavailable') {
    dot = '○';
    color = 'text-muted';
    label = `LSP no disponible (${status.server})`;
    title = status.message ?? title;
  }
  return (
    <>
      <span title={title} className={`flex items-center gap-1 ${color === 'text-muted' ? 'text-faint' : ''}`}>
        <span className={color}>{dot}</span>
        <span>{label}</span>
      </span>
      {(counts.errors > 0 || counts.warnings > 0) && (
        <span title="Diagnósticos del archivo">
          {counts.errors > 0 && <span className="text-stop-text">{counts.errors} err</span>}
          {counts.errors > 0 && counts.warnings > 0 && ' '}
          {counts.warnings > 0 && <span className="text-accent">{counts.warnings} warn</span>}
        </span>
      )}
    </>
  );
}

export function StatusBar() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const repo = useGit((s) => s.info);
  const repoName = useGit((s) => (s.repos.length > 1 ? s.repos.find((r) => r.top === s.activeRepo)?.name : undefined));
  const setCenterView = useUi((s) => s.setCenterView);
  const cursor = useEditor((s) => s.cursor);
  const configs = useServices((s) => s.configs);
  const statuses = useServices((s) => s.statuses);
  const [running, total] = runningCount(statuses, configs);

  useEffect(() => {
    let alive = true;
    appInfo()
      .then((i) => alive && setInfo(i))
      .catch((e) => alive && setError(toMessage(e)));
    return () => {
      alive = false;
    };
  }, []);

  return (
    <div className="flex h-6 shrink-0 items-center justify-between bg-canvas px-3 font-mono text-[11px] text-faint">
      <div className="flex items-center gap-3">
        <button type="button" onClick={() => setCenterView('git')} className="flex items-center gap-1 outline-none hover:text-fg" title="Rama git (abrir vista Git)">
          <GitBranch size={11} />
          {repoName && <span className="text-muted">{repoName} ·</span>}
          <span>{repo?.isRepo ? repo.branch ?? '—' : '—'}</span>
          {repo?.isRepo && repo.ahead > 0 && <span className="text-run-text">↑{repo.ahead}</span>}
          {repo?.isRepo && repo.behind > 0 && <span className="text-accent">↓{repo.behind}</span>}
        </button>
        <span title="Servicios en ejecución" className={running > 0 ? 'text-run-text' : ''}>
          {running}/{total} services running
        </span>
        <LspIndicator />
      </div>
      <div className="flex items-center gap-3">
        <span title="Cursor">{cursor ? `Ln ${cursor.line}, Col ${cursor.col}` : 'Ln —, Col —'}</span>
        {error ? (
          <span className="text-stop-text">{error}</span>
        ) : info ? (
          <span>
            v{info.version} · {info.platform}
          </span>
        ) : (
          <span>…</span>
        )}
      </div>
    </div>
  );
}
