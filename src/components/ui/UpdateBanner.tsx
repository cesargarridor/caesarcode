import { useEffect } from 'react';
import { X } from 'lucide-react';
import { useUpdater } from '../../store/updater';
import { Button } from './Button';

/** Aviso discreto de nueva versión. Comprueba al arrancar en silencio; los errores se ignoran. */
export function UpdateBanner() {
  const status = useUpdater((s) => s.status);
  const update = useUpdater((s) => s.update);
  const dismissed = useUpdater((s) => s.dismissed);
  useEffect(() => {
    void useUpdater.getState().check(true);
  }, []);
  if (!update || dismissed || (status !== 'available' && status !== 'installing')) return null;
  const busy = status === 'installing';
  return (
    <div className="fixed bottom-10 left-3 z-[120] flex items-center gap-3 rounded-lg border border-border-strong bg-elevated px-3 py-2 text-ui-lg text-fg shadow-xl" role="status">
      <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent" />
      <span>Nueva versión {update.version} disponible</span>
      <Button variant="primary" disabled={busy} onClick={() => void useUpdater.getState().install()}>
        {busy ? 'Actualizando…' : 'Actualizar y reiniciar'}
      </Button>
      {!busy && (
        <button type="button" aria-label="Cerrar aviso" onClick={() => useUpdater.getState().dismiss()} className="text-muted hover:text-fg">
          <X size={12} />
        </button>
      )}
    </div>
  );
}
