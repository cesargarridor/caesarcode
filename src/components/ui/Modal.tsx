import { useEffect, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import { IconButton } from './IconButton';

interface Props {
  title: string;
  onClose: () => void;
  children: ReactNode;
  width?: number;
}

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Modal con overlay translúcido, foco atrapado y cierre con Esc. */
export function Modal({ title, onClose, children, width = 520 }: Props) {
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null;
    const panel = panelRef.current;
    const first = panel?.querySelector<HTMLElement>('input, textarea, button');
    (first ?? panel)?.focus();
    return () => prev?.focus?.();
  }, []);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key !== 'Tab' || !panelRef.current) return;
    const items = Array.from(panelRef.current.querySelectorAll<HTMLElement>(FOCUSABLE));
    if (items.length === 0) {
      e.preventDefault();
      return;
    }
    const first = items[0];
    const last = items[items.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };

  return createPortal(
    <div
      className="fixed inset-0 z-[90] flex items-start justify-center bg-black/60 pt-[12vh] backdrop-blur-sm"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
        onKeyDown={onKeyDown}
        style={{ width }}
        className="max-h-[76vh] max-w-[92vw] overflow-auto rounded-xl border border-border-strong bg-elevated shadow-2xl outline-none"
      >
        <div className="flex items-center justify-between px-4 pb-2 pt-3">
          <h2 className="text-ui-lg font-medium text-fg">{title}</h2>
          <IconButton label="Cerrar" onClick={onClose}>
            <X size={14} />
          </IconButton>
        </div>
        <div className="px-4 pb-4 pt-1">{children}</div>
      </div>
    </div>,
    document.body,
  );
}
