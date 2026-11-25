import { mix } from './color';

export type ThemeId =
  | 'caesar-islands'
  | 'caesar-obsidian'
  | 'darcula'
  | 'monokai'
  | 'one-dark'
  | 'dracula'
  | 'nord'
  | 'solarized-dark'
  | 'github-dark'
  | 'tokyo-night'
  | 'intellij-light';

/** Tokens de UI (hex). Se exponen como `--cc-<kebab>` en canales RGB. */
export interface UiTokens {
  canvas: string;
  base: string;
  panel: string;
  elevated: string;
  elevatedHover: string;
  card: string;
  cardHover: string;
  tabActive: string;
  quick: string;
  fg: string;
  fgStrong: string;
  muted: string;
  faint: string;
  run: string;
  runText: string;
  stop: string;
  stopText: string;
  accentSoft: string;
  accentText: string;
  accent: string;
  accentHover: string;
  ok: string;
  danger: string;
  gold: string;
  crimson: string;
  crimsonDeep: string;
  crimsonDark: string;
  treeSrc: string;
  treeTest: string;
  treeCfg: string;
  treeInfra: string;
  treeExcluded: string;
  /** Gutter del editor / números de línea. */
  gutter: string;
}

export interface SyntaxTokens {
  keyword: string;
  annotation: string;
  type: string;
  string: string;
  func: string;
  field: string;
  fieldItalic: boolean;
  variable: string;
  comment: string;
  number: string;
  operator: string;
  heading: string;
  link: string;
  invalid: string;
}

export interface TermColors {
  black: string; red: string; green: string; yellow: string; blue: string; magenta: string; cyan: string; white: string;
  brightBlack: string; brightRed: string; brightGreen: string; brightYellow: string; brightBlue: string;
  brightMagenta: string; brightCyan: string; brightWhite: string;
  selection: string;
}

export interface Theme {
  id: ThemeId;
  name: string;
  dark: boolean;
  ui: UiTokens;
  syntax: SyntaxTokens;
  term: TermColors;
}

interface UiSeed extends Partial<UiTokens> {
  canvas: string;
  panel: string;
  elevated: string;
  fg: string;
  fgStrong: string;
  muted: string;
  faint: string;
  accent: string;
}

function buildUi(dark: boolean, s: UiSeed): UiTokens {
  const ink = dark ? '#ffffff' : '#000000';
  const d = <T,>(a: T, b: T) => (dark ? a : b);
  const ui: UiTokens = {
    canvas: s.canvas,
    base: s.panel,
    panel: s.panel,
    elevated: s.elevated,
    elevatedHover: mix(s.elevated, ink, 0.07),
    card: mix(s.panel, s.elevated, 0.5),
    cardHover: mix(s.panel, s.elevated, 0.8),
    tabActive: mix(s.panel, s.elevated, 0.7),
    quick: mix(s.panel, s.canvas, 0.4),
    fg: s.fg,
    fgStrong: s.fgStrong,
    muted: s.muted,
    faint: s.faint,
    run: d('#10b981', '#059669'),
    runText: d('#34d399', '#047857'),
    stop: '#e11d48',
    stopText: d('#fb7185', '#be123c'),
    accentSoft: mix(s.accent, ink, 0.3),
    accentText: mix(s.accent, ink, d(0.5, 0.15)),
    accent: s.accent,
    accentHover: mix(s.accent, '#000000', 0.18),
    ok: d('#22c55e', '#16a34a'),
    danger: d('#ef4444', '#dc2626'),
    gold: d('#fbbf24', '#b45309'),
    crimson: '#e11d48',
    crimsonDeep: '#991b1b',
    crimsonDark: '#7f1d1d',
    treeSrc: d('#38bdf8', '#0284c7'),
    treeTest: d('#10b981', '#059669'),
    treeCfg: d('#eab308', '#a16207'),
    treeInfra: d('#a78bfa', '#7c3aed'),
    treeExcluded: mix(s.muted, s.panel, 0.45),
    gutter: mix(s.panel, s.fg, 0.28),
  };
  return { ...ui, ...s } as UiTokens;
}

function buildTerm(ui: UiTokens, sy: SyntaxTokens, dark: boolean): TermColors {
  const up = (c: string) => mix(c, dark ? '#ffffff' : '#000000', 0.22);
  const red = sy.invalid;
  const green = sy.string;
  const yellow = sy.annotation;
  const blue = sy.func;
  const magenta = sy.keyword;
  const cyan = sy.type;
  return {
    black: dark ? mix(ui.panel, '#000000', 0.4) : '#3b3b3b',
    red, green, yellow, blue, magenta, cyan,
    white: ui.fg,
    brightBlack: sy.comment,
    brightRed: up(red), brightGreen: up(green), brightYellow: up(yellow), brightBlue: up(blue),
    brightMagenta: up(magenta), brightCyan: up(cyan),
    brightWhite: ui.fgStrong,
    selection: ui.accent + '40',
  };
}

