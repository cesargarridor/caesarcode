import { useEffect } from 'react';

const isMac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/i.test(navigator.platform || navigator.userAgent);

/** Cmd en macOS, Ctrl en el resto. */
export function isMod(e: { metaKey: boolean; ctrlKey: boolean }): boolean {
  return isMac ? e.metaKey : e.ctrlKey;
}

export const modLabel = isMac ? '⌘' : 'Ctrl';

interface ShortcutOpts { shift?: boolean; alt?: boolean; capture?: boolean }

/** Atajo global Mod+tecla. `key` en minúsculas (p. ej. 'p', 's'). */
export function useGlobalShortcut(key: string, handler: (e: KeyboardEvent) => void, opts: ShortcutOpts = {}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!isMod(e)) return;
      if (e.key.toLowerCase() !== key) return;
      if (!!opts.shift !== e.shiftKey) return;
      if (!!opts.alt !== e.altKey) return;
      e.preventDefault();
      // En captura se corta la propagación para ganarle a CodeMirror y al resto.
      if (opts.capture) e.stopPropagation();
      handler(e);
    };
    const capture = !!opts.capture;
    window.addEventListener('keydown', onKey, capture);
    return () => window.removeEventListener('keydown', onKey, capture);
  }, [key, handler, opts.shift, opts.alt, opts.capture]);
}
