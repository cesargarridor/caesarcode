import { RangeSetBuilder, type Extension } from '@codemirror/state';
import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from '@codemirror/view';
import { lspDefinition } from '../../../lib/ipc';
import { isMod } from '../../../lib/keys';
import { toMessage } from '../../../lib/errors';
import { registry } from '../../../store/editorRegistry';
import { useEditor } from '../../../store/editor';
import { useUi } from '../../../store/ui';
import { offsetToLsp } from './diagnostics';
import { flushChange, lspAvailable } from './sync';

/** Va a la definición del símbolo en `pos`. `false` si no hay servidor o no encontró nada. */
export async function goToDefinitionAt(view: EditorView, pos: number): Promise<boolean> {
  const path = registry.shownPath;
  if (!lspAvailable(path)) return false;
  try {
    await flushChange(path);
    const { line, character } = offsetToLsp(view.state.doc, pos);
    const locs = await lspDefinition(path, line, character);
    const loc = locs[0];
    if (!loc) return false;
    await useEditor.getState().openFileAt(loc.path, loc.line + 1, loc.character);
    return true;
  } catch (e) {
    useUi.getState().pushToast(toMessage(e));
    return false;
  }
}

/** Comando F12. */
export function goToDefinition(view: EditorView): boolean {
  void goToDefinitionAt(view, view.state.selection.main.head);
  return true;
}

const linkMark = Decoration.mark({ class: 'cm-lsp-link' });

/** Subraya la palabra bajo el ratón mientras Mod está pulsado. */
const linkPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet = Decoration.none;
    private x = 0;
    private y = 0;
    mod = false;

    set(view: EditorView, mod: boolean, x = this.x, y = this.y) {
      this.x = x;
      this.y = y;
      this.mod = mod;
      let next: DecorationSet = Decoration.none;
      if (mod && lspAvailable(registry.shownPath)) {
        const pos = view.posAtCoords({ x, y });
        const range = pos == null ? null : view.state.wordAt(pos);
        if (range) {
          const b = new RangeSetBuilder<Decoration>();
          b.add(range.from, range.to, linkMark);
          next = b.finish();
        }
      }
      this.decorations = next;
      view.update([]); // redibuja las decoraciones
    }

    update(_u: ViewUpdate) {
      /* las decoraciones solo cambian con el ratón / teclado */
    }
  },
  { decorations: (v) => v.decorations },
);

export function definitionExtension(): Extension {
  return [
    // Alt+clic añade cursores; Mod+clic queda libre para ir a la definición.
    EditorView.clickAddsSelectionRange.of((e) => e.altKey),
    linkPlugin,
    EditorView.domEventHandlers({
      mousedown(e, view) {
        if (e.button !== 0 || !isMod(e) || !lspAvailable(registry.shownPath)) return false;
        const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
        if (pos == null) return false;
        e.preventDefault();
        void goToDefinitionAt(view, pos);
        return true;
      },
      mousemove(e, view) {
        const p = view.plugin(linkPlugin);
        if (p && (isMod(e) || p.mod)) p.set(view, isMod(e), e.clientX, e.clientY);
        return false;
      },
      keydown(e, view) {
        const p = view.plugin(linkPlugin);
        if (p && isMod(e)) p.set(view, true);
        return false;
      },
      keyup(e, view) {
        const p = view.plugin(linkPlugin);
        if (p && p.mod && !isMod(e)) p.set(view, false);
        return false;
      },
      blur(_e, view) {
        const p = view.plugin(linkPlugin);
        if (p && p.mod) p.set(view, false);
        return false;
      },
    }),
  ];
}
