import { create } from 'zustand';
import type { LogLine } from '../lib/types';

// Los buffers viven FUERA de zustand: solo se guarda un contador `tick`
// que se incrementa como máximo una vez por frame.

export const LOG_CAP = 20_000;
const TRIM_CHUNK = 2_000;

const buffers = new Map<string, LogLine[]>();
let all: LogLine[] = [];

interface TickState {
  tick: number;
}
export const useLogsTick = create<TickState>(() => ({ tick: 0 }));

let scheduled = false;
function scheduleTick(): void {
  if (scheduled) return;
  scheduled = true;
  const bump = () => {
    scheduled = false;
    useLogsTick.setState((s) => ({ tick: s.tick + 1 }));
  };
  if (typeof requestAnimationFrame === 'function') requestAnimationFrame(bump);
  else setTimeout(bump, 16);
}

function trim(arr: LogLine[]): LogLine[] {
  // Recorte en bloques: solo cuando se pasa del tope por un margen.
  if (arr.length > LOG_CAP + TRIM_CHUNK) arr.splice(0, arr.length - LOG_CAP);
  return arr;
}

/** Líneas del servicio o de todos (`'all'`). El array es mutable: no guardar la referencia. */
export function getLines(id: string): LogLine[] {
  if (id === 'all') return all;
  return buffers.get(id) ?? EMPTY;
}
const EMPTY: LogLine[] = [];

export function pushLogs(lines: LogLine[]): void {
  if (lines.length === 0) return;
  for (const l of lines) {
    let b = buffers.get(l.serviceId);
    if (!b) {
      b = [];
      buffers.set(l.serviceId, b);
    }
    b.push(l);
    all.push(l);
  }
  for (const b of buffers.values()) trim(b);
  trim(all);
  scheduleTick();
}

/** Inserta el backlog (ordenado por seq) por delante, sin duplicar lo ya recibido por streaming. */
export function mergeBacklog(lines: LogLine[]): void {
  const firstSeq = all.length > 0 ? all[0].seq : Infinity;
  const older = lines.filter((l) => l.seq < firstSeq);
  if (older.length === 0) return;
  all = trim([...older, ...all]);
  const grouped = new Map<string, LogLine[]>();
  for (const l of older) {
    const g = grouped.get(l.serviceId);
    if (g) g.push(l);
    else grouped.set(l.serviceId, [l]);
  }
  for (const [id, g] of grouped) buffers.set(id, trim([...g, ...(buffers.get(id) ?? [])]));
  scheduleTick();
}

/** Borra el buffer local (`'all'` = todo). */
export function clearLocal(id: string): void {
  if (id === 'all') {
    buffers.clear();
    all = [];
  } else {
    buffers.delete(id);
    all = all.filter((l) => l.serviceId !== id);
  }
  scheduleTick();
}

export function resetLogs(): void {
  clearLocal('all');
}
