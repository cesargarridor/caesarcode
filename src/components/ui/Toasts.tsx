import { X } from 'lucide-react';
import { useUi } from '../../store/ui';

export function Toasts() {
  const toasts = useUi((s) => s.toasts);
  const dismiss = useUi((s) => s.dismissToast);
  if (toasts.length === 0) return null;
  return (
    <div className="pointer-events-none fixed bottom-8 right-3 z-[120] flex w-80 flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          role="alert"
          className={`pointer-events-auto flex items-start gap-2 rounded-lg border bg-elevated px-3 py-2 text-ui-lg shadow-xl ${
            t.kind === 'error' ? 'border-stop/40 text-fg' : 'border-border-strong text-fg'
          }`}
        >
          <span className={`mt-1 h-1.5 w-1.5 shrink-0 rounded-full ${t.kind === 'error' ? 'bg-stop' : 'bg-accent'}`} />
          <span className="min-w-0 flex-1 break-words font-mono text-[11px]">{t.message}</span>
          <button type="button" aria-label="Cerrar aviso" onClick={() => dismiss(t.id)} className="text-muted hover:text-fg">
            <X size={12} />
          </button>
        </div>
      ))}
    </div>
  );
}
