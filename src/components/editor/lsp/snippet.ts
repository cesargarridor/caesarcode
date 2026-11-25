// Convierte la sintaxis de snippets LSP (TextMate) a la plantilla de `snippet()` de CodeMirror.
//   LSP:  $1  ${1}  ${1:texto}  ${1|a,b|}  $0  $VAR  ${VAR:def}  \$ \} \\
//   CM:   ${1}  ${1:texto}  ${0}  llaves literales escapadas como \{ \}

interface Parsed { cm: string; plain: string }

const isDigit = (c: string | undefined) => c !== undefined && c >= '0' && c <= '9';
const isIdStart = (c: string | undefined) => c !== undefined && /[A-Za-z_]/.test(c);
const isIdChar = (c: string | undefined) => c !== undefined && /\w/.test(c);

export function lspSnippetToCm(src: string): string {
  let i = 0;
  // Texto por defecto ya visto de cada tabulador: los espejos `$1` posteriores lo repiten.
  const defaults = new Map<string, string>();

  const field = (n: string, text: string): Parsed => {
    if (n === '0') return { cm: '${0}', plain: '' };
    if (text) defaults.set(n, text);
    const shown = text || defaults.get(n) || '';
    const safe = shown.replace(/[{}]/g, ''); // el formato numerado de CM no admite llaves
    return { cm: safe ? `\${${n}:${safe}}` : `\${${n}}`, plain: shown };
  };

  const readDigits = () => {
    let n = '';
    while (isDigit(src[i])) n += src[i++];
    return n;
  };

  /** Salta hasta la `}` que cierra el nivel actual (para transformaciones que no soportamos). */
  const skipToClose = () => {
    let depth = 1;
    while (i < src.length && depth > 0) {
      const c = src[i++];
      if (c === '\\') i++;
      else if (c === '{') depth++;
      else if (c === '}') depth--;
    }
  };

  const dollar = (): Parsed => {
    // src[i] === '$'
    const next = src[i + 1];
    if (isDigit(next)) {
      i++;
      return field(readDigits(), '');
    }
    if (next === '{') {
      i += 2;
      if (isDigit(src[i])) {
        const n = readDigits();
        if (src[i] === '}') {
          i++;
          return field(n, '');
        }
        if (src[i] === ':') {
          i++;
          const inner = text(true);
          if (src[i] === '}') i++;
          return field(n, inner.plain);
        }
        if (src[i] === '|') {
          i++;
          let raw = '';
          while (i < src.length && !(src[i] === '|' && src[i + 1] === '}')) {
            if (src[i] === '\\') i++;
            raw += src[i++] ?? '';
          }
          i += 2;
          return field(n, raw.split(',')[0] ?? '');
        }
        skipToClose();
        return field(n, '');
      }
      if (isIdStart(src[i])) {
        while (isIdChar(src[i])) i++;
        if (src[i] === '}') {
          i++;
          return { cm: '', plain: '' };
        }
        if (src[i] === ':') {
          i++;
          const inner = text(true); // valor por defecto de la variable: se inserta como texto
          if (src[i] === '}') i++;
          return inner;
        }
      }
      skipToClose();
      return { cm: '', plain: '' };
    }
    if (isIdStart(next)) {
      i++;
      while (isIdChar(src[i])) i++;
      return { cm: '', plain: '' };
    }
    i++;
    return { cm: '$', plain: '$' };
  };

  const lit = (c: string): Parsed => ({ cm: c === '{' || c === '}' ? '\\' + c : c, plain: c });

  function text(insideBraces: boolean): Parsed {
    let cm = '';
    let plain = '';
    while (i < src.length) {
      const c = src[i];
      if (insideBraces && c === '}') break;
      let part: Parsed;
      if (c === '\\' && '$}\\'.includes(src[i + 1] ?? '\0')) {
        part = lit(src[i + 1]);
        i += 2;
      } else if (c === '$') {
        part = dollar();
      } else {
        part = lit(c);
        i++;
      }
      cm += part.cm;
      plain += part.plain;
    }
    return { cm, plain };
  }

  return text(false).cm;
}
