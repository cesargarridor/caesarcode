// Sincronización de documentos con el servidor LSP (full sync).
// Todo lo de un mismo archivo pasa por una cadena de promesas: Tauri ejecuta los comandos async
// en paralelo, así que sin esto un didChange podría adelantarse a su didOpen o a un completion.
import { lspDidChange, lspDidClose, lspDidOpen, lspDidSave } from '../../../lib/ipc';
import { registry } from '../../../store/editorRegistry';
import { useLsp } from '../../../store/lsp';
import { lspLanguage } from '../languages';

const DEBOUNCE_MS = 150;

const versions = new Map<string, number>();
const timers = new Map<string, ReturnType<typeof setTimeout>>();
const chains = new Map<string, Promise<void>>();
/** El servidor existe para este archivo (resultado del didOpen). */
const available = new Map<string, boolean>();

function chain(path: string, job: () => Promise<unknown>): Promise<void> {
  const next = (chains.get(path) ?? Promise.resolve()).then(job).then(
    () => undefined,
    () => undefined, // un fallo del servidor nunca debe romper el editor
  );
  chains.set(path, next);
  return next;
}

export function lspAvailable(path: string | null): path is string {
  return !!path && available.get(path) === true;
}

export function lspOpen(path: string, text: string) {
  if (!lspLanguage(path)) return;
  versions.set(path, 1);
  void chain(path, async () => {
    const st = await lspDidOpen(path, text);
    available.set(path, st.available);
    useLsp.getState().setDoc(path, st);
  });
}

function sendChange(path: string): Promise<void> {
  return chain(path, async () => {
    if (!available.get(path)) return;
    const st = registry.states.get(path);
    if (!st) return;
    const version = (versions.get(path) ?? 1) + 1;
    versions.set(path, version);
    await lspDidChange(path, version, st.sliceDoc());
  });
}

/** didChange con debounce de 150 ms (texto completo). */
export function scheduleDidChange(path: string) {
  if (!lspLanguage(path)) return;
  const prev = timers.get(path);
  if (prev) clearTimeout(prev);
  timers.set(
    path,
    setTimeout(() => {
      timers.delete(path);
      void sendChange(path);
    }, DEBOUNCE_MS),
  );
}

/** Envía ya el didChange pendiente y espera a que el servidor lo tenga. Llamar antes de completion/hover/definition. */
export function flushChange(path: string): Promise<void> {
  const t = timers.get(path);
  if (t) {
    clearTimeout(t);
    timers.delete(path);
    void sendChange(path);
  }
  return chains.get(path) ?? Promise.resolve();
}

export function lspSave(path: string, text: string) {
  if (!lspLanguage(path)) return;
  void flushChange(path);
  void chain(path, async () => {
    if (available.get(path)) await lspDidSave(path, text);
  });
}

export function lspClose(path: string) {
  const t = timers.get(path);
  if (t) clearTimeout(t);
  timers.delete(path);
  if (!versions.has(path)) return;
  void chain(path, async () => {
    if (available.get(path)) await lspDidClose(path);
  }).then(() => {
    available.delete(path);
    versions.delete(path);
    useLsp.getState().forgetDoc(path);
  });
}

/** Cambio de workspace: Rust ya soltó los servidores del anterior; solo se olvida el estado local. */
export function lspResetSync() {
  for (const t of timers.values()) clearTimeout(t);
  timers.clear();
  versions.clear();
  chains.clear();
  available.clear();
}
