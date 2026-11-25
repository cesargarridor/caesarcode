import { useEffect, useRef, useState } from 'react';
import { ChevronDown } from 'lucide-react';

export interface SelectOption<T extends string = string> { value: T; label: string }

interface Props<T extends string> {
  value: T;
  options: SelectOption<T>[];
  onChange: (v: T) => void;
  className?: string;
  ariaLabel?: string;
}

/** Select custom (popover + lista, teclado ↑↓ Enter Esc). Sin <select> nativo. */
export function Select<T extends string>({ value, options, onChange, className = '', ariaLabel }: Props<T>) {
  const [open, setOpen] = useState(false);
  const [cursor, setCursor] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);
  const current = options.find((o) => o.value === value);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener('mousedown', onDown);
    return () => window.removeEventListener('mousedown', onDown);
  }, [open]);

  const openList = () => {
    setCursor(Math.max(0, options.findIndex((o) => o.value === value)));
    setOpen(true);
  };

  const pick = (i: number) => {
    const o = options[i];
    if (o) onChange(o.value);
    setOpen(false);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (!open) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        openList();
      }
      return;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setCursor((c) => (c + 1) % options.length);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setCursor((c) => (c - 1 + options.length) % options.length);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      pick(cursor);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      setOpen(false);
    }
  };

  return (
    <div ref={rootRef} className={`relative ${className}`}>
      <button
        type="button"
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => (open ? setOpen(false) : openList())}
        onKeyDown={onKeyDown}
        className="flex h-7 w-full items-center justify-between gap-2 rounded-lg border border-border-strong bg-elevated px-2.5 text-ui-lg text-fg outline-none hover:bg-elevated-hover focus:border-accent/60"
      >
        <span className="truncate">{current?.label ?? ''}</span>
        <ChevronDown size={12} className="shrink-0 text-muted" />
      </button>
      {open && (
        <ul
          role="listbox"
          className="absolute left-0 right-0 top-full z-50 mt-1 max-h-56 overflow-auto rounded-lg border border-border-strong bg-elevated p-1 shadow-xl"
        >
          {options.map((o, i) => (
            <li
              key={o.value}
              role="option"
              aria-selected={o.value === value}
              onMouseEnter={() => setCursor(i)}
              onMouseDown={(e) => {
                e.preventDefault();
                pick(i);
              }}
              className={`cursor-pointer rounded-md px-2 py-1 text-ui-lg ${o.value === value ? 'bg-accent/12 text-accent-text' : `text-fg ${i === cursor ? 'bg-hover' : ''}`}`}
            >
              {o.label}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
