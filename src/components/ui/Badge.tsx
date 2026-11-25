import type { ReactNode } from 'react';

type Tone = 'neutral' | 'accent' | 'ok' | 'danger';

const TONES: Record<Tone, string> = {
  neutral: 'bg-ink/[0.05] text-muted',
  accent: 'bg-accent/12 text-accent-text',
  ok: 'bg-run/15 text-run-text',
  danger: 'bg-stop/15 text-stop-text',
};

export function Badge({ tone = 'neutral', children }: { tone?: Tone; children: ReactNode }) {
  return (
    <span className={`inline-flex items-center rounded-md px-1.5 font-mono text-[10px] leading-4 ${TONES[tone]}`}>
      {children}
    </span>
  );
}
