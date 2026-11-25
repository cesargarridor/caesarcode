import { useMemo, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { ChevronDown, Copy, FolderOpen, Plus, Trash2 } from 'lucide-react';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { IconButton } from '../ui/IconButton';
import { Select } from '../ui/Select';
import { TextInput } from '../ui/TextInput';
import { ContextMenu, type MenuItem } from '../ui/ContextMenu';
import { EnvEditor, envFromRows, rowsFromEnv, type EnvRow } from '../ui/EnvEditor';
import { TechIcon } from './TechIcon';
import { splitSelId, techForRunKind, useRunConfigStore } from '../../store/runConfigs';
import { useEditor } from '../../store/editor';
import { useUi } from '../../store/ui';
import { useWorkspace } from '../../store/workspace';
import { isSameOrInside, relativeTo, rootFor } from '../../lib/paths';
import { toMessage } from '../../lib/errors';
import { uniqueId } from '../../lib/slug';
import type { RunConfig, RunKind, RunTemplate } from '../../lib/types';

interface Draft {
  /** Clave local estable del borrador. */
  key: string;
  /** Id persistido; `null` = nueva (se genera del nombre al guardar). */
  id: string | null;
  /** Raíz del workspace donde vive (o vivirá) la config. */
  root: string;
  name: string;
  kind: RunKind;
  cwd: string;
  command: string;
  args: string;
  env: EnvRow[];
  envFile: string;
  port: string;
  showInServices: boolean;
}

const KIND_LABELS: Record<RunKind, string> = {
  springBoot: 'Spring Boot',
  python: 'Python',
  node: 'Node/TypeScript',
  go: 'Go',
  rust: 'Rust',
  shell: 'Custom Shell',
};
const KIND_OPTIONS = (Object.keys(KIND_LABELS) as RunKind[]).map((value) => ({ value, label: KIND_LABELS[value] }));

let keySeq = 0;
const newKey = () => `d${++keySeq}`;

const fromConfig = (c: RunConfig, root: string): Draft => ({
  key: newKey(),
  id: c.id,
  root,
  name: c.name,
  kind: c.kind,
  cwd: c.cwd,
  command: c.command,
  args: c.args,
  env: rowsFromEnv(c.env),
  envFile: c.envFile ?? '',
  port: c.port != null ? String(c.port) : '',
  showInServices: c.showInServices,
});

const fromTemplate = (t: RunTemplate, root: string): Draft => ({ ...fromConfig(t.config, root), id: null, name: t.label });

const badRel = (p: string) => /^([/\\]|[a-zA-Z]:)/.test(p) || p.split(/[\\/]/).some((s) => s === '..');

function validate(d: Draft): string | null {
  if (!d.name.trim()) return 'El nombre es obligatorio';
  if (!d.command.trim()) return 'El comando es obligatorio';
  if (badRel(d.cwd.trim())) return 'El directorio debe ser relativo al workspace (sin "..")';
  if (d.envFile.trim() && badRel(d.envFile.trim())) return 'El archivo .env debe ser relativo (sin "..")';
  if (d.port.trim() !== '') {
    const n = Number(d.port);
    if (!Number.isInteger(n) || n < 1 || n > 65535) return 'Puerto inválido (1-65535)';
  }
  return null;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1 block text-ui text-muted">{label}</span>
      {children}
    </label>
  );
}

