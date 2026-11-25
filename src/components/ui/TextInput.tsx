import { forwardRef, type InputHTMLAttributes } from 'react';

export const TextInput = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(function TextInput(
  { className = '', ...rest },
  ref,
) {
  return (
    <input
      ref={ref}
      spellCheck={false}
      autoComplete="off"
      className={`h-7 w-full rounded-lg border border-border-strong bg-elevated px-2.5 text-ui-lg text-fg outline-none placeholder:text-muted focus:border-accent/60 ${className}`}
      {...rest}
    />
  );
});
