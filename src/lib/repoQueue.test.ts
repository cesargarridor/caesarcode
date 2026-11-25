import { describe, expect, it } from 'vitest';
import { pickActiveRepo, pullSkipReason, repoOfRoot, runQueue, summarizeJobs } from './repoQueue';
import { repoFor } from './paths';
import type { RepoSummary } from './types';

const sum = (p: Partial<RepoSummary> = {}): RepoSummary => ({
  top: '/r', name: 'r', branch: 'main', detached: false, upstream: 'origin/main', ahead: 0, behind: 0,
  changed: 0, staged: 0, untracked: 0, conflicted: 0, state: null, error: null, ...p,
});

describe('runQueue', () => {
  it('respeta la concurrencia y el orden', async () => {
    let inFlight = 0;
    let peak = 0;
    const res = await runQueue([1, 2, 3, 4, 5, 6, 7], 3, async (n) => {
      inFlight++;
      peak = Math.max(peak, inFlight);
      await new Promise((r) => setTimeout(r, 5));
      inFlight--;
      return n * 2;
    });
    expect(peak).toBeLessThanOrEqual(3);
    expect(res.map((r) => (r.status === 'fulfilled' ? r.value : null))).toEqual([2, 4, 6, 8, 10, 12, 14]);
  });
  it('un fallo no aborta el resto', async () => {
    const res = await runQueue([1, 2, 3], 2, async (n) => {
      if (n === 2) throw new Error('x');
      return n;
    });
    expect(res.map((r) => r.status)).toEqual(['fulfilled', 'rejected', 'fulfilled']);
  });
  it('lista vacía', async () => {
    expect(await runQueue([], 3, async () => 1)).toEqual([]);
  });
});

describe('pullSkipReason', () => {
  it('limpio con upstream: se puede', () => expect(pullSkipReason(sum())).toBeNull());
  it('salta por cambios, sin upstream, merge, detached y error', () => {
    expect(pullSkipReason(sum({ changed: 2 }))).toMatch(/cambios/);
    expect(pullSkipReason(sum({ upstream: null }))).toMatch(/upstream/);
    expect(pullSkipReason(sum({ state: 'merging' }))).toMatch(/merging/);
    expect(pullSkipReason(sum({ detached: true }))).toMatch(/HEAD/);
    expect(pullSkipReason(sum({ error: 'boom' }))).toMatch(/error/);
    expect(pullSkipReason(undefined)).not.toBeNull();
  });
});

describe('summarizeJobs', () => {
  it('formatea el resumen', () => {
    const r = (status: 'ok' | 'skipped' | 'error') => ({ top: '/x', status, message: '' });
    expect(summarizeJobs([r('ok'), r('ok'), r('skipped'), r('error')])).toBe('2 ok · 1 saltado · 1 error');
    expect(summarizeJobs([r('ok')])).toBe('1 ok');
  });
});

describe('pickActiveRepo', () => {
  const repos = [
    { top: '/w/a', root: '/w/a' },
    { top: '/w/b', root: '/w/b' },
    { top: '/w/b/libs/x', root: '/w/b' },
  ];
  it('prioriza actual, persistido, primaria, primero', () => {
    expect(pickActiveRepo(repos, '/w/a', '/w/b', '/w/b/libs/x', repoFor)).toBe('/w/b/libs/x');
    expect(pickActiveRepo(repos, '/w/a', '/w/b', null, repoFor)).toBe('/w/b');
    expect(pickActiveRepo(repos, '/w/a', '/w/zzz', null, repoFor)).toBe('/w/a');
    expect(pickActiveRepo(repos, '/w/nada', null, null, repoFor)).toBe('/w/a');
    expect(pickActiveRepo([], '/w/a', null, null, repoFor)).toBeNull();
  });
  it('una primaria que agrupa repos usa el primero de su raíz', () => {
    const r = [{ top: '/m/svc1', root: '/m' }, { top: '/m/svc2', root: '/m' }];
    expect(pickActiveRepo(r, '/m', null, null, repoFor)).toBe('/m/svc1');
    expect(repoOfRoot(r, '/m', repoFor)?.top).toBe('/m/svc1');
  });
});