function make(id: ThemeId, name: string, dark: boolean, seed: UiSeed, syntax: SyntaxTokens): Theme {
  const ui = buildUi(dark, seed);
  return { id, name, dark, ui, syntax, term: buildTerm(ui, syntax, dark) };
}

const obsidianUi: UiTokens = {
  canvas: '#09090b', base: '#111114', panel: '#111114', elevated: '#18181c', elevatedHover: '#222228',
  card: '#16161b', cardHover: '#1c1c22', tabActive: '#1b1b20', quick: '#141418',
  fg: '#ececef', fgStrong: '#f4f4f6', muted: '#8e8e98', faint: '#71717a',
  run: '#10b981', runText: '#34d399', stop: '#e11d48', stopText: '#fb7185',
  accentSoft: '#fcd34d', accentText: '#fde68a', accent: '#f59e0b', accentHover: '#d97706',
  ok: '#22c55e', danger: '#ef4444', gold: '#fbbf24',
  crimson: '#e11d48', crimsonDeep: '#991b1b', crimsonDark: '#7f1d1d',
  treeSrc: '#38bdf8', treeTest: '#10b981', treeCfg: '#eab308', treeInfra: '#a78bfa', treeExcluded: '#52525b',
  gutter: '#3f3f46',
};

const obsidian: Theme = {
  id: 'caesar-obsidian',
  name: 'Caesar Obsidian',
  dark: true,
  ui: obsidianUi,
  syntax: {
    keyword: '#f59e0b', annotation: '#fcd34d', type: '#7dd3fc', string: '#6ee7b7', func: '#c4b5fd',
    field: '#e4e4e7', fieldItalic: false, variable: '#e4e4e7', comment: '#52525b', number: '#fdba74',
    operator: '#8e8e98', heading: '#f4f4f6', link: '#f59e0b', invalid: '#e11d48',
  },
  term: {
    black: '#1f1f1f', red: '#e11d48', green: '#10b981', yellow: '#eab308', blue: '#38bdf8', magenta: '#a78bfa',
    cyan: '#22d3ee', white: '#d4d4d4', brightBlack: '#525252', brightRed: '#ef4444', brightGreen: '#4ade80',
    brightYellow: '#fbbf24', brightBlue: '#7dd3fc', brightMagenta: '#c4b5fd', brightCyan: '#67e8f9',
    brightWhite: '#ffffff', selection: '#f59e0b40',
  },
};

const islands = make('caesar-islands', 'Caesar Islands', true,
  { canvas: '#1e1f22', panel: '#2b2d30', elevated: '#393b40', fg: '#dfe1e5', fgStrong: '#f2f3f5', muted: '#868a91', faint: '#6f737a', accent: '#f59e0b' },
  { keyword: '#cc7832', annotation: '#bbb529', type: '#6fafbd', string: '#6a8759', func: '#ffc66d', field: '#9876aa', fieldItalic: true,
    variable: '#a9b7c6', comment: '#808080', number: '#6897bb', operator: '#a9b7c6', heading: '#ffc66d', link: '#287bb5', invalid: '#ff6b68' });

const darcula = make('darcula', 'Darcula (IntelliJ)', true,
  { canvas: '#232425', panel: '#2b2b2b', elevated: '#3c3f41', fg: '#bbbbbb', fgStrong: '#dcdcdc', muted: '#8a8d90', faint: '#6e7173', accent: '#589df6' },
  { keyword: '#cc7832', annotation: '#bbb529', type: '#a9b7c6', string: '#6a8759', func: '#ffc66d', field: '#9876aa', fieldItalic: true,
    variable: '#a9b7c6', comment: '#808080', number: '#6897bb', operator: '#a9b7c6', heading: '#ffc66d', link: '#589df6', invalid: '#ff6b68' });

const monokai = make('monokai', 'Monokai', true,
  { canvas: '#1e1f1c', panel: '#272822', elevated: '#3e3d32', fg: '#f8f8f2', fgStrong: '#ffffff', muted: '#a09f93', faint: '#75715e', accent: '#a6e22e' },
  { keyword: '#f92672', annotation: '#a6e22e', type: '#66d9ef', string: '#e6db74', func: '#a6e22e', field: '#fd971f', fieldItalic: false,
    variable: '#f8f8f2', comment: '#75715e', number: '#ae81ff', operator: '#f92672', heading: '#a6e22e', link: '#66d9ef', invalid: '#f92672' });

