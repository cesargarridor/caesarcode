import type { ComponentType, SVGProps } from 'react';
import type { FolderIconName } from '../../../lib/fileIconRegistry';

type Icon = ComponentType<SVGProps<SVGSVGElement>>;
type Glyph = 'code' | 'badge' | null;

const BASE = '#9da0a8';

interface Spec { color: string; glyph?: Glyph; badge?: string }

const CLOSED = 'M1.5 4a1 1 0 0 1 1-1h3.1a1 1 0 0 1 .76.35L7.2 4.5h6.3a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z';
const OPEN = 'M1.5 12.5V4a1 1 0 0 1 1-1h3.1a1 1 0 0 1 .76.35L7.2 4.5h6.3a1 1 0 0 1 1 1V7M1.5 12.5l1.8-4.7a1 1 0 0 1 .93-.64H15a.5.5 0 0 1 .47.68l-1.7 4.4a1 1 0 0 1-.93.64H2.5a1 1 0 0 1-1-1z';

function make(spec: Spec, open: boolean): Icon {
  const { color, glyph = null, badge } = spec;
  return function IntellijFolder(props: SVGProps<SVGSVGElement>) {
    return (
      <svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg" {...props}>
        <path d={open ? OPEN : CLOSED} stroke={color} strokeWidth="1.1" strokeLinejoin="round" strokeLinecap="round" />
        {glyph === 'code' && !open && (
          <path d="M6.6 7.6 5.2 9l1.4 1.4M9.4 7.6 10.8 9l-1.4 1.4" stroke={color} strokeWidth="1" strokeLinecap="round" strokeLinejoin="round" />
        )}
        {glyph === 'code' && open && (
          <path d="M6.8 9.4 5.8 10.4l1 1M9.2 9.4l1 1-1 1" stroke={color} strokeWidth="1" strokeLinecap="round" strokeLinejoin="round" />
        )}
        {(glyph === 'badge' || badge) && <circle cx="12.2" cy="11.2" r="2.1" fill={badge ?? color} stroke="#1e1f22" strokeWidth=".6" />}
      </svg>
    );
  };
}

const SPECS: Partial<Record<FolderIconName, Spec>> = {
  ide: { color: '#2fb6a8' }, // .idea teal
  mvn: { color: '#d6455d' }, // .mvn carmesí
  src: { color: '#5b9cf6', glyph: 'code' }, sourceRoot: { color: '#5b9cf6', glyph: 'code' },
  target: { color: '#e9a23b' }, build: { color: '#e9a23b' },
  docs: { color: '#5b9cf6' },
  infra: { color: BASE, glyph: 'badge' },
  resources: { color: '#c58f3c' }, resourcesRoot: { color: '#c58f3c' },
  tests: { color: '#5fb370' }, testRoot: { color: '#5fb370' }, testResourcesRoot: { color: '#5fb370' },
  generatedRoot: { color: '#7f8aa3' },
  module: { color: '#f59e0b' },
};

const cache = new Map<string, Icon>();

/** Carpeta IntelliJ (contorno). Sin especificación propia: contorno gris simple. */
export function intellijFolder(name: FolderIconName, open: boolean): Icon {
  const k = `${name}:${open}`;
  let icon = cache.get(k);
  if (!icon) {
    icon = make(SPECS[name] ?? { color: BASE }, open);
    cache.set(k, icon);
  }
  return icon;
}
