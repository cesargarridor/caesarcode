// Réplica en el frontend de `src-tauri/src/projects/validate.rs` (feedback inmediato).
// La autoridad es Rust: aquí solo se evita enviar peticiones que Rust rechazaría.

const RESERVED_WIN = new Set([
  'con', 'prn', 'aux', 'nul', 'com1', 'com2', 'com3', 'com4', 'com5', 'com6', 'com7', 'com8', 'com9',
  'lpt1', 'lpt2', 'lpt3', 'lpt4', 'lpt5', 'lpt6', 'lpt7', 'lpt8', 'lpt9',
]);

const JAVA_KEYWORDS = new Set([
  'abstract', 'assert', 'boolean', 'break', 'byte', 'case', 'catch', 'char', 'class', 'const', 'continue',
  'default', 'do', 'double', 'else', 'enum', 'extends', 'final', 'finally', 'float', 'for', 'goto', 'if',
  'implements', 'import', 'instanceof', 'int', 'interface', 'long', 'native', 'new', 'package', 'private',
  'protected', 'public', 'return', 'short', 'static', 'strictfp', 'super', 'switch', 'synchronized', 'this',
  'throw', 'throws', 'transient', 'try', 'void', 'volatile', 'while', 'true', 'false', 'null', '_',
]);

/** `null` = válido; si no, el mensaje de error. */
export type Check = string | null;

export function checkProjectName(name: string): Check {
  if (!/^[A-Za-z][A-Za-z0-9._-]{0,63}$/.test(name)) {
    return 'Empieza por letra; solo letras, dígitos, ".", "_" o "-" (máx. 64)';
  }
  if (name.includes('..') || name.endsWith('.')) return 'Nombre no válido';
  if (RESERVED_WIN.has(name.split('.')[0].toLowerCase())) return 'Nombre reservado en Windows';
  return null;
}

export function checkServiceName(name: string): Check {
  const base = checkProjectName(name);
  if (base) return base;
  if (!/^[a-z][a-z0-9-]{0,40}$/.test(name) || name.endsWith('-') || name.includes('--')) {
    return 'Usa kebab-case: orders-service';
  }
  return null;
}

export function checkPackage(pkg: string): Check {
  if (!/^[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)*$/.test(pkg)) return 'Minúsculas, dígitos y "_" separados por "."';
  const bad = pkg.split('.').find((s) => JAVA_KEYWORDS.has(s));
  return bad ? `"${bad}" es una palabra reservada de Java` : null;
}

export function checkArtifact(a: string): Check {
  return /^[a-z][a-z0-9-]*$/.test(a) && a.length <= 80 ? null : 'Minúsculas, dígitos y "-"';
}

export function checkGoModule(m: string): Check {
  return /^[A-Za-z0-9.\-_/~]+$/.test(m) && !m.includes('..') && !m.startsWith('-') ? null : 'Módulo de Go no válido';
}

export function checkPort(p: number): Check {
  return Number.isInteger(p) && p >= 1024 && p <= 65535 ? null : 'Puerto 1024-65535';
}

export function checkBootVersion(v: string): Check {
  return /^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?$/.test(v) ? null : 'Versión no válida';
}

/** `orders-service` → `ordersservice` (como Rust `pkg_segment`). */
export function pkgSegment(name: string): string {
  return name.replace(/-/g, '');
}

/** `My App` → `my-app` (artifactId válido). */
export function deriveArtifact(name: string): string {
  const a = name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');
  return /^[a-z]/.test(a) ? a : a ? `app-${a}` : 'app';
}

/** Paquete por defecto `<group>.<artifact sin guiones>`; segmentos reservados se sufijan con `_`. */
export function derivePackage(group: string, artifact: string): string {
  const seg = artifact.replace(/[^a-z0-9]/g, '').replace(/^(\d)/, '_$1') || 'app';
  const parts = `${group}.${seg}`.split('.').map((p) => (JAVA_KEYWORDS.has(p) ? `${p}_` : p));
  return parts.join('.');
}

/** Siguiente puerto libre ≥ `from` que no esté en `used`. */
export function nextPort(used: Iterable<number>, from = 8081): number {
  const taken = new Set(used);
  let p = from;
  while (taken.has(p)) p++;
  return p;
}

/** Ruta final mostrada en el asistente (solo visual). */
export function previewPath(location: string, name: string): string {
  if (!location) return name;
  const sep = location.includes('\\') && !location.includes('/') ? '\\' : '/';
  return `${location.replace(/[\\/]+$/, '')}${sep}${name}`;
}

export function slugModule(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9._-]+/g, '-');
}
