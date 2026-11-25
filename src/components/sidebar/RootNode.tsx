import { memo, useEffect, useMemo, useState } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { FolderIcon } from '../icons/SymbolIcon';
import { ContextMenu, type MenuItem } from '../ui/ContextMenu';
import { FileTreeNode, NewEntryRow } from './FileTreeNode';
import { useWorkspace } from '../../store/workspace';
import { useGit } from '../../store/git';
import { useUi } from '../../store/ui';
import { copyText } from '../../lib/clipboard';
import { toMessage } from '../../lib/errors';
import { isSameOrInside, repoFor } from '../../lib/paths';
import type { FsEntry, RootInfo } from '../../lib/types';

interface Props {
  root: RootInfo;
  isPrimary: boolean;
  dirtyDirs: Set<string>;
  onContext: (e: React.MouseEvent, entry: FsEntry) => void;
}

/** Nodo de nivel superior de una raíz del workspace (estilo IntelliJ: cada proyecto adjunto es una raíz). */
export const RootNode = memo(function RootNode({ root, isPrimary, dirtyDirs, onContext }: Props) {
  const expanded = useWorkspace((s) => !!s.expanded[root.path]);
  const kids = useWorkspace((s) => s.children[root.path]);
  const selected = useWorkspace((s) => s.selected === root.path);
  const creatingHere = useWorkspace((s) => {
    const e = s.inlineEdit;
    return e && e.type !== 'rename' && e.parent === root.path ? e.type : null;
  });
  const gitByRepo = useWorkspace((s) => s.gitByRepo);
  const repos = useGit((s) => s.repos);
  const toggleDir = useWorkspace((s) => s.toggleDir);
  const select = useWorkspace((s) => s.select);
  const refreshDir = useWorkspace((s) => s.refreshDir);
  const startInlineEdit = useWorkspace((s) => s.startInlineEdit);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);

  // Carga perezosa: una raíz colapsada no pide `fs_read_dir`.
  useEffect(() => {
    if (root.exists && expanded && !kids) void refreshDir(root.path);
  }, [root.exists, root.path, expanded, kids, refreshDir]);

  const ownRepo = useMemo(() => repoFor(repos, root.path), [repos, root.path]);
  const inside = useMemo(() => repos.filter((r) => r.root === root.path && isSameOrInside(root.path, r.top)), [repos, root.path]);
  const branch = ownRepo ? (gitByRepo[ownRepo.top]?.branch ?? null) : null;
  const changes = useMemo(() => {
    let n = 0;
    for (const st of Object.values(gitByRepo)) for (const f of Object.keys(st.files)) if (isSameOrInside(root.path, f)) n++;
    return n;
  }, [gitByRepo, root.path]);
  const subRepos = ownRepo ? 0 : inside.length;

  const items = useMemo<MenuItem[]>(
    () => [
      { label: 'New File', disabled: !root.exists, onSelect: () => startInlineEdit({ type: 'new-file', parent: root.path }) },
      { label: 'New Folder', disabled: !root.exists, onSelect: () => startInlineEdit({ type: 'new-dir', parent: root.path }) },
      {
        label: 'Abrir en Git',
        separator: true,
        disabled: !ownRepo && inside.length === 0,
        title: !ownRepo && inside.length === 0 ? 'Esta raíz no contiene ningún repositorio Git' : undefined,
        onSelect: () => useGit.getState().openInGit(root.path),
      },
      {
        label: 'Copiar ruta',
        onSelect: () => void copyText(root.path).catch((e) => useUi.getState().pushToast(toMessage(e))),
      },
      {
        label: 'Quitar del workspace',
        separator: true,
        danger: true,
        disabled: isPrimary,
        title: isPrimary ? 'La raíz principal no se puede quitar; abre otra carpeta' : undefined,
        onSelect: () => void useWorkspace.getState().removeRoot(root.path),
      },
    ],
    [root.path, root.exists, isPrimary, ownRepo, inside.length, startInlineEdit],
  );

  const tooltip = root.exists ? root.path : `${root.path}\nCarpeta no encontrada (disco desmontado o borrada)`;

  return (
    <>
      <div
        role="treeitem"
        aria-expanded={root.exists ? expanded : undefined}
        aria-selected={selected}
        data-path={root.path}
        title={tooltip}
        onClick={() => {
          select(root.path);
          if (root.exists) toggleDir(root.path);
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          select(root.path);
          setMenu({ x: e.clientX, y: e.clientY });
        }}
        className={`group relative mx-1.5 flex h-[var(--tree-row)] cursor-pointer items-center gap-1 rounded-md pl-2 pr-2 text-[length:var(--tree-font)] ${
          selected ? 'bg-ink/[0.08] text-fg-strong' : 'hover:bg-hover'
        } ${root.exists ? '' : 'opacity-50'}`}
      >
        <span className="flex w-3 shrink-0 justify-center text-muted">
          {root.exists ? expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} /> : null}
        </span>
        <FolderIcon name={root.name} isOpen={expanded} isModule className="h-[var(--tree-icon)] w-[var(--tree-icon)]" />
        <span className="min-w-0 truncate font-sans font-medium text-fg-strong">{root.name}</span>
        {!root.exists && <span className="shrink-0 text-[11px] text-faint">(no encontrado)</span>}
        {root.exists && branch && <span className="min-w-0 shrink truncate font-mono text-[11px] text-faint">{branch}</span>}
        {root.exists && subRepos > 0 && <span className="shrink-0 font-mono text-[11px] text-faint">{subRepos} repos</span>}
        <span className="flex-1" />
        {changes > 0 && (
          <span className="shrink-0 font-mono text-[10px] text-accent" title={`${changes} archivo(s) con cambios`}>
            ●{changes}
          </span>
        )}
      </div>
      {root.exists && expanded && (
        <div className="relative">
          {creatingHere && <NewEntryRow parent={root.path} dir={creatingHere === 'new-dir'} depth={1} />}
          {kids?.map((e) => (
            <FileTreeNode key={e.path} entry={e} depth={1} dirtyDirs={dirtyDirs} onContext={onContext} />
          ))}
          {kids && kids.length === 0 && <div className="px-6 py-1 text-ui text-muted">Carpeta vacía</div>}
        </div>
      )}
      {menu && <ContextMenu x={menu.x} y={menu.y} items={items} onClose={() => setMenu(null)} />}
    </>
  );
});
