import { create } from 'zustand';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { toMessage } from '../lib/errors';

type Status = 'idle' | 'checking' | 'uptodate' | 'available' | 'installing' | 'error';

interface UpdaterState {
  status: Status;
  update: Update | null;
  error: string | null;
  dismissed: boolean;
  /** `silent`: sin mostrar errores (comprobación al arrancar). */
  check: (silent?: boolean) => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => void;
}

export const useUpdater = create<UpdaterState>((set, get) => ({
  status: 'idle',
  update: null,
  error: null,
  dismissed: false,
  async check(silent = false) {
    if (get().status === 'checking' || get().status === 'installing') return;
    set({ status: 'checking', error: null });
    try {
      const update = await check();
      if (update) set({ status: 'available', update, dismissed: false });
      else set({ status: 'uptodate', update: null });
    } catch (e) {
      set(silent ? { status: 'idle' } : { status: 'error', error: toMessage(e) });
    }
  },
  async install() {
    const update = get().update;
    if (!update || get().status === 'installing') return;
    set({ status: 'installing', error: null });
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (e) {
      set({ status: 'error', error: toMessage(e) });
    }
  },
  dismiss: () => set({ dismissed: true }),
}));
