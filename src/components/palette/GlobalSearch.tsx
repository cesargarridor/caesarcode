import { useEffect, useMemo, useRef, useState } from 'react';
import { CaseSensitive, ChevronDown, ChevronRight, Regex, WholeWord } from 'lucide-react';
import { TextInput } from '../ui/TextInput';
import { IconButton } from '../ui/IconButton';
import { searchGlobal } from '../../lib/ipc';
import { toMessage } from '../../lib/errors';
import { useWorkspace } from '../../store/workspace';
import { useEditor } from '../../store/editor';
import { useUi } from '../../store/ui';
import type { SearchMatch, SearchResult } from '../../lib/types';
import { FileIcon } from '../icons/SymbolIcon';
import { basename } from '../../lib/paths';

interface Group { path: string; relPath: string; root: string; matches: SearchMatch[] }

function Preview({ m }: { m: SearchMatch }) {
  const p = m.preview;
  // Los offsets son UTF-16, igual que los índices de string en JS.
  return (
    <span className="whitespace-pre font-mono text-[11px] text-muted">
      {p.slice(0, m.matchStart).trimStart()}
      <mark className="rounded-sm bg-selection text-accent">{p.slice(m.matchStart, m.matchEnd)}</mark>
      {p.slice(m.matchEnd)}
    </span>
  );
}

export function GlobalSearch() {
  const root = useWorkspace((s) => s.root);
  const multiRoot = useWorkspace((s) => s.roots.length > 1);
  const focusTick = useUi((s) => s.searchFocusTick);
  const openFileAt = useEditor((s) => s.openFileAt);
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState('');
  const [include, setInclude] = useState('');
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [wholeWord, setWholeWord] = useState(false);
  const [regex, setRegex] = useState(false);
  const [result, setResult] = useState<SearchResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [collapsed, setCollapsed] = useState<Record<string, true>>({});
  const seq = useRef(0);

  useEffect(() => {
    if (focusTick > 0) {
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [focusTick]);

  // Al cambiar de workspace se descartan los resultados anteriores.
  useEffect(() => {
    setResult(null);
    setError(null);
    setCollapsed({});
  }, [root]);

  useEffect(() => {
    const id = ++seq.current;
    if (!root || !query) {
      setResult(null);
      setError(null);
      setBusy(false);
      return;
    }
    setBusy(true);
    const t = setTimeout(async () => {
      try {
        const res = await searchGlobal(query, { caseSensitive, regex, wholeWord, include: include || null });
        if (id !== seq.current || res.cancelled) return;
        setResult(res);
        setError(null);
        setCollapsed({});
      } catch (e) {
        if (id !== seq.current) return;
        setResult(null);
        setError(toMessage(e));
      } finally {
        if (id === seq.current) setBusy(false);
      }
    }, 250);
    return () => clearTimeout(t);
  }, [query, include, caseSensitive, wholeWord, regex, root]);

  const groups = useMemo<Group[]>(() => {
    const map = new Map<string, Group>();
    for (const m of result?.matches ?? []) {
      let g = map.get(m.path);
      if (!g) {
        g = { path: m.path, relPath: m.relPath, root: m.root, matches: [] };
        map.set(m.path, g);
      }
      g.matches.push(m);
    }
    return [...map.values()];
  }, [result]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-col gap-1.5 p-2">
        <div className="flex items-center gap-1">
          <TextInput
            ref={inputRef}
            value={query}
            placeholder="Buscar"
            onChange={(e) => setQuery(e.target.value)}
          />
          <IconButton label="Distinguir mayúsculas" active={caseSensitive} onClick={() => setCaseSensitive((v) => !v)}>
            <CaseSensitive size={14} />
          </IconButton>
          <IconButton label="Palabra completa" active={wholeWord} onClick={() => setWholeWord((v) => !v)}>
            <WholeWord size={14} />
          </IconButton>
          <IconButton label="Expresión regular" active={regex} onClick={() => setRegex((v) => !v)}>
            <Regex size={14} />
          </IconButton>
        </div>
        <TextInput value={include} placeholder="Incluir (p. ej. *.ts, src/**)" onChange={(e) => setInclude(e.target.value)} />
      </div>

      <div className="min-h-0 flex-1 overflow-auto pb-6">
        {error && <div className="break-words p-3 font-mono text-[11px] text-stop-text">{error}</div>}
        {!error && query && result && (
          <div className="px-3 py-1.5 text-[11px] text-muted">
            {result.matches.length} resultado{result.matches.length === 1 ? '' : 's'} en {groups.length} archivo
            {groups.length === 1 ? '' : 's'}
            {result.truncated && ' (resultados limitados)'}
            {busy && ' …'}
          </div>
        )}
        {!error && query && !result && busy && <div className="px-3 py-2 text-ui text-muted">Buscando…</div>}
        {!error && query && result && result.matches.length === 0 && !busy && (
          <div className="px-3 py-2 text-ui text-muted">Sin resultados.</div>
        )}
        {groups.map((g) => {
          const closed = !!collapsed[g.path];
          return (
            <div key={g.path}>
              <button
                type="button"
                onClick={() =>
                  setCollapsed((c) => {
                    const n = { ...c };
                    if (n[g.path]) delete n[g.path];
                    else n[g.path] = true;
                    return n;
                  })
                }
                className="flex h-6 w-full items-center gap-1 px-2 text-left outline-none hover:bg-hover"
              >
                {closed ? <ChevronRight size={12} className="shrink-0 text-muted" /> : <ChevronDown size={12} className="shrink-0 text-muted" />}
                <FileIcon filename={basename(g.path)} path={g.path} />
                <span className="min-w-0 flex-1 truncate font-mono text-[12px] text-fg" title={g.path}>
                  {g.relPath}
                </span>
                {multiRoot && g.root && <span className="shrink-0 rounded bg-hover px-1.5 font-mono text-[10px] text-muted">{g.root}</span>}
                <span className="shrink-0 rounded bg-hover px-1.5 font-mono text-[10px] text-muted">{g.matches.length}</span>
              </button>
              {!closed &&
                g.matches.map((m, i) => (
                  <button
                    key={`${m.line}:${i}`}
                    type="button"
                    onClick={() => void openFileAt(m.path, m.line, m.column)}
                    className="flex h-5 w-full items-center gap-2 overflow-hidden pl-6 pr-2 text-left outline-none hover:bg-hover"
                    title={`${m.relPath}:${m.line}`}
                  >
                    <span className="w-8 shrink-0 text-right font-mono text-[10px] text-muted">{m.line}</span>
                    <span className="min-w-0 overflow-hidden">
                      <Preview m={m} />
                    </span>
                  </button>
                ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}
