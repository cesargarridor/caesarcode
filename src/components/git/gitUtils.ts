import { laneColors } from '../../lib/palette';

export const shortSha = (sha: string) => sha.slice(0, 7);

export function relTime(sec: number): string {
  const d = Math.max(0, Date.now() / 1000 - sec);
  if (d < 60) return 'ahora';
  if (d < 3600) return `hace ${Math.floor(d / 60)} min`;
  if (d < 86400) return `hace ${Math.floor(d / 3600)} h`;
  if (d < 86400 * 30) return `hace ${Math.floor(d / 86400)} d`;
  if (d < 86400 * 365) return `hace ${Math.floor(d / 86400 / 30)} mes`;
  return `hace ${Math.floor(d / 86400 / 365)} a`;
}

export function fullDate(sec: number): string {
  return new Date(sec * 1000).toLocaleString();
}

export function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return '?';
  const a = parts[0][0] ?? '?';
  const b = parts.length > 1 ? parts[parts.length - 1][0] : '';
  return (a + b).toUpperCase();
}

export function hashColor(s: string): string {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) >>> 0;
  return laneColors[h % laneColors.length];
}

/** Ruta absoluta de un archivo relativo al top del repo. */
export function absPath(top: string | null | undefined, rel: string): string {
  if (!top) return rel;
  return `${top.replace(/[\\/]+$/, '')}/${rel}`;
}

export const STATUS_COLOR: Record<string, string> = {
  M: 'text-accent', A: 'text-run-text', D: 'text-stop-text', R: 'text-tree-src', C: 'text-tree-src', T: 'text-accent',
  '?': 'text-run-text', U: 'text-stop-text',
};

export function splitPath(p: string): { name: string; dir: string } {
  const i = p.lastIndexOf('/');
  return i < 0 ? { name: p, dir: '' } : { name: p.slice(i + 1), dir: p.slice(0, i) };
}

/** Nombre de rama válido (reglas conservadoras de git). Devuelve el error o null. */
export function branchNameError(name: string): string | null {
  if (!name) return 'Escribe un nombre';
  if (name.startsWith('-')) return 'No puede empezar por «-»';
  if (/\s/.test(name)) return 'No puede contener espacios';
  if (/[~^:?*[\\]/.test(name) || name.includes('..') || name.includes('@{') || name.includes('//')) return 'Caracteres no permitidos';
  if (name.startsWith('/') || name.endsWith('/') || name.endsWith('.') || name.endsWith('.lock')) return 'Formato no válido';
  if (name === '@') return 'Nombre reservado';
  // eslint-disable-next-line no-control-regex
  if (/[\x00-\x1f\x7f]/.test(name)) return 'Caracteres no permitidos';
  return null;
}
