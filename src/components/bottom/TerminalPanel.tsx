import { useEffect, useRef, useState } from 'react';
import { Plus, X } from 'lucide-react';
import { IconButton } from '../ui/IconButton';
import { TerminalView } from './TerminalView';
import { useWorkspace } from '../../store/workspace';
import { useEditor } from '../../store/editor';
import { rootFor } from '../../lib/paths';

interface Tab {
  key: number;
  title: string;
  /** Carpeta de trabajo fijada al crear la terminal. */
  cwd: string;
}

/** Sub-pestañas de terminales. Todas permanecen montadas (ocultas con display:none). */
export function TerminalPanel({ visible }: { visible: boolean }) {
  const root = useWorkspace((s) => s.root);
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeKey, setActiveKey] = useState<number | null>(null);
  const seq = useRef(0);
  const lastRoot = useRef<string | null>(null);

  const add = () => {
    const key = ++seq.current;
    // cwd: raíz del archivo activo (si hay varias raíces) o la primaria.
    const ws = useWorkspace.getState();
    const ed = useEditor.getState();
    const file = ed.active && ed.tabs.some((x) => x.path === ed.active) ? ed.active : null;
    const cwd = (file && rootFor(ws.roots.filter((r) => r.exists), file)?.path) || ws.root || '';
    setTabs((t) => [...t, { key, title: `zsh ${key}`, cwd }]);
    setActiveKey(key);
  };

  // Cambio de workspace: se descartan las terminales (el desmontaje las cierra en Rust).
  useEffect(() => {
    if (lastRoot.current !== root) {
      lastRoot.current = root;
      setTabs([]);
      setActiveKey(null);
    }
  }, [root]);

  // Primera terminal: se crea de forma perezosa al ver la pestaña.
  useEffect(() => {
    if (visible && root && tabs.length === 0 && lastRoot.current === root) add();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visible, root, tabs.length]);

  const close = (key: number) => {
    setTabs((t) => {
      const next = t.filter((x) => x.key !== key);
      setActiveKey((cur) => (cur === key ? (next[next.length - 1]?.key ?? null) : cur));
      return next;
    });
  };

  if (!root) {
    return <div className="flex h-full items-center justify-center text-ui text-muted">Abre una carpeta</div>;
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-9 shrink-0 items-center gap-1 px-2">
        {tabs.map((t) => (
          <div
            key={t.key}
            className={`group flex h-6 items-center gap-1 rounded-md pl-2.5 pr-1 text-xs ${
              t.key === activeKey ? 'bg-hover-strong text-fg-strong' : 'text-muted hover:bg-hover'
            }`}
          >
            <button type="button" className="outline-none" onClick={() => setActiveKey(t.key)}>
              {t.title}
            </button>
            <button
              type="button"
              aria-label={`Cerrar ${t.title}`}
              className="rounded-md p-0.5 outline-none hover:text-stop-text"
              onClick={() => close(t.key)}
            >
              <X size={11} />
            </button>
          </div>
        ))}
        <IconButton label="Nueva terminal" onClick={add}>
          <Plus size={13} />
        </IconButton>
      </div>
      <div className="relative min-h-0 flex-1">
        {tabs.map((t) => (
          <div key={`${root}:${t.key}`} className="absolute inset-0" style={{ display: t.key === activeKey ? 'block' : 'none' }}>
            <TerminalView cwd={t.cwd || root} active={visible && t.key === activeKey} />
          </div>
        ))}
      </div>
    </div>
  );
}
