import { memo, useEffect, useMemo, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Search, X } from 'lucide-react';
import { useGit } from '../../store/git';
import { laneCount, type GraphRow } from '../../lib/gitGraph';
import { laneColors } from '../../lib/palette';
import type { CommitRow, RefLabel } from '../../lib/types';
import { ContextMenu, type MenuItem } from '../ui/ContextMenu';
import { TextInput } from '../ui/TextInput';
import { IconButton } from '../ui/IconButton';
import { commitMenu, branchMenu } from './gitActions';
import { fullDate, hashColor, initials, relTime, shortSha } from './gitUtils';

const ROW_H = 26;
const LANE_W = 14;
const MAX_LANES = 14;
const PAD = 8;

const laneX = (l: number) => PAD + l * LANE_W;

function edgePath(e: GraphRow['edges'][number], lane: number): string {
  const mid = ROW_H / 2;
  if (e.type === 'pass') return `M${laneX(e.fromLane)} 0V${ROW_H}`;
  if (e.type === 'merge') {
    const x0 = laneX(e.fromLane);
    const x1 = laneX(lane);
    return x0 === x1 ? `M${x0} 0V${mid}` : `M${x0} 0C${x0} ${mid} ${x1} 0 ${x1} ${mid}`;
  }
  const x0 = laneX(lane);
  const x1 = laneX(e.toLane);
  return x0 === x1 ? `M${x0} ${mid}V${ROW_H}` : `M${x0} ${mid}C${x0} ${ROW_H} ${x1} ${mid} ${x1} ${ROW_H}`;
}

/** SVG mínimo por fila: aristas de colores + nodo (merge = anillo). */
const GraphCell = memo(function GraphCell({ row, width, isHead }: { row: GraphRow; width: number; isHead: boolean }) {
  const color = laneColors[row.color];
  const cx = laneX(row.lane);
  return (
    <svg width={width} height={ROW_H} className="shrink-0" style={{ overflow: 'hidden' }}>
      {row.edges.map((e, i) => (
        <path key={i} d={edgePath(e, row.lane)} stroke={laneColors[e.color]} strokeWidth={2} fill="none" strokeLinecap="round" />
      ))}
      {isHead && <circle cx={cx} cy={ROW_H / 2} r={7} fill="none" stroke={color} strokeOpacity={0.5} strokeWidth={1.5} />}
      {row.isMerge ? (
        <circle cx={cx} cy={ROW_H / 2} r={4} className="fill-canvas" stroke={color} strokeWidth={2} />
      ) : (
        <circle cx={cx} cy={ROW_H / 2} r={4.5} fill={color} />
      )}
    </svg>
  );
});

function chipStyle(r: RefLabel, color: string): { cls: string; style: React.CSSProperties } {
  if (r.kind === 'head') return { cls: 'bg-accent text-base font-medium', style: {} };
  if (r.kind === 'tag') return { cls: 'border border-gold/30 text-gold', style: {} };
  if (r.kind === 'stash') return { cls: 'border border-border-strong text-muted', style: {} };
  if (r.kind === 'remote') return { cls: 'border text-muted', style: { borderColor: color + '66' } };
  return { cls: 'border text-fg', style: { borderColor: color + 'aa', backgroundColor: color + '1f' } };
}

interface RowProps {
  c: CommitRow;
  g: GraphRow | undefined;
  width: number;
  selected: boolean;
  isHead: boolean;
  showGraph: boolean;
  onSelect: (sha: string) => void;
  onMenu: (e: React.MouseEvent, c: CommitRow, ref?: RefLabel) => void;
}

