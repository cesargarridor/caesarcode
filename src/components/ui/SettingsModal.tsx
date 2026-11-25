import { useEffect, useState, type ReactNode } from 'react';
import { Check } from 'lucide-react';
import { Modal } from './Modal';
import { Button } from './Button';
import { Select } from './Select';
import { useUi } from '../../store/ui';
import { EDITOR_FONTS, UI_FONTS, useSettings, type EditorFont, type IconPack, type UiFont } from '../../store/settings';
import { THEMES } from '../../theme/themes';
import { useUpdater } from '../../store/updater';
import { appInfo } from '../../lib/ipc';

function Slider({ label, value, min, max, step = 1, unit = 'px', onChange }: {
  label: string; value: number; min: number; max: number; step?: number; unit?: string; onChange: (v: number) => void;
}) {
  return (
    <label className="flex items-center gap-3 text-ui-lg text-fg">
      <span className="w-40 shrink-0">{label}</span>
      <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} className="min-w-0 flex-1 accent-accent" />
      <span className="w-14 shrink-0 text-right font-mono text-[12px] text-muted">{value}{unit}</span>
    </label>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center gap-3 text-ui-lg text-fg">
      <span className="w-40 shrink-0">{label}</span>
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  );
}

const SECTIONS = [
  { id: 'appearance', label: 'Apariencia' },
  { id: 'typography', label: 'Tipografía' },
  { id: 'tree', label: 'Árbol de archivos' },
  { id: 'updates', label: 'Actualizaciones' },
] as const;
type SectionId = (typeof SECTIONS)[number]['id'];

const ICON_PACKS: { id: IconPack; label: string; hint: string }[] = [
  { id: 'intellij', label: 'IntelliJ', hint: 'Carpetas en contorno' },
  { id: 'material', label: 'Material (Atom)', hint: 'Iconos de colores' },
  { id: 'symbols', label: 'Symbols', hint: 'Estilo anterior' },
];

