import { AlertTriangle, WifiOff } from 'lucide-react';
import { Button } from '../ui/Button';
import type { GeneratorInfo, ToolsReport } from '../../lib/types';

const INSTALL: Record<string, string> = {
  mvn: 'Apache Maven: https://maven.apache.org (macOS: brew install maven)',
  gradle: 'Gradle: https://gradle.org/install',
  node: 'Node.js: https://nodejs.org',
  npm: 'npm viene con Node.js: https://nodejs.org',
  npx: 'npx viene con Node.js: https://nodejs.org',
  ng: 'Angular CLI: npm i -g @angular/cli',
  cargo: 'Rust: https://rustup.rs',
  go: 'Go: https://go.dev/dl',
  python3: 'Python: https://python.org',
  git: 'Git: https://git-scm.com',
  java: 'JDK: https://adoptium.net',
  javac: 'JDK: https://adoptium.net',
};

interface Props {
  generator: GeneratorInfo | undefined;
  tools: ToolsReport | null;
  /** Solo si existe plantilla local alternativa. */
  onUseLocal?: () => void;
  jvm: boolean;
}

/** Avisos no bloqueantes: herramienta ausente, sin red (usa plantilla offline), sin JDK. */
export function ToolWarning({ generator, tools, onUseLocal, jvm }: Props) {
  if (!generator) return null;
  const items: JSX.Element[] = [];
  if (generator.missing.length > 0) {
    items.push(
      <div key="missing" className="rounded-lg border border-stop/30 bg-stop/10 p-2.5 text-ui-lg">
        <div className="flex items-center gap-1.5 font-medium text-stop-text">
          <AlertTriangle size={13} /> Falta {generator.missing.map((m) => `'${m}'`).join(', ')} en el PATH
        </div>
        <ul className="mt-1 list-disc pl-5 text-[11px] text-muted">
          {generator.missing.map((m) => <li key={m}>{INSTALL[m] ?? m}</li>)}
        </ul>
        <div className="mt-1 text-[11px] text-muted">Instálalo, reinicia CaesarCode y vuelve a intentarlo. Sin esta herramienta no se puede crear este tipo de proyecto.</div>
        {generator.offlineFallback && onUseLocal && (
          <Button className="mt-2" onClick={onUseLocal}>Usar plantilla local</Button>
        )}
      </div>,
    );
  }
  if (generator.strategy === 'httpWithFallback' && tools?.online === false) {
    items.push(
      <div key="offline" className="flex items-start gap-1.5 rounded-lg border border-gold/30 bg-gold/10 p-2 text-[11px] text-gold">
        <WifiOff size={12} className="mt-[2px] shrink-0" />
        Sin conexión con el generador oficial: se creará con la plantilla offline (versiones fijas, algunas dependencias pueden omitirse).
      </div>,
    );
  }
  if (jvm && tools && tools.jdks.length === 0) {
    items.push(
      <div key="jdk" className="flex items-start gap-1.5 rounded-lg border border-border bg-elevated p-2 text-[11px] text-muted">
        <AlertTriangle size={12} className="mt-[2px] shrink-0" />
        No se detectó ningún JDK. Puedes crear el proyecto, pero necesitarás uno para compilarlo ({INSTALL.java}).
      </div>,
    );
  }
  return items.length ? <div className="mb-3 space-y-2">{items}</div> : null;
}
