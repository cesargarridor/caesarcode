export function slug(s: string, fallback = 'service'): string {
  const out = s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return out || fallback;
}

/** Id slug que no colisiona con `used` (lo añade). Nunca contiene ':'. */
export function uniqueId(base: string, used: Set<string>, fallback = 'service'): string {
  const b = slug(base, fallback);
  let id = b;
  let n = 2;
  while (used.has(id)) id = `${b}-${n++}`;
  used.add(id);
  return id;
}
