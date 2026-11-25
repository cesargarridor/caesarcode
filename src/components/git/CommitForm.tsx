import { useEffect, useRef, useState } from 'react';
import { useGit } from '../../store/git';
import { gitCommitDetail } from '../../lib/ipc';
import { Button } from '../ui/Button';
import { TextInput } from '../ui/TextInput';
import { toMessage } from '../../lib/errors';
import { useUi } from '../../store/ui';

function Toggle({ on, onChange, children }: { on: boolean; onChange: (v: boolean) => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      onClick={() => onChange(!on)}
      className="flex items-center gap-1.5 text-ui text-muted outline-none hover:text-fg"
    >
      <span className={`flex h-3.5 w-6 items-center rounded-full border px-0.5 transition-colors ${on ? 'border-accent bg-accent/30' : 'border-border-strong bg-elevated'}`}>
        <span className={`h-2.5 w-2.5 rounded-full transition-transform ${on ? 'translate-x-2.5 bg-accent' : 'bg-muted'}`} />
      </span>
      {children}
    </button>
  );
}

export function CommitForm() {
  const status = useGit((s) => s.status);
  const info = useGit((s) => s.info);
  const busy = useGit((s) => s.busy);
  const [summary, setSummary] = useState('');
  const [body, setBody] = useState('');
  const [amend, setAmend] = useState(false);
  const [signOff, setSignOff] = useState(false);
  const prefilled = useRef(false);
  const staged = status?.staged.length ?? 0;
  const canCommit = !busy && summary.trim().length > 0 && (staged > 0 || amend) && (status?.conflicted.length ?? 0) === 0;

  // Al activar Amend con el formulario vacío, precarga el mensaje del último commit.
  useEffect(() => {
    if (!amend) {
      if (prefilled.current) {
        setSummary('');
        setBody('');
        prefilled.current = false;
      }
      return;
    }
    if (summary || body || !info?.head) return;
    const repo = useGit.getState().activeRepo;
    if (!repo) return;
    let alive = true;
    gitCommitDetail(repo, info.head)
      .then((d) => {
        if (!alive) return;
        setSummary(d.row.subject);
        setBody(d.body.replace(d.row.subject, '').trim());
        prefilled.current = true;
      })
      .catch((e) => useUi.getState().pushToast(toMessage(e)));
    return () => { alive = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [amend]);

  const message = () => (body.trim() ? `${summary.trim()}\n\n${body.trim()}` : summary.trim());
  const submit = async (push: boolean) => {
    if (!canCommit) return;
    const ok = await useGit.getState().commit(message(), amend, signOff);
    if (!ok) return;
    setSummary('');
    setBody('');
    setAmend(false);
    prefilled.current = false;
    if (push) {
      const g = useGit.getState();
      const noUp = !g.info?.upstream;
      await g.remote({ type: 'push', remote: noUp ? g.refs?.remotes[0]?.name ?? null : null, branch: null, forceWithLease: amend, setUpstream: noUp, tags: false });
    }
  };
  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      void submit(false);
    }
  };

  return (
    <div className="shrink-0 p-2">
      <div className="relative">
        <TextInput
          value={summary}
          onChange={(e) => setSummary(e.target.value)}
          onKeyDown={onKey}
          placeholder="Resumen del commit"
          className="pr-9"
        />
        <span className={`pointer-events-none absolute right-2 top-1.5 font-mono text-[10.5px] ${summary.length > 72 ? 'text-stop-text' : 'text-muted'}`}>
          {72 - summary.length}
        </span>
      </div>
      <textarea
        value={body}
        onChange={(e) => setBody(e.target.value)}
        onKeyDown={onKey}
        placeholder="Descripción (opcional)"
        rows={3}
        spellCheck={false}
        className="mt-1.5 w-full resize-none rounded-lg border border-border-strong bg-elevated px-2.5 py-1.5 text-ui-lg text-fg outline-none placeholder:text-muted focus:border-accent/60"
      />
      <div className="mt-2 flex items-center justify-between">
        <div className="flex flex-col gap-1">
          <Toggle on={amend} onChange={setAmend}>Amend</Toggle>
          <Toggle on={signOff} onChange={setSignOff}>Sign-off</Toggle>
        </div>
        <div className="flex gap-1.5">
          <Button disabled={!canCommit} onClick={() => void submit(true)} title="Commit y push">Commit & Push</Button>
          <Button variant="primary" disabled={!canCommit} onClick={() => void submit(false)}>
            {amend ? 'Amend' : 'Commit'}
          </Button>
        </div>
      </div>
      <div className="mt-1.5 text-[10.5px] text-muted">
        {staged} archivo{staged === 1 ? '' : 's'} en stage{info?.branch ? ` · ${info.branch}` : info?.detached ? ' · HEAD desacoplado' : ''} · ⌘/Ctrl+Enter
      </div>
    </div>
  );
}
