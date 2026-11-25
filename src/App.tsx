import { useCallback, useEffect, useState } from 'react';
import { AppLayout } from './components/layout/AppLayout';
import { panelRefs } from './components/layout/panelRefs';
import { QuickOpen } from './components/palette/QuickOpen';
import { NewProjectWizard } from './components/projects/NewProjectWizard';
import { ProjectsScreen } from './components/projects/ProjectsScreen';
import { ConfirmHost } from './components/ui/ConfirmHost';
import { Toasts } from './components/ui/Toasts';
import { UpdateBanner } from './components/ui/UpdateBanner';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { externalOpenDialog } from './lib/ipc';
import { isMod, useGlobalShortcut } from './lib/keys';
import { toMessage } from './lib/errors';
import { useEditor } from './store/editor';
import { useRunConfigStore } from './store/runConfigs';
import { useLsp } from './store/lsp';
import { useProjects } from './store/projects';
import { useUi } from './store/ui';
import { useWorkspace } from './store/workspace';
import { useAgents } from './store/agents';

const LAST_WORKSPACE_KEY = 'cc.lastWorkspace';

function readLastWorkspace(): string | null {
  try {
    return localStorage.getItem(LAST_WORKSPACE_KEY);
  } catch {
    return null; // almacenamiento no disponible
  }
}

