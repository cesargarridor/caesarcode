import type { ImperativePanelHandle } from 'react-resizable-panels';

/** Referencias imperativas a paneles (para expandir el sidebar desde atajos globales). */
export const panelRefs = {
  sidebar: null as ImperativePanelHandle | null,
  agents: null as ImperativePanelHandle | null,
};
