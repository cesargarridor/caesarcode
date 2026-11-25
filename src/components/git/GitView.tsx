import { useEffect } from 'react';
import { Panel, PanelGroup } from 'react-resizable-panels';
import { ResizeHandle } from '../layout/ResizeHandle';
import { Button } from '../ui/Button';
import { useGit } from '../../store/git';
import { useWorkspace } from '../../store/workspace';
import { RefsSidebar } from './RefsSidebar';
import { CommitGraph } from './CommitGraph';
import { ChangesPanel } from './ChangesPanel';
import { CommitForm } from './CommitForm';
import { CommitDetail } from './CommitDetail';
import { DiffViewer } from './DiffViewer';
import { ConflictBanner } from './ConflictBanner';
import { RemoteBar } from './RemoteBar';
import { FileHistory } from './FileHistory';
import { GitDialogHost } from './BranchDialogs';
import { RepoTabs } from './RepoTabs';
import { AllReposView } from './AllReposView';

/** Vista Git estilo GitKraken. Solo se monta con `centerView === 'git'`. */
export function GitView() {
  const root = useWorkspace((s) => s.root);
  const roots = useWorkspace((s) => s.roots);
  const repos = useGit((s) => s.repos);
  const reposLoaded = useGit((s) => s.reposLoaded);
  const view = useGit((s) => s.view);
  const activeRepo = useGit((s) => s.activeRepo);
  const info = useGit((s) => s.info);
  const selected = useGit((s) => s.selected);
  const diff = useGit((s) => s.diff);
  const fileHistory = useGit((s) => s.fileHistory);
  const error = useGit((s) => s.error);

  useEffect(() => {
    const g = useGit.getState();
    g.setActive(1);
    if (g.reposLoaded) void g.refreshAll();
    else void g.loadRepos();
    return () => useGit.getState().setActive(-1);
  }, [root]);


  if (!root) return <div className="flex h-full items-center justify-center text-ui-lg text-muted">Abre una carpeta para usar Git.</div>;
  if (!reposLoaded) {
    return <div className="flex h-full items-center justify-center text-ui-lg text-muted">{error ?? 'Buscando repositorios…'}</div>;
  }
  if (repos.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3">
        <div className="text-ui-lg text-fg">No hay repositorios Git en el workspace.</div>
        <div className="flex flex-col items-stretch gap-1.5">
          {roots.filter((r) => r.exists).map((r) => (
            <Button key={r.path} variant="primary" onClick={() => void useGit.getState().initRepo(r.path)} title={r.path}>
              git init en {r.name}
            </Button>
          ))}
        </div>
      </div>
    );
  }
  if (view === 'all') {
    return (
      <div className="flex h-full min-h-0 flex-col">
        <div className="flex h-10 shrink-0 items-center px-3 min-w-0"><RepoTabs /></div>
        <div className="min-h-0 flex-1"><AllReposView /></div>
      </div>
    );
  }
  if (info && !info.isRepo) {
    return (
      <div className="flex h-full min-h-0 flex-col">
        <div className="flex h-10 shrink-0 items-center px-3 min-w-0"><RepoTabs /></div>
        <div className="flex flex-1 items-center justify-center text-ui-lg text-muted">Este repositorio ya no es válido. Elige otro.</div>
      </div>
    );
  }
  if (!info || !activeRepo) {
    return (
      <div className="flex h-full min-h-0 flex-col">
        <div className="flex h-10 shrink-0 items-center px-3 min-w-0"><RepoTabs /></div>
        <div className="flex flex-1 items-center justify-center text-ui-lg text-muted">{error ?? 'Cargando repositorio…'}</div>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex h-10 shrink-0 items-center px-3 min-w-0"><RepoTabs /></div>
      <RemoteBar />
      <ConflictBanner />
      <div className="min-h-0 flex-1">
        <PanelGroup direction="horizontal" autoSaveId="cc-git">
          <Panel id="git-refs" order={1} defaultSize={18} minSize={10} collapsible>
            <RefsSidebar />
          </Panel>
          <ResizeHandle direction="horizontal" />
          <Panel id="git-center" order={2} defaultSize={52} minSize={25}>
            {/* El grafo se oculta (no se desmonta) al abrir un diff: conserva el scroll. */}
            <div className={diff ? 'hidden' : 'h-full'}>
              {fileHistory ? <FileHistory path={fileHistory} /> : <CommitGraph />}
            </div>
            {diff && <DiffViewer />}
          </Panel>
          <ResizeHandle direction="horizontal" />
          <Panel id="git-side" order={3} defaultSize={30} minSize={18}>
            <div className="flex h-full min-h-0 flex-col">
              {selected?.type === 'commit' ? (
                <CommitDetail />
              ) : (
                <>
                  <ChangesPanel />
                  <CommitForm />
                </>
              )}
            </div>
          </Panel>
        </PanelGroup>
      </div>
      <GitDialogHost />
    </div>
  );
}
