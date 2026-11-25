import { useMemo, useState } from 'react';
import { Plus, Trash2, X } from 'lucide-react';
import { TextInput } from '../../ui/TextInput';
import { Select } from '../../ui/Select';
import { Button } from '../../ui/Button';
import { IconButton } from '../../ui/IconButton';
import { checkBootVersion, checkPackage, checkPort, checkServiceName, derivePackage, nextPort } from '../../../lib/projectValidate';
import {
  INFRA_NAMES, SPRING_PRESETS, nextKey, type FormState, type InfraForm, type ServiceForm, type SpringForm,
} from '../../../lib/projectRequest';
import { deriveArtifact } from '../../../lib/projectValidate';
import type { BuildSystem, SpringCatalog, SpringDep } from '../../../lib/types';
import { CheckRow, Field } from './Field';

interface Props {
  form: SpringForm;
  common: FormState;
  catalog: SpringCatalog | null;
  /** Spring Boot simple: un solo servicio, sin infraestructura. */
  single: boolean;
  online: boolean | null;
  onChange: (next: SpringForm) => void;
  disabled?: boolean;
}

const BUILDS: { value: BuildSystem; label: string }[] = [
  { value: 'maven', label: 'Maven' },
  { value: 'gradleKotlin', label: 'Gradle (Kotlin DSL)' },
  { value: 'gradleGroovy', label: 'Gradle (Groovy DSL)' },
];

