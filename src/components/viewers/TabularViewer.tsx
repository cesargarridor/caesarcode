import { useEffect, useMemo, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { readTabularFile } from '../../lib/ipc';
import { copyText } from '../../lib/clipboard';
import { toMessage } from '../../lib/errors';
import { extOf } from '../../lib/paths';
import { useUi } from '../../store/ui';
import type { TabularData } from '../../lib/types';

const LIMIT = 5000;
const ROW_H = 24;
const COL_W = 160;
const NUM_W = 56;

function fmtSize(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

function toText(d: TabularData): string {
  const sep = d.delimiter ?? ',';
  return [d.headers, ...d.rows].map((r) => r.join(sep)).join('\n');
}

export function TabularViewer({ path }: { path: string }) {
  const [data, setData] = useState<TabularData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sheet, setSheet] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [plain, setPlain] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const isText = ['csv', 'tsv'].includes((extOf(path) ?? '').toLowerCase());

  useEffect(() => {
    let cancelled = false;
    setError(null);
    readTabularFile(path, sheet, LIMIT)
      .then((d) => {
        if (!cancelled) setData(d);
      })
      .catch((e) => {
        if (!cancelled) setError(toMessage(e));
      });
    return () => {
      cancelled = true;
    };
  }, [path, sheet]);

  const rows = useMemo(() => {
    if (!data) return [];
    const q = query.trim().toLowerCase();
    if (!q) return data.rows;
    return data.rows.filter((r) => r.some((c) => c.toLowerCase().includes(q)));
  }, [data, query]);

  const text = useMemo(() => (plain && data ? toText(data) : ''), [plain, data]);
  const cols = data?.headers.length ?? 0;

  const rowVirt = useVirtualizer({ count: rows.length, getScrollElement: () => scrollRef.current, estimateSize: () => ROW_H, overscan: 15 });
  const colVirt = useVirtualizer({
    horizontal: true,
    count: cols,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => COL_W,
    overscan: 3,
  });

  const copyCell = (v: string) => {
    copyText(v)
      .then(() => useUi.getState().pushToast('Celda copiada'))
      .catch((e) => useUi.getState().pushToast(toMessage(e)));
  };

  if (error) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-1 bg-base">
        <div className="text-ui-lg text-fg">No se puede leer la tabla</div>
        <div className="max-w-[80%] truncate font-mono text-[11px] text-muted">{error}</div>
      </div>
    );
  }
  if (!data) return <div className="flex h-full items-center justify-center bg-base text-ui text-muted">Cargando…</div>;

  const totalW = NUM_W + colVirt.getTotalSize();
  const truncatedRows = data.totalRows > data.rows.length;

  return (
    <div className="flex h-full flex-col bg-base">
      <div className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-1.5">
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Filtrar filas…"
          disabled={plain}
          className="w-64 rounded-md border border-border bg-panel px-2 py-1 text-ui text-fg outline-none placeholder:text-faint focus:border-accent disabled:opacity-50"
        />
        <span className="text-ui text-muted">
          {plain ? '' : `${rows.length.toLocaleString()} de ${data.totalRows.toLocaleString()} filas`}
          {truncatedRows && ` (se muestran las primeras ${data.rows.length.toLocaleString()})`} · {fmtSize(data.fileSize)}
        </span>
        {isText && (
          <div className="ml-auto flex overflow-hidden rounded-md border border-border text-ui">
            {([false, true] as const).map((v) => (
              <button
                key={String(v)}
                type="button"
                onClick={() => setPlain(v)}
                className={`px-2.5 py-1 ${plain === v ? 'bg-accent-soft text-accent-text' : 'text-muted hover:bg-hover'}`}
              >
                {v ? 'Texto' : 'Tabla'}
              </button>
            ))}
          </div>
        )}
      </div>

      {plain ? (
        <pre className="min-h-0 flex-1 overflow-auto p-3 font-mono text-[12px] text-fg">{text}</pre>
      ) : (
        <div ref={scrollRef} className="min-h-0 flex-1 overflow-auto font-mono text-[12px]">
          <div style={{ width: totalW, minWidth: '100%' }}>
            <div className="sticky top-0 z-10 flex border-b border-border-strong bg-panel" style={{ height: ROW_H, width: totalW }}>
              <div className="shrink-0 border-r border-border px-2 leading-6 text-faint" style={{ width: NUM_W }}>#</div>
              <div className="relative" style={{ width: colVirt.getTotalSize() }}>
                {colVirt.getVirtualItems().map((c) => (
                  <div
                    key={c.key}
                    title={data.headers[c.index]}
                    className="absolute top-0 truncate border-r border-border px-2 leading-6 text-fg-strong"
                    style={{ left: c.start, width: c.size, height: ROW_H }}
                  >
                    {data.headers[c.index]}
                  </div>
                ))}
              </div>
            </div>
            <div className="relative" style={{ height: rowVirt.getTotalSize(), width: totalW }}>
              {rowVirt.getVirtualItems().map((r) => {
                const row = rows[r.index];
                return (
                  <div
                    key={r.key}
                    className="absolute left-0 flex border-b border-border hover:bg-hover"
                    style={{ top: r.start, height: ROW_H, width: totalW }}
                  >
                    <div className="shrink-0 border-r border-border px-2 leading-6 text-faint" style={{ width: NUM_W }}>
                      {r.index + 1}
                    </div>
                    <div className="relative" style={{ width: colVirt.getTotalSize() }}>
                      {colVirt.getVirtualItems().map((c) => (
                        <div
                          key={c.key}
                          onClick={() => copyCell(row[c.index] ?? '')}
                          title={row[c.index] ?? ''}
                          className="absolute top-0 cursor-pointer truncate border-r border-border px-2 leading-6 text-fg"
                          style={{ left: c.start, width: c.size, height: ROW_H }}
                        >
                          {row[c.index] ?? ''}
                        </div>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      )}

      {data.sheets.length > 0 && (
        <div role="tablist" className="flex shrink-0 gap-1 overflow-x-auto border-t border-border bg-panel px-2 py-1">
          {data.sheets.map((s) => (
            <button
              key={s}
              type="button"
              role="tab"
              aria-selected={s === (data.sheet ?? data.sheets[0])}
              onClick={() => setSheet(s)}
              className={`shrink-0 rounded-md px-2.5 py-0.5 text-ui ${
                s === (data.sheet ?? data.sheets[0]) ? 'bg-accent-soft text-accent-text' : 'text-muted hover:bg-hover'
              }`}
            >
              {s}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
