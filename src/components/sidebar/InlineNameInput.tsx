import { useEffect, useRef } from 'react';

interface Props {
  initial?: string;
  placeholder?: string;
  onSubmit: (name: string) => void;
  onCancel: () => void;
}

/** Input de nombre dentro del árbol: Enter confirma, Esc o perder foco cancela. */
export function InlineNameInput({ initial = '', placeholder, onSubmit, onCancel }: Props) {
  const ref = useRef<HTMLInputElement>(null);
  const done = useRef(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.focus();
    // En renombrado se selecciona el nombre sin la extensión.
    const dot = initial.lastIndexOf('.');
    el.setSelectionRange(0, dot > 0 ? dot : initial.length);
  }, [initial]);

  const finish = (fn: () => void) => {
    if (done.current) return;
    done.current = true;
    fn();
  };

  return (
    <input
      ref={ref}
      defaultValue={initial}
      placeholder={placeholder}
      spellCheck={false}
      autoComplete="off"
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === 'Enter') {
          const v = e.currentTarget.value.trim();
          finish(() => (v ? onSubmit(v) : onCancel()));
        } else if (e.key === 'Escape') {
          finish(onCancel);
        }
      }}
      onBlur={() => finish(onCancel)}
      className="h-5 min-w-0 flex-1 rounded-md border border-accent/60 bg-elevated px-1.5 font-mono text-[12px] text-fg outline-none"
    />
  );
}
