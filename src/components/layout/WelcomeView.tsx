import { BrandMark } from './BrandMark';
import { Kbd } from '../ui/Kbd';
import { modLabel } from '../../lib/keys';

export function WelcomeView() {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-6">
      <div className="flex flex-col items-center text-center">
        <BrandMark size={96} className="mb-3" />
        <div className="font-mono text-2xl font-medium tracking-tight text-fg">
          Caesar<span className="text-accent">Code</span>
        </div>
        <div className="mt-2 h-px w-16 bg-crimson-deep" />
        <div className="mt-1 text-ui text-muted">Editor nativo, ligero y local-first</div>
      </div>
      <ul className="space-y-2 text-ui-lg text-muted">
        <li className="flex items-center justify-between gap-8">
          <span>Abrir archivo</span>
          <span className="flex gap-1"><Kbd>{modLabel}</Kbd><Kbd>P</Kbd></span>
        </li>
        <li className="flex items-center justify-between gap-8">
          <span>Buscar en el proyecto</span>
          <span className="flex gap-1"><Kbd>{modLabel}</Kbd><Kbd>⇧</Kbd><Kbd>F</Kbd></span>
        </li>
        <li className="flex items-center justify-between gap-8">
          <span>Guardar</span>
          <span className="flex gap-1"><Kbd>{modLabel}</Kbd><Kbd>S</Kbd></span>
        </li>
      </ul>
    </div>
  );
}
