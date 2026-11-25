import type { ButtonHTMLAttributes } from 'react';

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'default' | 'primary' | 'danger';
}

export function Button({ variant = 'default', className = '', type = 'button', ...rest }: Props) {
  const styles =
    variant === 'primary'
      ? 'border-transparent bg-accent/15 text-accent-text hover:bg-accent/25 font-medium'
      : variant === 'danger'
        ? 'border-transparent bg-stop/15 text-stop-text hover:bg-stop/25'
        : 'border-border-strong bg-elevated text-fg hover:bg-elevated-hover';
  return (
    <button
      type={type}
      className={`inline-flex h-7 items-center justify-center gap-1.5 rounded-md border px-3 text-ui outline-none transition-colors focus-visible:ring-1 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-50 ${styles} ${className}`}
      {...rest}
    />
  );
}
