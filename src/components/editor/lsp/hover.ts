import { hoverTooltip, type Tooltip } from '@codemirror/view';
import { lspHover } from '../../../lib/ipc';
import { registry } from '../../../store/editorRegistry';
import { offsetToLsp } from './diagnostics';
import { flushChange, lspAvailable } from './sync';

/** Markdown del servidor → DOM. Solo `textContent`: nunca se interpreta HTML del servidor. */
export function renderHover(markdown: string): HTMLElement {
  const root = document.createElement('div');
  root.className = 'cm-lsp-hover';
  const lines = markdown.replace(/\r\n/g, '\n').split('\n');
  let code: string[] | null = null;
  let text: string[] = [];

  const flushText = () => {
    const joined = text.join('\n').trim();
    text = [];
    if (!joined) return;
    const div = document.createElement('div');
    div.className = 'cm-lsp-text';
    div.textContent = joined;
    root.appendChild(div);
  };

  for (const line of lines) {
    if (line.trimStart().startsWith('```')) {
      if (code === null) {
        flushText();
        code = [];
      } else {
        const pre = document.createElement('pre');
        pre.textContent = code.join('\n');
        root.appendChild(pre);
        code = null;
      }
      continue;
    }
    (code ?? text).push(line);
  }
  if (code !== null) {
    const pre = document.createElement('pre');
    pre.textContent = code.join('\n');
    root.appendChild(pre);
  }
  flushText();
  return root;
}

export const lspHoverTooltip = hoverTooltip(
  async (view, pos): Promise<Tooltip | null> => {
    const path = registry.shownPath;
    if (!lspAvailable(path)) return null;
    try {
      await flushChange(path);
      const { line, character } = offsetToLsp(view.state.doc, pos);
      const hover = await lspHover(path, line, character);
      if (!hover || !hover.markdown.trim()) return null;
      const dom = renderHover(hover.markdown);
      return { pos, above: true, create: () => ({ dom }) };
    } catch {
      return null;
    }
  },
  { hoverTime: 350 },
);
