import type { ReactNode } from 'react';
import type { SymbolInfo, SymbolKind } from '../../lib/types';
import { stereotypeOverlay, type SymbolOverlay } from '../../lib/fileIconRegistry';
import { ICON_COLORS } from '../../lib/iconColors';

const C = ICON_COLORS;

const SPRING_LEAF = 'M13 2C7 2.4 3.4 6.2 3.4 10.6c0 .9.2 1.7.6 2.4 1.2.3 2.6-.2 3.9-1.1C11.2 9.6 12.9 6.6 13 2z';

/** Contenedor SVG 16x16, inline, decorativo. Tamaño por clase (por defecto `h-4 w-4`). */
function Svg({ className = 'h-4 w-4', children }: { className?: string; children: ReactNode }) {
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" focusable="false" className={`shrink-0 ${className}`}>
      {children}
    </svg>
  );
}

interface CircleSpec { letter: string; color: string; dashed?: boolean }

const CIRCLES: Partial<Record<SymbolKind, CircleSpec>> = {
  class: { letter: 'C', color: C.class },
  abstractClass: { letter: 'C', color: C.class, dashed: true },
  interface: { letter: 'I', color: C.interface },
  enum: { letter: 'E', color: C.enum },
  record: { letter: 'R', color: C.record },
  annotation: { letter: '@', color: C.annotation },
  exception: { letter: 'C', color: C.exception },
  object: { letter: 'O', color: C.class },
  dataClass: { letter: 'C', color: C.class },
  sealedClass: { letter: 'C', color: C.class },
};

const KOTLIN_K = 'M2.5 2.5h11L8 8l5.5 5.5h-11z';

function Overlay({ kind }: { kind: SymbolOverlay }) {
  const back = <circle cx="11.6" cy="11.6" r="3.9" style={{ fill: 'rgb(var(--cc-panel))' }} />;
  let glyph: ReactNode;
  switch (kind) {
    case 'springRun':
      glyph = (
        <>
          <circle cx="11.6" cy="11.6" r="3.1" fill={C.spring} />
          <path d="M10.6 9.9 13.4 11.6 10.6 13.3z" fill="#ffffff" />
        </>
      );
      break;
    case 'run':
      glyph = <path d="M9.7 9.3 14 11.6 9.7 13.9z" fill={C.run} />;
      break;
    case 'test':
      glyph = (
        <>
          <path d="M9.7 9.3 14 11.6 9.7 13.9z" fill={C.run} />
          <circle cx="14" cy="9.2" r="1.2" fill={C.testFail} />
        </>
      );
      break;
    default:
      glyph = <path transform="translate(7.6 7.6) scale(.5)" d={SPRING_LEAF} fill={C.spring} />;
  }
  return (
    <g>
      {back}
      {glyph}
    </g>
  );
}

/** Icono de símbolo Java/Kotlin: círculo con letra + overlay de estereotipo. */
export function JvmSymbolIcon({ symbol, className }: { symbol: SymbolInfo; className?: string }) {
  const overlay = stereotypeOverlay(symbol.stereotype);
  const { kind } = symbol;
  let body: ReactNode;
  if (kind === 'kotlinFile' || kind === 'script') {
    body = (
      <>
        <path d={KOTLIN_K} fill={C.kotlin} />
        {kind === 'script' && (
          <text x="12.3" y="14.6" textAnchor="middle" fontSize="6" fontWeight={800} fontFamily="ui-sans-serif, system-ui, sans-serif" fill={C.kotlin}>
            S
          </text>
        )}
      </>
    );
  } else {
    const spec = CIRCLES[kind] ?? CIRCLES.class!;
    body = (
      <>
        <circle
          cx="7.6"
          cy="7.6"
          r="6"
          fill={spec.color}
          fillOpacity={0.22}
          stroke={spec.color}
          strokeWidth={1.2}
          strokeDasharray={spec.dashed ? '2 1.4' : undefined}
        />
        <text
          x="7.6"
          y="10.5"
          textAnchor="middle"
          fontSize="8.6"
          fontWeight={700}
          fontFamily="ui-sans-serif, system-ui, sans-serif"
          fill={spec.color}
        >
          {spec.letter}
        </text>
        {kind === 'dataClass' && <circle cx="12.4" cy="3" r="1.6" fill={spec.color} />}
        {kind === 'sealedClass' && (
          <>
            <rect x="10.6" y="3.2" width="3.6" height="2.8" rx="0.6" fill={spec.color} />
            <path d="M11.3 3.2V2.4a1.1 1.1 0 0 1 2.2 0v.8" stroke={spec.color} strokeWidth={0.9} />
          </>
        )}
      </>
    );
  }
  return (
    <Svg className={className}>
      {body}
      {overlay && <Overlay kind={overlay} />}
    </Svg>
  );
}