function DepPicker({ catalog, selected, onToggle }: { catalog: SpringCatalog | null; selected: string[]; onToggle: (id: string) => void }) {
  const [q, setQ] = useState('');
  const groups = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return (catalog?.dependencies ?? [])
      .map((g) => ({
        name: g.name,
        items: g.items.filter((d) => !needle || d.id.includes(needle) || d.name.toLowerCase().includes(needle) || d.description.toLowerCase().includes(needle)),
      }))
      .filter((g) => g.items.length > 0);
  }, [catalog, q]);
  return (
    <div className="mt-1.5 rounded-lg border border-border bg-canvas p-2">
      <TextInput autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder="Buscar dependencia…" aria-label="Buscar dependencia" />
      <div className="mt-2 max-h-48 space-y-2 overflow-auto pr-1">
        {groups.length === 0 && <div className="text-[11px] text-muted">Sin resultados</div>}
        {groups.map((g) => (
          <div key={g.name}>
            <div className="mb-0.5 text-[10px] uppercase tracking-wide text-muted">{g.name}</div>
            <div className="flex flex-wrap gap-1">
              {g.items.map((d: SpringDep) => {
                const on = selected.includes(d.id);
                return (
                  <button
                    key={d.id}
                    type="button"
                    title={`${d.description}${d.offline ? '' : ' (solo con start.spring.io)'}`}
                    onClick={() => onToggle(d.id)}
                    className={`rounded-md border px-1.5 py-0.5 text-[11px] outline-none focus-visible:ring-1 focus-visible:ring-accent ${on ? 'border-accent/30 bg-accent/12 text-accent-text' : 'border-border text-fg hover:bg-hover'} ${d.offline ? '' : 'italic'}`}
                  >
                    {d.name}
                  </button>
                );
              })}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function ServiceCard({
  svc, index, single, catalog, form, projectName, disabled, onChange, onRemove, canRemove,
}: {
  svc: ServiceForm; index: number; single: boolean; catalog: SpringCatalog | null; form: SpringForm; projectName: string;
  disabled?: boolean; onChange: (s: ServiceForm) => void; onRemove: () => void; canRemove: boolean;
}) {
  const [picking, setPicking] = useState(false);
  const nameError = single ? null : checkServiceName(svc.name);
  const portError = checkPort(svc.port);
  const toggle = (id: string) => onChange({ ...svc, deps: svc.deps.includes(id) ? svc.deps.filter((d) => d !== id) : [...svc.deps, id] });
  const nameOf = (id: string) => catalog?.dependencies.flatMap((g) => g.items).find((d) => d.id === id)?.name ?? id;
  const presets = SPRING_PRESETS.filter((p) => !svc.deps.includes(p)).slice(0, 10);
  void form;
  return (
    <div className="rounded-lg border border-border bg-card p-2.5">
      <div className="flex items-start gap-2">
        <div className="grid flex-1 grid-cols-[1fr_96px] gap-2">
          <div>
            {single ? (
              <div className="flex h-7 items-center truncate font-mono text-ui-lg text-muted" title="El servicio toma el nombre del proyecto">{deriveArtifact(projectName)}</div>
            ) : (
              <TextInput value={svc.name} disabled={disabled} onChange={(e) => onChange({ ...svc, name: e.target.value })} placeholder="orders-service" aria-label={`Nombre del servicio ${index + 1}`} />
            )}
            {nameError && <div className="mt-0.5 text-[11px] text-stop-text">{nameError}</div>}
          </div>
          <div>
            <TextInput type="number" value={svc.port} disabled={disabled} onChange={(e) => onChange({ ...svc, port: Number(e.target.value) })} aria-label={`Puerto del servicio ${index + 1}`} />
            {portError && <div className="mt-0.5 text-[11px] text-stop-text">{portError}</div>}
          </div>
        </div>
        {canRemove && (
          <IconButton label="Quitar servicio" onClick={onRemove} disabled={disabled}>
            <Trash2 size={13} />
          </IconButton>
        )}
      </div>
      <div className="mt-2 flex flex-wrap items-center gap-1">
        {svc.deps.map((d) => (
          <span key={d} className="inline-flex items-center gap-1 rounded-md bg-accent/12 px-1.5 py-0.5 text-[11px] text-accent-text">
            {nameOf(d)}
            <button type="button" aria-label={`Quitar ${d}`} onClick={() => toggle(d)} disabled={disabled} className="hover:text-fg">
              <X size={10} />
            </button>
          </span>
        ))}
        <Button className="!h-5 !px-1.5 !text-[11px]" onClick={() => setPicking((p) => !p)} disabled={disabled}>
          <Plus size={10} /> Dependencias
        </Button>
      </div>
      {!picking && presets.length > 0 && (
        <div className="mt-1.5 flex flex-wrap gap-1">
          {presets.map((p) => (
            <button key={p} type="button" onClick={() => toggle(p)} disabled={disabled} className="rounded-md border border-border px-1.5 py-0.5 font-mono text-[10px] text-muted hover:bg-hover hover:text-fg">
              + {p}
            </button>
          ))}
        </div>
      )}
      {picking && <DepPicker catalog={catalog} selected={svc.deps} onToggle={toggle} />}
      <div className="mt-2">
        <label className="flex cursor-pointer items-center gap-2 text-ui text-fg">
          <input type="checkbox" checked={svc.layers} disabled={disabled} onChange={(e) => onChange({ ...svc, layers: e.target.checked })} className="h-3.5 w-3.5 accent-accent" />
          Capas controller / service / repository / model
        </label>
      </div>
    </div>
  );
}

function InfraRow({ label, value, name, disabled, onChange }: { label: string; value: InfraForm; name: string; disabled?: boolean; onChange: (v: InfraForm) => void }) {
  const err = value.on ? checkPort(value.port) : null;
  return (
    <div className="flex items-center gap-2">
      <label className="flex min-w-[190px] cursor-pointer items-center gap-2 text-ui-lg text-fg">
        <input type="checkbox" checked={value.on} disabled={disabled} onChange={(e) => onChange({ ...value, on: e.target.checked })} className="h-3.5 w-3.5 accent-accent" />
        {label}
      </label>
      <span className="font-mono text-[11px] text-muted">{name}</span>
      <TextInput className="!w-20" type="number" value={value.port} disabled={disabled || !value.on} onChange={(e) => onChange({ ...value, port: Number(e.target.value) })} aria-label={`Puerto de ${label}`} />
      {err && <span className="text-[11px] text-stop-text">{err}</span>}
    </div>
  );
}

/** Inicializador de microservicios Spring: servicios, dependencias, infraestructura y opciones. */
export function SpringMicroservicesForm({ form, common, catalog, single, online, onChange, disabled }: Props) {
  const set = (patch: Partial<SpringForm>) => onChange({ ...form, ...patch });
  const usedPorts = () => [
    ...form.services.map((s) => s.port),
    ...(form.gateway.on ? [form.gateway.port] : []),
    ...(form.discovery.on ? [form.discovery.port] : []),
    ...(form.config.on ? [form.config.port] : []),
  ];
  const addService = () => {
    const n = form.services.length + 1;
    const used = new Set(form.services.map((s) => s.name));
    let name = `service-${n}`;
    while (used.has(name)) name = `${name}-x`;
    set({ services: [...form.services, { key: nextKey(), name, port: nextPort(usedPorts()), deps: ['web', 'actuator'], layers: true }] });
  };
  const javaVersions = catalog?.javaVersions?.length ? catalog.javaVersions : [17, 21, 25];
  const boots = [{ value: '', label: `Recomendada${catalog ? ` (${catalog.defaultBoot})` : ''}` }, ...(catalog?.bootVersions ?? []).map((v) => ({ value: v, label: v }))];
  const bootError = form.bootVersion ? checkBootVersion(form.bootVersion) : null;
  const gw = (key: 'gateway' | 'discovery' | 'config') => (v: InfraForm) => set({ [key]: v } as Partial<SpringForm>);
  const shownServices = single ? form.services.slice(0, 1) : form.services;

  return (
    <div className="space-y-3">
      <div className="space-y-2.5">
        <Field label="GroupId" error={checkPackage(form.groupId)}>
          <TextInput value={form.groupId} disabled={disabled} onChange={(e) => set({ groupId: e.target.value, ...(form.basePackageTouched ? {} : { basePackage: derivePackage(e.target.value, deriveArtifact(common.name || 'shop')) }) })} aria-label="GroupId" />
        </Field>
        <Field label="Base package" error={checkPackage(form.basePackage)} hint={single ? undefined : 'Cada servicio usa <base>.<nombre sin guiones>'}>
          <TextInput value={form.basePackage} disabled={disabled} onChange={(e) => set({ basePackage: e.target.value, basePackageTouched: true })} aria-label="Base package" />
        </Field>
        <Field label="Java version">
          <Select value={String(form.javaVersion)} options={javaVersions.map((v) => ({ value: String(v), label: String(v) }))} onChange={(v) => set({ javaVersion: Number(v) })} ariaLabel="Java version" />
        </Field>
        <Field label="Spring Boot" error={bootError} hint={catalog?.source === 'offline' ? 'Catálogo offline: se usa la versión de la plantilla local' : undefined}>
          <Select value={form.bootVersion ?? ''} options={boots} onChange={(v) => set({ bootVersion: v || null })} ariaLabel="Spring Boot" />
        </Field>
        <Field label="Build system">
          <Select value={form.build} options={BUILDS} onChange={(build) => set({ build })} ariaLabel="Build system" />
        </Field>
      </div>

      <div>
        <div className="mb-1.5 flex items-center justify-between">
          <h3 className="text-ui font-medium uppercase tracking-wide text-muted">{single ? 'Servicio' : `Microservicios (${form.services.length})`}</h3>
          {!single && (
            <Button onClick={addService} disabled={disabled || form.services.length >= 30} className="!h-6">
              <Plus size={12} /> Añadir servicio
            </Button>
          )}
        </div>
        <div className="space-y-2">
          {shownServices.map((svc, i) => (
            <ServiceCard
              key={svc.key}
              svc={svc}
              index={i}
              single={single}
              catalog={catalog}
              form={form}
              projectName={common.name}
              disabled={disabled}
              canRemove={!single && form.services.length > 1}
              onChange={(s) => set({ services: form.services.map((x) => (x.key === svc.key ? s : x)) })}
              onRemove={() => set({ services: form.services.filter((x) => x.key !== svc.key) })}
            />
          ))}
        </div>
      </div>

      {!single && (
        <div>
          <h3 className="mb-1.5 text-ui font-medium uppercase tracking-wide text-muted">Infraestructura</h3>
          <div className="space-y-1.5">
            <InfraRow label="API Gateway" name={INFRA_NAMES.gateway} value={form.gateway} disabled={disabled} onChange={gw('gateway')} />
            <InfraRow label="Eureka Server" name={INFRA_NAMES.discovery} value={form.discovery} disabled={disabled} onChange={gw('discovery')} />
            <InfraRow label="Config Server" name={INFRA_NAMES.config} value={form.config} disabled={disabled} onChange={gw('config')} />
          </div>
        </div>
      )}

      <div className="space-y-1.5">
        <h3 className="text-ui font-medium uppercase tracking-wide text-muted">Opciones</h3>
        <CheckRow label="docker-compose.yml con bases de datos y mensajería" checked={form.dockerCompose} disabled={disabled} onChange={(dockerCompose) => set({ dockerCompose })} />
        <CheckRow label="Dockerfile multi-stage por servicio" checked={form.dockerfiles} disabled={disabled} onChange={(dockerfiles) => set({ dockerfiles })} />
        <CheckRow
          label="Usar start.spring.io si hay red"
          checked={form.preferOnline}
          disabled={disabled}
          onChange={(preferOnline) => set({ preferOnline })}
          hint={online === false ? 'Sin conexión detectada: se generará con la plantilla offline' : 'Si falla un servicio se usa la plantilla offline para ese servicio'}
        />
      </div>
    </div>
  );
}
