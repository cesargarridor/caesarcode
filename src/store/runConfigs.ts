import { create } from 'zustand';
import {
  runConfigsLoad,
  runConfigsSave,
  runDetect,
  runStart,
  runTemplates,
  serviceRestart,
  serviceStart,
  serviceStop,
} from '../lib/ipc';
import { toMessage } from '../lib/errors';
import { isSameOrInside, qualifyEphemeral, relativeTo, rootFor } from '../lib/paths';
import type { RunConfig, RunConfigFile, RunTarget, RunTemplate, ServiceConfig, Tech } from '../lib/types';
import { useEditor } from './editor';
import { useServices } from './services';
import { useUi } from './ui';
import { useWorkspace } from './workspace';

export type RunSelection =
  | { kind: 'file' }
  | { kind: 'detected'; id: string }
  | { kind: 'config'; id: string }
  | { kind: 'service'; id: string };

/** Id de selección de una config/detectado: `"<raíz>::<id>"`; sin raíz (selección antigua) = primaria. */
export function selId(root: string, id: string): string {
  return `${root}::${id}`;
}
export function splitSelId(qid: string, primary: string | null): { root: string | null; id: string } {
  const i = qid.lastIndexOf('::');
  return i < 0 ? { root: primary, id: qid } : { root: qid.slice(0, i), id: qid.slice(i + 2) };
}

export interface Launched {
  runtimeId: string;
  label: string;
  showInServices: boolean;
  tech?: Tech;
}

interface RunConfigStore {
  /** Configs persistidas por raíz (ruta absoluta). */
  files: Record<string, RunConfigFile>;
  templates: RunTemplate[];
  /** Servicios autodetectados de todas las raíces (cada uno con su `root`; `id` crudo). */
  detected: ServiceConfig[];
  selection: RunSelection;
  /** clave = selectionKey */
  launched: Record<string, Launched>;
  /** Modal de configuraciones: abierto y config enfocada (o `null` = nueva). */
  editor: { open: boolean; focusId: string | null };

  init: () => Promise<void>;
  loadRoot: (root: string) => Promise<void>;
  dropRoot: (root: string) => void;
  reset: () => void;
  refreshDetected: () => Promise<void>;
  save: (root: string, file: RunConfigFile) => Promise<boolean>;
  select: (sel: RunSelection) => void;
  runtimeIdFor: (sel?: RunSelection) => string | null;
  run: (sel?: RunSelection) => Promise<void>;
  stop: (sel?: RunSelection) => Promise<void>;
  openEditor: (focusId?: string | null) => void;
  closeEditor: () => void;
}

const EMPTY: RunConfigFile = { version: 1, configs: [], fileRunners: {} };
const fail = (e: unknown) => useUi.getState().pushToast(toMessage(e));
let initToken = 0;

async function detectAll(roots: string[]): Promise<ServiceConfig[]> {
  const lists = await Promise.all(roots.map(async (r) => (await runDetect(r).catch(() => [])).map((d) => ({ ...d, root: r }))));
  return lists.flat();
}

function existingRoots(): string[] {
  const ws = useWorkspace.getState();
  const list = ws.roots.filter((r) => r.exists).map((r) => r.path);
  return list.length > 0 ? list : ws.root ? [ws.root] : [];
}

export function selectionKey(sel: RunSelection): string {
  return sel.kind === 'file' ? 'file' : `${sel.kind}:${sel.id}`;
}

const storageKey = (root: string) => `cc.runSelection:${root}`;

function loadSelection(root: string): RunSelection {
  try {
    const raw = localStorage.getItem(storageKey(root));
    if (raw) {
      const v = JSON.parse(raw) as RunSelection;
      if (v && (v.kind === 'file' || (typeof (v as { id?: unknown }).id === 'string' && ['detected', 'config', 'service'].includes(v.kind)))) {
        return v;
      }
    }
  } catch {
    /* almacenamiento no disponible o JSON inválido */
  }
  return { kind: 'file' };
}

function saveSelection(root: string | null, sel: RunSelection) {
  if (!root) return;
  try {
    localStorage.setItem(storageKey(root), JSON.stringify(sel));
  } catch {
    /* ignorar */
  }
}

/** Una selección que apunta a algo borrado cae a `{kind:'file'}`. */
export function sanitizeSelection(
  sel: RunSelection,
  files: Record<string, RunConfigFile>,
  detected: ServiceConfig[],
  services: ServiceConfig[],
): RunSelection {
  const primary = useWorkspace.getState().root;
  if (sel.kind === 'config') {
    const { root, id } = splitSelId(sel.id, primary);
    if (!root || !files[root]?.configs.some((c) => c.id === id)) return { kind: 'file' };
  }
  if (sel.kind === 'detected') {
    const { root, id } = splitSelId(sel.id, primary);
    if (!detected.some((c) => c.id === id && c.root === root)) return { kind: 'file' };
  }
  if (sel.kind === 'service' && !services.some((c) => c.id === sel.id)) return { kind: 'file' };
  return sel;
}