export function RunConfigModal() {
  const root = useWorkspace((s) => s.root);
  const files = useRunConfigStore((s) => s.files);
  const roots = useWorkspace((s) => s.roots).filter((r) => r.exists);
  const multi = roots.length > 1;
  const templates = useRunConfigStore((s) => s.templates);
  const focusId = useRunConfigStore((s) => s.editor.focusId);
  const save = useRunConfigStore((s) => s.save);
  const closeEditor = useRunConfigStore((s) => s.closeEditor);

  const [init] = useState(() => {
    const list = roots.flatMap((r) => (files[r.path]?.configs ?? []).map((c) => fromConfig(c, r.path)));
    const f = focusId ? splitSelId(focusId, root) : null;
    return { list, key: (list.find((d) => f && d.id === f.id && d.root === f.root) ?? list[0])?.key ?? null };
  });
  const [drafts, setDrafts] = useState<Draft[]>(init.list);
  const [selKey, setSelKey] = useState<string | null>(init.key);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const current = drafts.find((d) => d.key === selKey) ?? null;
  const patch = (p: Partial<Draft>) =>
    setDrafts((list) => list.map((d) => (d.key === selKey ? { ...d, ...p } : d)));

  // Raíz por defecto de una config nueva: la del borrador seleccionado, si no la del archivo activo, si no la primaria.
  const defaultRoot = (): string => {
    if (current) return current.root;
    const ed = useEditor.getState();
    const active = ed.active && ed.tabs.some((t) => t.path === ed.active) ? ed.active : null;
    return (active && rootFor(roots, active)?.path) || root || roots[0]?.path || '';
  };

  const add = (d: Draft) => {
    setDrafts((list) => [...list, d]);
    setSelKey(d.key);
    setError(null);
  };

  const templateItems = useMemo<MenuItem[]>(
    () => templates.map((t) => ({ label: t.label, onSelect: () => add(fromTemplate(t, defaultRoot())) })),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [templates, current?.root],
  );

  const duplicate = () => {
    if (!current) return;
    add({ ...current, key: newKey(), id: null, name: `${current.name} (copia)`, env: current.env.map((r) => ({ ...r })) });
  };

  const remove = async () => {
    if (!current) return;
    const choice = await useUi.getState().ask('Eliminar configuración', `¿Eliminar "${current.name || 'sin nombre'}"? Se aplica al guardar.`, [
      { label: 'Cancelar', value: 'cancel' },
      { label: 'Eliminar', value: 'delete', variant: 'danger' },
    ]);
    if (choice !== 'delete') return;
    const i = drafts.findIndex((d) => d.key === current.key);
    const rest = drafts.filter((d) => d.key !== current.key);
    setDrafts(rest);
    setSelKey(rest[Math.min(i, rest.length - 1)]?.key ?? null);
    setError(null);
  };

  const changeKind = (kind: RunKind) => {
    if (!current) return;
    const prev = templates.find((t) => t.kind === current.kind)?.config;
    const next = templates.find((t) => t.kind === kind)?.config;
    const p: Partial<Draft> = { kind };
    // Config nueva: rellena command/args/puerto de la plantilla si no se han tocado.
    if (current.id === null && next) {
      const untouched = (!current.command.trim() && !current.args.trim()) || (prev && current.command === prev.command && current.args === prev.args);
      if (untouched) {
        p.command = next.command;
        p.args = next.args;
        if (current.port.trim() === '' || (prev?.port != null && current.port === String(prev.port))) {
          p.port = next.port != null ? String(next.port) : '';
        }
      }
      if (!current.name.trim() || current.name === KIND_LABELS[current.kind]) p.name = KIND_LABELS[kind];
    }
    patch(p);
  };

  const pickCwd = async () => {
    const base = current?.root ?? root;
    if (!base) return;
    try {
      const sel = await open({ directory: true, multiple: false, defaultPath: base, title: 'Directorio de trabajo' });
      if (typeof sel !== 'string') return;
      if (!isSameOrInside(base, sel)) {
        const msg = 'La carpeta debe estar dentro del workspace';
        setError(msg);
        useUi.getState().pushToast(msg);
        return;
      }
      setError(null);
      patch({ cwd: relativeTo(base, sel) });
    } catch (e) {
      useUi.getState().pushToast(toMessage(e));
    }
  };

  const submit = async () => {
    for (const d of drafts) {
      const err = validate(d);
      if (err) {
        setSelKey(d.key);
        setError(err);
        return;
      }
    }
    // Ids: los existentes se conservan; los nuevos se generan del nombre (slug, sin ':') y son únicos POR raíz.
    const byRoot = new Map<string, RunConfig[]>();
    for (const r of Object.keys(files)) if ((files[r]?.configs.length ?? 0) > 0) byRoot.set(r, []);
    for (const d of drafts) if (!byRoot.has(d.root)) byRoot.set(d.root, []);
    for (const [r, list] of byRoot) {
      const mine = drafts.filter((d) => d.root === r);
      const used = new Set(mine.filter((d) => d.id !== null).map((d) => d.id as string));
      for (const d of mine) {
        list.push({
          id: d.id ?? uniqueId(d.name, used, 'run'),
          name: d.name.trim(),
          kind: d.kind,
          cwd: d.cwd.trim().replace(/^\.?[/\\]+|[/\\]+$/g, ''),
          command: d.command.trim(),
          args: d.args,
          env: envFromRows(d.env),
          envFile: d.envFile.trim() || null,
          port: d.port.trim() === '' ? null : Number(d.port),
          showInServices: d.showInServices,
        });
      }
    }
    setSaving(true);
    let ok = true;
    for (const [r, configs] of byRoot) {
      ok = (await save(r, { ...(files[r] ?? { version: 1, fileRunners: {} }), configs })) && ok;
    }
    setSaving(false);
    if (ok) closeEditor();
  };

  return (
    <Modal title="Configuraciones de ejecución" onClose={closeEditor} width={780}>
      <div className="flex gap-3">
        <div className="flex w-52 shrink-0 flex-col gap-2">
          <div className="flex items-center gap-1">
            <Button
              className="flex-1"
              onClick={(e) => {
                const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
                setMenu({ x: r.left, y: r.bottom + 2 });
              }}
              aria-haspopup="menu"
            >
              <Plus size={13} /> Nuevo <ChevronDown size={11} className="text-muted" />
            </Button>
            <IconButton label="Duplicar" onClick={duplicate} disabled={!current} className="h-7 w-7 border border-border">
              <Copy size={13} />
            </IconButton>
            <IconButton label="Eliminar" onClick={() => void remove()} disabled={!current} className="h-7 w-7 border border-border">
              <Trash2 size={13} />
            </IconButton>
          </div>
          <ul role="listbox" aria-label="Configuraciones" className="min-h-[220px] flex-1 overflow-auto rounded border border-border">
            {drafts.map((d) => (
              <li
                key={d.key}
                role="option"
                aria-selected={d.key === selKey}
                onClick={() => {
                  setSelKey(d.key);
                  setError(null);
                }}
                className={`flex cursor-pointer items-center gap-2 px-2 py-1.5 text-ui-lg ${
                  d.key === selKey ? 'bg-hover text-accent' : 'text-fg hover:bg-hover'
                }`}
              >
                <TechIcon tech={techForRunKind(d.kind)} />
                <span className="min-w-0 flex-1 truncate">{d.name.trim() || 'Sin nombre'}</span>
                {multi && <span className="max-w-[40%] truncate font-mono text-[10px] text-faint">{roots.find((r) => r.path === d.root)?.name}</span>}
                {d.id === null && <span className="font-mono text-[10px] text-muted">nuevo</span>}
              </li>
            ))}
            {drafts.length === 0 && <li className="px-2 py-3 text-ui text-muted">Sin configuraciones. Crea una con Nuevo.</li>}
          </ul>
        </div>

        <form
          className="min-w-0 flex-1 space-y-3"
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
        >
          {current ? (
            <>
              <div className="grid grid-cols-2 gap-3">
                <Field label="Nombre">
                  <TextInput value={current.name} onChange={(e) => patch({ name: e.target.value })} placeholder="API dev" />
                </Field>
                <div>
                  <span className="mb-1 block text-ui text-muted">Tipo</span>
                  <Select<RunKind> ariaLabel="Tipo" value={current.kind} options={KIND_OPTIONS} onChange={changeKind} />
                </div>
              </div>
              {multi && (
                <div>
                  <span className="mb-1 block text-ui text-muted">Proyecto (raíz)</span>
                  {current.id === null ? (
                    <Select<string>
                      ariaLabel="Proyecto"
                      value={current.root}
                      options={roots.map((r) => ({ value: r.path, label: r.name }))}
                      onChange={(r) => patch({ root: r, cwd: '' })}
                    />
                  ) : (
                    <div className="truncate rounded-lg border border-border px-2 py-1 text-ui-lg text-muted" title={current.root}>
                      {roots.find((r) => r.path === current.root)?.name ?? current.root}
                    </div>
                  )}
                </div>
              )}
              <Field label="Working Directory (relativo al proyecto)">
                <div className="flex gap-1">
                  <TextInput className="font-mono" value={current.cwd} onChange={(e) => patch({ cwd: e.target.value })} placeholder="(raíz)" />
                  <IconButton label="Elegir carpeta" onClick={() => void pickCwd()} className="h-7 w-7 border border-border">
                    <FolderOpen size={14} />
                  </IconButton>
                </div>
              </Field>
              <div className="grid grid-cols-2 gap-3">
                <Field label="Comando / Entrypoint">
                  <TextInput className="font-mono" value={current.command} onChange={(e) => patch({ command: e.target.value })} placeholder="npm run dev" />
                </Field>
                <Field label="Argumentos">
                  <TextInput className="font-mono" value={current.args} onChange={(e) => patch({ args: e.target.value })} placeholder="--port 3000" />
                </Field>
              </div>
              <EnvEditor rows={current.env} onRows={(env) => patch({ env })} envFile={current.envFile} onEnvFile={(envFile) => patch({ envFile })} />
              <div className="grid grid-cols-2 items-end gap-3">
                <Field label="Puerto esperado">
                  <TextInput
                    className="font-mono"
                    inputMode="numeric"
                    value={current.port}
                    onChange={(e) => patch({ port: e.target.value.replace(/[^0-9]/g, '') })}
                    placeholder="3000"
                  />
                </Field>
                <label className="flex h-7 cursor-pointer items-center gap-2 text-ui-lg text-fg">
                  <input
                    type="checkbox"
                    className="accent-accent"
                    checked={current.showInServices}
                    onChange={(e) => patch({ showInServices: e.target.checked })}
                  />
                  Mostrar en panel Services
                </label>
              </div>
            </>
          ) : (
            <p className="py-10 text-center text-ui text-muted">Selecciona una configuración o crea una nueva.</p>
          )}
          {error && <p className="text-ui text-stop-text">{error}</p>}
          <div className="flex items-center justify-between gap-2 pt-1">
            <span className="text-[11px] text-muted">Los cambios se aplican al pulsar Guardar.</span>
            <div className="flex gap-2">
              <Button onClick={closeEditor}>Cancelar</Button>
              <Button type="submit" variant="primary" disabled={saving}>
                Guardar
              </Button>
            </div>
          </div>
        </form>
      </div>
      {menu && <ContextMenu x={menu.x} y={menu.y} items={templateItems} onClose={() => setMenu(null)} />}
    </Modal>
  );
}
