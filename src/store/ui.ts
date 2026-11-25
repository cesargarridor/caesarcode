import { create } from 'zustand';

export type BottomTab = 'services' | 'logs' | 'terminal' | `run:${string}`;
export interface RunTab { id: string; label: string }
export type SidebarView = 'files' | 'search';
export type CenterView = 'editor' | 'git';
export type ToastKind = 'error' | 'info';
export interface Toast { id: number; kind: ToastKind; message: string }

export interface AskButton { label: string; value: string; variant?: 'default' | 'primary' | 'danger' }
export interface AskRequest {
  title: string;
  message: string;
  buttons: AskButton[];
  resolve: (value: string | null) => void;
}

interface UiState {
  bottomTab: BottomTab;
  setBottomTab: (t: BottomTab) => void;
  /** Pestañas dinámicas `Run: <archivo>`; `id` = runtimeId del servicio efímero. */
  runTabs: RunTab[];
  openRunTab: (id: string, label: string) => void;
  closeRunTab: (id: string) => void;
  resetRunTabs: () => void;
  /** Vista central: editor o Git (el editor sigue montado, solo se oculta). */
  centerView: CenterView;
  setCenterView: (v: CenterView) => void;
  sidebarView: SidebarView;
  setSidebarView: (v: SidebarView) => void;
  /** Se incrementa para pedir foco en el campo de búsqueda global. */
  searchFocusTick: number;
  focusSearch: () => void;
  /** Modales abiertos por nombre (quickOpen, ...). */
  modals: Record<string, boolean>;
  openModal: (name: string) => void;
  closeModal: (name: string) => void;
  toasts: Toast[];
  pushToast: (message: string, kind?: ToastKind) => void;
  dismissToast: (id: number) => void;
  /** Diálogo de confirmación con botones; resuelve con el `value` pulsado o `null` (cancelado). */
  asking: AskRequest | null;
  ask: (title: string, message: string, buttons: AskButton[]) => Promise<string | null>;
  resolveAsk: (value: string | null) => void;
}

let toastSeq = 0;

export const useUi = create<UiState>((set, get) => ({
  bottomTab: 'services',
  setBottomTab: (t) => set({ bottomTab: t }),
  runTabs: [],
  openRunTab: (id, label) =>
    set((s) => ({
      runTabs: s.runTabs.some((t) => t.id === id)
        ? s.runTabs.map((t) => (t.id === id ? { ...t, label } : t))
        : [...s.runTabs, { id, label }],
      bottomTab: `run:${id}`,
    })),
  closeRunTab: (id) =>
    set((s) => ({
      runTabs: s.runTabs.filter((t) => t.id !== id),
      bottomTab: s.bottomTab === `run:${id}` ? 'services' : s.bottomTab,
    })),
  resetRunTabs: () =>
    set((s) => ({ runTabs: [], bottomTab: s.bottomTab.startsWith('run:') ? 'services' : s.bottomTab })),
  centerView: 'editor',
  setCenterView: (v) => set({ centerView: v }),
  sidebarView: 'files',
  setSidebarView: (v) => set({ sidebarView: v }),
  searchFocusTick: 0,
  focusSearch: () => set((s) => ({ sidebarView: 'search', searchFocusTick: s.searchFocusTick + 1 })),
  modals: {},
  openModal: (name) => set((s) => ({ modals: { ...s.modals, [name]: true } })),
  closeModal: (name) => set((s) => ({ modals: { ...s.modals, [name]: false } })),
  toasts: [],
  pushToast: (message, kind = 'error') => {
    const id = ++toastSeq;
    set((s) => ({ toasts: [...s.toasts, { id, kind, message }] }));
    setTimeout(() => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })), 6000);
  },
  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
  asking: null,
  ask: (title, message, buttons) =>
    new Promise<string | null>((resolve) => {
      get().asking?.resolve(null); // una pregunta pendiente anterior se cancela
      set({ asking: { title, message, buttons, resolve } });
    }),
  resolveAsk: (value) => {
    const cur = get().asking;
    if (!cur) return;
    set({ asking: null });
    cur.resolve(value);
  },
}));