/** Ruta del archivo activo del editor (solo pestañas de archivo). */
function activeFilePath(): string | null {
  const { active, tabs } = useEditor.getState();
  return active && tabs.some((t) => t.path === active) ? active : null;
}

/** Id del servicio efímero de "Current File": relativo a la raíz dueña, calificado con su nombre fuera de la primaria. */
export function fileRuntimeId(path: string): string | null {
  const { roots } = useWorkspace.getState();
  const r = rootFor(roots, path);
  if (!r) return null;
  return qualifyEphemeral(roots, r.path, 'file', relativeTo(r.path, path).replace(/\\/g, '/'));
}

/** `ServiceConfig` mostrable de un servicio efímero (para el panel Services). */
export function ephemeralInfo(
  runtimeId: string,
  state: Pick<RunConfigStore, 'files' | 'detected' | 'launched'>,
): { config: ServiceConfig | null; showInServices: boolean; tech?: Tech } {
  const { roots } = useWorkspace.getState();
  const launched = Object.values(state.launched).find((l) => l.runtimeId === runtimeId);
  if (runtimeId.startsWith('run:')) {
    for (const [root, file] of Object.entries(state.files)) {
      const c = file.configs.find((x) => qualifyEphemeral(roots, root, 'run', x.id) === runtimeId);
      if (c) {
        return {
          config: { ...runConfigToService(c), id: runtimeId, root },
          showInServices: c.showInServices,
          tech: techForRunKind(c.kind),
        };
      }
    }
  }
  if (runtimeId.startsWith('auto:')) {
    const d = state.detected.find((x) => qualifyEphemeral(roots, x.root ?? '', 'auto', x.id) === runtimeId);
    if (d) return { config: { ...d, id: runtimeId }, showInServices: true, tech: d.tech ?? undefined };
  }
  return { config: null, showInServices: launched?.showInServices ?? false, tech: launched?.tech };
}

/** Selección `"<raíz>::<id>"` de la config Run a la que corresponde un servicio efímero `run:…` (para abrir el editor). */
export function runConfigRef(runtimeId: string, files: Record<string, RunConfigFile>): string | null {
  const { roots } = useWorkspace.getState();
  for (const [root, file] of Object.entries(files)) {
    const c = file.configs.find((x) => qualifyEphemeral(roots, root, 'run', x.id) === runtimeId);
    if (c) return selId(root, c.id);
  }
  return null;
}

export function runConfigToService(c: RunConfig): ServiceConfig {
  return {
    id: `run:${c.id}`,
    name: c.name,
    command: `${c.command.trim()} ${c.args}`.trim(),
    cwd: c.cwd,
    env: c.env,
    envFile: c.envFile,
    port: c.port,
    kind: 'custom',
    tech: techForRunKind(c.kind),
  };
}

export function techForRunKind(kind: RunConfig['kind']): Tech {
  switch (kind) {
    case 'springBoot':
      return 'spring';
    case 'python':
      return 'python';
    case 'node':
      return 'node';
    case 'go':
      return 'go';
    case 'rust':
      return 'rust';
    default:
      return 'other';
  }
}

