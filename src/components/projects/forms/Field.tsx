import type { ReactNode } from 'react';

/** Fila etiqueta + control (+ error opcional) de los formularios del asistente. */
export function Field({ label, error, hint, children }: { label: string; error?: string | null; hint?: string; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[140px_1fr] items-start gap-x-3 gap-y-0.5">
      <label className="pt-1 text-ui-lg text-muted">{label}</label>
      <div className="min-w-0">
        {children}
        {error ? <div className="mt-0.5 text-[11px] text-stop-text">{error}</div> : hint ? <div className="mt-0.5 text-[11px] text-muted">{hint}</div> : null}
      </div>
    </div>
  );
}

export function CheckRow({ label, checked, onChange, disabled, hint }: { label: string; checked: boolean; onChange: (v: boolean) => void; disabled?: boolean; hint?: string }) {
  return (
    <div className="grid grid-cols-[140px_1fr] items-start gap-x-3">
      <span />
      <label className={`flex cursor-pointer items-start gap-2 text-ui-lg ${disabled ? 'opacity-50' : 'text-fg'}`}>
        <input type="checkbox" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} className="mt-[3px] h-3.5 w-3.5 accent-accent" />
        <span>
          {label}
          {hint && <span className="block text-[11px] text-muted">{hint}</span>}
        </span>
      </label>
    </div>
  );
}
