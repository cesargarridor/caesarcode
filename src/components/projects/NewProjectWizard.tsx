import { useEffect, useMemo, useRef, useState } from 'react';
import {
  Atom, Boxes, Code2, Coffee, Cpu, FileCode, Folder, Globe, Hammer, Layers, Leaf, Network, Package, Rocket,
  Server, Terminal, Triangle, Zap, type LucideIcon,
} from 'lucide-react';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { GenerationLog, type GenStatus } from './GenerationLog';
import { ToolWarning } from './ToolWarning';
import { CommonFields } from './forms/CommonFields';
import { JvmFields } from './forms/JvmFields';
import { WebFields } from './forms/WebFields';
import { SpringMicroservicesForm } from './forms/SpringMicroservicesForm';
import { projectCancel, projectCreate, springCatalog } from '../../lib/ipc';
import { toMessage } from '../../lib/errors';
import {
  GENERATOR_META, buildRequest, defaultForm, defaultSpring, isJvm, validateForm, withName,
  type FormState, type GeneratorId, type SpringForm,
} from '../../lib/projectRequest';
import type { GenEvent, SpringCatalog } from '../../lib/types';
import { useEditor } from '../../store/editor';
import { useProjects } from '../../store/projects';
import { useUi } from '../../store/ui';
import { useWorkspace } from '../../store/workspace';

const LOCATION_KEY = 'cc.newProject.location';
const MAX_EVENTS = 3000;

const ICONS: Record<GeneratorId, LucideIcon> = {
  java: Coffee, kotlin: Code2, groovy: Code2, empty: Folder, mavenArchetype: Package, javaFx: Layers,
  jakartaEe: Boxes, springBoot: Leaf, springMicroservices: Network, quarkus: Zap, micronaut: Rocket, ktor: Server,
  html: Globe, react: Atom, vue: Triangle, vite: Zap, angular: Hammer, express: Server, node: Cpu, python: Terminal,
  go: FileCode, rust: Hammer,
};
const GROUPS = ['Java/JVM', 'Frameworks', 'Web', 'Otros'];
const SPRING: GeneratorId[] = ['springBoot', 'springMicroservices'];

function savedLocation(): string {
  try {
    return localStorage.getItem(LOCATION_KEY) ?? '';
  } catch {
    return '';
  }
}