function Appearance() {
  const s = useSettings();
  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-col gap-2">
        <div className="text-[11px] font-medium uppercase tracking-wider text-muted">Tema</div>
        <div className="grid grid-cols-2 gap-1.5" role="radiogroup" aria-label="Tema">
          {THEMES.map((t) => {
            const on = s.theme === t.id;
            return (
              <button
                key={t.id}
                role="radio"
                aria-checked={on}
                onClick={() => s.set({ theme: t.id })}
                className={`flex items-center gap-2 rounded-md border px-2 py-1.5 text-left text-ui-lg ${
                  on ? 'border-accent/60 bg-accent/10 text-fg-strong' : 'border-border text-fg hover:bg-hover'
                }`}
              >
                <span className="flex shrink-0 overflow-hidden rounded border border-border-strong">
                  {[t.ui.canvas, t.ui.panel, t.ui.accent, t.syntax.keyword].map((c, i) => (
                    <span key={i} className="h-4 w-3.5" style={{ backgroundColor: c }} />
                  ))}
                </span>
                <span className="min-w-0 flex-1 truncate">{t.name}</span>
                {on && <Check size={13} className="shrink-0 text-accent" />}
              </button>
            );
          })}
        </div>
      </div>
      <div className="flex flex-col gap-2">
        <div className="text-[11px] font-medium uppercase tracking-wider text-muted">Pack de iconos</div>
        <div className="grid grid-cols-3 gap-1.5" role="radiogroup" aria-label="Pack de iconos">
          {ICON_PACKS.map((p) => {
            const on = s.iconPack === p.id;
            return (
              <button
                key={p.id}
                role="radio"
                aria-checked={on}
                onClick={() => s.set({ iconPack: p.id })}
                className={`flex flex-col rounded-md border px-2 py-1.5 text-left ${
                  on ? 'border-accent/60 bg-accent/10' : 'border-border hover:bg-hover'
                }`}
              >
                <span className="text-ui-lg text-fg-strong">{p.label}</span>
                <span className="text-[11px] text-muted">{p.hint}</span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function Typography() {
  const s = useSettings();
  const uiOpts = (Object.keys(UI_FONTS) as UiFont[]).map((v) => ({ value: v, label: UI_FONTS[v].label }));
  const edOpts = (Object.keys(EDITOR_FONTS) as EditorFont[]).map((v) => ({ value: v, label: EDITOR_FONTS[v].label }));
  return (
    <div className="flex flex-col gap-4">
      <div className="text-[11px] font-medium uppercase tracking-wider text-muted">Interfaz</div>
      <Row label="Fuente"><Select value={s.uiFont} options={uiOpts} onChange={(v) => s.set({ uiFont: v })} ariaLabel="Fuente de la interfaz" /></Row>
      <Slider label="Tamaño" value={s.uiSize} min={12} max={15} step={0.5} onChange={(v) => s.set({ uiSize: v })} />
      <div className="text-[11px] font-medium uppercase tracking-wider text-muted">Editor</div>
      <Row label="Fuente"><Select value={s.editorFont} options={edOpts} onChange={(v) => s.set({ editorFont: v })} ariaLabel="Fuente del editor" /></Row>
      <Slider label="Tamaño" value={s.editorSize} min={11} max={20} step={0.5} onChange={(v) => s.set({ editorSize: v })} />
      <Slider label="Interlineado" value={s.editorLineHeight} min={1.3} max={2} step={0.05} unit="" onChange={(v) => s.set({ editorLineHeight: Math.round(v * 100) / 100 })} />
      <label className="flex items-center gap-3 text-ui-lg text-fg">
        <span className="w-40 shrink-0">Ligaduras</span>
        <input type="checkbox" checked={s.ligatures} onChange={(e) => s.set({ ligatures: e.target.checked })} className="accent-accent" />
        <span className="text-muted">{s.ligatures ? 'Activadas' : 'Desactivadas'}</span>
      </label>
      <pre
        className="m-0 select-text rounded-md border border-border bg-panel px-3 py-2 text-fg"
        style={{ fontFamily: 'var(--cc-editor-font)', fontSize: 'var(--cc-editor-size)', lineHeight: 'var(--cc-editor-lh)', fontVariantLigatures: 'var(--cc-ligatures)' }}
      >{'@GetMapping("/items")\nif (a != b && x >= 0) return -> ok;'}</pre>
    </div>
  );
}

function Tree() {
  const s = useSettings();
  return (
    <div className="flex flex-col gap-4">
      <Slider label="Alto de fila" value={s.treeRow} min={24} max={44} onChange={(v) => s.set({ treeRow: v })} />
      <Slider label="Tamaño de iconos" value={s.treeIcon} min={14} max={28} onChange={(v) => s.set({ treeIcon: v })} />
      <Slider label="Tamaño de letra" value={s.treeFont} min={11} max={18} step={0.5} onChange={(v) => s.set({ treeFont: v })} />
    </div>
  );
}

function Updates() {
  const status = useUpdater((u) => u.status);
  const update = useUpdater((u) => u.update);
  const error = useUpdater((u) => u.error);
  const [version, setVersion] = useState('');
  useEffect(() => {
    appInfo().then((i) => setVersion(i.version)).catch(() => {});
  }, []);
  const busy = status === 'checking' || status === 'installing';
  const msg =
    status === 'checking' ? 'Buscando…'
    : status === 'uptodate' ? 'Estás al día.'
    : status === 'available' ? `Nueva versión ${update?.version} disponible.`
    : status === 'installing' ? 'Descargando e instalando…'
    : status === 'error' ? `No se pudo comprobar: ${error}`
    : '';
  return (
    <div className="flex flex-col gap-4">
      <Row label="Versión actual"><span className="font-mono text-[12px] text-muted">{version || '—'}</span></Row>
      <div className="flex items-center gap-2">
        <Button disabled={busy} onClick={() => void useUpdater.getState().check()}>Buscar actualizaciones</Button>
        {status === 'available' && (
          <Button variant="primary" disabled={busy} onClick={() => void useUpdater.getState().install()}>Actualizar y reiniciar</Button>
        )}
      </div>
      {msg && <div className={`text-ui-lg ${status === 'error' ? 'text-stop-text' : 'text-muted'}`}>{msg}</div>}
    </div>
  );
}

/** Ajustes por secciones: Apariencia · Tipografía · Árbol de archivos. */
export function SettingsModal() {
  const s = useSettings();
  const [section, setSection] = useState<SectionId>('appearance');
  return (
    <Modal title="Ajustes" onClose={() => useUi.getState().closeModal('settings')} width={560}>
      <div className="flex flex-col gap-4 p-1">
        <div className="flex gap-1 border-b border-border pb-2" role="tablist">
          {SECTIONS.map((x) => (
            <button
              key={x.id}
              role="tab"
              aria-selected={section === x.id}
              onClick={() => setSection(x.id)}
              className={`rounded-md px-2.5 py-1 text-ui-lg ${section === x.id ? 'bg-hover-strong text-fg-strong' : 'text-muted hover:bg-hover hover:text-fg'}`}
            >
              {x.label}
            </button>
          ))}
        </div>
        {section === 'appearance' && <Appearance />}
        {section === 'typography' && <Typography />}
        {section === 'tree' && <Tree />}
        {section === 'updates' && <Updates />}
        {section !== 'updates' && <div className="flex justify-end"><Button onClick={s.reset}>Restablecer</Button></div>}
      </div>
    </Modal>
  );
}