export default function App() {
  const openModal = useUi((s) => s.openModal);
  const focusSearch = useUi((s) => s.focusSearch);
  const root = useWorkspace((s) => s.root);
  const screenOpen = useProjects((s) => s.screenOpen);
  const wizardOpen = useProjects((s) => s.wizardOpen);
  // Mientras se reabre el último workspace no se muestra la pantalla Projects (evita un parpadeo).
  const [booting, setBooting] = useState(() => readLastWorkspace() !== null);

  // Reabre el último workspace.
  useEffect(() => {
    const last = readLastWorkspace();
    if (!last) return;
    useWorkspace
      .getState()
      .openWorkspace(last)
      .finally(() => setBooting(false))
      .catch((e) => {
        useUi.getState().pushToast(`No se pudo reabrir ${last}: ${toMessage(e)}`);
        try {
          localStorage.removeItem(LAST_WORKSPACE_KEY);
        } catch {
          /* ignorar */
        }
      });
  }, []);

  // Canal de eventos LSP (diagnósticos y estado); una sola suscripción para toda la sesión.
  useEffect(() => {
    void useLsp.getState().init();
  }, []);

  // Cierre de ventana con cambios sin guardar: pregunta (Guardar todo / Descartar / Cancelar).
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    const win = getCurrentWindow();
    win
      .onCloseRequested(async (event) => {
        const dirty = useEditor.getState().tabs.filter((t) => t.dirty);
        if (dirty.length === 0) return;
        event.preventDefault();
        const choice = await useUi.getState().ask(
          'Cambios sin guardar',
          `${dirty.length === 1 ? `${dirty[0].name} tiene` : `${dirty.length} archivos tienen`} cambios sin guardar. ¿Quieres guardarlos antes de salir?`,
          [
            { label: 'Cancelar', value: 'cancel' },
            { label: 'Descartar', value: 'discard', variant: 'danger' },
            { label: 'Guardar todo', value: 'save', variant: 'primary' },
          ],
        );
        if (choice === null || choice === 'cancel') return;
        if (choice === 'save') {
          for (const t of dirty) await useEditor.getState().save(t.path);
          if (useEditor.getState().tabs.some((t) => t.dirty)) return; // algún guardado falló
        }
        await win.destroy();
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch((e) => useUi.getState().pushToast(toMessage(e)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // Pestañas: Mod+W cierra, Mod+Shift+[ / ] cambian. En captura para ganar a CodeMirror
  // (Ctrl+Shift+[ es "plegar" en su keymap). `code` porque `key` cambia con Shift y el teclado.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!isMod(e) || e.altKey) return;
      const ed = useEditor.getState();
      if (!e.shiftKey && e.key.toLowerCase() === 'w') {
        e.preventDefault();
        e.stopPropagation();
        if (ed.active) void ed.requestClose(ed.active);
      } else if (e.shiftKey && (e.code === 'BracketLeft' || e.code === 'BracketRight')) {
        e.preventDefault();
        e.stopPropagation();
        ed.cycleTab(e.code === 'BracketRight' ? 1 : -1);
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, []);

  // Run Widget: Shift+F10 / Ctrl+Shift+R ejecutan la selección, Ctrl+F2 la detiene.
  // Nunca Cmd+R (recarga del webview). En captura para ganar a CodeMirror.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const runKey =
        (e.key === 'F10' && e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey) ||
        (e.code === 'KeyR' && e.ctrlKey && e.shiftKey && !e.metaKey && !e.altKey);
      const stopKey = e.key === 'F2' && e.ctrlKey && !e.metaKey && !e.shiftKey && !e.altKey;
      if (!runKey && !stopKey) return;
      if (!useWorkspace.getState().root) return;
      e.preventDefault();
      e.stopPropagation();
      if (runKey) void useRunConfigStore.getState().run();
      else void useRunConfigStore.getState().stop();
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, []);

  // Panel de agentes: Mod+Shift+A alterna; Mod+L envía el contexto (@ruta#L25-42) al PTY.
  // En captura + stopPropagation para ganar a CodeMirror (Mod+L = selectLine).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!isMod(e) || e.altKey) return;
      const k = e.key.toLowerCase();
      if (e.shiftKey && k === 'a') {
        e.preventDefault();
        e.stopPropagation();
        const p = panelRefs.agents;
        if (p) {
          if (p.isCollapsed()) p.expand();
          else p.collapse();
        }
      } else if (!e.shiftKey && k === 'l') {
        if (!useWorkspace.getState().root) return;
        e.preventDefault();
        e.stopPropagation();
        panelRefs.agents?.expand();
        void useAgents.getState().sendContext();
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, []);

  // Archivos externos: raíces -> isExternal; evento `external-open` (diálogo, drag&drop, Opened/CLI);
  // Mod+O abre el diálogo nativo. openFile deduplica por ruta (el evento y el retorno del diálogo coinciden).
  const rootPaths = useWorkspace((s) => s.roots);
  useEffect(() => {
    useEditor.getState().setRoots(rootPaths.map((r) => r.path));
  }, [rootPaths]);
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    listen<{ path: string }>('external-open', (ev) => {
      if (ev.payload?.path) void useEditor.getState().openFile(ev.payload.path);
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch((e) => useUi.getState().pushToast(toMessage(e)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!isMod(e) || e.altKey || e.shiftKey || e.key.toLowerCase() !== 'o') return;
      e.preventDefault();
      e.stopPropagation();
      externalOpenDialog()
        .then((paths) => {
          for (const p of paths) void useEditor.getState().openFile(p);
        })
        .catch((e2) => useUi.getState().pushToast(toMessage(e2)));
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, []);

  // Mod+P y Mod+Shift+F en fase de captura: ganan a CodeMirror y evitan imprimir.
  const onQuickOpen = useCallback(() => openModal('quickOpen'), [openModal]);
  const onSearch = useCallback(() => {
    panelRefs.sidebar?.expand();
    focusSearch();
  }, [focusSearch]);
  const onSave = useCallback(() => {
    const { active, save } = useEditor.getState();
    if (active) void save(active);
  }, []);
  useGlobalShortcut('p', onQuickOpen, { capture: true });
  useGlobalShortcut('f', onSearch, { shift: true, capture: true });
  useGlobalShortcut('s', onSave);

  return (
    <>
      {booting ? (
        <div className="h-full bg-base" />
      ) : root ? (
        <>
          <AppLayout />
          {screenOpen && (
            <div className="fixed inset-0 z-[80]">
              <ProjectsScreen />
            </div>
          )}
        </>
      ) : (
        <ProjectsScreen />
      )}
      {wizardOpen && <NewProjectWizard />}
      <QuickOpen />
      <ConfirmHost />
      <Toasts />
      <UpdateBanner />
    </>
  );
}
