import { useCallback, useMemo, useState } from 'react';
import { RootNode } from './RootNode';
import { ContextMenu, type MenuItem } from '../ui/ContextMenu';
import { useWorkspace } from '../../store/workspace';
import { useUi } from '../../store/ui';
import { copyText } from '../../lib/clipboard';
import { useGit } from '../../store/git';
import { useEditor } from '../../store/editor';
import { toggleBlame } from '../editor/blame';
import { discardWithConfirm } from '../git/gitActions';
import { toMessage } from '../../lib/errors';
import { basename, dirname, relativeTo, rootFor } from '../../lib/paths';
import type { FsEntry } from '../../lib/types';

interface MenuState { x: number; y: number; entry: FsEntry | null }

/** Directorios que contienen (a cualquier profundidad) algún archivo con estado git, hasta la raíz dueña. */
function useDirtyDirs(roots: readonly { path: string }[], files: Record<string, unknown>): Set<string> {
  return useMemo(() => {
    const set = new Set<string>();
    for (const f of Object.keys(files)) {
      const rootLen = rootFor(roots, f)?.path.length;
      if (rootLen === undefined) continue;
      let p = dirname(f);
      while (p.length > rootLen && !set.has(p)) {
        set.add(p);
        p = dirname(p);
      }
    }
    return set;
  }, [roots, files]);
}

export function FileTree() {
  const root = useWorkspace((s) => s.root) ?? '';
  const roots = useWorkspace((s) => s.roots);
  const gitFiles = useWorkspace((s) => s.git.files);
  const selected = useWorkspace((s) => s.selected);
  const inlineEdit = useWorkspace((s) => s.inlineEdit);
  const startInlineEdit = useWorkspace((s) => s.startInlineEdit);
  const deleteEntry = useWorkspace((s) => s.deleteEntry);
  const pushToast = useUi((s) => s.pushToast);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const dirtyDirs = useDirtyDirs(roots, gitFiles);

  const onContext = useCallback((e: React.MouseEvent, entry: FsEntry) => {
    e.preventDefault();
    e.stopPropagation();
    useWorkspace.getState().select(entry.path);
    setMenu({ x: e.clientX, y: e.clientY, entry });
  }, []);

  const copy = (text: string) => copyText(text).catch((e) => pushToast(toMessage(e)));

  const items = useMemo<MenuItem[]>(() => {
    if (!menu) return [];
    const { entry } = menu;
    const entryRoot = entry ? rootFor(roots, entry.path)?.path ?? root : root;
    const target = entry ? (entry.isDir ? entry.path : dirname(entry.path)) : root;
    const list: MenuItem[] = [
      { label: 'New File', onSelect: () => startInlineEdit({ type: 'new-file', parent: target }) },
      { label: 'New Folder', onSelect: () => startInlineEdit({ type: 'new-dir', parent: target }) },
    ];
    if (entry) {
      list.push(
        { label: 'Rename', separator: true, onSelect: () => startInlineEdit({ type: 'rename', path: entry.path }) },
        { label: 'Delete', danger: true, onSelect: () => void deleteEntry(entry) },
      );
    }
    if (entry && !entry.isDir) {
      const st = gitFiles[entry.path];
      list.push(
        {
          label: 'Git: Show History',
          separator: true,
          onSelect: () => useGit.getState().showFileHistory(entry.path),
        },
        {
          label: 'Abrir en Git',
          onSelect: () => useGit.getState().openInGit(entry.path),
        },
        {
          label: 'Git: Annotate',
          onSelect: async () => {
            await useEditor.getState().openFile(entry.path);
            useUi.getState().setCenterView('editor');
            setTimeout(() => void toggleBlame(entry.path), 60);
          },
        },
      );
      if (st) {
        list.push({
          label: 'Git: Stage',
          onSelect: async () => {
            if (await useGit.getState().activateRepoFor(entry.path)) await useGit.getState().stage([entry.path]);
          },
        });
        list.push({ label: 'Git: Discard Changes…', danger: true, onSelect: () => void discardWithConfirm([entry.path], st === 'U') });
      }
    }
    const p = entry ? entry.path : root;
    list.push({ label: 'Copy Path', separator: true, onSelect: () => void copy(p) });
    if (entry) list.push({ label: 'Copy Relative Path', onSelect: () => void copy(relativeTo(entryRoot, entry.path)) });
    return list;
  }, [menu, root, roots, startInlineEdit, deleteEntry, gitFiles]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (!selected || inlineEdit || (e.target as HTMLElement).tagName === 'INPUT') return;
    const all = Object.values(useWorkspace.getState().children).flat();
    const found = all.find((x) => x.path === selected || x.compact?.path === selected);
    if (!found) return;
    // Cadena compactada: F2/Supr actúan sobre la carpeta hoja.
    const entry = found.compact ? { ...found, path: found.compact.path, name: basename(found.compact.path) } : found;
    if (e.key === 'F2') {
      e.preventDefault();
      startInlineEdit({ type: 'rename', path: entry.path });
    } else if (e.key === 'Delete' || (e.key === 'Backspace' && e.metaKey)) {
      e.preventDefault();
      void deleteEntry(entry);
    }
  };


  return (
    <>
      <div
        role="tree"
        tabIndex={0}
        onKeyDown={onKeyDown}
        onContextMenu={(e) => {
          e.preventDefault();
          setMenu({ x: e.clientX, y: e.clientY, entry: null });
        }}
        onClick={(e) => {
          if (e.target === e.currentTarget) useWorkspace.getState().select(null);
        }}
        className="min-h-0 flex-1 overflow-auto py-1 pb-6 outline-none"
      >
        {roots.map((r, i) => (
          <RootNode key={r.path} root={r} isPrimary={i === 0} dirtyDirs={dirtyDirs} onContext={onContext} />
        ))}
      </div>
      {menu && <ContextMenu x={menu.x} y={menu.y} items={items} onClose={() => setMenu(null)} />}
    </>
  );
}
