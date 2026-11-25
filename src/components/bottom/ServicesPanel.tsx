import { useMemo, useState } from 'react';
import { Panel, PanelGroup } from 'react-resizable-panels';
import { ChevronDown, ChevronRight, Play, Plus, Square, Wand2 } from 'lucide-react';
import { ResizeHandle } from '../layout/ResizeHandle';
import { IconButton } from '../ui/IconButton';
import { LogViewer } from './LogViewer';
import { ServiceModal } from './ServiceModal';
import { ServiceRow } from './ServiceRow';
import { TechIcon } from '../run/TechIcon';
import { useServices } from '../../store/services';
import { ephemeralInfo, runConfigRef, useRunConfigStore } from '../../store/runConfigs';
import { TECH_LABEL, TECH_ORDER, techOf } from '../../lib/tech';
import { useUi } from '../../store/ui';
import { useWorkspace } from '../../store/workspace';
import { FolderIcon } from '../icons/SymbolIcon';
import type { ServiceConfig, Tech } from '../../lib/types';

interface Entry {
  config: ServiceConfig;
  ephemeral: boolean;
}

interface RootGroup {
  root: string;
  name: string;
  techs: { tech: Tech; entries: Entry[] }[];
  count: number;
}

type ModalState = null | { mode: 'add' } | { mode: 'detect' } | { mode: 'edit'; config: ServiceConfig };

