import type { RepoSummary } from './types';

/** Ejecuta `worker` sobre `items` con como mucho `concurrency` en vuelo; conserva el orden de resultados. */
export async function runQueue<T, R>(
  items: readonly T[],
  concurrency: number,
  worker: (item: T, index: number) => Promise<R>,
): Promise<PromiseSettledResult<R>[]> {
  const results: PromiseSettledResult<R>[] = new Array(items.length);
  let next = 0;
  const lanes = Math.max(1, Math.min(concurrency, items.length));
  await Promise.all(
    Array.from({ length: lanes }, async () => {
      while (next < items.length) {
        const i = next++;
        try {
          results[i] = { status: 'fulfilled', value: await worker(items[i], i) };
        } catch (reason) {
          results[i] = { status: 'rejected', reason };
        }
      }
    }),
  );
  return results;
}

/** "Pull all" (ff-only) solo toca repos limpios, con upstream y sin merge/rebase en curso. Devuelve el motivo de salto. */
export function pullSkipReason(s: RepoSummary | undefined): string | null {
  if (!s) return 'sin resumen';
  if (s.error) return 'error al leer el estado';
  if (s.state) return `en ${s.state}`;
  if (s.detached) return 'HEAD desacoplado';
  if (!s.upstream) return 'sin upstream';
  if (s.changed > 0) return 'cambios sin commitear';
  return null;
}

export type RepoJobStatus = 'ok' | 'skipped' | 'error';
export interface RepoJobResult { top: string; status: RepoJobStatus; message: string }

/** "7 ok · 2 saltados · 1 error". */
export function summarizeJobs(results: readonly RepoJobResult[]): string {
  const n = (s: RepoJobStatus) => results.filter((r) => r.status === s).length;
  const parts = [`${n('ok')} ok`];
  if (n('skipped') > 0) parts.push(`${n('skipped')} saltado${n('skipped') === 1 ? '' : 's'}`);
  if (n('error') > 0) parts.push(`${n('error')} error${n('error') === 1 ? '' : 'es'}`);
  return parts.join(' · ');
}

/** Repo "de" una raíz: el que la contiene o, si la raíz agrupa repos (monorepo/carpeta de microservicios), el primero detectado en ella. */
export function repoOfRoot<T extends { top: string; root: string }>(
  repos: readonly T[],
  rootPath: string,
  find: (repos: readonly T[], p: string) => T | null,
): T | null {
  return find(repos, rootPath) ?? repos.find((r) => r.root === rootPath) ?? null;
}

/**
 * Repo activo al cargar: el actual si sigue existiendo, luego el persistido, luego el de la raíz primaria,
 * luego el primero; `null` si no hay repos.
 */
export function pickActiveRepo(
  repos: readonly { top: string; root: string }[],
  primary: string | null,
  persisted: string | null,
  current: string | null,
  find: (repos: readonly { top: string; root: string }[], p: string) => { top: string; root: string } | null,
): string | null {
  const has = (t: string | null): t is string => !!t && repos.some((r) => r.top === t);
  if (has(current)) return current;
  if (has(persisted)) return persisted;
  if (primary) {
    const r = repoOfRoot(repos, primary, find);
    if (r) return r.top;
  }
  return repos[0]?.top ?? null;
}
