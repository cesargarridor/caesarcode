import { memo, useEffect, useMemo } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { FileIcon, FolderIcon } from '../icons/SymbolIcon';
import { InlineNameInput } from './InlineNameInput';
import { useWorkspace } from '../../store/workspace';
import { useEditor } from '../../store/editor';
import { useSymbols } from '../../store/symbols';
import { symbolLabel } from '../../lib/fileIconRegistry';
import { basename } from '../../lib/paths';
import { classifyFolder } from '../../lib/classifyFolder';
import type { FsEntry, GitFileStatus } from '../../lib/types';

const INDENT = 12;
const STATUS_COLOR: Record<GitFileStatus, string> = {
  M: 'text-accent', U: 'text-ok', A: 'text-ok', D: 'text-danger', R: 'text-accent',
};

interface Props {
  entry: FsEntry;
  depth: number;
  dirtyDirs: Set<string>;
  onContext: (e: React.MouseEvent, entry: FsEntry) => void;
}

function NewEntryRow({ parent, dir, depth }: { parent: string; dir: boolean; depth: number }) {
  const createEntry = useWorkspace((s) => s.createEntry);
  const startInlineEdit = useWorkspace((s) => s.startInlineEdit);
  return (
    <div className="mx-1.5 flex h-[var(--tree-row)] items-center gap-1 rounded-md pr-2" style={{ paddingLeft: depth * INDENT + 8 }}>
      <span className="w-3 shrink-0" />
      {dir ? <FolderIcon name="" /> : <FileIcon filename="" />}
      <InlineNameInput
        placeholder={dir ? 'nombre-carpeta' : 'nombre-archivo'}
        onSubmit={(name) => void createEntry(parent, name, dir)}
        onCancel={() => startInlineEdit(null)}
      />
    </div>
  );
}

export const FileTreeNode = memo(function FileTreeNode({ entry, depth, dirtyDirs, onContext }: Props) {
  // Con cadena compactada (com.example.app) el nodo representa a la carpeta hoja:
  // listado, expansión, selección, badges y acciones usan su ruta (`key`).
  const key = entry.compact?.path ?? entry.path;
  const expanded = useWorkspace((s) => !!s.expanded[key]);
  const selected = useWorkspace((s) => s.selected === key);
  const kids = useWorkspace((s) => s.children[key]);
  const status = useWorkspace((s) => s.git.files[entry.path]);
  const renaming = useWorkspace((s) => s.inlineEdit?.type === 'rename' && s.inlineEdit.path === key);
  const creatingHere = useWorkspace((s) => {
    const e = s.inlineEdit;
    return e && e.type !== 'rename' && e.parent === key ? e.type : null;
  });
  const isRoot = useWorkspace((s) => s.root === entry.path);
  const active = useEditor((s) => s.active === entry.path);
  const symbol = useSymbols((s) => (entry.isDir ? undefined : s.symbols[entry.path]));
  const toggleDir = useWorkspace((s) => s.toggleDir);
  const select = useWorkspace((s) => s.select);
  const refreshDir = useWorkspace((s) => s.refreshDir);
  const renameEntry = useWorkspace((s) => s.renameEntry);
  const startInlineEdit = useWorkspace((s) => s.startInlineEdit);
  const openFile = useEditor((s) => s.openFile);

  // Carga perezosa: solo al expandir.
  useEffect(() => {
    if (entry.isDir && expanded && !kids) void refreshDir(key);
  }, [entry.isDir, key, expanded, kids, refreshDir]);

  // Las acciones (menú contextual, rename, delete) actúan sobre la hoja.
  const target = useMemo<FsEntry>(
    () => (entry.compact ? { ...entry, path: key, name: basename(key) } : entry),
    [entry, key],
  );

  const onClick = () => {
    select(key);
    if (entry.isDir) toggleDir(key);
    else void openFile(entry.path);
  };

  const folder = useMemo(
    () =>
      entry.isDir
        ? classifyFolder(entry.name, key, kids?.map((k) => k.name), { isRoot, module: entry.module, role: entry.role })
        : undefined,
    [entry.isDir, entry.name, key, entry.module, entry.role, kids, isRoot],
  );
  const nameColor = status ? STATUS_COLOR[status] : folder?.dimName ? 'text-[var(--cc-excluded-text)]' : selected ? 'text-fg-strong' : 'text-fg';
  const hasChanges = entry.isDir && dirtyDirs.has(key);
  const label = symbolLabel(symbol);
  const title = entry.compact
    ? `${key}\nRename solo renombra la última carpeta`
    : label
      ? `${label}\n${entry.path}`
      : entry.path;

  return (
    <>
      <div
        role="treeitem"
        aria-expanded={entry.isDir ? expanded : undefined}
        aria-selected={selected}
        data-path={key}
        onClick={onClick}
        onContextMenu={(e) => onContext(e, target)}
        style={{ paddingLeft: depth * INDENT + 8 }}
        className={`group relative mx-1.5 flex h-[var(--tree-row)] cursor-pointer items-center gap-1 rounded-md pr-2 text-[length:var(--tree-font)] ${
          selected
            ? 'bg-ink/[0.08] font-medium text-fg-strong before:absolute before:bottom-1.5 before:left-0.5 before:top-1.5 before:w-[2px] before:rounded-full before:bg-accent'
            : active
              ? 'bg-hover'
              : 'hover:bg-hover'
        }`}
      >
        <span className="flex w-3 shrink-0 justify-center text-muted">
          {entry.isDir ? expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} /> : null}
        </span>
        {entry.isDir ? (
          <FolderIcon
            name={entry.compact ? basename(key) : entry.name}
            isOpen={expanded}
            isModule={folder?.cls === 'module'}
            style={folder}
            className="h-[var(--tree-icon)] w-[var(--tree-icon)]"
          />
        ) : (
          <FileIcon filename={entry.name} symbol={symbol} className="h-[var(--tree-icon)] w-[var(--tree-icon)]" />
        )}
        {renaming ? (
          <InlineNameInput
            initial={basename(key)}
            onSubmit={(name) => void renameEntry(key, name)}
            onCancel={() => startInlineEdit(null)}
          />
        ) : (
          <>
            <span className={`min-w-0 flex-1 truncate font-sans ${nameColor}`} title={title}>
              {entry.compact?.name ?? entry.name}
            </span>
            {folder?.badge && (
              <span
                className="shrink-0 rounded border border-accent/40 px-1 font-mono text-[9px] leading-[13px] text-accent"
                title={`Módulo ${folder.badge}`}
              >
                {folder.badge}
              </span>
            )}
            {status && <span className={`shrink-0 font-mono text-[10px] ${STATUS_COLOR[status]}`}>{status}</span>}
            {hasChanges && <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent" title="Contiene cambios" />}
          </>
        )}
      </div>
      {entry.isDir && expanded && (
        // Una sola guía por contenedor de hijos (no por fila): alineada bajo el chevron de la carpeta.
        <div className="relative">
          <span
            aria-hidden
            className="pointer-events-none absolute bottom-0 top-0 border-l border-ink/[0.05]"
            style={{ left: depth * INDENT + 8 + 6 + 6 }}
          />
          {creatingHere && <NewEntryRow parent={key} dir={creatingHere === 'new-dir'} depth={depth + 1} />}
          {kids?.map((k) => (
            <FileTreeNode key={k.path} entry={k} depth={depth + 1} dirtyDirs={dirtyDirs} onContext={onContext} />
          ))}
        </div>
      )}
    </>
  );
});

export { NewEntryRow };
