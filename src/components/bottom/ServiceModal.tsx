import { useEffect, useMemo, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { FolderOpen } from 'lucide-react';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { IconButton } from '../ui/IconButton';
import { Select } from '../ui/Select';
import { TextInput } from '../ui/TextInput';
import { Badge } from '../ui/Badge';
import { EnvEditor, envFromRows, rowsFromEnv, type EnvRow } from '../ui/EnvEditor';
import { useServices } from '../../store/services';
import { useUi } from '../../store/ui';
import { useWorkspace } from '../../store/workspace';
import { isSameOrInside, relativeTo } from '../../lib/paths';
import { toMessage } from '../../lib/errors';
import { uniqueId } from '../../lib/slug';
import type { ServiceConfig, ServiceKind } from '../../lib/types';

export type ServiceModalState = { mode: 'add' } | { mode: 'detect' } | { mode: 'edit'; config: ServiceConfig };

const KINDS: { value: ServiceKind; label: string }[] = [
  { value: 'custom', label: 'Custom' },
  { value: 'node', label: 'Node' },
  { value: 'python', label: 'Python' },
  { value: 'rust', label: 'Rust' },
  { value: 'maven', label: 'Maven' },
  { value: 'gradle', label: 'Gradle' },
  { value: 'docker', label: 'Docker' },
  { value: 'go', label: 'Go' },
  { value: 'java', label: 'Java' },
  { value: 'shell', label: 'Shell' },
];

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1 block text-ui text-muted">{label}</span>
      {children}
    </label>
  );
}

/** Id crudo (sin el prefijo `<raíz>/` de los ids visibles de raíces no primarias). */
function rawId(c: ServiceConfig, roots: { path: string; name: string }[], primary: string | null): string {
  if (!c.root || c.root === primary) return c.id;
  const name = roots.find((r) => r.path === c.root)?.name;
  return name && c.id.startsWith(`${name}/`) ? c.id.slice(name.length + 1) : c.id;
}