/** Asistente New Project estilo IntelliJ: generadores a la izquierda, formulario a la derecha. */
export function NewProjectWizard() {
  const closeWizard = useProjects((s) => s.closeWizard);
  const tools = useProjects((s) => s.tools);
  const generators = useProjects((s) => s.generators);
  const [gen, setGen] = useState<GeneratorId>('java');
  const [form, setForm] = useState<FormState>(() => defaultForm(savedLocation()));
  const [spring, setSpring] = useState<SpringForm>(() => defaultSpring(''));
  const [catalog, setCatalog] = useState<SpringCatalog | null>(null);
  const [phase, setPhase] = useState<'form' | GenStatus>('form');
  const [events, setEvents] = useState<GenEvent[]>([]);
  const jobRef = useRef<string | null>(null);
  const jdkApplied = useRef(false);
  const catalogRequested = useRef(false);

  useEffect(() => {
    void useProjects.getState().loadTools(true);
  }, []);

  // JDK por defecto: uno de 21 si existe, si no el más reciente detectado.
  useEffect(() => {
    if (!tools || jdkApplied.current || tools.jdks.length === 0) return;
    jdkApplied.current = true;
    const pick = tools.jdks.find((j) => j.major === 21) ?? tools.jdks[0];
    setForm((f) => ({ ...f, jdkHome: pick.home, javaVersion: pick.major }));
    setSpring((s) => ({ ...s, javaVersion: [17, 21, 25].includes(pick.major) ? pick.major : s.javaVersion }));
  }, [tools]);

  // Catálogo Spring: el offline llega al instante; el online lo reemplaza si la red responde.
  useEffect(() => {
    if (!SPRING.includes(gen) || catalogRequested.current) return;
    catalogRequested.current = true;
    springCatalog(false).then(setCatalog).catch((e) => useUi.getState().pushToast(toMessage(e)));
    springCatalog(true).then((c) => c.source === 'online' && setCatalog(c)).catch(() => undefined);
  }, [gen]);

  const info = generators.find((g) => g.id === gen);
  const errors = useMemo(() => validateForm(gen, form, spring), [gen, form, spring]);
  const running = phase === 'running';
  const patch = (p: Partial<FormState>) => setForm((f) => ({ ...f, ...p }));
  const onName = (name: string) => {
    setForm((f) => withName(f, name));
    setSpring((s) => (s.basePackageTouched ? s : { ...s, basePackage: withName(defaultForm(), name).packageName }));
  };
  const canCreate = phase === 'form' && errors.length === 0 && (info?.available ?? true);

  const create = async () => {
    const req = buildRequest(gen, form, spring);
    const jobId = crypto.randomUUID();
    jobRef.current = jobId;
    setEvents([]);
    setPhase('running');
    try {
      localStorage.setItem(LOCATION_KEY, form.location);
    } catch {
      /* almacenamiento no disponible */
    }
    let sawFailure = false;
    const push = (ev: GenEvent) => {
      if (ev.type === 'failed') sawFailure = true;
      setEvents((prev) => (prev.length >= MAX_EVENTS ? [...prev.slice(prev.length - MAX_EVENTS + 1), ev] : [...prev, ev]));
    };
    try {
      const root = await projectCreate(req, jobId, push);
      setPhase('done');
      await useWorkspace.getState().openWorkspace(root);
      closeWizard();
      const sep = root.includes('\\') && !root.includes('/') ? '\\' : '/';
      void useEditor.getState().openFile(`${root}${sep}README.md`).catch(() => undefined);
    } catch (e) {
      const msg = toMessage(e);
      if (!sawFailure) push({ type: 'failed', message: msg });
      setPhase(msg === 'Cancelado' ? 'cancelled' : 'failed');
    } finally {
      jobRef.current = null;
    }
  };

  const cancel = () => {
    if (jobRef.current) void projectCancel(jobRef.current);
  };

  const specificFields = () => {
    if (SPRING.includes(gen)) {
      return (
        <SpringMicroservicesForm
          form={spring}
          common={form}
          catalog={catalog}
          single={gen === 'springBoot'}
          online={tools?.online ?? null}
          onChange={setSpring}
          disabled={running}
        />
      );
    }
    if (isJvm(gen)) return <JvmFields generator={gen} form={form} jdks={tools?.jdks ?? []} onChange={patch} disabled={running} />;
    return <WebFields generator={gen} form={form} onChange={patch} disabled={running} />;
  };

  return (
    <Modal title="New Project" onClose={() => !running && closeWizard()} width={1020}>
      <div className="grid h-[62vh] min-h-[380px] grid-cols-[210px_1fr] gap-3">
        <nav aria-label="Generadores" className="overflow-auto rounded-lg border border-border bg-canvas p-1">
          {GROUPS.map((group) => (
            <div key={group} className="mb-1">
              <div className="px-3 pb-0.5 pt-2 text-[10px] uppercase tracking-wide text-muted">{group}</div>
              {GENERATOR_META.filter((g) => g.group === group).map((g) => {
                const Icon = ICONS[g.id];
                const gi = generators.find((x) => x.id === g.id);
                const active = g.id === gen;
                return (
                  <button
                    key={g.id}
                    type="button"
                    disabled={running}
                    aria-current={active}
                    onClick={() => setGen(g.id)}
                    title={gi && !gi.available ? `Falta: ${gi.missing.join(', ')}` : undefined}
                    className={`flex w-full items-center gap-2 rounded-md px-2 py-1 text-left text-ui-lg outline-none focus-visible:bg-hover ${active ? 'bg-accent/12 text-accent-text' : 'text-fg hover:bg-hover'} ${gi && !gi.available ? 'opacity-50' : ''}`}
                  >
                    <Icon size={13} className={active ? 'text-accent' : 'text-muted'} />
                    <span className="truncate">{g.label}</span>
                  </button>
                );
              })}
            </div>
          ))}
        </nav>

        <section className="flex min-h-0 flex-col">
          {phase === 'form' ? (
            <>
              <div className="min-h-0 flex-1 overflow-auto pr-1">
                <ToolWarning generator={info} tools={tools} jvm={isJvm(gen)} />
                <div className="space-y-4">
                  <CommonFields form={form} onName={onName} onChange={patch} />
                  {specificFields()}
                </div>
              </div>
              <div className="mt-3 flex items-center justify-between gap-3 border-t border-border pt-2.5">
                <div className="min-w-0 truncate text-[11px] text-stop-text" title={errors.join('\n')}>{errors[0] ?? ''}</div>
                <div className="flex shrink-0 gap-2">
                  <Button onClick={closeWizard}>Cancelar</Button>
                  <Button variant="primary" disabled={!canCreate} onClick={() => void create()}>Crear</Button>
                </div>
              </div>
            </>
          ) : (
            <GenerationLog events={events} status={phase} onCancel={cancel} onBack={() => setPhase('form')} />
          )}
        </section>
      </div>
    </Modal>
  );
}
