import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

export interface MenuItem {
  label: string;
  onSelect: () => void;
  danger?: boolean;
  disabled?: boolean;
  /** Dibuja un separador encima de este item. */
  separator?: boolean;
  /** Tooltip (útil para explicar por qué está deshabilitado). */
  title?: string;
}

interface Props {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
}

/** Menú contextual posicionado en clientX/Y; cierra con Esc o clic fuera. */
export function ContextMenu({ x, y, items, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    setPos({
      x: Math.max(4, Math.min(x, window.innerWidth - r.width - 4)),
      y: Math.max(4, Math.min(y, window.innerHeight - r.height - 4)),
    });
  }, [x, y, items]);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('mousedown', onDown);
    window.addEventListener('keydown', onKey);
    window.addEventListener('blur', onClose);
    window.addEventListener('resize', onClose);
    return () => {
      window.removeEventListener('mousedown', onDown);
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('blur', onClose);
      window.removeEventListener('resize', onClose);
    };
  }, [onClose]);

  return createPortal(
    <div
      ref={ref}
      role="menu"
      style={{ left: pos.x, top: pos.y }}
      onContextMenu={(e) => e.preventDefault()}
      className="fixed z-[100] min-w-[170px] rounded-lg border border-border-strong bg-elevated p-1 shadow-xl"
    >
      {items.map((it, i) => (
        <div key={i}>
          {it.separator && <div className="my-1 h-px bg-border" />}
          <button
            type="button"
            role="menuitem"
            disabled={it.disabled}
            title={it.title}
            onClick={() => {
              onClose();
              it.onSelect();
            }}
            className={`block w-full rounded-md px-2.5 py-1 text-left text-ui-lg outline-none hover:bg-hover disabled:opacity-40 ${it.danger ? 'text-stop-text' : 'text-fg'}`}
          >
            {it.label}
          </button>
        </div>
      ))}
    </div>,
    document.body,
  );
}
