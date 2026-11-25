import { create } from 'zustand';
import { applyTheme } from '../theme/applyTheme';
import { DEFAULT_THEME_ID, getTheme, type ThemeId } from '../theme/themes';
import { registry } from './editorRegistry';

export type IconPack = 'intellij' | 'material' | 'symbols';
export type UiFont = 'inter' | 'system';
export type EditorFont = 'jetbrains' | 'system-mono' | 'menlo';

export interface AppSettings {
  theme: ThemeId;
  iconPack: IconPack;
  uiFont: UiFont;
  /** Tamaño de fuente de la UI en px (12-15). */
  uiSize: number;
  editorFont: EditorFont;
  /** Tamaño de fuente del editor en px (11-20). */
  editorSize: number;
  /** Interlineado del editor (1.3-2.0). */
  editorLineHeight: number;
  ligatures: boolean;
  /** Alto de fila del árbol en px. */
  treeRow: number;
  /** Tamaño de iconos del árbol en px. */
  treeIcon: number;
  /** Tamaño de fuente del árbol en px. */
  treeFont: number;
}

const KEY = 'cc.settings.v2';
const KEY_V1 = 'cc.settings.v1';

export const DEFAULTS: AppSettings = {
  theme: DEFAULT_THEME_ID,
  iconPack: 'intellij',
  uiFont: 'inter',
  uiSize: 13,
  editorFont: 'jetbrains',
  editorSize: 13.5,
  editorLineHeight: 1.6,
  ligatures: true,
  treeRow: 30,
  treeIcon: 18,
  treeFont: 13,
};

export const UI_FONTS: Record<UiFont, { label: string; stack: string }> = {
  inter: { label: 'Inter', stack: '"Inter Variable", Inter, -apple-system, BlinkMacSystemFont, system-ui, sans-serif' },
  system: { label: 'Sistema', stack: '-apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif' },
};

export const EDITOR_FONTS: Record<EditorFont, { label: string; stack: string }> = {
  jetbrains: { label: 'JetBrains Mono', stack: '"JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace' },
  'system-mono': { label: 'Monoespaciada del sistema', stack: 'ui-monospace, SFMono-Regular, "Cascadia Mono", Consolas, monospace' },
  menlo: { label: 'Menlo / Consolas', stack: 'Menlo, Monaco, Consolas, "Liberation Mono", monospace' },
};

function pick<T extends string>(v: unknown, allowed: readonly T[], def: T): T {
  return typeof v === 'string' && (allowed as readonly string[]).includes(v) ? (v as T) : def;
}
function num(v: unknown, min: number, max: number, def: number): number {
  return typeof v === 'number' && Number.isFinite(v) ? Math.min(max, Math.max(min, v)) : def;
}

function sanitize(raw: Record<string, unknown>): AppSettings {
  return {
    theme: getTheme(typeof raw.theme === 'string' ? raw.theme : undefined).id,
    iconPack: pick(raw.iconPack, ['intellij', 'material', 'symbols'], DEFAULTS.iconPack),
    uiFont: pick(raw.uiFont, ['inter', 'system'], DEFAULTS.uiFont),
    uiSize: num(raw.uiSize, 12, 15, DEFAULTS.uiSize),
    editorFont: pick(raw.editorFont, ['jetbrains', 'system-mono', 'menlo'], DEFAULTS.editorFont),
    editorSize: num(raw.editorSize, 11, 20, DEFAULTS.editorSize),
    editorLineHeight: num(raw.editorLineHeight, 1.3, 2, DEFAULTS.editorLineHeight),
    ligatures: typeof raw.ligatures === 'boolean' ? raw.ligatures : DEFAULTS.ligatures,
    treeRow: num(raw.treeRow, 24, 44, DEFAULTS.treeRow),
    treeIcon: num(raw.treeIcon, 14, 28, DEFAULTS.treeIcon),
    treeFont: num(raw.treeFont, 11, 18, DEFAULTS.treeFont),
  };
}

function load(): AppSettings {
  try {
    const v2 = localStorage.getItem(KEY);
    if (v2) return sanitize(JSON.parse(v2));
    // Migración desde v1 (solo árbol): se conservan esos 3 valores.
    const v1 = localStorage.getItem(KEY_V1);
    if (v1) return sanitize(JSON.parse(v1));
  } catch { /* sin storage o JSON roto */ }
  return DEFAULTS;
}

function pickSettings(s: AppSettings): AppSettings {
  return {
    theme: s.theme, iconPack: s.iconPack, uiFont: s.uiFont, uiSize: s.uiSize, editorFont: s.editorFont,
    editorSize: s.editorSize, editorLineHeight: s.editorLineHeight, ligatures: s.ligatures,
    treeRow: s.treeRow, treeIcon: s.treeIcon, treeFont: s.treeFont,
  };
}

export function applySettings(s: AppSettings) {
  const st = document.documentElement.style;
  st.setProperty('--tree-row', `${s.treeRow}px`);
  st.setProperty('--tree-icon', `${s.treeIcon}px`);
  st.setProperty('--tree-font', `${s.treeFont}px`);
  st.setProperty('--cc-ui-font', UI_FONTS[s.uiFont].stack);
  st.setProperty('--cc-ui-size', `${s.uiSize}px`);
  st.setProperty('--cc-editor-font', EDITOR_FONTS[s.editorFont].stack);
  st.setProperty('--cc-editor-size', `${s.editorSize}px`);
  st.setProperty('--cc-editor-lh', String(s.editorLineHeight));
  st.setProperty('--cc-ligatures', s.ligatures ? 'normal' : 'none');
  applyTheme(getTheme(s.theme));
}

interface State extends AppSettings {
  set: (p: Partial<AppSettings>) => void;
  reset: () => void;
}

export const useSettings = create<State>((set, get) => ({
  ...load(),
  set: (p) => {
    set(p);
    const next = pickSettings(get());
    applySettings(next);
    // CM6 cachea alturas de línea: re-medir tras cambiar fuente/tamaño/interlineado.
    requestAnimationFrame(() => registry.view?.requestMeasure());
    try { localStorage.setItem(KEY, JSON.stringify(next)); } catch { /* ignore */ }
  },
  reset: () => get().set(DEFAULTS),
}));

applySettings(pickSettings(useSettings.getState()));
