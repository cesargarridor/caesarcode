import { channels, withAlpha } from './color';
import { getTheme, type Theme, type UiTokens } from './themes';

let current: Theme = getTheme(undefined);

/** Tema aplicado actualmente (para xterm y otros consumidores que necesitan hex). */
export function currentTheme(): Theme {
  return current;
}

const kebab = (s: string) => s.replace(/[A-Z]/g, (m) => `-${m.toLowerCase()}`);

/** Tokens que se exponen como color completo en vez de canales (llevan alfa propio o no pasan por <alpha-value>). */
const FULL_COLOR = new Set<keyof UiTokens>(['gutter']);

/** Escribe las variables --cc-* en :root y fija color-scheme. Cambio en vivo, sin recargar. */
export function applyTheme(theme: Theme): void {
  current = theme;
  const root = document.documentElement;
  const st = root.style;
  for (const [k, v] of Object.entries(theme.ui) as [keyof UiTokens, string][]) {
    st.setProperty(`--cc-${kebab(k)}`, FULL_COLOR.has(k) ? v : channels(v));
  }
  const ink = theme.dark ? '255 255 255' : '0 0 0';
  st.setProperty('--cc-ink', ink);
  st.setProperty('--cc-border', `rgb(${ink} / ${theme.dark ? 0.07 : 0.1})`);
  st.setProperty('--cc-border-strong', `rgb(${ink} / ${theme.dark ? 0.1 : 0.16})`);
  st.setProperty('--cc-hover', `rgb(${ink} / ${theme.dark ? 0.05 : 0.05})`);
  st.setProperty('--cc-hover-strong', `rgb(${ink} / ${theme.dark ? 0.09 : 0.09})`);
  st.setProperty('--cc-selection', withAlpha(theme.ui.accent, theme.dark ? 0.25 : 0.22));
  st.setProperty('--cc-scroll', `rgb(${ink} / ${theme.dark ? 0.1 : 0.18})`);
  st.setProperty('--cc-scroll-hover', `rgb(${ink} / ${theme.dark ? 0.18 : 0.3})`);
  st.setProperty('--cc-caret', theme.ui.accent);
  st.setProperty('--cc-excluded-text', '#e8883a');
  const s = theme.syntax;
  st.setProperty('--cc-syn-keyword', s.keyword);
  st.setProperty('--cc-syn-annotation', s.annotation);
  st.setProperty('--cc-syn-type', s.type);
  st.setProperty('--cc-syn-string', s.string);
  st.setProperty('--cc-syn-func', s.func);
  st.setProperty('--cc-syn-field', s.field);
  st.setProperty('--cc-syn-field-style', s.fieldItalic ? 'italic' : 'normal');
  st.setProperty('--cc-syn-variable', s.variable);
  st.setProperty('--cc-syn-comment', s.comment);
  st.setProperty('--cc-syn-number', s.number);
  st.setProperty('--cc-syn-operator', s.operator);
  st.setProperty('--cc-syn-heading', s.heading);
  st.setProperty('--cc-syn-link', s.link);
  st.setProperty('--cc-syn-invalid', s.invalid);
  st.setProperty('color-scheme', theme.dark ? 'dark' : 'light');
  root.style.colorScheme = theme.dark ? 'dark' : 'light';
}
