import { PanelResizeHandle } from 'react-resizable-panels';

/** Invisible en reposo (hueco de canvas); línea ámbar fina en hover/arrastre. */
export function ResizeHandle({ direction }: { direction: 'horizontal' | 'vertical' }) {
  const h = direction === 'horizontal';
  return (
    <PanelResizeHandle
      className={`group relative flex items-center justify-center bg-transparent ${h ? 'w-1.5' : 'h-1.5'}`}
    >
      <div
        className={`pointer-events-none absolute bg-transparent transition-colors group-hover:bg-accent group-data-[resize-handle-state=drag]:bg-accent ${
          h ? 'inset-y-0 left-1/2 w-px -translate-x-1/2' : 'inset-x-0 top-1/2 h-px -translate-y-1/2'
        }`}
      />
    </PanelResizeHandle>
  );
}
