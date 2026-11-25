import { TextInput } from '../../ui/TextInput';
import { Select } from '../../ui/Select';
import { checkGoModule } from '../../../lib/projectValidate';
import { ANGULAR_STYLES, VITE_TEMPLATES, type FormState, type GeneratorId } from '../../../lib/projectRequest';
import { CheckRow, Field } from './Field';

interface Props {
  generator: GeneratorId;
  form: FormState;
  onChange: (patch: Partial<FormState>) => void;
  disabled?: boolean;
}

/** Opciones específicas de Web y otros lenguajes (TS/JS, plantilla de Vite, Angular, venv, módulo Go...). */
export function WebFields({ generator, form, onChange, disabled }: Props) {
  switch (generator) {
    case 'react':
    case 'express':
    case 'node':
      return <CheckRow label="TypeScript" checked={form.typescript} disabled={disabled} onChange={(typescript) => onChange({ typescript })} />;
    case 'vue':
      return (
        <div className="space-y-2">
          <CheckRow label="TypeScript" checked={form.typescript} disabled={disabled} onChange={(typescript) => onChange({ typescript })} />
          <CheckRow label="Vue Router" checked={form.router} disabled={disabled} onChange={(router) => onChange({ router })} />
          <CheckRow label="Pinia" checked={form.pinia} disabled={disabled} onChange={(pinia) => onChange({ pinia })} />
        </div>
      );
    case 'vite':
      return (
        <Field label="Template">
          <Select value={form.viteTemplate} options={VITE_TEMPLATES.map((t) => ({ value: t, label: t }))} onChange={(viteTemplate) => onChange({ viteTemplate })} ariaLabel="Template" />
        </Field>
      );
    case 'angular':
      return (
        <div className="space-y-2.5">
          <CheckRow label="Routing" checked={form.ngRouting} disabled={disabled} onChange={(ngRouting) => onChange({ ngRouting })} />
          <Field label="Stylesheet format">
            <Select value={form.ngStyle} options={ANGULAR_STYLES.map((t) => ({ value: t, label: t }))} onChange={(ngStyle) => onChange({ ngStyle })} ariaLabel="Stylesheet format" />
          </Field>
        </div>
      );
    case 'python':
      return <CheckRow label="Create virtual environment (.venv)" checked={form.venv} disabled={disabled} onChange={(venv) => onChange({ venv })} />;
    case 'go':
      return (
        <Field label="Module" error={form.goModule ? checkGoModule(form.goModule) : null}>
          <TextInput value={form.goModule} disabled={disabled} onChange={(e) => onChange({ goModule: e.target.value, goModuleTouched: true })} placeholder="github.com/you/project" />
        </Field>
      );
    case 'rust':
      return (
        <Field label="Crate type">
          <Select
            value={form.rustBin ? 'bin' : 'lib'}
            options={[{ value: 'bin', label: 'Binary (application)' }, { value: 'lib', label: 'Library' }]}
            onChange={(v) => onChange({ rustBin: v === 'bin' })}
            ariaLabel="Crate type"
          />
        </Field>
      );
    default:
      return null;
  }
}