function ManualForm({ initial, onDone }: { initial: ServiceConfig | null; onDone: () => void }) {
  const primary = useWorkspace((s) => s.root);
  const allRoots = useWorkspace((s) => s.roots);
  const roots = allRoots.filter((r) => r.exists);
  const [selRoot, setSelRoot] = useState<string | null>(initial?.root ?? primary);
  const root = selRoot;
  const configs = useServices((s) => s.configs);
  const save = useServices((s) => s.save);
  const [name, setName] = useState(initial?.name ?? '');
  const [command, setCommand] = useState(initial?.command ?? '');
  const [cwd, setCwd] = useState(initial?.cwd ?? '');
  const [envFile, setEnvFile] = useState(initial?.envFile ?? '');
  const [port, setPort] = useState(initial?.port != null ? String(initial.port) : '');
  const [kind, setKind] = useState<ServiceKind>(initial?.kind ?? 'custom');
  const [env, setEnv] = useState<EnvRow[]>(() => rowsFromEnv(initial?.env));
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const pickCwd = async () => {
    if (!root) return;
    try {
      const sel = await open({ directory: true, multiple: false, defaultPath: root, title: 'Carpeta del servicio' });
      if (typeof sel !== 'string') return;
      if (!isSameOrInside(root, sel)) {
        const msg = 'La carpeta debe estar dentro del workspace';
        setError(msg);
        useUi.getState().pushToast(msg);
        return;
      }
      setError(null);
      setCwd(relativeTo(root, sel));
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    }
  };

  const submit = async () => {
    if (!name.trim()) return setError('El nombre es obligatorio');
    if (!command.trim()) return setError('El comando es obligatorio');
    const portNum = port.trim() === '' ? null : Number(port);
    if (portNum !== null && (!Number.isInteger(portNum) || portNum < 1 || portNum > 65535)) {
      return setError('Puerto inválido (1-65535)');
    }
    const envObj = envFromRows(env);
    const cfg: ServiceConfig = {
      id:
        initial?.id ??
        uniqueId(name, new Set(configs.filter((c) => (c.root ?? primary) === root).map((c) => rawId(c, allRoots, primary)))),
      root: root ?? undefined,
      name: name.trim(),
      command: command.trim(),
      cwd: cwd.trim().replace(/^\/+|\/+$/g, ''),
      env: envObj,
      envFile: envFile.trim() || null,
      port: portNum,
      kind,
      tech: initial?.tech ?? null,
      framework: initial?.framework ?? null,
    };
    setSaving(true);
    const list = initial ? configs.map((c) => (c.id === initial.id ? cfg : c)) : [...configs, cfg];
    const ok = await save(list);
    setSaving(false);
    if (ok) onDone();
  };

  return (
    <form
      className="space-y-3"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <Field label="Nombre">
        <TextInput value={name} onChange={(e) => setName(e.target.value)} placeholder="api" autoFocus />
      </Field>
      <Field label="Comando">
        <TextInput
          className="font-mono"
          value={command}
          onChange={(e) => setCommand(e.target.value)}
          placeholder="npm run dev"
        />
      </Field>
      {roots.length > 1 && (
        <div>
          <span className="mb-1 block text-ui text-muted">Proyecto (raíz)</span>
          {initial ? (
            <div className="truncate rounded-lg border border-border px-2 py-1 text-ui-lg text-muted" title={root ?? ''}>
              {roots.find((r) => r.path === root)?.name ?? root}
            </div>
          ) : (
            <Select<string>
              ariaLabel="Proyecto"
              value={root ?? ''}
              options={roots.map((r) => ({ value: r.path, label: r.name }))}
              onChange={(v) => {
                setSelRoot(v);
                setCwd('');
              }}
            />
          )}
        </div>
      )}
      <Field label="Directorio (relativo al proyecto)">
        <div className="flex gap-1">
          <TextInput className="font-mono" value={cwd} onChange={(e) => setCwd(e.target.value)} placeholder="(raíz)" />
          <IconButton label="Elegir carpeta" onClick={() => void pickCwd()} className="h-7 w-7 border border-border-strong">
            <FolderOpen size={14} />
          </IconButton>
        </div>
      </Field>
      <div className="grid grid-cols-2 gap-3">
        <Field label="Puerto esperado">
          <TextInput
            className="font-mono"
            inputMode="numeric"
            value={port}
            onChange={(e) => setPort(e.target.value.replace(/[^0-9]/g, ''))}
            placeholder="3000"
          />
        </Field>
        <div>
          <span className="mb-1 block text-ui text-muted">Tipo</span>
          <Select<ServiceKind> ariaLabel="Tipo" value={kind} options={KINDS} onChange={setKind} />
        </div>
      </div>
      <EnvEditor rows={env} onRows={setEnv} envFile={envFile} onEnvFile={setEnvFile} />
      {error && <p className="text-ui text-stop-text">{error}</p>}
      <div className="flex justify-end gap-2 pt-1">
        <Button onClick={onDone}>Cancelar</Button>
        <Button type="submit" variant="primary" disabled={saving}>
          Guardar
        </Button>
      </div>
    </form>
  );
}

