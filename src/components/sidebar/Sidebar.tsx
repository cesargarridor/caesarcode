import { ChevronsDownUp, FilePlus, Files, FolderOpen, FolderPlus, PackagePlus, Search, Settings } from 'lucide-react';
import { FileTree } from './FileTree';
import { GlobalSearch } from '../palette/GlobalSearch';
import { Button } from '../ui/Button';
import { IconButton } from '../ui/IconButton';
import { addRootDialog, openFolderDialog, useWorkspace } from '../../store/workspace';
import { useUi } from '../../store/ui';

export function Sidebar() {
  const root = useWorkspace((s) => s.root);
  const name = useWorkspace((s) => s.name);
  const roots = useWorkspace((s) => s.roots);
  const collapseAll = useWorkspace((s) => s.collapseAll);
  const startInlineEdit = useWorkspace((s) => s.startInlineEdit);
  const view = useUi((s) => s.sidebarView);
  const setView = useUi((s) => s.setSidebarView);

  // Destino de "New ...": carpeta seleccionada, o la carpeta del archivo seleccionado, o la raíz.
  const newIn = (type: 'new-file' | 'new-dir') => {
    const { root: r, roots, selected, children } = useWorkspace.getState();
    if (!r) return;
    let parent = r;
    if (selected && roots.some((x) => x.path === selected && x.exists)) {
      parent = selected;
    } else if (selected) {
      for (const [dir, list] of Object.entries(children)) {
        const e = list.find((x) => x.path === selected);
        if (e) {
          parent = e.isDir ? e.path : dir;
          break;
        }
      }
    }
    startInlineEdit({ type, parent });
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-9 shrink-0 items-center gap-1 px-2">
        <IconButton label="Archivos" active={view === 'files'} onClick={() => setView('files')}>
          <Files size={14} />
        </IconButton>
        <IconButton label="Buscar" active={view === 'search'} onClick={() => setView('search')}>
          <Search size={14} />
        </IconButton>
        <div className="flex-1" />
        <IconButton label="Ajustes" onClick={() => useUi.getState().openModal('settings')}>
          <Settings size={14} />
        </IconButton>
        <IconButton label="Abrir carpeta" onClick={() => void openFolderDialog()}>
          <FolderOpen size={14} />
        </IconButton>
      </div>

      {!root ? (
        <div className="flex flex-1 flex-col gap-3 p-3">
          <Button variant="primary" onClick={() => void openFolderDialog()}>
            <FolderOpen size={13} />
            Open Folder
          </Button>
          <div className="text-ui text-muted">Ninguna carpeta abierta.</div>
        </div>
      ) : (
        <>
          <div className={view === 'files' ? 'flex min-h-0 flex-1 flex-col' : 'hidden'}>
            <div className="flex h-8 shrink-0 items-center justify-between px-3">
              <span className="truncate text-[11px] font-medium uppercase tracking-wider text-muted" title={root}>
                {roots.length > 1 ? `${name} · ${roots.length} proyectos` : name}
              </span>
              <div className="flex shrink-0 items-center gap-0.5">
                <IconButton label="Añadir proyecto al workspace" onClick={() => void addRootDialog()}>
                  <PackagePlus size={13} />
                </IconButton>
                <IconButton label="Nuevo archivo" onClick={() => newIn('new-file')}>
                  <FilePlus size={13} />
                </IconButton>
                <IconButton label="Nueva carpeta" onClick={() => newIn('new-dir')}>
                  <FolderPlus size={13} />
                </IconButton>
                <IconButton label="Contraer todo" onClick={collapseAll}>
                  <ChevronsDownUp size={13} />
                </IconButton>
              </div>
            </div>
            <FileTree />
          </div>
          <div className={view === 'search' ? 'flex min-h-0 flex-1 flex-col' : 'hidden'}>
            <GlobalSearch />
          </div>
        </>
      )}
    </div>
  );
}