const CommitLine = memo(function CommitLine({ c, g, width, selected, isHead, showGraph, onSelect, onMenu }: RowProps) {
  const color = g ? laneColors[g.color] : '#f59e0b';
  return (
    <div
      onClick={() => onSelect(c.sha)}
      onContextMenu={(e) => onMenu(e, c)}
      className={`flex h-full cursor-pointer items-center text-ui-lg ${selected ? 'bg-accent/12' : 'hover:bg-hover'}`}
    >
      {showGraph && g && <GraphCell row={g} width={width} isHead={isHead} />}
      <div className="flex min-w-0 flex-1 items-center gap-1.5 pr-2">
        {c.refs.slice(0, 4).map((r) => {
          const st = chipStyle(r, color);
          return (
            <span
              key={r.kind + r.name}
              onContextMenu={(e) => onMenu(e, c, r)}
              style={st.style}
              className={`max-w-[140px] shrink-0 truncate rounded px-1.5 font-mono text-[10.5px] leading-[16px] ${st.cls}`}
              title={r.name}
            >
              {r.name}
            </span>
          );
        })}
        {c.refs.length > 4 && <span className="shrink-0 text-[10.5px] text-muted">+{c.refs.length - 4}</span>}
        <span className="truncate text-fg">{c.subject}</span>
      </div>
      <div className="flex w-[130px] shrink-0 items-center gap-1.5 pr-2" title={`${c.author} <${c.email}>`}>
        <span
          className="flex h-[18px] w-[18px] shrink-0 items-center justify-center rounded-full text-[9px] font-semibold text-base"
          style={{ backgroundColor: hashColor(c.email || c.author) }}
        >
          {initials(c.author)}
        </span>
        <span className="truncate text-muted">{c.author}</span>
      </div>
      <div className="w-[92px] shrink-0 truncate pr-2 text-muted" title={fullDate(c.time)}>{relTime(c.time)}</div>
      <div className="w-[64px] shrink-0 pr-2 font-mono text-[11px] text-muted">{shortSha(c.sha)}</div>
    </div>
  );
});

function WipLine({ count, selected, onSelect, headLane, width }: { count: number; selected: boolean; onSelect: () => void; headLane: number; width: number }) {
  const x = laneX(headLane);
  return (
    <div onClick={onSelect} className={`flex h-full cursor-pointer items-center text-ui-lg ${selected ? 'bg-accent/12' : 'hover:bg-hover'}`}>
      <svg width={width} height={ROW_H} className="shrink-0" style={{ overflow: 'hidden' }}>
        <path d={`M${x} ${ROW_H / 2}V${ROW_H}`} className="stroke-muted" strokeWidth={2} strokeDasharray="3 3" />
        <circle cx={x} cy={ROW_H / 2} r={4.5} className="fill-canvas stroke-muted" strokeWidth={2} strokeDasharray="3 2" />
      </svg>
      <span className="font-mono text-muted">// WIP</span>
      <span className={`ml-2 rounded px-1.5 text-[11px] ${count > 0 ? 'bg-accent/20 text-accent' : 'text-muted'}`}>
        {count > 0 ? `${count} cambio${count === 1 ? '' : 's'}` : 'sin cambios'}
      </span>
    </div>
  );
}