function DetectList({ onDone }: { onDone: () => void }) {
  const configs = useServices((s) => s.configs);
  const detect = useServices((s) => s.detect);
  const save = useServices((s) => s.save);
  const primary = useWorkspace((s) => s.root);
  const roots = useWorkspace((s) => s.roots);
  const multi = roots.filter((r) => r.exists).length > 1;
  const [found, setFound] = useState<ServiceConfig[] | null>(null);
  const [checked, setChecked] = useState<Record<string, boolean>>({});
  const keyOf = (c: ServiceConfig) => `${c.root ?? primary}::${c.id}`;

  const exists = useMemo(
    () => (c: ServiceConfig) =>
      configs.some((x) => (x.root ?? primary) === (c.root ?? primary) && x.cwd === c.cwd && x.command === c.command),
    [configs, primary],
  );

  useEffect(() => {
    let alive = true;
    void detect().then((list) => {
      if (!alive) return;
      setFound(list);
      const init: Record<string, boolean> = {};
      for (const c of list) init[keyOf(c)] = !exists(c);
      setChecked(init);
    });
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const selected = (found ?? []).filter((c) => checked[keyOf(c)] && !exists(c));

  const add = async () => {
    const usedByRoot = new Map<string, Set<string>>();
    const usedFor = (r: string) => {
      let u = usedByRoot.get(r);
      if (!u) {
        u = new Set(configs.filter((c) => (c.root ?? primary) === r).map((c) => rawId(c, roots, primary)));
        usedByRoot.set(r, u);
      }
      return u;
    };
    const added = selected.map((c) => ({ ...c, id: uniqueId(c.id, usedFor(c.root ?? primary ?? '')) }));
    if (await save([...configs, ...added])) onDone();
  };

  if (found === null) return <p className="text-ui text-muted">Buscando…</p>;
  if (found.length === 0) {
    return (
      <div className="space-y-3">
        <p className="text-ui text-muted">
          No se encontraron proyectos (package.json con dev/start, Cargo.toml, pom.xml, build.gradle, pyproject/requirements,
          docker-compose).
        </p>
        <div className="flex justify-end">
          <Button onClick={onDone}>Cerrar</Button>
        </div>
      </div>
    );
  }
  return (
    <div className="space-y-3">
      <ul className="max-h-[44vh] overflow-auto rounded-lg border border-border">
        {found.map((c) => {
          const already = exists(c);
          return (
            <li key={keyOf(c)} className="flex items-start gap-2 border-b border-border px-2 py-1.5 last:border-0">
              <input
                type="checkbox"
                className="mt-1 accent-accent"
                disabled={already}
                checked={!!checked[keyOf(c)] && !already}
                onChange={(e) => setChecked((m) => ({ ...m, [keyOf(c)]: e.target.checked }))}
                aria-label={`Añadir ${c.name}`}
              />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="truncate text-ui-lg text-fg">{c.name}</span>
                  <Badge>{c.kind}</Badge>
                  {c.port != null && <Badge tone="accent">:{c.port}</Badge>}
                  {multi && <Badge>{roots.find((r) => r.path === c.root)?.name ?? ''}</Badge>}
                  {already && <Badge tone="ok">ya añadido</Badge>}
                </div>
                <div className="truncate font-mono text-[11px] text-muted">
                  {c.cwd ? `${c.cwd}/ ` : ''}$ {c.command}
                </div>
              </div>
            </li>
          );
        })}
      </ul>
      <div className="flex justify-end gap-2">
        <Button onClick={onDone}>Cancelar</Button>
        <Button variant="primary" disabled={selected.length === 0} onClick={() => void add()}>
          Añadir {selected.length > 0 ? `(${selected.length})` : ''}
        </Button>
      </div>
    </div>
  );
}

export function ServiceModal({ state, onClose }: { state: ServiceModalState; onClose: () => void }) {
  const [tab, setTab] = useState<'manual' | 'detect'>(state.mode === 'detect' ? 'detect' : 'manual');
  const title = state.mode === 'edit' ? 'Editar servicio' : 'Nuevo servicio';
  return (
    <Modal title={title} onClose={onClose} width={560}>
      {state.mode !== 'edit' && (
        <div className="mb-3 flex gap-1" role="tablist">
          {(['manual', 'detect'] as const).map((t) => (
            <button
              key={t}
              type="button"
              role="tab"
              aria-selected={tab === t}
              onClick={() => setTab(t)}
              className={`h-6 rounded-md px-2.5 text-xs outline-none hover:bg-hover focus-visible:ring-1 focus-visible:ring-accent ${
                tab === t ? 'bg-hover-strong text-fg-strong' : 'text-muted'
              }`}
            >
              {t === 'manual' ? 'Manual' : 'Detectados'}
            </button>
          ))}
        </div>
      )}
      {tab === 'manual' ? (
        <ManualForm initial={state.mode === 'edit' ? state.config : null} onDone={onClose} />
      ) : (
        <DetectList onDone={onClose} />
      )}
    </Modal>
  );
}