export const useRunConfigStore = create<RunConfigStore>((set, get) => ({
  files: {},
  templates: [],
  detected: [],
  selection: { kind: 'file' },
  launched: {},
  editor: { open: false, focusId: null },

  init: async () => {
    const token = ++initToken;
    const root = useWorkspace.getState().root;
    set({ files: {}, detected: [], launched: {}, selection: root ? loadSelection(root) : { kind: 'file' } });
    try {
      const roots = existingRoots();
      const [loaded, templates, detected] = await Promise.all([
        Promise.all(roots.map(async (r) => [r, await runConfigsLoad(r).catch(() => EMPTY)] as const)),
        runTemplates(),
        detectAll(roots),
      ]);
      if (token !== initToken) return;
      const files = Object.fromEntries(loaded);
      set((s) => ({
        files,
        templates,
        detected,
        selection: sanitizeSelection(s.selection, files, detected, useServices.getState().configs),
      }));
    } catch (e) {
      if (token === initToken) fail(e);
    }
  },

  loadRoot: async (root) => {
    try {
      const [file, det] = await Promise.all([runConfigsLoad(root), runDetect(root)]);
      set((s) => ({
        files: { ...s.files, [root]: file },
        detected: [...s.detected.filter((d) => d.root !== root), ...det.map((d) => ({ ...d, root }))],
      }));
    } catch (e) {
      fail(e);
    }
  },

  dropRoot: (root) =>
    set((s) => {
      const files = { ...s.files };
      delete files[root];
      const detected = s.detected.filter((d) => !d.root || !isSameOrInside(root, d.root));
      return { files, detected, selection: sanitizeSelection(s.selection, files, detected, useServices.getState().configs) };
    }),

  reset: () => {
    initToken++;
    set({ files: {}, detected: [], launched: {}, selection: { kind: 'file' }, editor: { open: false, focusId: null } });
    useUi.getState().resetRunTabs();
  },

  refreshDetected: async () => {
    const token = initToken;
    try {
      const detected = await detectAll(existingRoots());
      if (token !== initToken) return;
      set({ detected });
    } catch (e) {
      fail(e);
    }
  },

  save: async (root, file) => {
    try {
      await runConfigsSave(root, file);
      set((s) => {
        const files = { ...s.files, [root]: file };
        return { files, selection: sanitizeSelection(s.selection, files, s.detected, useServices.getState().configs) };
      });
      return true;
    } catch (e) {
      fail(e);
      return false;
    }
  },

  select: (sel) => {
    set({ selection: sel });
    saveSelection(useWorkspace.getState().root, sel);
  },

  runtimeIdFor: (sel) => {
    const s = sel ?? get().selection;
    const ws = useWorkspace.getState();
    switch (s.kind) {
      case 'file': {
        const p = activeFilePath();
        return p ? fileRuntimeId(p) : null;
      }
      case 'detected': {
        const { root, id } = splitSelId(s.id, ws.root);
        return root ? qualifyEphemeral(ws.roots, root, 'auto', id) : null;
      }
      case 'config': {
        const { root, id } = splitSelId(s.id, ws.root);
        return root ? qualifyEphemeral(ws.roots, root, 'run', id) : null;
      }
      case 'service':
        return s.id;
    }
  },

  run: async (selArg) => {
    const st = get();
    const root = useWorkspace.getState().root;
    if (!root) return;
    const sel = sanitizeSelection(selArg ?? st.selection, st.files, st.detected, useServices.getState().configs);
    let targetRoot: string | undefined;
    try {
      let target: RunTarget | null = null;
      let label = '';
      let tech: Tech | undefined;
      let showInServices = false;
      if (sel.kind === 'file') {
        const path = activeFilePath();
        if (!path) {
          useUi.getState().pushToast('No hay archivo activo para ejecutar');
          return;
        }
        // Guardar antes: si no, se ejecutaría la versión vieja del disco.
        if (useEditor.getState().tabs.find((t) => t.path === path)?.dirty) {
          await useEditor.getState().save(path);
          if (useEditor.getState().tabs.find((t) => t.path === path)?.dirty) return; // el guardado falló (ya hay toast)
        }
        target = { type: 'file', path };
        targetRoot = rootFor(useWorkspace.getState().roots, path)?.path;
      } else if (sel.kind === 'detected') {
        const { root: dRoot, id } = splitSelId(sel.id, root);
        const service = st.detected.find((d) => d.id === id && d.root === dRoot);
        if (!service) return;
        targetRoot = dRoot ?? undefined;
        target = { type: 'detected', service };
        label = service.name;
        tech = service.tech ?? undefined;
        showInServices = true;
      } else if (sel.kind === 'config') {
        const { root: cRoot, id } = splitSelId(sel.id, root);
        const c = cRoot ? st.files[cRoot]?.configs.find((x) => x.id === id) : undefined;
        if (!c) return;
        targetRoot = cRoot ?? undefined;
        target = { type: 'config', id: c.id };
        label = c.name;
        tech = techForRunKind(c.kind);
        showInServices = c.showInServices;
      } else {
        // Servicio persistido de services.json: start, o restart si ya corre.
        const svc = useServices.getState();
        const cfg = svc.configs.find((c) => c.id === sel.id);
        if (!cfg) return;
        const state = svc.statuses[cfg.id]?.state;
        if (state === 'running' || state === 'starting') await serviceRestart(cfg.id);
        else await serviceStart(cfg.id);
        useUi.getState().openRunTab(cfg.id, `Run: ${cfg.name}`);
        return;
      }
      const started = await runStart(target, targetRoot);
      const key = selectionKey(sel);
      set((s) => ({
        launched: {
          ...s.launched,
          [key]: { runtimeId: started.runtimeId, label: started.label, showInServices: sel.kind === 'file' ? false : showInServices, tech },
        },
      }));
      const tabLabel = started.label || label;
      useUi.getState().openRunTab(started.runtimeId, tabLabel.startsWith('Run:') ? tabLabel : `Run: ${tabLabel}`);
    } catch (e) {
      fail(e);
    }
  },

  stop: async (selArg) => {
    const id = get().runtimeIdFor(selArg);
    if (!id) return;
    try {
      await serviceStop(id);
    } catch (e) {
      fail(e);
    }
  },

  openEditor: (focusId = null) => set({ editor: { open: true, focusId } }),
  closeEditor: () => set({ editor: { open: false, focusId: null } }),
}));
