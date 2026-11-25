import { open } from '@tauri-apps/plugin-dialog';
import { FolderOpen } from 'lucide-react';
import { TextInput } from '../../ui/TextInput';
import { Button } from '../../ui/Button';
import { checkProjectName, previewPath } from '../../../lib/projectValidate';
import { toMessage } from '../../../lib/errors';
import { useUi } from '../../../store/ui';
import type { FormState } from '../../../lib/projectRequest';
import { CheckRow, Field } from './Field';

interface Props {
  form: FormState;
  onName: (name: string) => void;
  onChange: (patch: Partial<FormState>) => void;
  disabled?: boolean;
}

/** Name, Location (diálogo de carpeta), Create Git repository y vista previa de la ruta final. */
export function CommonFields({ form, onName, onChange, disabled }: Props) {
  const nameError = form.name ? checkProjectName(form.name) : null;
  const browse = async () => {
    try {
      const sel = await open({ directory: true, multiple: false, title: 'Carpeta de destino', defaultPath: form.location || undefined });
      if (typeof sel === 'string') onChange({ location: sel });
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    }
  };
  return (
    <div className="space-y-2.5">
      <Field label="Name" error={nameError}>
        <TextInput autoFocus value={form.name} disabled={disabled} onChange={(e) => onName(e.target.value)} placeholder="my-project" aria-label="Name" />
      </Field>
      <Field label="Location" hint={form.name && form.location ? `Se creará en ${previewPath(form.location, form.name)}` : undefined}>
        <div className="flex gap-1.5">
          <TextInput value={form.location} disabled={disabled} onChange={(e) => onChange({ location: e.target.value })} placeholder="Carpeta donde crear el proyecto" aria-label="Location" />
          <Button onClick={browse} disabled={disabled} aria-label="Elegir carpeta" title="Elegir carpeta">
            <FolderOpen size={13} />
          </Button>
        </div>
      </Field>
      <CheckRow label="Create Git repository" checked={form.git} disabled={disabled} onChange={(git) => onChange({ git })} />
    </div>
  );
}
