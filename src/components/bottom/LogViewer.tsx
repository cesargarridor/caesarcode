import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { ArrowDownToLine, Trash2 } from 'lucide-react';
import { Select } from '../ui/Select';
import { IconButton } from '../ui/IconButton';
import { TextInput } from '../ui/TextInput';
import { getLines, useLogsTick } from '../../store/logs';
import { useServices } from '../../store/services';
import { palette } from '../../lib/palette';
import type { LogLevel, LogLine } from '../../lib/types';

type LevelFilter = 'all' | 'error' | 'warn' | 'info';

const LEVEL_OPTIONS: { value: LevelFilter; label: string }[] = [
  { value: 'info', label: 'INFO+' },
  { value: 'warn', label: 'WARN+' },
  { value: 'error', label: 'ERROR' },
];
const RANK: Record<LogLevel, number> = { info: 0, warn: 1, error: 2 };
const LEVEL_MIN: Record<LevelFilter, number> = { all: 0, info: 0, warn: 1, error: 2 };

const LEVEL_COLOR: Record<LogLevel, string> = {
  error: 'text-stop-text',
  warn: 'text-accent',
  info: 'text-fg',
};

const PALETTE = palette.serviceColors;
function colorFor(id: string): string {
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) >>> 0;
  return PALETTE[h % PALETTE.length];
}

function hhmmss(ts: number): string {
  const d = new Date(ts);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

interface Props {
  /** Servicio a mostrar o `'all'` (con selector de servicio propio). */
  serviceId: string;
}

/** Visor de logs virtualizado con filtro de texto, nivel, follow y Clear. */
export function LogViewer({ serviceId }: Props) {
  const configs = useServices((s) => s.configs);
  const statuses = useServices((s) => s.statuses);
  const clearLogs = useServices((s) => s.clearLogs);
  const tick = useLogsTick((s) => s.tick);

  const [picked, setPicked] = useState<string>('all');
  const [text, setText] = useState('');
  const [query, setQuery] = useState('');
  const [level, setLevel] = useState<LevelFilter>('info');
  const [follow, setFollow] = useState(true);
  const scrollRef = useRef<HTMLDivElement>(null);

  const isAll = serviceId === 'all';
  const source = isAll ? picked : serviceId;
  const names = useMemo(() => {
    const m = new Map(configs.map((c) => [c.id, c.name]));
    // Servicios efímeros de Run (no están en configs): el nombre viene en el estado.
    for (const st of Object.values(statuses)) if (st.ephemeral && st.name && !m.has(st.id)) m.set(st.id, st.name);
    return m;
  }, [configs, statuses]);

  // Si el servicio elegido desaparece de la config, vuelve a "all".
  useEffect(() => {
    if (picked !== 'all' && !names.has(picked)) setPicked('all');
  }, [picked, names]);

  useEffect(() => {
    const t = setTimeout(() => setQuery(text.trim().toLowerCase()), 150);
    return () => clearTimeout(t);
  }, [text]);

  const lines: LogLine[] = useMemo(() => {
    void tick; // los buffers son mutables: el tick invalida
    const src = getLines(source);
    const min = LEVEL_MIN[level];
    if (!query && min === 0) return src;
    return src.filter((l) => RANK[l.level] >= min && (!query || l.text.toLowerCase().includes(query)));
  }, [tick, source, query, level]);

  const virt = useVirtualizer({
    count: lines.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 18,
    overscan: 20,
  });

  // Follow: pegado al final mientras el usuario no suba.
  useLayoutEffect(() => {
    if (!follow) return;
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines.length, tick, follow, source]);

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
    setFollow((f) => (f === atBottom ? f : atBottom));
  };

  const serviceOptions = useMemo(
    () => [{ value: 'all', label: 'All Services' }, ...configs.map((c) => ({ value: c.id, label: c.name }))],
    [configs],
  );

  return (
    <div className="flex h-full min-w-0 flex-col">
      <div className="flex h-10 shrink-0 items-center gap-2 px-2">
        {isAll && (
          <Select className="w-36 shrink-0" ariaLabel="Servicio" value={picked} options={serviceOptions} onChange={setPicked} />
        )}
        <TextInput
          className="max-w-[260px] flex-1"
          placeholder="Filtrar texto…"
          aria-label="Filtrar logs"
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
        <Select<LevelFilter>
          className="w-24 shrink-0"
          ariaLabel="Nivel"
          value={level}
          options={LEVEL_OPTIONS}
          onChange={setLevel}
        />
        <span className="ml-auto font-mono text-[11px] text-muted">
          {lines.length.toLocaleString()} líneas
        </span>
        <IconButton
          label={follow ? 'Follow activado' : 'Follow desactivado (ir al final)'}
          active={follow}
          onClick={() => {
            setFollow(true);
            const el = scrollRef.current;
            if (el) el.scrollTop = el.scrollHeight;
          }}
        >
          <ArrowDownToLine size={14} />
        </IconButton>
        <IconButton label="Limpiar logs" onClick={() => void clearLogs(source)}>
          <Trash2 size={14} />
        </IconButton>
      </div>
      <div ref={scrollRef} onScroll={onScroll} className="min-h-0 flex-1 overflow-auto font-mono text-[12px]">
        {lines.length === 0 ? (
          <div className="flex h-full items-center justify-center text-ui text-muted">Sin logs</div>
        ) : (
          <div style={{ height: virt.getTotalSize(), position: 'relative', width: '100%' }}>
            {virt.getVirtualItems().map((vi) => {
              const l = lines[vi.index];
              if (!l) return null;
              return (
                <div
                  key={l.seq}
                  title={l.text}
                  className="absolute left-0 flex w-full select-text items-center gap-2 whitespace-pre px-2 hover:bg-hover"
                  style={{ height: vi.size, transform: `translateY(${vi.start}px)`, lineHeight: '18px' }}
                >
                  <span className="shrink-0 text-muted">{hhmmss(l.ts)}</span>
                  {isAll && source === 'all' && (
                    <span className="shrink-0 truncate" style={{ color: colorFor(l.serviceId), maxWidth: 120 }}>
                      {names.get(l.serviceId) ?? l.serviceId}
                    </span>
                  )}
                  <span className={`min-w-0 flex-1 truncate ${LEVEL_COLOR[l.level]}`}>{l.text}</span>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
