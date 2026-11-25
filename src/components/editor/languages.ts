import type { Extension } from '@codemirror/state';
import { python } from '@codemirror/lang-python';
import { javascript } from '@codemirror/lang-javascript';
import { rust } from '@codemirror/lang-rust';
import { java } from '@codemirror/lang-java';
import { json } from '@codemirror/lang-json';
import { yaml } from '@codemirror/lang-yaml';
import { markdown } from '@codemirror/lang-markdown';
import { xml } from '@codemirror/lang-xml';
import { html } from '@codemirror/lang-html';
import { css } from '@codemirror/lang-css';
import { sass } from '@codemirror/lang-sass';
import { sql } from '@codemirror/lang-sql';
import { go } from '@codemirror/lang-go';
import { StreamLanguage, type StreamParser } from '@codemirror/language';
import { toml } from '@codemirror/legacy-modes/mode/toml';
import { properties } from '@codemirror/legacy-modes/mode/properties';
import { shell } from '@codemirror/legacy-modes/mode/shell';
import { dockerFile } from '@codemirror/legacy-modes/mode/dockerfile';
import { extOf } from '../../lib/paths';

/** Parser de logs: timestamps, niveles ERROR/WARN/INFO/DEBUG/TRACE. */
export const logParser: StreamParser<unknown> = {
  name: 'log',
  token(stream) {
    if (stream.eatSpace()) return null;
    if (stream.match(/^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:[.,]\d+)?(?:Z|[+-]\d{2}:?\d{2})?/)) return 'number';
    if (stream.match(/^\d{2}:\d{2}:\d{2}(?:[.,]\d+)?/)) return 'number';
    if (stream.match(/^(?:ERROR|SEVERE|FATAL|CRITICAL)\b/)) return 'invalid';
    if (stream.match(/^(?:WARN|WARNING)\b/)) return 'keyword';
    if (stream.match(/^INFO\b/)) return 'string';
    if (stream.match(/^(?:DEBUG|TRACE)\b/)) return 'comment';
    stream.next();
    return null;
  },
};

const lang = (p: StreamParser<unknown>): Extension => StreamLanguage.define(p);

function ext(path: string): string {
  return (extOf(path) ?? '').toLowerCase().replace(/^\./, '');
}

function baseName(path: string): string {
  return (path.split(/[\\/]/).pop() ?? '').toLowerCase();
}

/** Lenguaje de sintaxis de CodeMirror según la extensión. Otros → sin lenguaje. */
export function languageFor(path: string): Extension {
  const name = baseName(path);
  if (name === 'dockerfile' || name.startsWith('dockerfile.') || name.endsWith('.dockerfile')) return lang(dockerFile);
  if (name === '.env' || name.startsWith('.env.')) return lang(properties);
  if (name === '.bashrc' || name === '.zshrc' || name === '.profile' || name === 'mvnw') return lang(shell);
  switch (ext(path)) {
    case 'py':
    case 'pyi':
      return python();
    case 'js':
    case 'mjs':
    case 'cjs':
      return javascript();
    case 'jsx':
      return javascript({ jsx: true });
    case 'ts':
      return javascript({ typescript: true });
    case 'tsx':
      return javascript({ typescript: true, jsx: true });
    case 'rs':
      return rust();
    case 'java':
      return java();
    case 'json':
    case 'jsonc':
      return json();
    case 'yaml':
    case 'yml':
      return yaml();
    case 'md':
    case 'markdown':
      return markdown();
    case 'xml':
    case 'xsd':
    case 'xsl':
    case 'svg':
    case 'plist':
    case 'pom':
      return xml();
    case 'html':
    case 'htm':
      return html();
    case 'css':
      return css();
    case 'scss':
      return sass();
    case 'sass':
      return sass({ indented: true });
    case 'sql':
      return sql();
    case 'go':
      return go();
    case 'toml':
      return lang(toml);
    case 'properties':
    case 'env':
      return lang(properties);
    case 'sh':
    case 'bash':
    case 'zsh':
      return lang(shell);
    case 'dockerfile':
      return lang(dockerFile);
    case 'log':
      return lang(logParser);
    default:
      return [];
  }
}

/** Lenguaje LSP (mismo mapeo que `language_for` en Rust) o `null` si no hay servidor posible. */
export function lspLanguage(path: string): 'python' | 'typescript' | 'rust' | 'java' | null {
  switch (ext(path)) {
    case 'py':
    case 'pyi':
      return 'python';
    case 'ts':
    case 'tsx':
    case 'js':
    case 'jsx':
    case 'mjs':
    case 'cjs':
      return 'typescript';
    case 'rs':
      return 'rust';
    case 'java':
      return 'java';
    default:
      return null;
  }
}
