import { create } from 'zustand';
import { open } from '@tauri-apps/plugin-dialog';
import {
  projectGenerators, projectsRecentList, projectsRecentPin, projectsRecentRemove, projectsToolsDetect,
} from '../lib/ipc';
import { toMessage } from '../lib/errors';
import type { GeneratorInfo, RecentProject, ToolsReport } from '../lib/types';
import { useUi } from './ui';
import { useWorkspace } from './workspace';

interface ProjectsStore {
  /** Pantalla Projects a pantalla completa (siempre visible sin workspace; opcional con uno abierto). */
  screenOpen: boolean;
  wizardOpen: boolean;
  recent: RecentProject[];
  recentLoaded: boolean;
  tools: ToolsReport | null;
  generators: GeneratorInfo[];
  openScreen: () => void;
  closeScreen: () => void;
  openWizard: () => void;
  closeWizard: () => void;
  loadRecent: () => Promise<void>;
  remove: (path: string) => Promise<void>;
  pin: (path: string, pinned: boolean) => Promise<void>;
  /** Detecta herramientas/JDKs (y red) y carga los generadores. */
  loadTools: (checkOnline: boolean) => Promise<void>;
  /** Abre un proyecto reciente: si ya no existe ofrece quitarlo. */
  openProject: (p: RecentProject) => Promise<void>;
  /** Añade un reciente como raíz del workspace abierto (Attach). */
  attachProject: (p: RecentProject) => Promise<void>;
  /** Diálogo nativo "Abrir carpeta". */
  openFolder: () => Promise<void>;
}

export const useProjects = create<ProjectsStore>((set, get) => ({
  screenOpen: false,
  wizardOpen: false,
  recent: [],
  recentLoaded: false,
  tools: null,
  generators: [],

  openScreen: () => set({ screenOpen: true }),
  closeScreen: () => set({ screenOpen: false }),
  openWizard: () => set({ wizardOpen: true }),
  closeWizard: () => set({ wizardOpen: false }),

  loadRecent: async () => {
    try {
      set({ recent: await projectsRecentList(), recentLoaded: true });
    } catch (e) {
      set({ recentLoaded: true });
      useUi.getState().pushToast(toMessage(e));
    }
  },

  remove: async (path) => {
    try {
      await projectsRecentRemove(path);
      set((s) => ({ recent: s.recent.filter((r) => r.path !== path) }));
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    }
  },

  pin: async (path, pinned) => {
    try {
      await projectsRecentPin(path, pinned);
      await get().loadRecent();
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    }
  },

  loadTools: async (checkOnline) => {
    try {
      const tools = await projectsToolsDetect(checkOnline);
      const generators = await projectGenerators();
      set({ tools, generators });
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    }
  },

  openProject: async (p) => {
    if (!p.exists) {
      const c = await useUi.getState().ask('Proyecto no encontrado', `${p.path} ya no existe en el disco.`, [
        { label: 'Cancelar', value: 'cancel' },
        { label: 'Quitar de recientes', value: 'remove', variant: 'danger' },
      ]);
      if (c === 'remove') await get().remove(p.path);
      return;
    }
    try {
      // Con un workspace abierto: "This window / Attach" de IntelliJ (no hay multiventana).
      if (useWorkspace.getState().root !== null) {
        const c = await useUi.getState().ask('Abrir proyecto', `¿Qué hacer con "${p.name}"?`, [
          { label: 'Cancelar', value: 'cancel' },
          { label: 'Añadir al workspace', value: 'attach' },
          { label: 'Abrir aquí (reemplaza)', value: 'replace', variant: 'danger' },
        ]);
        if (c === 'attach') await get().attachProject(p);
        else if (c === 'replace') await useWorkspace.getState().openWorkspace(p.path);
        return;
      }
      await useWorkspace.getState().openWorkspace(p.path);
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    }
  },

  attachProject: async (p) => {
    if (!p.exists) {
      useUi.getState().pushToast(`${p.path} ya no existe en el disco.`);
      return;
    }
    if (await useWorkspace.getState().addRoot(p.path)) {
      useUi.getState().pushToast(`"${p.name}" añadido al workspace`, 'info');
      set({ screenOpen: false });
    }
  },

  openFolder: async () => {
    try {
      const sel = await open({ directory: true, multiple: false, title: 'Abrir carpeta' });
      if (typeof sel === 'string') await useWorkspace.getState().openWorkspace(sel);
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    }
  },
}));

// Al abrir cualquier workspace (reciente, diálogo, asistente) la pantalla Projects se cierra.
useWorkspace.subscribe((s, prev) => {
  if (s.root !== prev.root) useProjects.setState({ screenOpen: false });
});
