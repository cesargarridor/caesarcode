import { describe, expect, it } from 'vitest';
import { layoutGraph, laneCount } from './gitGraph';
import type { CommitRow } from './types';

const c = (sha: string, ...parents: string[]): CommitRow => ({
  sha, parents, author: 'a', email: 'a@x', time: 0, subject: sha, refs: [],
});

describe('layoutGraph', () => {
  it('historia lineal usa una sola lane', () => {
    const { rows, state } = layoutGraph(null, [c('c3', 'c2'), c('c2', 'c1'), c('c1')]);
    expect(rows.map((r) => r.lane)).toEqual([0, 0, 0]);
    expect(laneCount(rows)).toBe(1);
    expect(state.lanes).toEqual([]);
    expect(rows[0].edges).toEqual([{ fromLane: 0, toLane: 0, color: 0, type: 'branch' }]);
  });

  it('rama + merge: 2 lanes y aristas de merge', () => {
    // m (merge de a y b) ; a -> base ; b -> base
    const commits = [c('m', 'a', 'b'), c('a', 'base'), c('b', 'base'), c('base')];
    const { rows } = layoutGraph(null, commits);
    expect(rows[0].isMerge).toBe(true);
    expect(rows[0].edges.filter((e) => e.type === 'branch').map((e) => e.toLane)).toEqual([0, 1]);
    expect(rows[1].lane).toBe(0);
    expect(rows[2].lane).toBe(1);
    // b converge hacia la lane de base (arista branch 1 -> 0) y base solo recibe una por arriba
    expect(rows[2].edges.find((e) => e.type === 'branch')).toMatchObject({ fromLane: 1, toLane: 0 });
    expect(rows[3].lane).toBe(0);
    expect(rows[3].edges.filter((e) => e.type === 'merge').map((e) => e.fromLane)).toEqual([0]);
    expect(laneCount(rows)).toBe(2);
  });

  it('dos ramas paralelas largas mantienen lanes estables', () => {
    const commits = [c('a3', 'a2'), c('b3', 'b2'), c('a2', 'a1'), c('b2', 'b1'), c('a1'), c('b1')];
    const { rows } = layoutGraph(null, commits);
    expect(rows.map((r) => r.lane)).toEqual([0, 1, 0, 1, 0, 1]);
    // b3 pasa por encima de la lane 0
    expect(rows[1].edges.some((e) => e.type === 'pass' && e.fromLane === 0)).toBe(true);
  });

  it('octopus con 3 padres', () => {
    const { rows } = layoutGraph(null, [c('o', 'p1', 'p2', 'p3'), c('p1'), c('p2'), c('p3')]);
    expect(rows[0].edges.filter((e) => e.type === 'branch').map((e) => e.toLane)).toEqual([0, 1, 2]);
    expect(rows.slice(1).map((r) => r.lane)).toEqual([0, 1, 2]);
  });

  it('varias raíces (historias no relacionadas) reutilizan lanes libres', () => {
    const { rows, state } = layoutGraph(null, [c('x2', 'x1'), c('x1'), c('y1')]);
    expect(rows.map((r) => r.lane)).toEqual([0, 0, 0]);
    expect(state.lanes).toEqual([]);
  });

  it('continuidad entre páginas', () => {
    const commits = [c('m', 'a', 'b'), c('a', 'a0'), c('b', 'b0'), c('a0', 'base'), c('b0', 'base'), c('base')];
    const full = layoutGraph(null, commits);
    const p1 = layoutGraph(null, commits.slice(0, 3));
    const p2 = layoutGraph(p1.state, commits.slice(3));
    expect([...p1.rows, ...p2.rows]).toEqual(full.rows);
    expect(p2.state).toEqual(full.state);
  });

  it('no muta el estado de entrada', () => {
    const p1 = layoutGraph(null, [c('m', 'a', 'b')]);
    const snap = JSON.stringify(p1.state);
    layoutGraph(p1.state, [c('a')]);
    expect(JSON.stringify(p1.state)).toBe(snap);
  });

  it('10 000 commits sintéticos en < 200 ms', () => {
    const commits: CommitRow[] = [];
    for (let i = 0; i < 10000; i++) {
      const parents = [`s${i + 1}`];
      if (i % 7 === 0) parents.push(`s${i + 3}`);
      commits.push(c(`s${i}`, ...parents));
    }
    const t0 = performance.now();
    const { rows } = layoutGraph(null, commits);
    const ms = performance.now() - t0;
    expect(rows).toHaveLength(10000);
    expect(ms).toBeLessThan(200);
  });
});