export function ServicesPanel() {
  const root = useWorkspace((s) => s.root);
  const roots = useWorkspace((s) => s.roots);
  const configs = useServices((s) => s.configs);
  const statuses = useServices((s) => s.statuses);
  const selectedId = useServices((s) => s.selectedId);
  const svc = useServices.getState;
  const [modal, setModal] = useState<ModalState>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const runFiles = useRunConfigStore((s) => s.files);
  const runDetected = useRunConfigStore((s) => s.detected);
  const runLaunched = useRunConfigStore((s) => s.launched);
  const openEditor = useRunConfigStore((s) => s.openEditor);

  // Servicios persistidos + runs efímeros con "Mostrar en Services", agrupados por raíz y luego por tecnología.
  const groups = useMemo<RootGroup[]>(() => {
    const byRoot = new Map<string, Map<Tech, Entry[]>>();
    const push = (rt: string, t: Tech, e: Entry) => {
      const m = byRoot.get(rt) ?? new Map<Tech, Entry[]>();
      m.set(t, [...(m.get(t) ?? []), e]);
      byRoot.set(rt, m);
    };
    for (const c of configs) push(c.root ?? root ?? '', techOf(c), { config: c, ephemeral: false });
    for (const st of Object.values(statuses)) {
      if (!st.ephemeral) continue;
      const info = ephemeralInfo(st.id, { files: runFiles, detected: runDetected, launched: runLaunched });
      if (!info.showInServices) continue;
      const config: ServiceConfig = info.config ?? {
        id: st.id, name: st.name || st.id, command: '', cwd: '', env: {}, envFile: null, port: st.port, kind: 'custom',
      };
      push(st.root || config.root || root || '', info.tech ?? techOf(config), { config, ephemeral: true });
    }
    const out: RootGroup[] = [];
    for (const r of roots) {
      const m = byRoot.get(r.path);
      if (!m) continue;
      const techs = TECH_ORDER.filter((t) => m.has(t)).map((t) => ({ tech: t, entries: m.get(t) as Entry[] }));
      out.push({ root: r.path, name: r.name, techs, count: techs.reduce((n, t) => n + t.entries.length, 0) });
    }
    return out;
  }, [configs, statuses, runFiles, runDetected, runLaunched, roots, root]);
  const multi = roots.length > 1;
  const toggle = (key: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });

  if (!root) {
    return <div className="flex h-full items-center justify-center text-ui text-muted">Abre una carpeta</div>;
  }

  const remove = async (c: ServiceConfig) => {
    const choice = await useUi.getState().ask('Eliminar servicio', `¿Eliminar "${c.name}"? Si está en marcha se detendrá.`, [
      { label: 'Cancelar', value: 'cancel' },
      { label: 'Eliminar', value: 'delete', variant: 'danger' },
    ]);
    if (choice === 'delete') await svc().save(svc().configs.filter((x) => x.id !== c.id));
  };

  return (
    <div className="h-full">
      <PanelGroup direction="horizontal" autoSaveId="cc-services">
        <Panel id="svc-list" order={1} defaultSize={38} minSize={22}>
          <div className="flex h-full flex-col">
            <div className="flex h-9 shrink-0 items-center gap-0.5 px-2">
              <IconButton label="Start All" onClick={() => void svc().startAll()} disabled={configs.length === 0}>
                <Play size={14} />
              </IconButton>
              <IconButton label="Stop All" onClick={() => void svc().stopAll()} disabled={configs.length === 0}>
                <Square size={14} />
              </IconButton>
              <span className="mx-1 h-4 w-px bg-border" />
              <IconButton label="Añadir servicio" onClick={() => setModal({ mode: 'add' })}>
                <Plus size={14} />
              </IconButton>
              <IconButton label="Detectar servicios" onClick={() => setModal({ mode: 'detect' })}>
                <Wand2 size={14} />
              </IconButton>
            </div>
            <div role="listbox" aria-label="Servicios" className="min-h-0 flex-1 overflow-auto py-1.5">
              <div
                role="option"
                aria-selected={selectedId === 'all'}
                onClick={() => svc().select('all')}
                className={`mx-2 mb-1.5 cursor-pointer rounded-lg px-2.5 py-1.5 text-ui-lg ${
                  selectedId === 'all' ? 'bg-ink/[0.06] text-fg' : 'text-fg hover:bg-ink/[0.04]'
                }`}
              >
                All Services
              </div>
              {groups.map((rg) => {
                const rootKey = `r:${rg.root}`;
                const rootCollapsed = multi && collapsed.has(rootKey);
                return (
                  <div key={rg.root}>
                    {multi && (
                      <button
                        type="button"
                        aria-expanded={!rootCollapsed}
                        onClick={() => toggle(rootKey)}
                        title={rg.root}
                        className="flex w-full items-center gap-1.5 px-3 py-1 text-left text-ui font-medium text-fg outline-none hover:bg-hover focus-visible:ring-1 focus-visible:ring-accent"
                      >
                        {rootCollapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
                        <FolderIcon name={rg.name} isModule />
                        <span className="truncate">{rg.name}</span>
                        <span className="ml-auto font-mono text-[11px] text-muted">{rg.count}</span>
                      </button>
                    )}
                    {!rootCollapsed &&
                      rg.techs.map((g) => {
                        const key = `${rg.root}|${g.tech}`;
                        const isCollapsed = collapsed.has(key);
                        return (
                          <div key={key}>
                            <button
                              type="button"
                              aria-expanded={!isCollapsed}
                              onClick={() => toggle(key)}
                              className={`flex w-full items-center gap-1.5 py-1 text-left text-[11px] uppercase tracking-wider text-muted outline-none hover:bg-hover focus-visible:ring-1 focus-visible:ring-accent ${multi ? 'pl-6 pr-3' : 'px-3'}`}
                            >
                              {isCollapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
                              <TechIcon tech={g.tech} className="h-3.5 w-3.5" />
                              <span>{TECH_LABEL[g.tech]}</span>
                              <span className="ml-auto font-mono">{g.entries.length}</span>
                            </button>
                            {!isCollapsed &&
                              g.entries.map(({ config: c, ephemeral }) => (
                                <ServiceRow
                                  key={c.id}
                                  config={c}
                                  status={statuses[c.id]}
                                  selected={selectedId === c.id}
                                  onSelect={() => svc().select(c.id)}
                                  onStart={() => void svc().start(c.id)}
                                  onStop={() => void svc().stop(c.id)}
                                  onRestart={() => void svc().restart(c.id)}
                                  onEdit={
                                    !ephemeral
                                      ? () => setModal({ mode: 'edit', config: c })
                                      : c.id.startsWith('run:')
                                        ? () => openEditor(runConfigRef(c.id, runFiles))
                                        : undefined
                                  }
                                  onDelete={!ephemeral ? () => void remove(c) : undefined}
                                  onKillPort={(p) => void svc().killPort(p)}
                                />
                              ))}
                          </div>
                        );
                      })}
                  </div>
                );
              })}
              {groups.length === 0 && (
                <div className="px-3 py-4 text-ui text-muted">
                  No hay servicios. Añade uno con <b className="text-fg">+</b> o pulsa la varita para detectarlos.
                </div>
              )}
            </div>
          </div>
        </Panel>
        <ResizeHandle direction="horizontal" />
        <Panel id="svc-logs" order={2} defaultSize={62} minSize={25}>
          <LogViewer key={selectedId === 'all' ? 'all' : selectedId} serviceId={selectedId} />
        </Panel>
      </PanelGroup>
      {modal && <ServiceModal state={modal} onClose={() => setModal(null)} />}
    </div>
  );
}
