import type { FileDiff } from './types';

/** Parche válido para `git apply --recount` con UN hunk de `diff` (cabecera + hunk). */
export function hunkPatch(diff: FileDiff, index: number): string {
  const h = diff.hunks[index];
  if (!h) return '';
  let p = diff.header.endsWith('\n') || diff.header === '' ? diff.header : diff.header + '\n';
  p += h.header + '\n';
  for (const l of h.lines) {
    p += l.kind === '\\' ? '\\ ' : l.kind;
    p += l.text + '\n';
  }
  return p;
}

/** Cuenta líneas añadidas/eliminadas de un diff. */
export function diffStats(diff: FileDiff): { additions: number; deletions: number } {
  let additions = 0;
  let deletions = 0;
  for (const h of diff.hunks) {
    for (const l of h.lines) {
      if (l.kind === '+') additions++;
      else if (l.kind === '-') deletions++;
    }
  }
  return { additions, deletions };
}
