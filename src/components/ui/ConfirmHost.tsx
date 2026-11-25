import { Modal } from './Modal';
import { Button } from './Button';
import { useUi } from '../../store/ui';

/** Renderiza el diálogo pendiente de `useUi.ask(...)`. */
export function ConfirmHost() {
  const asking = useUi((s) => s.asking);
  const resolve = useUi((s) => s.resolveAsk);
  if (!asking) return null;
  return (
    <Modal title={asking.title} width={420} onClose={() => resolve(null)}>
      <p className="mb-4 break-words text-ui-lg text-fg">{asking.message}</p>
      <div className="flex justify-end gap-2">
        {asking.buttons.map((b) => (
          <Button key={b.value} variant={b.variant ?? 'default'} onClick={() => resolve(b.value)}>
            {b.label}
          </Button>
        ))}
      </div>
    </Modal>
  );
}
