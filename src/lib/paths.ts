// Utilidades de ruta solo para mostrar/comparar. El frontend nunca construye rutas
// para operar con disco: Rust hace el join (parent, name).

function lastSep(p: string): number {
  return Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
}

export function basename(p: string): string {
  const trimmed = p.replace(/[\\/]+$/, '');
  return trimmed.slice(lastSep(trimmed) + 1);
}

export function dirname(p: string): string {
  const trimmed = p.replace(/[\\/]+$/, '');
  const i = lastSep(trimmed);
  if (i < 0) return '';
  if (i === 0) return trimmed.slice(0, 1);
  return trimmed.slice(0, i);
}

export function extOf(p: string): string | null {
  const name = basename(p);
  const i = name.lastIndexOf('.');
  return i > 0 ? name.slice(i + 1).toLowerCase() : null;
}

function norm(p: string): string {
  return p.replace(/\\/g, '/').replace(/\/+$/, '');
}

/** `p` relativo a `root` (con `/`); si no está dentro devuelve `p` tal cual. */
export function relativeTo(root: string, p: string): string {
  const r = norm(root);
  const q = norm(p);
  if (q === r) return '';
  return q.startsWith(r + '/') ? q.slice(r.length + 1) : p;
}

/** `child` es `parent` o está dentro de él. */
export function isSameOrInside(parent: string, child: string): boolean {
  const a = norm(parent);
  const b = norm(child);
  return b === a || b.startsWith(a + '/');
}

/** Raíz del workspace que contiene `p` (prefijo más largo; las raíces no se solapan, pero por robustez). */
export function rootFor<T extends { path: string }>(roots: readonly T[], p: string): T | null {
  let best: T | null = null;
  for (const r of roots) {
    if (isSameOrInside(r.path, p) && (!best || norm(r.path).length > norm(best.path).length)) best = r;
  }
  return best;
}

/** Repo dueño de `p`: el `top` más largo que lo contiene (repos anidados). */
export function repoFor<T extends { top: string }>(repos: readonly T[], p: string): T | null {
  let best: T | null = null;
  for (const r of repos) {
    if (isSameOrInside(r.top, p) && (!best || norm(r.top).length > norm(best.top).length)) best = r;
  }
  return best;
}

/** Id visible de algo de una raíz: el crudo en la primaria (`roots[0]`), `"<nombreRaíz>/<id>"` en las demás. */
export function qualifyId(roots: readonly { path: string; name: string }[], rootPath: string, raw: string): string {
  const i = roots.findIndex((r) => norm(r.path) === norm(rootPath));
  return i <= 0 ? raw : `${roots[i].name}/${raw}`;
}

/** Igual que `qualifyId` pero conserva el prefijo de servicio efímero: `run:id` -> `run:<raíz>/id`. */
export function qualifyEphemeral(roots: readonly { path: string; name: string }[], rootPath: string, prefix: string, raw: string): string {
  const i = roots.findIndex((r) => norm(r.path) === norm(rootPath));
  return i <= 0 ? `${prefix}:${raw}` : `${prefix}:${roots[i].name}/${raw}`;
}
