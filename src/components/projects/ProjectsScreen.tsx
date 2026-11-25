import { useEffect, useState } from 'react';
import { ArrowLeft, FolderOpen, PackagePlus, Plus, Search } from 'lucide-react';
import { BrandMark } from '../layout/BrandMark';
import { Button } from '../ui/Button';
import { TextInput } from '../ui/TextInput';
import { RecentList } from './RecentList';
import { useProjects } from '../../store/projects';
import { addRootDialog, useWorkspace } from '../../store/workspace';
import { isMac, TRAFFIC_LIGHTS_PAD } from '../../lib/platform';

/** Pantalla Projects: recientes (buscar, abrir, fijar, quitar), "Open…" y "New Project". */
export function ProjectsScreen() {
  const recent = useProjects((s) => s.recent);
  const loaded = useProjects((s) => s.recentLoaded);
  const hasWorkspace = useWorkspace((s) => s.root !== null);
  const [query, setQuery] = useState('');

  useEffect(() => {
    void useProjects.getState().loadRecent();
  }, []);

  const st = useProjects.getState;
  return (
    <div className="flex h-full flex-col bg-canvas">
      <header
        data-tauri-drag-region
        style={isMac ? { paddingLeft: TRAFFIC_LIGHTS_PAD } : undefined}
        className={`flex h-11 shrink-0 items-center gap-2 bg-canvas pr-3 text-ui ${isMac ? '' : 'pl-3'}`}
      >
        <BrandMark size={16} />
        <span className="font-medium text-fg">Caesar<span className="text-crimson">Code</span></span>
        <span className="text-muted">/ Projects</span>
        {hasWorkspace && (
          <Button className="ml-auto !h-6" onClick={() => st().closeScreen()}>
            <ArrowLeft size={12} /> Volver al proyecto
          </Button>
        )}
      </header>
      <div className="mx-3 mb-3 flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-border bg-panel">
      <div className="mx-auto flex min-h-0 w-full max-w-3xl flex-1 flex-col px-6 py-8">
        <div className="mb-6 flex flex-col items-center text-center">
          <BrandMark size={72} className="mb-2" />
          <div className="font-mono text-xl font-medium tracking-tight text-fg">Caesar<span className="text-accent">Code</span></div>
          <div className="mt-1 text-ui text-muted">Editor nativo, ligero y local-first</div>
        </div>
        <div className="mb-3 flex items-center gap-2">
          <div className="relative flex-1">
            <Search size={13} className="pointer-events-none absolute left-2 top-2 text-muted" />
            <TextInput autoFocus className="!pl-7" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Buscar proyectos…" aria-label="Buscar proyectos" />
          </div>
          <Button variant="primary" onClick={() => st().openWizard()}>
            <Plus size={13} /> New Project
          </Button>
          <Button onClick={() => void st().openFolder()}>
            <FolderOpen size={13} /> Open…
          </Button>
          {hasWorkspace && (
            <Button
              onClick={() =>
                void addRootDialog().then((n) => {
                  if (n > 0) st().closeScreen();
                })
              }
              title="Añade carpetas como raíces del workspace abierto"
            >
              <PackagePlus size={13} /> Añadir carpeta al workspace…
            </Button>
          )}
        </div>
        <div className="min-h-0 flex-1 overflow-auto py-1">
          {loaded ? (
            <RecentList
              items={recent}
              query={query}
              onOpen={(p) => void st().openProject(p)}
              onRemove={(p) => void st().remove(p.path)}
              onPin={(p) => void st().pin(p.path, !p.pinned)}
              onAttach={hasWorkspace ? (p) => void st().attachProject(p) : undefined}
            />
          ) : (
            <div className="px-3 py-8 text-center text-ui-lg text-muted">Cargando…</div>
          )}
        </div>
      </div>
      </div>
    </div>
  );
}
