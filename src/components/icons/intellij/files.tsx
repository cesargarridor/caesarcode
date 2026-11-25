import type { ComponentType, SVGProps } from 'react';
import type { FileIconName } from '../../../lib/fileIconRegistry';

type Icon = ComponentType<SVGProps<SVGSVGElement>>;
type P = SVGProps<SVGSVGElement>;

const Svg = (p: P) => <svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg" {...p} />;

/** Git: rombo rojo con ramas. */
export const GitFile = (p: P) => (
  <Svg {...p}>
    <path d="M8 1.2 14.8 8 8 14.8 1.2 8z" fill="#e8553d" />
    <circle cx="6.2" cy="6" r="1" fill="#fff" /><circle cx="6.2" cy="10.2" r="1" fill="#fff" /><circle cx="9.8" cy="8" r="1" fill="#fff" />
    <path d="M6.2 6.8v2.4M6.2 7.4c2 0 2.6.2 3.6.6" stroke="#fff" strokeWidth=".9" />
  </Svg>
);

/** Markdown naranja «M↓». */
export const HelpMdFile = (p: P) => (
  <Svg {...p}>
    <rect x="1" y="3" width="14" height="10" rx="2" fill="#f08a24" />
    <path d="M3.4 10.4V5.8l1.7 2 1.7-2v4.6M10.6 5.8v3.6M9.2 8.4l1.4 1.6 1.4-1.6" stroke="#fff" strokeWidth="1.1" strokeLinecap="round" strokeLinejoin="round" />
  </Svg>
);

/** Documento simple. */
export const DocFile = (p: P) => (
  <Svg {...p}>
    <path d="M3.5 1.5h6l3 3v10h-9z" stroke="#9da0a8" strokeWidth="1.1" strokeLinejoin="round" />
    <path d="M9.5 1.5v3h3M5.5 8h5M5.5 10.5h5" stroke="#9da0a8" strokeWidth="1" strokeLinecap="round" />
  </Svg>
);

/** Shell (mvnw). */
export const ShellFile = (p: P) => (
  <Svg {...p}>
    <rect x="1.5" y="2.5" width="13" height="11" rx="2" fill="#4a4d55" />
    <path d="M4.4 6.2 6.6 8l-2.2 1.8M8 10h3.4" stroke="#6fcf97" strokeWidth="1.1" strokeLinecap="round" strokeLinejoin="round" />
  </Svg>
);

/** Cmd azul (mvnw.cmd). */
export const CmdFile = (p: P) => (
  <Svg {...p}>
    <rect x="1.5" y="2.5" width="13" height="11" rx="2" fill="#3b7bd8" />
    <path d="M4.4 6.2 6.6 8l-2.2 1.8M8 10h3.4" stroke="#fff" strokeWidth="1.1" strokeLinecap="round" strokeLinejoin="round" />
  </Svg>
);

/** Maven «m» rosa. */
export const MavenFile = (p: P) => (
  <Svg {...p}>
    <rect x="1.5" y="1.5" width="13" height="13" rx="3" fill="#e0457b" />
    <path d="M4 11.5V6.2c0-.7.9-.9 1.3-.4L8 8.6l2.7-2.8c.4-.5 1.3-.3 1.3.4v5.3" stroke="#fff" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
  </Svg>
);

/** Archivos con icono IntelliJ propio; el resto cae a Material. */
export const INTELLIJ_FILE: Partial<Record<FileIconName, Icon>> = {
  git: GitFile, helpMd: HelpMdFile, readme: DocFile, mvnw: ShellFile, mvnwCmd: CmdFile, pom: MavenFile,
};
