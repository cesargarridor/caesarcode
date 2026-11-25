import { create } from 'zustand';
import {
  portKill,
  serviceClearLogs,
  serviceLogs,
  serviceRestart,
  serviceStart,
  serviceStop,
  servicesDetect,
  servicesLoad,
  servicesSave,
  servicesStartAll,
  servicesStopAll,
  servicesSubscribe,
} from '../lib/ipc';
import { toMessage } from '../lib/errors';
import type { ServiceConfig, ServiceStatus } from '../lib/types';
import { clearLocal, mergeBacklog, pushLogs, resetLogs } from './logs';
import { isSameOrInside } from '../lib/paths';
import { useUi } from './ui';
import { useWorkspace } from './workspace';

export type Selection = string | 'all';

interface ServicesStore {
  configs: ServiceConfig[];
  statuses: Record<string, ServiceStatus>;
  selectedId: Selection;

  /** Carga configs, suscribe canales y trae el backlog. Se llama al abrir un workspace. */
  init: () => Promise<void>;
  /** (Re)carga los servicios de una raíz (p. ej. al añadirla o tras guardar). */
  loadRoot: (root: string) => Promise<void>;
  /** Olvida los servicios de una raíz quitada del workspace. */
  dropRoot: (root: string) => void;
  reset: () => void;
  select: (id: Selection) => void;
  save: (configs: ServiceConfig[]) => Promise<boolean>;
  /** Detecta en todas las raíces (o solo en `root`); cada resultado lleva su `root`. */
  detect: (root?: string) => Promise<ServiceConfig[]>;
  start: (id: string) => Promise<void>;
  stop: (id: string) => Promise<void>;
  restart: (id: string) => Promise<void>;
  startAll: () => Promise<void>;
  stopAll: () => Promise<void>;
  killPort: (port: number) => Promise<void>;
  clearLogs: (id: Selection) => Promise<void>;
}

const fail = (e: unknown) => useUi.getState().pushToast(toMessage(e));

/** Raíces existentes del workspace (la primaria primero). */
function existingRoots(): string[] {
  const ws = useWorkspace.getState();
  const list = ws.roots.filter((r) => r.exists).map((r) => r.path);
  return list.length > 0 ? list : ws.root ? [ws.root] : [];
}
let initToken = 0;

export const useServices = create<ServicesStore>((set) => ({
  configs: [],
  statuses: {},
  selectedId: 'all',

  init: async () => {
    const token = ++initToken;
    resetLogs();
    set({ configs: [], statuses: {}, selectedId: 'all' });
    try {
      const roots = existingRoots();
      const lists = await Promise.all(roots.map((r) => servicesLoad(r).catch(() => [] as ServiceConfig[])));
      if (token !== initToken) return;
      set({ configs: lists.flat() });
      await servicesSubscribe(
        (lines) => {
          if (token === initToken) pushLogs(lines);
        },
        (list) => {
          if (token !== initToken) return;
          const statuses: Record<string, ServiceStatus> = {};
          for (const s of list) statuses[s.id] = s;
          set({ statuses });
        },
      );
      // Backlog (recarga del webview o servicios ya corriendo): lo anterior al streaming.
      const backlog = await serviceLogs(null, 20_000);
      if (token === initToken) mergeBacklog(backlog);
    } catch (e) {
      fail(e);
    }
  },

  loadRoot: async (root) => {
    try {
      const list = await servicesLoad(root);
      set((s) => ({ configs: [...s.configs.filter((c) => c.root !== root), ...list] }));
    } catch (e) {
      fail(e);
    }
  },

  dropRoot: (root) =>
    set((s) => ({
      configs: s.configs.filter((c) => !c.root || !isSameOrInside(root, c.root)),
      selectedId: s.configs.some((c) => c.id === s.selectedId && c.root && isSameOrInside(root, c.root)) ? 'all' : s.selectedId,
    })),

  reset: () => {
    initToken++;
    resetLogs();
    set({ configs: [], statuses: {}, selectedId: 'all' });
  },

  select: (id) => set({ selectedId: id }),

  save: async (configs) => {
    const primary = useWorkspace.getState().root;
    if (!primary) return false;
    const groups = new Map<string, ServiceConfig[]>();
    // Las raíces que tenían servicios y ahora no, también se escriben (lista vacía).
    for (const c of useServices.getState().configs) groups.set(c.root ?? primary, []);
    for (const c of configs) {
      const r = c.root ?? primary;
      groups.set(r, [...(groups.get(r) ?? []), c]);
    }
    try {
      await Promise.all([...groups].map(([root, list]) => servicesSave(root, list)));
      const lists = await Promise.all([...groups.keys()].map((r) => servicesLoad(r)));
      set((s) => {
        const roots = new Set(groups.keys());
        const next = [...s.configs.filter((c) => !roots.has(c.root ?? primary)), ...lists.flat()];
        return { configs: next, selectedId: s.selectedId !== 'all' && !next.some((c) => c.id === s.selectedId) ? 'all' : s.selectedId };
      });
      return true;
    } catch (e) {
      fail(e);
      return false;
    }
  },

  detect: async (root) => {
    const roots = root ? [root] : existingRoots();
    try {
      const lists = await Promise.all(
        roots.map(async (r) => (await servicesDetect(r)).map((c) => ({ ...c, root: r }))),
      );
      return lists.flat();
    } catch (e) {
      fail(e);
      return [];
    }
  },

  start: async (id) => {
    try {
      await serviceStart(id);
    } catch (e) {
      fail(e);
    }
  },
  stop: async (id) => {
    try {
      await serviceStop(id);
    } catch (e) {
      fail(e);
    }
  },
  restart: async (id) => {
    try {
      await serviceRestart(id);
    } catch (e) {
      fail(e);
    }
  },
  startAll: async () => {
    try {
      await servicesStartAll();
    } catch (e) {
      fail(e);
    }
  },
  stopAll: async () => {
    try {
      await servicesStopAll();
    } catch (e) {
      fail(e);
    }
  },
  killPort: async (port) => {
    try {
      const pids = await portKill(port);
      useUi.getState().pushToast(
        pids.length ? `Puerto ${port} liberado (pid ${pids.join(', ')})` : `Nada escuchando en el puerto ${port}`,
        'info',
      );
    } catch (e) {
      fail(e);
    }
  },
  clearLogs: async (id) => {
    try {
      await serviceClearLogs(id === 'all' ? null : id);
      clearLocal(id);
    } catch (e) {
      fail(e);
    }
  },
}));

/** (en ejecución, total) para la StatusBar. */
export function runningCount(statuses: Record<string, ServiceStatus>, configs: ServiceConfig[]): [number, number] {
  let n = 0;
  for (const c of configs) if (statuses[c.id]?.state === 'running') n++;
  return [n, configs.length];
}