const oneDark = make('one-dark', 'One Dark', true,
  { canvas: '#21252b', panel: '#282c34', elevated: '#333842', fg: '#abb2bf', fgStrong: '#d7dae0', muted: '#7f848e', faint: '#636d83', accent: '#61afef' },
  { keyword: '#c678dd', annotation: '#e5c07b', type: '#e5c07b', string: '#98c379', func: '#61afef', field: '#e06c75', fieldItalic: false,
    variable: '#abb2bf', comment: '#5c6370', number: '#d19a66', operator: '#56b6c2', heading: '#e06c75', link: '#61afef', invalid: '#e06c75' });

const dracula = make('dracula', 'Dracula', true,
  { canvas: '#1e1f29', panel: '#282a36', elevated: '#343746', fg: '#f8f8f2', fgStrong: '#ffffff', muted: '#9aa0c4', faint: '#6272a4', accent: '#bd93f9' },
  { keyword: '#ff79c6', annotation: '#50fa7b', type: '#8be9fd', string: '#f1fa8c', func: '#50fa7b', field: '#ffb86c', fieldItalic: false,
    variable: '#f8f8f2', comment: '#6272a4', number: '#bd93f9', operator: '#ff79c6', heading: '#bd93f9', link: '#8be9fd', invalid: '#ff5555' });

const nord = make('nord', 'Nord', true,
  { canvas: '#242933', panel: '#2e3440', elevated: '#3b4252', fg: '#d8dee9', fgStrong: '#eceff4', muted: '#8d98ad', faint: '#6b7690', accent: '#88c0d0' },
  { keyword: '#81a1c1', annotation: '#d08770', type: '#8fbcbb', string: '#a3be8c', func: '#88c0d0', field: '#d8dee9', fieldItalic: false,
    variable: '#d8dee9', comment: '#616e88', number: '#b48ead', operator: '#81a1c1', heading: '#88c0d0', link: '#88c0d0', invalid: '#bf616a' });

const solarized = make('solarized-dark', 'Solarized Dark', true,
  { canvas: '#00212b', panel: '#002b36', elevated: '#073642', fg: '#93a1a1', fgStrong: '#eee8d5', muted: '#839496', faint: '#657b83', accent: '#b58900' },
  { keyword: '#859900', annotation: '#cb4b16', type: '#b58900', string: '#2aa198', func: '#268bd2', field: '#6c71c4', fieldItalic: false,
    variable: '#93a1a1', comment: '#586e75', number: '#d33682', operator: '#859900', heading: '#268bd2', link: '#268bd2', invalid: '#dc322f' });

const github = make('github-dark', 'GitHub Dark', true,
  { canvas: '#010409', panel: '#0d1117', elevated: '#161b22', fg: '#c9d1d9', fgStrong: '#f0f6fc', muted: '#8b949e', faint: '#6e7681', accent: '#58a6ff' },
  { keyword: '#ff7b72', annotation: '#d2a8ff', type: '#ffa657', string: '#a5d6ff', func: '#d2a8ff', field: '#79c0ff', fieldItalic: false,
    variable: '#c9d1d9', comment: '#8b949e', number: '#79c0ff', operator: '#ff7b72', heading: '#79c0ff', link: '#58a6ff', invalid: '#f85149' });

const tokyo = make('tokyo-night', 'Tokyo Night', true,
  { canvas: '#16161e', panel: '#1a1b26', elevated: '#24283b', fg: '#c0caf5', fgStrong: '#e0e6ff', muted: '#787c99', faint: '#565f89', accent: '#7aa2f7' },
  { keyword: '#bb9af7', annotation: '#e0af68', type: '#2ac3de', string: '#9ece6a', func: '#7aa2f7', field: '#73daca', fieldItalic: false,
    variable: '#c0caf5', comment: '#565f89', number: '#ff9e64', operator: '#89ddff', heading: '#7aa2f7', link: '#7dcfff', invalid: '#f7768e' });

const light = make('intellij-light', 'IntelliJ Light', false,
  { canvas: '#f2f3f5', panel: '#ffffff', elevated: '#f7f8fa', fg: '#1f2329', fgStrong: '#000000', muted: '#6c707e', faint: '#8c8f99', accent: '#3574f0',
    elevatedHover: '#e8eaee', treeExcluded: '#a0a3ab' },
  { keyword: '#0033b3', annotation: '#9e880d', type: '#00627a', string: '#067d17', func: '#00627a', field: '#871094', fieldItalic: true,
    variable: '#080808', comment: '#8c8c8c', number: '#1750eb', operator: '#080808', heading: '#00627a', link: '#3574f0', invalid: '#f50000' });

export const THEMES: readonly Theme[] = [islands, obsidian, darcula, monokai, oneDark, dracula, nord, solarized, github, tokyo, light];
export const DEFAULT_THEME_ID: ThemeId = 'caesar-islands';

export function getTheme(id: string | undefined): Theme {
  return THEMES.find((t) => t.id === id) ?? THEMES[0];
}
