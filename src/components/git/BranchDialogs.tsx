import { useEffect, useMemo, useState } from 'react';
import { create } from 'zustand';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { TextInput } from '../ui/TextInput';
import { Select } from '../ui/Select';
import { useGit } from '../../store/git';
import { branchNameError, shortSha } from './gitUtils';

export type GitDialog =
  | { type: 'createBranch'; start: string }
  | { type: 'renameBranch'; from: string }
  | { type: 'createTag'; sha: string }
  | { type: 'setUpstream'; name: string }
  | { type: 'stashPush' };

const NO_REMOTES: never[] = [];
const useDialog = create<{ dialog: GitDialog | null }>(() => ({ dialog: null }));
export const openGitDialog = (d: GitDialog) => useDialog.setState({ dialog: d });
const close = () => useDialog.setState({ dialog: null });

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="mb-3 block">
      <span className="mb-1 block text-ui text-muted">{label}</span>
      {children}
    </label>
  );
}

function Footer({ ok, onOk, label, error }: { ok: boolean; onOk: () => void; label: string; error?: string | null }) {
  return (
    <div className="mt-1 flex items-center justify-between gap-2">
      <span className="min-w-0 truncate text-ui text-stop-text">{error ?? ''}</span>
      <div className="flex shrink-0 gap-2">
        <Button onClick={close}>Cancelar</Button>
        <Button variant="primary" disabled={!ok} onClick={onOk}>{label}</Button>
      </div>
    </div>
  );
}

function NameDialog({
  title, initial, label, taken, submitLabel, onSubmit, extra,
}: {
  title: string; initial: string; label: string; taken: Set<string>; submitLabel: string;
  onSubmit: (name: string) => Promise<boolean>; extra?: React.ReactNode;
}) {
  const [name, setName] = useState(initial);
  const [busy, setBusy] = useState(false);
  const err = useMemo(() => (name === initial && initial ? null : branchNameError(name) ?? (taken.has(name) ? 'Ya existe' : null)), [name, initial, taken]);
  const ok = !busy && !!name && !err && name !== initial;
  const submit = async () => {
    if (!ok) return;
    setBusy(true);
    const done = await onSubmit(name);
    setBusy(false);
    if (done) close();
  };
  return (
    <Modal title={title} onClose={close} width={420}>
      <Row label={label}>
        <TextInput autoFocus value={name} onChange={(e) => setName(e.target.value.trim())} onKeyDown={(e) => e.key === 'Enter' && void submit()} />
      </Row>
      {extra}
      <Footer ok={ok} onOk={() => void submit()} label={submitLabel} error={err} />
    </Modal>
  );
}

function CreateBranch({ start }: { start: string }) {
  const refs = useGit((s) => s.refs);
  const taken = useMemo(() => new Set((refs?.local ?? []).map((b) => b.name)), [refs]);
  const [checkout, setCheckout] = useState(true);
  return (
    <NameDialog
      title="Nueva rama" initial="" label={`Nombre (desde ${start.length >= 40 ? shortSha(start) : start})`} taken={taken} submitLabel="Crear"
      onSubmit={(name) => useGit.getState().branch({ type: 'create', name, start, checkout })}
      extra={
        <label className="mb-3 flex cursor-pointer items-center gap-2 text-ui-lg text-fg">
          <input type="checkbox" checked={checkout} onChange={(e) => setCheckout(e.target.checked)} className="accent-accent" />
          Hacer checkout
        </label>
      }
    />
  );
}

function CreateTag({ sha }: { sha: string }) {
  const refs = useGit((s) => s.refs);
  const taken = useMemo(() => new Set((refs?.tags ?? []).map((t) => t.name)), [refs]);
  const [msg, setMsg] = useState('');
  return (
    <NameDialog
      title={`Nuevo tag en ${shortSha(sha)}`} initial="" label="Nombre del tag" taken={taken} submitLabel="Crear"
      onSubmit={(name) => useGit.getState().tag({ type: 'create', name, sha, message: msg.trim() || null })}
      extra={
        <Row label="Mensaje (opcional, crea tag anotado)">
          <TextInput value={msg} onChange={(e) => setMsg(e.target.value)} />
        </Row>
      }
    />
  );
}

function SetUpstream({ name }: { name: string }) {
  const remote = useGit((s) => s.refs?.remote ?? NO_REMOTES);
  const options = remote.filter((r) => !r.name.endsWith('/HEAD')).map((r) => ({ value: r.name, label: r.name }));
  const [up, setUp] = useState(options[0]?.value ?? '');
  useEffect(() => {
    if (!up && options[0]) setUp(options[0].value);
  }, [up, options]);
  return (
    <Modal title={`Upstream de ${name}`} onClose={close} width={420}>
      <Row label="Rama remota">
        {options.length > 0 ? <Select value={up} options={options} onChange={setUp} /> : <div className="text-ui text-muted">No hay ramas remotas.</div>}
      </Row>
      <Footer
        ok={!!up}
        label="Asignar"
        onOk={async () => {
          if (await useGit.getState().branch({ type: 'setUpstream', name, upstream: up })) close();
        }}
      />
    </Modal>
  );
}

function StashPush() {
  const [msg, setMsg] = useState('');
  const [untracked, setUntracked] = useState(true);
  return (
    <Modal title="Guardar en stash" onClose={close} width={420}>
      <Row label="Mensaje (opcional)">
        <TextInput autoFocus value={msg} onChange={(e) => setMsg(e.target.value)} />
      </Row>
      <label className="mb-3 flex cursor-pointer items-center gap-2 text-ui-lg text-fg">
        <input type="checkbox" checked={untracked} onChange={(e) => setUntracked(e.target.checked)} className="accent-accent" />
        Incluir archivos sin seguimiento
      </label>
      <Footer
        ok
        label="Stash"
        onOk={async () => {
          close();
          await useGit.getState().stash({ type: 'push', message: msg.trim() || null, includeUntracked: untracked, keepIndex: false });
        }}
      />
    </Modal>
  );
}

/** Renderiza el diálogo Git abierto con `openGitDialog`. */
export function GitDialogHost() {
  const d = useDialog((s) => s.dialog);
  if (!d) return null;
  switch (d.type) {
    case 'createBranch': return <CreateBranch start={d.start} />;
    case 'createTag': return <CreateTag sha={d.sha} />;
    case 'stashPush': return <StashPush />;
    case 'setUpstream': return <SetUpstream name={d.name} />;
    case 'renameBranch': {
      const from = d.from;
      return <RenameBranch from={from} />;
    }
  }
}

function RenameBranch({ from }: { from: string }) {
  const refs = useGit((s) => s.refs);
  const taken = useMemo(() => new Set((refs?.local ?? []).map((b) => b.name)), [refs]);
  return (
    <NameDialog
      title={`Renombrar ${from}`} initial={from} label="Nuevo nombre" taken={taken} submitLabel="Renombrar"
      onSubmit={(name) => useGit.getState().branch({ type: 'rename', from, to: name })}
    />
  );
}
