import { useUi } from '../../store/ui';
import { useGit } from '../../store/git';
import { useEditor } from '../../store/editor';
import { copyText } from '../../lib/clipboard';
import { toMessage } from '../../lib/errors';
import { absPath, shortSha } from './gitUtils';
import type { BranchRef, CommitRow, StashRef, TagRef } from '../../lib/types';
import type { MenuItem } from '../ui/ContextMenu';
import { openGitDialog } from './BranchDialogs';

const ask = (title: string, message: string, label: string, danger = true) =>
  useUi
    .getState()
    .ask(title, message, [
      { label: 'Cancelar', value: 'cancel' },
      { label, value: 'ok', variant: danger ? 'danger' : 'primary' },
    ])
    .then((v) => v === 'ok');

export const confirmAction = ask;

export async function discardWithConfirm(paths: string[], untracked: boolean): Promise<void> {
  const what = paths.length === 1 ? paths[0] : `${paths.length} archivos`;
  const ok = await ask(
    'Descartar cambios',
    untracked
      ? `${what} no está bajo control de versiones y se enviará a la papelera.`
      : `Se perderán los cambios sin confirmar de ${what}. No se puede deshacer.`,
    'Descartar',
  );
  if (ok) await useGit.getState().discard(paths, untracked);
}

export function openInEditor(rel: string): void {
  const top = useGit.getState().info?.top;
  void useEditor.getState().openFile(absPath(top, rel));
  useUi.getState().setCenterView('editor');
}

const copy = (t: string) => copyText(t).catch((e) => useUi.getState().pushToast(toMessage(e)));

/** Menú contextual de commit (§4.2). */
export function commitMenu(row: CommitRow): MenuItem[] {
  const g = useGit.getState();
  const cur = g.info?.branch ?? null;
  const tips = row.refs.filter((r) => r.kind === 'local' || r.kind === 'head');
  const items: MenuItem[] = [
    { label: 'Checkout (detached)', onSelect: () => void g.checkoutCommit(row.sha) },
    { label: 'Create branch here…', onSelect: () => openGitDialog({ type: 'createBranch', start: row.sha }) },
    { label: 'Create tag here…', onSelect: () => openGitDialog({ type: 'createTag', sha: row.sha }) },
    { label: 'Cherry-pick', separator: true, onSelect: () => void g.cherryPick([row.sha]) },
    {
      label: 'Revert',
      onSelect: async () => {
        if (await ask('Revert', `Crear un commit que revierte ${shortSha(row.sha)}?`, 'Revert', false)) await g.revert(row.sha);
      },
    },
    { label: 'Reset current branch here ▸ Soft', separator: true, onSelect: () => void g.resetTo(row.sha, 'soft') },
    { label: 'Reset current branch here ▸ Mixed', onSelect: () => void g.resetTo(row.sha, 'mixed') },
    {
      label: 'Reset current branch here ▸ Hard',
      danger: true,
      onSelect: async () => {
        if (await ask('Reset hard', `Se descartarán TODOS los cambios sin confirmar y la rama ${cur ?? 'actual'} apuntará a ${shortSha(row.sha)}.`, 'Reset hard'))
          await g.resetTo(row.sha, 'hard');
      },
    },
  ];
  for (const t of tips) {
    if (t.name !== cur) {
      items.push({ label: `Merge ${t.name} into current`, separator: items.length > 0 && t === tips[0], onSelect: () => void g.merge(t.name) });
      items.push({ label: `Rebase current onto ${t.name}`, onSelect: () => void g.rebase(t.name) });
    }
  }
  items.push({ label: 'Copy SHA', separator: true, onSelect: () => void copy(row.sha) });
  items.push({ label: 'Copy message', onSelect: () => void copy(row.subject) });
  return items;
}

/** Menú de rama local o remota. */
export function branchMenu(b: BranchRef, remote: boolean): MenuItem[] {
  const g = useGit.getState();
  const info = g.info;
  const cur = info?.branch ?? null;
  const isCurrent = !remote && b.isHead;
  const items: MenuItem[] = [];
  items.push({
    label: 'Checkout',
    disabled: isCurrent,
    onSelect: () => void g.branch(remote ? { type: 'checkoutRemote', remoteBranch: b.name } : { type: 'checkout', name: b.name }),
  });
  if (!isCurrent) {
    items.push({ label: `Merge ${b.name} into ${cur ?? 'current'}`, onSelect: () => void g.merge(b.name) });
    items.push({ label: `Rebase ${cur ?? 'current'} onto ${b.name}`, onSelect: () => void g.rebase(b.name) });
  }
  if (!remote) {
    items.push({ label: 'Create branch from here…', separator: true, onSelect: () => openGitDialog({ type: 'createBranch', start: b.name }) });
    items.push({ label: 'Rename…', onSelect: () => openGitDialog({ type: 'renameBranch', from: b.name }) });
    items.push({ label: 'Set upstream…', onSelect: () => openGitDialog({ type: 'setUpstream', name: b.name }) });
    if (isCurrent) {
      items.push({ label: 'Pull', separator: true, onSelect: () => void g.remote({ type: 'pull', rebase: false }) });
    }
    items.push({
      label: 'Push',
      separator: !isCurrent,
      onSelect: () => void g.remote({ type: 'push', remote: null, branch: b.name, forceWithLease: false, setUpstream: !b.upstream, tags: false }),
    });
    items.push({
      label: 'Delete…',
      danger: true,
      separator: true,
      disabled: isCurrent,
      onSelect: async () => {
        const ok = await ask('Borrar rama', `Borrar la rama ${b.name}? Si no está fusionada se pedirá forzar.`, 'Borrar');
        if (!ok) return;
        const done = await g.branch({ type: 'delete', name: b.name, force: false });
        if (!done) {
          if (await ask('Forzar borrado', `${b.name} no está fusionada. Forzar el borrado perderá esos commits (siguen en el reflog).`, 'Forzar borrado'))
            await g.branch({ type: 'delete', name: b.name, force: true });
        }
      },
    });
  }
  items.push({ label: 'Copy name', separator: true, onSelect: () => void copy(b.name) });
  return items;
}

export function stashMenu(s: StashRef): MenuItem[] {
  const g = useGit.getState();
  return [
    { label: 'Apply', onSelect: () => void g.stash({ type: 'apply', index: s.index }) },
    { label: 'Pop', onSelect: () => void g.stash({ type: 'pop', index: s.index }) },
    {
      label: 'Drop…',
      danger: true,
      separator: true,
      onSelect: async () => {
        if (await ask('Borrar stash', `Se perderá stash@{${s.index}}: ${s.message}`, 'Drop')) await g.stash({ type: 'drop', index: s.index });
      },
    },
  ];
}

export function tagMenu(t: TagRef): MenuItem[] {
  const g = useGit.getState();
  const remote = g.refs?.remotes[0]?.name;
  return [
    { label: 'Checkout (detached)', onSelect: () => void g.checkoutCommit(t.sha) },
    { label: 'Push', disabled: !remote, onSelect: () => remote && void g.tag({ type: 'push', remote, name: t.name }) },
    {
      label: 'Delete…',
      danger: true,
      separator: true,
      onSelect: async () => {
        if (await ask('Borrar tag', `Borrar el tag local ${t.name}?`, 'Borrar')) await g.tag({ type: 'delete', name: t.name });
      },
    },
    { label: 'Copy name', separator: true, onSelect: () => void copy(t.name) },
  ];
}

export { copy as copyToClipboard };
