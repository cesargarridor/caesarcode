import { useEffect, useRef, useState } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebglAddon } from '@xterm/addon-webgl';
import '@xterm/xterm/css/xterm.css';
import { terminalClose, terminalOpen, terminalResize, terminalWrite } from '../../lib/ipc';
import { toMessage } from '../../lib/errors';
import { useUi } from '../../store/ui';
import { Button } from '../ui/Button';
import { useSettings, EDITOR_FONTS } from '../../store/settings';
import { getTheme } from '../../theme/themes';
import { xtermTheme } from '../../theme/xterm';

interface Props {
  cwd: string;
  /** Pestaña visible: pide foco y reajusta. */
  active: boolean;
}

/** Terminal interactiva: xterm.js (WebGL con fallback a DOM) conectado a un PTY de Rust. */
export function TerminalView({ cwd, active }: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const glRef = useRef<WebglAddon | null>(null);
  const [session, setSession] = useState(0);
  const [exited, setExited] = useState(false);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    setExited(false);
    let disposed = false;
    let id: string | null = null;

    const term = new Terminal({
      fontFamily: EDITOR_FONTS[useSettings.getState().editorFont].stack,
      fontSize: 13,
      cursorBlink: true,
      allowProposedApi: true,
      scrollback: 5000,
      theme: xtermTheme(getTheme(useSettings.getState().theme)),
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host);
    try {
      const gl = new WebglAddon();
      gl.onContextLoss(() => {
        gl.dispose();
        glRef.current = null;
      });
      term.loadAddon(gl);
      glRef.current = gl;
    } catch {
      /* sin WebGL: renderer DOM */
    }
    termRef.current = term;
    fitRef.current = fit;

    const doFit = (): boolean => {
      if (host.clientWidth === 0 || host.clientHeight === 0) return false; // oculto
      try {
        fit.fit();
        return true;
      } catch {
        return false;
      }
    };
    doFit();

    const dataSub = term.onData((d) => {
      if (id) void terminalWrite(id, d).catch(() => undefined);
    });

    terminalOpen(cwd, term.cols || 80, term.rows || 24, (ev) => {
      if (disposed) return;
      if (ev.type === 'data') term.write(ev.data);
      else {
        term.write('\r\n\x1b[90m[proceso terminado]\x1b[0m\r\n');
        id = null;
        setExited(true);
      }
    })
      .then((tid) => {
        if (disposed) {
          void terminalClose(tid).catch(() => undefined);
          return;
        }
        id = tid;
        // Por si el tamaño cambió mientras se abría.
        void terminalResize(tid, term.cols, term.rows).catch(() => undefined);
      })
      .catch((e) => {
        if (!disposed) {
          useUi.getState().pushToast(toMessage(e));
          term.write(`\x1b[31m${toMessage(e)}\x1b[0m\r\n`);
          setExited(true);
        }
      });

    let timer: ReturnType<typeof setTimeout> | null = null;
    const ro = new ResizeObserver(() => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        if (disposed || !doFit()) return;
        if (id) void terminalResize(id, term.cols, term.rows).catch(() => undefined);
      }, 50);
    });
    ro.observe(host);

    return () => {
      disposed = true;
      if (timer) clearTimeout(timer);
      ro.disconnect();
      dataSub.dispose();
      if (id) void terminalClose(id).catch(() => undefined);
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
      glRef.current = null;
    };
  }, [cwd, session]);

  // El terminal sigue el tema y la fuente en vivo (xterm necesita hex, no lee CSS vars).
  useEffect(() => {
    const apply = () => {
      const term = termRef.current;
      if (!term) return;
      const st = useSettings.getState();
      term.options.theme = xtermTheme(getTheme(st.theme));
      term.options.fontFamily = EDITOR_FONTS[st.editorFont].stack;
      try { glRef.current?.clearTextureAtlas(); } catch { /* sin WebGL */ }
      try { fitRef.current?.fit(); } catch { /* oculto */ }
    };
    apply();
    return useSettings.subscribe((s, p) => {
      if (s.theme !== p.theme || s.editorFont !== p.editorFont) apply();
    });
  }, [session]);

  useEffect(() => {
    if (active) termRef.current?.focus();
  }, [active, session]);

  return (
    <div className="relative h-full w-full">
      <div ref={hostRef} className="h-full w-full" />
      {exited && (
        <div className="absolute bottom-3 right-3">
          <Button variant="primary" onClick={() => setSession((s) => s + 1)}>
            Reabrir terminal
          </Button>
        </div>
      )}
    </div>
  );
}
