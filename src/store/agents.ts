import { create } from 'zustand';
import { agentsDetect, type AgentId, type AgentInfo } from '../lib/ipcAgents';
import { terminalWrite } from '../lib/ipc';
import { relativeTo, rootFor } from '../lib/paths';
import { toMessage } from '../lib/errors';
import { registry } from './editorRegistry';
import { useEditor } from './editor';
import { useUi } from './ui';
import { useWorkspace } from './workspace';

export const AGENT_LABELS: Record<AgentId, string> = {
  claude: 'Claude Code',
  codex: 'Codex',
  gemini: 'Gemini CLI',
};
export const AGENT_IDS: AgentId[] = ['claude', 'codex', 'gemini'];

interface AgentsStore {
  infos: AgentInfo[];
  detecting: boolean;
  selected: AgentId;
  /** Agentes ya iniciados (su terminal sigue montada aunque se cambie de pastilla). */
  started: Partial<Record<AgentId, boolean>>;
  /** Id de terminal PTY por agente (null si no hay sesión viva). */
  termIds: Partial<Record<AgentId, string | null>>;
  /** Raíz primaria con la que se abrió cada sesión. */
  sessionRoot: Partial<Record<AgentId, string>>;
  detect: () => Promise<void>;
  select: (id: AgentId) => void;
  setTermId: (id: AgentId, tid: string | null) => void;
  setSessionRoot: (id: AgentId, root: string | null) => void;
  /** Escribe `@ruta#L25-42` en el PTY del agente seleccionado (sin Enter). */
  sendContext: () => Promise<boolean>;
}

export const useAgents = create<AgentsStore>((set, get) => ({
  infos: [],
  detecting: false,
  selected: 'claude',
  started: {},
  termIds: {},
  sessionRoot: {},
  async detect() {
    set({ detecting: true });
    try {
      set({ infos: await agentsDetect() });
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    } finally {
      set({ detecting: false });
    }
  },
  select(id) {
    const found = get().infos.find((i) => i.id === id)?.found;
    set((s) => ({ selected: id, started: found ? { ...s.started, [id]: true } : s.started }));
  },
  setTermId(id, tid) {
    set((s) => ({ termIds: { ...s.termIds, [id]: tid } }));
  },
  setSessionRoot(id, root) {
    set((s) => {
      const next = { ...s.sessionRoot };
      if (root) next[id] = root;
      else delete next[id];
      return { sessionRoot: next };
    });
  },
  async sendContext() {
    const ref = currentContextRef();
    const { selected, termIds } = get();
    const tid = termIds[selected];
    if (!ref || !tid) return false;
    try {
      await terminalWrite(tid, `${ref} `);
      return true;
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
      return false;
    }
  },
}));

export interface ContextInfo {
  /** Ruta relativa a su raíz (o absoluta si está fuera). */
  rel: string;
  /** Nombre corto del archivo. */
  name: string;
  /** Rango 1-based o null sin selección. */
  range: { from: number; to: number } | null;
}

/** Archivo activo + líneas seleccionadas, leído de `registry.view` (sin tocar el store del editor). */
export function readContext(): ContextInfo | null {
  const { active, tabs } = useEditor.getState();
  if (!active) return null;
  const tab = tabs.find((t) => t.path === active);
  if (!tab || tab.kind !== 'text') return null;
  const ws = useWorkspace.getState();
  const root = rootFor(ws.roots, active)?.path ?? null;
  const rel = root ? relativeTo(root, active) : active;
  let range: ContextInfo['range'] = null;
  const view = registry.view;
  if (view && registry.shownPath === active) {
    const sel = view.state.selection.main;
    if (!sel.empty) {
      const from = view.state.doc.lineAt(sel.from).number;
      // Si la selección termina justo al inicio de una línea, esa línea no cuenta.
      const endPos = sel.to > sel.from && view.state.doc.lineAt(sel.to).from === sel.to ? sel.to - 1 : sel.to;
      const to = view.state.doc.lineAt(endPos).number;
      range = { from, to: Math.max(from, to) };
    }
  }
  return { rel, name: tab.name, range };
}

export function formatRange(r: ContextInfo['range']): string {
  if (!r) return '';
  return r.from === r.to ? `L${r.from}` : `L${r.from}-${r.to}`;
}

function currentContextRef(): string | null {
  const c = readContext();
  if (!c) return null;
  return `@${c.rel}${c.range ? `#${formatRange(c.range)}` : ''}`;
}
