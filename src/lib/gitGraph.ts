import type { CommitRow } from './types';

/**
 * Cálculo de lanes del grafo de commits (puro e incremental).
 *
 * Cada fila se dibuja en una caja de altura fija: `top` (y=0) → nodo (y=mitad) → `bottom` (y=alto).
 * Tipos de arista:
 *  - `pass`:   lane que atraviesa la fila sin tocar el nodo (fromLane === toLane, top → bottom).
 *  - `merge`:  llega al nodo desde arriba (fromLane en `top` → nodo en `lane`). Hay una por cada hijo
 *              que esperaba a este commit.
 *  - `branch`: sale del nodo hacia abajo (nodo en `lane` → toLane en `bottom`). Una por padre; las de
 *              padres extra de un merge-commit apuntan a otra lane.
 */
export type EdgeType = 'pass' | 'merge' | 'branch';
export interface Edge { fromLane: number; toLane: number; color: number; type: EdgeType }
export interface GraphRow {
  sha: string;
  lane: number;
  color: number;
  edges: Edge[];
  /** Mayor lane usada en esta fila (para ancho del SVG). */
  maxLane: number;
  isMerge: boolean;
}
/** Lanes activas al final de la última fila calculada: sha esperado por cada lane (o null si libre). */
export interface GraphState { lanes: (string | null)[] }

export const LANE_COLORS = 8;
export const laneColor = (lane: number): number => lane % LANE_COLORS;

/**
 * Calcula las filas de `commits` continuando desde `state` (null = inicio). No muta `state`.
 * Los commits deben venir en orden topológico (hijos antes que padres), como `git log --topo-order`.
 */
export function layoutGraph(state: GraphState | null, commits: CommitRow[]): { state: GraphState; rows: GraphRow[] } {
  const lanes: (string | null)[] = state ? state.lanes.slice() : [];
  const rows: GraphRow[] = new Array(commits.length);

  for (let n = 0; n < commits.length; n++) {
    const c = commits[n];
    let lane = lanes.indexOf(c.sha);
    if (lane < 0) {
      lane = lanes.indexOf(null);
      if (lane < 0) {
        lane = lanes.length;
        lanes.push(null);
      }
    }
    const edges: Edge[] = [];
    let maxLane = lane;
    const incoming: number[] = [];
    for (let i = 0; i < lanes.length; i++) {
      const s = lanes[i];
      if (s === null) continue;
      if (s === c.sha) {
        incoming.push(i);
        edges.push({ fromLane: i, toLane: lane, color: laneColor(i), type: 'merge' });
      } else {
        edges.push({ fromLane: i, toLane: i, color: laneColor(i), type: 'pass' });
        if (i > maxLane) maxLane = i;
      }
      if (i > maxLane) maxLane = i;
    }
    for (const i of incoming) lanes[i] = null;

    for (let k = 0; k < c.parents.length; k++) {
      const p = c.parents[k];
      let target = lanes.indexOf(p);
      if (target < 0) {
        if (k === 0) {
          target = lane;
        } else {
          target = lanes.indexOf(null);
          if (target < 0) {
            target = lanes.length;
            lanes.push(null);
          }
        }
        lanes[target] = p;
      }
      edges.push({ fromLane: lane, toLane: target, color: laneColor(target), type: 'branch' });
      if (target > maxLane) maxLane = target;
    }

    while (lanes.length > 0 && lanes[lanes.length - 1] === null) lanes.pop();
    rows[n] = { sha: c.sha, lane, color: laneColor(lane), edges, maxLane, isMerge: c.parents.length > 1 };
  }
  return { state: { lanes }, rows };
}

/** Número de lanes necesarias para dibujar `rows` (mayor maxLane + 1). */
export function laneCount(rows: GraphRow[]): number {
  let m = 0;
  for (const r of rows) if (r.maxLane > m) m = r.maxLane;
  return m + 1;
}
