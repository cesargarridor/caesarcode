import type { ITheme } from '@xterm/xterm';
import type { Theme } from './themes';

/** xterm necesita hex reales (no lee variables CSS). El fondo es exactamente el de la island. */
export function xtermTheme(theme: Theme): ITheme {
  const t = theme.term;
  return {
    background: theme.ui.panel,
    foreground: theme.ui.fg,
    cursor: theme.ui.accent,
    cursorAccent: theme.ui.panel,
    selectionBackground: t.selection,
    black: t.black, red: t.red, green: t.green, yellow: t.yellow,
    blue: t.blue, magenta: t.magenta, cyan: t.cyan, white: t.white,
    brightBlack: t.brightBlack, brightRed: t.brightRed, brightGreen: t.brightGreen, brightYellow: t.brightYellow,
    brightBlue: t.brightBlue, brightMagenta: t.brightMagenta, brightCyan: t.brightCyan, brightWhite: t.brightWhite,
  };
}
