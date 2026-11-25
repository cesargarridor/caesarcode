import type { ReactNode } from 'react';

/** Panel flotante redondeado sobre el canvas. */
export function Island({ children, className = '' }: { children: ReactNode; className?: string }) {
  return <div className={`h-full overflow-hidden rounded-xl border border-border bg-panel ${className}`}>{children}</div>;
}
