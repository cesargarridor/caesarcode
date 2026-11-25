import type { ServiceConfig, ServiceKind, Tech } from './types';

export const TECH_ORDER: Tech[] = ['spring', 'python', 'node', 'go', 'rust', 'docker', 'other'];

export const TECH_LABEL: Record<Tech, string> = {
  spring: 'Spring Boot',
  python: 'Python',
  node: 'Node/TS',
  go: 'Go',
  rust: 'Rust',
  docker: 'Docker',
  other: 'Other',
};

const KIND_TECH: Partial<Record<ServiceKind, Tech>> = {
  node: 'node',
  python: 'python',
  rust: 'rust',
  go: 'go',
  docker: 'docker',
};

/** Tecnología para agrupar: la del detector o, en servicios antiguos, derivada del `kind`. */
export function techOf(c: Pick<ServiceConfig, 'tech' | 'kind'>): Tech {
  return c.tech ?? KIND_TECH[c.kind] ?? 'other';
}
