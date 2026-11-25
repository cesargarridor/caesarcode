import { useEffect, useMemo, useState } from 'react';
import { Play, RotateCw, Square } from 'lucide-react';
import { RunDropdown } from './RunDropdown';
import { sanitizeSelection, splitSelId, useRunConfigStore } from '../../store/runConfigs';
import { useServices } from '../../store/services';
import { useEditor } from '../../store/editor';
import { useWorkspace } from '../../store/workspace';
import { runResolveFile } from '../../lib/ipc';
import { basename } from '../../lib/paths';
import { toMessage } from '../../lib/errors';
import type { ResolvedRun } from '../../lib/types';

/** `[● selección ▾] [▶ Run | ⟳ Restart] [■ Stop]` — arriba a la derecha del header. */
export function RunToolbar() {
  const root = useWorkspace((s) => s.root);
  const files = useRunConfigStore((s) => s.files);
  const detected = useRunConfigStore((s) => s.detected);
  const selection = useRunConfigStore((s) => s.selection);
  const run = useRunConfigStore((s) => s.run);
  const stop = useRunConfigStore((s) => s.stop);
  const services = useServices((s) => s.configs);
  const activePath = useEditor((s) => (s.active && s.tabs.some((t) => t.path === s.active) ? s.active : null));

  const sel = useMemo(() => sanitizeSelection(selection, files, detected, services), [selection, files, detected, services]);
  const runtimeId = useMemo(
    () => useRunConfigStore.getState().runtimeIdFor(sel),
    // `activePath` y `root` cambian el id de Current File.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [sel, activePath, root],
  );
  const status = useServices((s) => (runtimeId ? s.statuses[runtimeId] : undefined));
  const state = status?.state ?? 'stopped';
  const active = state === 'running' || state === 'starting';

  // Preview del runner de Current File (debounced; no en cada tecla: solo al cambiar de archivo).
  const [resolved, setResolved] = useState<ResolvedRun | null>(null);
  const [resolveError, setResolveError] = useState<string | null>(null);
  useEffect(() => {
    setResolved(null);
    setResolveError(null);
    if (!activePath) return;
    let alive = true;
    const t = setTimeout(() => {
      runResolveFile(activePath)
        .then((r) => alive && setResolved(r))
        .catch((e) => alive && setResolveError(toMessage(e)));
    }, 250);
    return () => {
      alive = false;
      clearTimeout(t);
    };
  }, [activePath]);

  const { label, command } = useMemo(() => {
    switch (sel.kind) {
      case 'file':
        return {
          label: activePath ? basename(activePath) : 'Current File',
          command: resolved ? `${resolved.runner}: ${resolved.command}` : resolveError ?? '',
        };
      case 'detected': {
        const { root: dr, id } = splitSelId(sel.id, root);
        const d = detected.find((x) => x.id === id && x.root === dr);
        return { label: d?.name ?? id, command: d?.command ?? '' };
      }
      case 'config': {
        const { root: cr, id } = splitSelId(sel.id, root);
        const c = cr ? files[cr]?.configs.find((x) => x.id === id) : undefined;
        return { label: c?.name ?? id, command: c ? `${c.command} ${c.args}`.trim() : '' };
      }
      case 'service': {
        const c = services.find((x) => x.id === sel.id);
        return { label: c?.name ?? sel.id, command: c?.command ?? '' };
      }
    }
  }, [sel, activePath, resolved, resolveError, detected, files, services, root]);

  const noTarget = sel.kind === 'file' && !activePath;
  const canRun = !!root && !noTarget && state !== 'stopping';
  const canStop = active || state === 'stopping';

  return (
    <div className="flex items-center gap-0.5 rounded-lg border border-border-strong bg-elevated p-0.5">
      <RunDropdown sel={sel} label={label} state={state} resolved={resolved} />
      <button
        type="button"
        disabled={!canRun}
        onClick={() => void run(sel)}
        title={`${active ? 'Restart' : 'Run'}${command ? ` — ${command}` : ''}  (Shift+F10 / Ctrl+Shift+R)`}
        aria-label={active ? 'Restart' : 'Run'}
        className="inline-flex items-center justify-center rounded-md p-1.5 text-run-text outline-none transition-colors hover:bg-run/15 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
      >
        {active ? <RotateCw size={13} /> : <Play size={13} fill="currentColor" />}
      </button>
      <button
        type="button"
        disabled={!canStop}
        onClick={() => void stop(sel)}
        title="Stop  (Ctrl+F2)"
        aria-label="Stop"
        className="inline-flex items-center justify-center rounded-md p-1.5 text-stop-text outline-none transition-colors hover:bg-stop/15 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
      >
        <Square size={12} fill="currentColor" />
      </button>
    </div>
  );
}
