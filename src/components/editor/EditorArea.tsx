import { TabBar } from './TabBar';
import { Breadcrumbs } from './Breadcrumbs';
import { CodeEditor } from './CodeEditor';
import { WelcomeView } from '../layout/WelcomeView';
import { TabularViewer } from '../viewers/TabularViewer';
import { MediaViewer } from '../viewers/MediaViewer';
import { useEditor } from '../../store/editor';

function Notice({ title, text }: { title: string; text: string }) {
  return (
    <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-1 bg-base">
      <div className="text-ui-lg text-fg">{title}</div>
      <div className="font-mono text-[11px] text-muted">{text}</div>
    </div>
  );
}

export function EditorArea() {
  const tabs = useEditor((s) => s.tabs);
  const activeTab = useEditor((s) => s.tabs.find((t) => t.path === s.active) ?? null);

  return (
    <div className="flex h-full flex-col bg-base">
      {tabs.length > 0 && <TabBar />}
      {tabs.length > 0 && <Breadcrumbs />}
      <div className="relative min-h-0 flex-1">
        {/* El EditorView único siempre está montado; las capas lo tapan cuando no aplica. */}
        <CodeEditor />
        {!activeTab && (
          <div className="absolute inset-0 z-10">
            <WelcomeView />
          </div>
        )}
        {activeTab?.kind === 'binary' && <Notice title="Archivo binario" text="No se puede mostrar en el editor." />}
        {activeTab?.kind === 'tabular' && (
          <div className="absolute inset-0 z-10 bg-base">
            <TabularViewer key={activeTab.path} path={activeTab.path} />
          </div>
        )}
        {activeTab?.kind === 'media' && (
          <div className="absolute inset-0 z-10 bg-base">
            <MediaViewer key={activeTab.path} path={activeTab.path} />
          </div>
        )}
        {activeTab?.kind === 'tooLarge' && <Notice title="Archivo demasiado grande" text="El límite del editor es 10 MB." />}
      </div>
    </div>
  );
}
