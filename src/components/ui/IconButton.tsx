import type { ButtonHTMLAttributes, ReactNode } from 'react';

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  children: ReactNode;
  active?: boolean;
}

export function IconButton({ label, children, active = false, className = '', type = 'button', ...rest }: Props) {
  return (
    <button
      type={type}
      title={label}
      aria-label={label}
      className={`inline-flex h-6 w-6 items-center justify-center rounded-md text-muted outline-none transition-colors hover:bg-hover hover:text-fg focus-visible:ring-1 focus-visible:ring-accent disabled:opacity-40 ${active ? 'bg-hover-strong text-accent' : ''} ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
