import { create } from 'zustand';
import { lspStatus, lspSubscribe } from '../lib/ipc';
import { toMessage } from '../lib/errors';
import type { DiagnosticDto, LspDocStatus, LspEvent, LspServerStatus } from '../lib/types';
import { applyDiagnostics } from '../components/editor/lsp/diagnostics';
import { registry } from './editorRegistry';
import { useUi } from './ui';

export interface DiagCounts { errors: number; warnings: number }

interface LspStore {
  /** Estado del servidor por lenguaje (`python`, `typescript`, `rust`, `java`). */
  statuses: Record<string, LspServerStatus>;
  /** Resultado del didOpen por archivo abierto. */
  docs: Record<string, LspDocStatus>;
  diagnostics: Record<string, DiagnosticDto[]>;
  counts: Record<string, DiagCounts>;
  init: () => Promise<void>;
  reset: () => void;
  setDoc: (path: string, status: LspDocStatus) => void;
  forgetDoc: (path: string) => void;
}

const NONE: DiagCounts = { errors: 0, warnings: 0 };

function count(diags: DiagnosticDto[]): DiagCounts {
  let errors = 0;
  let warnings = 0;
  for (const d of diags) {
    if (d.severity === 'error') errors++;
    else if (d.severity === 'warning') warnings++;
  }
  return errors || warnings ? { errors, warnings } : NONE;
}

function handleEvent(ev: LspEvent) {
  if (ev.type === 'status') {
    useLsp.setState((s) => ({ statuses: { ...s.statuses, [ev.status.language]: ev.status } }));
    return;
  }
  const { path, diagnostics } = ev;
  useLsp.setState((s) => {
    // Los diagnósticos de archivos no abiertos también se guardan (los pedirá la pestaña al abrirse).
    const diag = { ...s.diagnostics };
    const counts = { ...s.counts };
    if (diagnostics.length === 0) {
      delete diag[path];
      delete counts[path];
    } else {
      diag[path] = diagnostics;
      counts[path] = count(diagnostics);
    }
    return { diagnostics: diag, counts };
  });
  if (registry.view && registry.shownPath === path) applyDiagnostics(registry.view, diagnostics);
}

export const useLsp = create<LspStore>((set) => ({
  statuses: {},
  docs: {},
  diagnostics: {},
  counts: {},

  init: async () => {
    try {
      await lspSubscribe(handleEvent);
      const list = await lspStatus();
      set((s) => {
        const statuses = { ...s.statuses };
        for (const st of list) statuses[st.language] = st;
        return { statuses };
      });
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    }
  },

  reset: () => set({ statuses: {}, docs: {}, diagnostics: {}, counts: {} }),

  setDoc: (path, status) => set((s) => ({ docs: { ...s.docs, [path]: status } })),

  forgetDoc: (path) =>
    set((s) => {
      const docs = { ...s.docs };
      const diagnostics = { ...s.diagnostics };
      const counts = { ...s.counts };
      delete docs[path];
      delete diagnostics[path];
      delete counts[path];
      return { docs, diagnostics, counts };
    }),
}));

export function countsFor(counts: Record<string, DiagCounts>, path: string | null): DiagCounts {
  return (path && counts[path]) || NONE;
}