export function CommitGraph() {
  const commits = useGit((s) => s.commits);
  const rows = useGit((s) => s.rows);
  const hasMore = useGit((s) => s.hasMore);
  const loadingLog = useGit((s) => s.loadingLog);
  const selected = useGit((s) => s.selected);
  const search = useGit((s) => s.search);
  const info = useGit((s) => s.info);
  const status = useGit((s) => s.status);
  const [text, setText] = useState(search);
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Debounce de la búsqueda (no relanza git en cada tecla).
  useEffect(() => {
    if (text === search) return;
    const t = setTimeout(() => useGit.getState().setSearch(text.trim()), 350);
    return () => clearTimeout(t);
  }, [text, search]);

  const showGraph = !search;
  const lanes = useMemo(() => Math.min(laneCount(rows), MAX_LANES), [rows]);
  const width = PAD * 2 + lanes * LANE_W - (LANE_W - 8);

  const headIndex = useMemo(() => (info?.head ? commits.findIndex((c) => c.sha === info.head) : -1), [commits, info?.head]);
  const headLane = headIndex >= 0 && rows[headIndex] ? Math.min(rows[headIndex].lane, MAX_LANES - 1) : 0;
  const wipCount = status ? status.staged.length + status.unstaged.length + status.untracked.length + status.conflicted.length : 0;

  const count = commits.length + 1;
  const virt = useVirtualizer({ count, getScrollElement: () => scrollRef.current, estimateSize: () => ROW_H, overscan: 14 });
  const items = virt.getVirtualItems();
  const last = items.length ? items[items.length - 1].index : 0;
  useEffect(() => {
    if (hasMore && !loadingLog && last >= count - 25) void useGit.getState().loadMore();
  }, [last, count, hasMore, loadingLog]);

  // Selección en el grafo (p. ej. desde la sidebar): trae la fila a la vista.
  const selSha = selected?.type === 'commit' ? selected.sha : null;
  useEffect(() => {
    if (!selSha) return;
    const i = useGit.getState().commits.findIndex((c) => c.sha === selSha);
    if (i >= 0) virt.scrollToIndex(i + 1, { align: 'auto' });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selSha]);

  const onSelect = useMemo(() => (sha: string) => void useGit.getState().selectCommit(sha), []);
  const onMenu = useMemo(
    () => (e: React.MouseEvent, c: CommitRow, ref?: RefLabel) => {
      e.preventDefault();
      e.stopPropagation();
      const refs = useGit.getState().refs;
      let items: MenuItem[];
      const b = ref && (ref.kind === 'local' ? refs?.local.find((x) => x.name === ref.name) : ref.kind === 'remote' ? refs?.remote.find((x) => x.name === ref.name) : undefined);
      if (ref && b) items = branchMenu(b, ref.kind === 'remote');
      else if (ref?.kind === 'head') {
        const cur = refs?.local.find((x) => x.isHead);
        items = cur ? branchMenu(cur, false) : commitMenu(c);
      } else items = commitMenu(c);
      setMenu({ x: e.clientX, y: e.clientY, items });
    },
    [],
  );

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex h-9 shrink-0 items-center gap-2 px-2">
        <Search size={12} className="text-muted" />
        <TextInput
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="Buscar en mensajes de commit…"
          className="h-6 border-transparent bg-transparent"
        />
        {text && (
          <IconButton label="Limpiar" onClick={() => { setText(''); useGit.getState().setSearch(''); }}>
            <X size={12} />
          </IconButton>
        )}
        <span className="shrink-0 font-mono text-[11px] text-muted">{commits.length}{hasMore ? '+' : ''}</span>
      </div>
      <div ref={scrollRef} className="min-h-0 flex-1 overflow-auto">
        <div style={{ height: virt.getTotalSize(), position: 'relative' }}>
          {items.map((v) => (
            <div key={v.index} style={{ position: 'absolute', top: 0, left: 0, right: 0, height: ROW_H, transform: `translateY(${v.start}px)` }}>
              {v.index === 0 ? (
                <WipLine
                  count={wipCount}
                  selected={selected?.type === 'wip'}
                  onSelect={() => useGit.getState().selectWip()}
                  headLane={showGraph ? headLane : 0}
                  width={showGraph ? width : 22}
                />
              ) : (
                <CommitLine
                  c={commits[v.index - 1]}
                  g={rows[v.index - 1]}
                  width={width}
                  selected={selSha === commits[v.index - 1].sha}
                  isHead={commits[v.index - 1].sha === info?.head}
                  showGraph={showGraph}
                  onSelect={onSelect}
                  onMenu={onMenu}
                />
              )}
            </div>
          ))}
        </div>
        {loadingLog && commits.length === 0 && <div className="p-3 text-ui text-muted">Cargando historial…</div>}
        {!loadingLog && commits.length === 0 && <div className="p-3 text-ui text-muted">{search ? 'Sin resultados' : 'Sin commits todavía'}</div>}
      </div>
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
    </div>
  );
}
