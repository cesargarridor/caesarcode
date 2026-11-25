import { Panel, PanelGroup } from 'react-resizable-panels';
import { ResizeHandle } from './ResizeHandle';
import { StatusBar } from './StatusBar';
import { panelRefs } from './panelRefs';
import { Sidebar } from '../sidebar/Sidebar';
import { EditorArea } from '../editor/EditorArea';
import { GitView } from '../git/GitView';
import { SettingsModal } from '../ui/SettingsModal';
import { ErrorBoundary } from '../ui/ErrorBoundary';
import { useUi } from '../../store/ui';
import { BottomPanel } from '../bottom/BottomPanel';
import { AppHeader } from './AppHeader';
import { Island } from './Island';
import { RunConfigModal } from '../run/RunConfigModal';
import { useRunConfigStore } from '../../store/runConfigs';
import { AgentsPanel } from '../agents/AgentsPanel';

export function AppLayout() {
  const editorOpen = useRunConfigStore((s) => s.editor.open);
  const centerView = useUi((s) => s.centerView);
  const settingsOpen = useUi((s) => !!s.modals.settings);
  return (
    <div className="flex h-full flex-col bg-canvas">
      <AppHeader />
      <div className="min-h-0 flex-1 px-1.5">
        <PanelGroup direction="horizontal" autoSaveId="cc-main">
          <Panel ref={(r) => { panelRefs.sidebar = r; }} id="sidebar" order={1} defaultSize={20} minSize={12} collapsible>
            <Island>
              <Sidebar />
            </Island>
          </Panel>
          <ResizeHandle direction="horizontal" />
          <Panel id="center" order={2} defaultSize={55} minSize={30}>
            <PanelGroup direction="vertical" autoSaveId="cc-center">
              <Panel id="editor" order={1} defaultSize={70} minSize={20}>
                {/* El editor sigue montado (estado CM intacto) y solo se oculta en la vista Git. */}
                <Island>
                  <div className={centerView === 'git' ? 'hidden' : 'h-full'}>
                    <EditorArea />
                  </div>
                  {centerView === 'git' && <ErrorBoundary label="Error en la vista Git"><GitView /></ErrorBoundary>}
                </Island>
              </Panel>
              <ResizeHandle direction="vertical" />
              <Panel id="bottom" order={2} defaultSize={30} minSize={10} collapsible>
                <Island>
                  <BottomPanel />
                </Island>
              </Panel>
            </PanelGroup>
          </Panel>
          <ResizeHandle direction="horizontal" />
          <Panel ref={(r) => { panelRefs.agents = r; }} id="agents" order={3} defaultSize={25} minSize={15} collapsible>
            <Island>
              <ErrorBoundary label="Error en el panel de agentes"><AgentsPanel /></ErrorBoundary>
            </Island>
          </Panel>
        </PanelGroup>
      </div>
      <StatusBar />
      {editorOpen && <RunConfigModal />}
      {settingsOpen && <SettingsModal />}
    </div>
  );
}
