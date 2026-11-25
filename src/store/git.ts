import { create } from 'zustand';
import {
  gitApplyPatch, gitBranch, gitCheckoutCommit, gitCherryPick, gitCommit, gitCommitDetail, gitConflict, gitDiff,
  gitCancel, gitDiscard, gitInit, gitLog, gitMerge, gitRebase, gitRefs, gitRemote, gitRepoInfo, gitReset, gitRevert,
  gitSequence, gitStage, gitStash, gitStatusV2, gitTag, gitUnstage, gitRepos, gitRepoSummary, gitReposSummary,
} from '../lib/ipc';
import { repoFor } from '../lib/paths';
import { pickActiveRepo, pullSkipReason, runQueue, summarizeJobs, type RepoJobResult } from '../lib/repoQueue';
import { toMessage } from '../lib/errors';
import { layoutGraph, type GraphRow, type GraphState } from '../lib/gitGraph';
import { hunkPatch } from '../lib/gitDiff';
import type {
  BranchAction, CommitDetail, CommitRow, DiffTarget, FileDiff, GitConflictAction, GitProgress, GitRefs, GitResetMode,
  GitSeqAction, GitSeqOp, OpOutcome, RemoteOp, RepoEntry, RepoInfo, RepoSummary, StashAction, StatusV2, TagAction,
} from '../lib/types';
import { useUi } from './ui';
import { useWorkspace } from './workspace';

export const PAGE = 500;

export type GitSelection = { type: 'wip' } | { type: 'commit'; sha: string };
export type DiffSource = 'unstaged' | 'staged' | 'untracked' | 'conflict' | 'commit';
export type DiffMode = 'unified' | 'split';
export interface DiffState {
  source: DiffSource;
  target: DiffTarget;
  path: string;
  /** Ruta anterior (renombrados) para el lado izquierdo del split. */
  oldPath: string | null;
  /** Commit padre (lado izquierdo del split en un commit). */
  parent: string | null;
  data: FileDiff | null;
  loading: boolean;
  error: string | null;
}

export type GitViewMode = 'repo' | 'all';
export interface BulkJob { kind: 'fetch' | 'pull'; running: boolean; total: number; done: number; results: RepoJobResult[] }

const ACTIVE_KEY = 'cc.activeRepo:';
const SUMMARY_TTL = 10_000;

interface GitStore {
  /** Repos detectados en todas las raíces del workspace. */
  repos: RepoEntry[];
  reposLoaded: boolean;
  reposTruncated: boolean;
  summaries: Record<string, RepoSummary>;
  /** Repo (toplevel) sobre el que operan grafo, cambios, commit, ramas…; `null` si no hay repos. */
  activeRepo: string | null;
  view: GitViewMode;
  busyByRepo: Record<string, string>;
  remoteJobs: Record<string, string>;
  bulk: BulkJob | null;
  loadRepos: () => Promise<void>;
  loadSummaries: (force?: boolean) => Promise<void>;
  refreshSummary: (repo: string) => Promise<void>;
  setActiveRepo: (top: string | null) => Promise<void>;
  setView: (v: GitViewMode) => void;
  openInGit: (path: string) => void;
  /** Activa el repo dueño de `path` (sin cambiar de vista). `false` si no pertenece a ninguno. */
  activateRepoFor: (path: string) => Promise<boolean>;
  /** Abre el historial de un archivo en la vista Git (activando su repo). */
  showFileHistory: (path: string) => void;
  fetchAll: () => Promise<void>;
  pullAll: () => Promise<void>;
  /** Cuántas vistas Git (GitView, paneles) están montadas; solo entonces se refresca todo. */
  active: number;
  setActive: (delta: 1 | -1) => void;
  info: RepoInfo | null;
  status: StatusV2 | null;
  refs: GitRefs | null;
  commits: CommitRow[];
  rows: GraphRow[];
  graphState: GraphState | null;
  hasMore: boolean;
  loadingLog: boolean;
  search: string;
  selected: GitSelection | null;
  detail: CommitDetail | null;
  diff: DiffState | null;
  diffMode: DiffMode;
  /** Historial de archivo abierto (ruta relativa al top) o null. */
  fileHistory: string | null;
  busy: string | null;
  progress: GitProgress[];
  remoteJob: string | null;
  error: string | null;

  init: () => Promise<void>;
  reset: () => void;
  refreshInfo: () => Promise<void>;
  refreshAll: () => Promise<void>;
  refreshSoft: (repo?: string | null) => void;
  reloadLog: () => Promise<void>;
  loadMore: () => Promise<void>;
  setSearch: (q: string) => void;
  selectWip: () => void;
  selectCommit: (sha: string) => Promise<void>;
  openDiff: (source: DiffSource, path: string, extra?: { sha?: string; oldPath?: string | null; parent?: string | null }) => Promise<void>;
  closeDiff: () => void;
  setDiffMode: (m: DiffMode) => void;
  openFileHistory: (path: string | null) => void;

  initRepo: (path: string) => Promise<void>;
  stage: (paths: string[]) => Promise<void>;
  unstage: (paths: string[]) => Promise<void>;
  discard: (paths: string[], untracked: boolean) => Promise<void>;
  applyHunk: (mode: 'stage' | 'unstage' | 'discard', index: number) => Promise<void>;
  commit: (message: string, amend: boolean, signOff: boolean) => Promise<boolean>;
  branch: (action: BranchAction) => Promise<boolean>;
  checkoutCommit: (sha: string) => Promise<void>;
  merge: (name: string, noFf?: boolean) => Promise<void>;
  rebase: (onto: string) => Promise<void>;
  cherryPick: (shas: string[]) => Promise<void>;
  revert: (sha: string) => Promise<void>;
  resetTo: (sha: string, mode: GitResetMode) => Promise<void>;
  sequence: (op: GitSeqOp, action: GitSeqAction) => Promise<void>;
  conflict: (path: string, action: GitConflictAction) => Promise<void>;
  stash: (action: StashAction) => Promise<void>;
  tag: (action: TagAction) => Promise<boolean>;
  remote: (op: RemoteOp) => Promise<void>;
  cancelRemote: () => void;
}

let logToken = 0;
let diffToken = 0;
let detailToken = 0;
let refreshToken = 0;
let softTimer: ReturnType<typeof setTimeout> | null = null;
let refreshing: Promise<void> | null = null;
let refreshPending = false;
let lastLogSig = '';
const busyCounts = new Map<string, number>();
let summariesAt = 0;
let summariesInflight: Promise<void> | null = null;
const summaryTimers = new Map<string, ReturnType<typeof setTimeout>>();
let loadReposToken = 0;

function persistedRepo(primary: string | null): string | null {
  if (!primary) return null;
  try { return localStorage.getItem(ACTIVE_KEY + primary); } catch { return null; }
}
function persistRepo(primary: string | null, top: string | null): void {
  if (!primary) return;
  try {
    if (top) localStorage.setItem(ACTIVE_KEY + primary, top);
    else localStorage.removeItem(ACTIVE_KEY + primary);
  } catch { /* almacenamiento no disponible */ }
}

/** Firma de lo que, si cambia, invalida el grafo (HEAD + todas las refs). */
function logSignature(info: RepoInfo | null, refs: GitRefs | null): string {
  if (!info || !refs) return '';
  const parts: string[] = [info.head ?? '', info.branch ?? ''];
  for (const b of refs.local) parts.push(b.sha);
  for (const b of refs.remote) parts.push(b.sha);
  for (const t of refs.tags) parts.push(t.sha);
  for (const s of refs.stashes) parts.push(s.sha);
  return parts.join('|');
}

const toast = (m: string, kind: 'error' | 'info' = 'error') => useUi.getState().pushToast(m, kind);

export const useGit = create<GitStore>((set, get) => {
  const active = () => get().activeRepo;

  /** Sincroniza `busy`/`remoteJob`/`progress` (vista del repo activo) con los mapas por repo. */
  function syncActive(): void {
    const a = get().activeRepo;
    set((s) => ({
      busy: a ? s.busyByRepo[a] ?? null : null,
      remoteJob: a ? s.remoteJobs[a] ?? null : null,
      progress: a ? progressByRepo.get(a) ?? [] : [],
    }));
  }
  const progressByRepo = new Map<string, GitProgress[]>();
  const incBusy = (repo: string, label: string) => {
    busyCounts.set(repo, (busyCounts.get(repo) ?? 0) + 1);
    set((s) => ({ busyByRepo: { ...s.busyByRepo, [repo]: label } }));
    syncActive();
  };
  const decBusy = (repo: string) => {
    const n = (busyCounts.get(repo) ?? 1) - 1;
    if (n <= 0) {
      busyCounts.delete(repo);
      set((s) => {
        const b = { ...s.busyByRepo };
        delete b[repo];
        return { busyByRepo: b };
      });
    } else busyCounts.set(repo, n);
    syncActive();
  };

  /** Ejecuta una mutación sobre `repo`: marca ocupado, avisa de errores/conflictos y refresca una sola vez. */
  async function run(label: string, fn: (repo: string) => Promise<OpOutcome>, okMessage?: string): Promise<OpOutcome | null> {
    const repo = active();
    if (!repo) return null;
    incBusy(repo, label);
    let out: OpOutcome | null = null;
    try {
      out = await fn(repo);
      if (out.conflicts.length > 0) toast(`Conflictos en ${out.conflicts.length} archivo(s). Resuélvelos y continúa.`, 'info');
      else if (!out.ok) toast(out.message || `${label} falló`);
      else if (okMessage) toast(okMessage, 'info');
    } catch (e) {
      toast(toMessage(e));
    } finally {
      decBusy(repo);
    }
    if (active() === repo) await get().refreshAll();
    void get().refreshSummary(repo);
    return out;
  }

  async function loadDiff(repo: string, d: DiffState, token: number) {
    try {
      const data = await gitDiff(repo, d.target, 3);
      if (token !== diffToken) return;
      set((s) => (s.diff ? { diff: { ...s.diff, data, loading: false, error: null } } : {}));
    } catch (e) {
      if (token !== diffToken) return;
      set((s) => (s.diff ? { diff: { ...s.diff, data: null, loading: false, error: toMessage(e) } } : {}));
    }
  }

  const clearRepoState = () => ({
    info: null, status: null, refs: null, commits: [], rows: [], graphState: null, hasMore: false,
    search: '', selected: { type: 'wip' } as GitSelection, detail: null, diff: null, fileHistory: null, error: null,
  });

  async function doRefreshAll(): Promise<void> {
    const repo = active();
    if (!repo) return;
    const token = ++refreshToken;
    try {
      const [status, refs0] = await Promise.all([gitStatusV2(repo), gitRefs(repo).catch(() => null)]);
      if (token !== refreshToken || active() !== repo) return;
      if (!status.info.isRepo || !refs0) {
        set({ status, info: status.info, refs: null, commits: [], rows: [], graphState: null, hasMore: false, error: null });
        lastLogSig = '';
        return;
      }
      const refs = refs0;
      const sig = logSignature(status.info, refs);
      set({ status, info: status.info, refs, error: null });
      if (sig !== lastLogSig) {
        lastLogSig = sig;
        await get().reloadLog();
        const sel = get().selected;
        if (sel?.type === 'commit') void get().selectCommit(sel.sha);
      }
      // Diff abierto: se recarga solo si el archivo sigue en su lista.
      const d = get().diff;
      if (d && d.source !== 'commit') {
        const list =
          d.source === 'staged' ? status.staged : d.source === 'unstaged' ? status.unstaged
          : d.source === 'untracked' ? status.untracked : status.conflicted;
        if (!list.some((e) => e.path === d.path)) set({ diff: null });
        else void loadDiff(repo, d, ++diffToken);
      }
    } catch (e) {
      if (token === refreshToken) set({ error: toMessage(e) });
    }
  }

  return {
    repos: [],
    reposLoaded: false,
    reposTruncated: false,
    summaries: {},
    activeRepo: null,
    view: 'repo',
    busyByRepo: {},
    remoteJobs: {},
    bulk: null,

    active: 0,
    setActive: (delta) => set((s) => ({ active: Math.max(0, s.active + delta) })),
    info: null,
    status: null,
    refs: null,
    commits: [],
    rows: [],
    graphState: null,
    hasMore: false,
    loadingLog: false,
    search: '',
    selected: { type: 'wip' },
    detail: null,
    diff: null,
    diffMode: 'unified',
    fileHistory: null,
    busy: null,
    progress: [],
    remoteJob: null,
    error: null,

    reset: () => {
      logToken++; diffToken++; detailToken++; refreshToken++; loadReposToken++;
      lastLogSig = '';
      summariesAt = 0;
      set({
        ...clearRepoState(), repos: [], reposLoaded: false, reposTruncated: false, summaries: {}, activeRepo: null,
        view: 'repo', busyByRepo: {}, remoteJobs: {}, bulk: null, busy: null, progress: [], remoteJob: null,
      });
    },

    init: async () => {
      get().reset();
      await get().loadRepos();
    },

    loadRepos: async () => {
      const token = ++loadReposToken;
      const ws = useWorkspace.getState();
      if (!ws.root) return;
      try {
        const res = await gitRepos();
        if (token !== loadReposToken) return;
        const cur = get().activeRepo;
        const next = pickActiveRepo(res.repos, ws.root, persistedRepo(ws.root), cur, repoFor);
        set({ repos: res.repos, reposTruncated: res.truncated, reposLoaded: true });
        useWorkspace.getState().refreshGit();
        if (next !== cur) await get().setActiveRepo(next);
        else if (get().active > 0) void get().refreshAll();
        else void get().refreshInfo();
        void get().loadSummaries(true);
      } catch (e) {
        if (token !== loadReposToken) return;
        set({ reposLoaded: true, error: toMessage(e) });
      }
    },

    loadSummaries: (force = false) => {
      if (summariesInflight) return summariesInflight;
      if (!force && Date.now() - summariesAt < SUMMARY_TTL) return Promise.resolve();
      summariesInflight = (async () => {
        try {
          const list = await gitReposSummary();
          const map: Record<string, RepoSummary> = {};
          for (const r of list) map[r.top] = r;
          summariesAt = Date.now();
          set({ summaries: map });
        } catch { /* sin resumen: el selector se muestra sin estado */ } finally {
          summariesInflight = null;
        }
      })();
      return summariesInflight;
    },

    refreshSummary: async (repo) => {
      try {
        const sm = await gitRepoSummary(repo);
        set((s) => ({ summaries: { ...s.summaries, [repo]: sm } }));
      } catch { /* ignorado */ }
    },

    setActiveRepo: async (top) => {
      const ws = useWorkspace.getState();
      persistRepo(ws.root, top);
      logToken++; diffToken++; detailToken++; refreshToken++;
      lastLogSig = '';
      set({ ...clearRepoState(), activeRepo: top, view: 'repo' });
      syncActive();
      if (!top) return;
      if (get().active > 0) await get().refreshAll();
      else await get().refreshInfo();
    },

    setView: (v) => set({ view: v }),

    activateRepoFor: async (path) => {
      const { repos, activeRepo } = get();
      const r = repoFor(repos, path) ?? repos.find((x) => x.root === path) ?? null;
      if (!r) return false;
      if (r.top !== activeRepo) await get().setActiveRepo(r.top);
      return true;
    },

    openInGit: (path) => {
      const { repos, activeRepo } = get();
      const r = repoFor(repos, path) ?? repos.find((x) => x.root === path) ?? null;
      if (!r) {
        toast('Esa ruta no pertenece a ningún repositorio Git', 'info');
        return;
      }
      if (r.top !== activeRepo) void get().setActiveRepo(r.top);
      else set({ view: 'repo' });
      useUi.getState().setCenterView('git');
    },

    showFileHistory: (path) => {
      get().openInGit(path);
      get().openFileHistory(path);
    },

    refreshInfo: async () => {
      const repo = active();
      if (!repo) {
        set({ info: null });
        return;
      }
      try {
        const info = await gitRepoInfo(repo);
        if (active() === repo) set({ info });
      } catch {
        if (active() === repo) set({ info: null });
      }
    },

    refreshAll: () => {
      // Una sola refresco a la vez; si llega otro mientras tanto, se repite UNA vez al terminar.
      if (refreshing) {
        refreshPending = true;
        return refreshing;
      }
      refreshing = (async () => {
        try {
          do {
            refreshPending = false;
            await doRefreshAll();
          } while (refreshPending);
        } finally {
          refreshing = null;
        }
      })();
      return refreshing;
    },

    refreshSoft: (repo) => {
      const a = get().activeRepo;
      // Evento de otro repo: solo su resumen (selector / vista All).
      if (repo && repo !== a) {
        const prev = summaryTimers.get(repo);
        if (prev) clearTimeout(prev);
        summaryTimers.set(repo, setTimeout(() => {
          summaryTimers.delete(repo);
          if ((busyCounts.get(repo) ?? 0) === 0) void get().refreshSummary(repo);
        }, 600));
        return;
      }
      if (softTimer) clearTimeout(softTimer);
      softTimer = setTimeout(() => {
        softTimer = null;
        const s = get();
        const cur = s.activeRepo;
        if (!cur) {
          if (s.reposLoaded && s.repos.length === 0) void s.loadRepos();
          return;
        }
        if ((busyCounts.get(cur) ?? 0) > 0) return; // las mutaciones refrescan al terminar; evita bucles git↔watcher
        void s.refreshSummary(cur);
        if (s.active > 0) void s.refreshAll();
        else void s.refreshInfo(); // solo cabecera/estado de rama
      }, 300);
    },

    reloadLog: async () => {
      const repo = active();
      if (!repo) return;
      const token = ++logToken;
      set({ loadingLog: true });
      const { search, commits } = get();
      const limit = Math.min(2000, Math.max(PAGE, commits.length));
      try {
        const page = await gitLog(repo, { skip: 0, limit, all: true, path: null, search: search || null, author: null });
        if (token !== logToken) return;
        const { state, rows } = layoutGraph(null, page.commits);
        set({ commits: page.commits, rows, graphState: state, hasMore: page.hasMore, loadingLog: false });
      } catch (e) {
        if (token !== logToken) return;
        set({ loadingLog: false, error: toMessage(e) });
      }
    },

    loadMore: async () => {
      const repo = active();
      const { hasMore, loadingLog, commits, graphState, search } = get();
      if (!repo || !hasMore || loadingLog) return;
      const token = logToken;
      set({ loadingLog: true });
      try {
        const page = await gitLog(repo, { skip: commits.length, limit: PAGE, all: true, path: null, search: search || null, author: null });
        if (token !== logToken) return;
        const { state, rows } = layoutGraph(graphState, page.commits);
        set((s) => ({
          commits: s.commits.concat(page.commits),
          rows: s.rows.concat(rows),
          graphState: state,
          hasMore: page.hasMore,
          loadingLog: false,
        }));
      } catch (e) {
        if (token === logToken) set({ loadingLog: false, error: toMessage(e) });
      }
    },

    setSearch: (q) => {
      set({ search: q, commits: [] });
      void get().reloadLog();
    },

    selectWip: () => set({ selected: { type: 'wip' }, detail: null, diff: null }),

    selectCommit: async (sha) => {
      const repo = active();
      if (!repo) return;
      const token = ++detailToken;
      set((s) => ({
        selected: { type: 'commit', sha },
        detail: s.detail?.row.sha === sha ? s.detail : null,
        diff: s.selected?.type === 'commit' && s.selected.sha === sha ? s.diff : null,
      }));
      try {
        const detail = await gitCommitDetail(repo, sha);
        if (token === detailToken) set({ detail });
      } catch (e) {
        if (token === detailToken) toast(toMessage(e));
      }
    },

    openDiff: async (source, path, extra) => {
      const repo = active();
      if (!repo) return;
      const target: DiffTarget =
        source === 'staged' ? { type: 'index', path }
        : source === 'commit' ? { type: 'commit', sha: extra?.sha ?? '', path }
        : { type: 'worktree', path };
      const d: DiffState = {
        source, target, path, oldPath: extra?.oldPath ?? null, parent: extra?.parent ?? null,
        data: null, loading: true, error: null,
      };
      const token = ++diffToken;
      set({ diff: d });
      await loadDiff(repo, d, token);
    },

    closeDiff: () => {
      diffToken++;
      set({ diff: null });
    },
    setDiffMode: (m) => set({ diffMode: m }),
    openFileHistory: (path) => set({ fileHistory: path, diff: null }),

    initRepo: async (path) => {
      busyCounts.set(path, 1);
      set((s) => ({ busyByRepo: { ...s.busyByRepo, [path]: 'init' } }));
      try {
        const out = await gitInit(path);
        if (!out.ok) toast(out.message || 'init falló');
        else toast('Repositorio creado', 'info');
      } catch (e) {
        toast(toMessage(e));
      } finally {
        busyCounts.delete(path);
        set((s) => {
          const b = { ...s.busyByRepo };
          delete b[path];
          return { busyByRepo: b };
        });
      }
      await get().loadRepos();
    },
    stage: async (paths) => { await run('stage', (r) => gitStage(r, paths)); },
    unstage: async (paths) => { await run('unstage', (r) => gitUnstage(r, paths)); },
    discard: async (paths, untracked) => { await run('discard', (r) => gitDiscard(r, paths, untracked)); },

    applyHunk: async (mode, index) => {
      const d = get().diff;
      if (!d?.data) return;
      const patch = hunkPatch(d.data, index);
      if (!patch) return;
      const [cached, reverse] = mode === 'stage' ? [true, false] : mode === 'unstage' ? [true, true] : [false, true];
      await run(mode, (r) => gitApplyPatch(r, patch, cached, reverse));
    },

    commit: async (message, amend, signOff) => {
      const out = await run('commit', (r) => gitCommit(r, message, amend, signOff), amend ? 'Commit enmendado' : 'Commit creado');
      return !!out?.ok;
    },
    branch: async (action) => {
      const out = await run('branch', (r) => gitBranch(r, action));
      return !!out?.ok;
    },
    checkoutCommit: async (sha) => { await run('checkout', (r) => gitCheckoutCommit(r, sha)); },
    merge: async (name, noFf = false) => { await run('merge', (r) => gitMerge(r, name, noFf, false)); },
    rebase: async (onto) => { await run('rebase', (r) => gitRebase(r, onto)); },
    cherryPick: async (shas) => { await run('cherry-pick', (r) => gitCherryPick(r, shas)); },
    revert: async (sha) => { await run('revert', (r) => gitRevert(r, sha)); },
    resetTo: async (sha, mode) => { await run('reset', (r) => gitReset(r, sha, mode, mode === 'hard')); },
    sequence: async (op, action) => { await run(`${op} ${action}`, (r) => gitSequence(r, op, action)); },
    conflict: async (path, action) => { await run('conflict', (r) => gitConflict(r, path, action)); },
    stash: async (action) => { await run('stash', (r) => gitStash(r, action)); },
    tag: async (action) => {
      const out = await run('tag', (r) => gitTag(r, action));
      return !!out?.ok;
    },

    remote: async (op) => {
      const repo = active();
      if (!repo || get().remoteJobs[repo]) return;
      const jobId = `git-${Date.now().toString(36)}`;
      progressByRepo.set(repo, []);
      set((s) => ({ remoteJobs: { ...s.remoteJobs, [repo]: jobId } }));
      incBusy(repo, op.type);
      try {
        const out = await gitRemote(repo, op, jobId, (ev) => {
          progressByRepo.set(repo, (progressByRepo.get(repo) ?? []).concat(ev).slice(-200));
          if (active() === repo) syncActive();
        });
        if (!out.ok) toast(out.message || `${op.type} falló`);
      } catch (e) {
        toast(toMessage(e));
      } finally {
        decBusy(repo);
        set((s) => {
          const j = { ...s.remoteJobs };
          delete j[repo];
          return { remoteJobs: j };
        });
        syncActive();
      }
      if (active() === repo) {
        lastLogSig = ''; // los remotos mueven refs: fuerza recarga del grafo
        await get().refreshAll();
      }
      void get().refreshSummary(repo);
    },
    cancelRemote: () => {
      const a = active();
      const id = a ? get().remoteJobs[a] : null;
      if (id) void gitCancel(id).catch(() => undefined);
    },

    fetchAll: async () => {
      await bulk('fetch');
    },
    pullAll: async () => {
      await bulk('pull');
    },
  };

  async function bulk(kind: 'fetch' | 'pull'): Promise<void> {
    if (get().bulk?.running) return;
    await get().loadSummaries(true);
    const repos = get().repos;
    const sums = get().summaries;
    const results: RepoJobResult[] = [];
    const work: RepoEntry[] = [];
    for (const r of repos) {
      const reason = kind === 'pull' ? pullSkipReason(sums[r.top]) : null;
      if (reason) results.push({ top: r.top, status: 'skipped', message: reason });
      else if (get().remoteJobs[r.top]) results.push({ top: r.top, status: 'skipped', message: 'ocupado' });
      else work.push(r);
    }
    set({ bulk: { kind, running: true, total: repos.length, done: results.length, results: [...results] } });
    const bump = (res: RepoJobResult) => {
      results.push(res);
      set((s) => (s.bulk ? { bulk: { ...s.bulk, done: results.length, results: [...results] } } : {}));
    };
    await runQueue(work, 3, async (r) => {
      const jobId = `git-${kind}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
      incBusy(r.top, kind);
      set((s) => ({ remoteJobs: { ...s.remoteJobs, [r.top]: jobId } }));
      try {
        const op: RemoteOp = kind === 'fetch' ? { type: 'fetch', remote: null, prune: true } : { type: 'pull', rebase: false, ffOnly: true };
        const out = await gitRemote(r.top, op, jobId, () => undefined);
        bump({ top: r.top, status: out.ok ? 'ok' : 'error', message: out.ok ? '' : out.message });
      } catch (e) {
        bump({ top: r.top, status: 'error', message: toMessage(e) });
      } finally {
        decBusy(r.top);
        set((s) => {
          const j = { ...s.remoteJobs };
          delete j[r.top];
          return { remoteJobs: j };
        });
        syncActive();
      }
    });
    set((s) => (s.bulk ? { bulk: { ...s.bulk, running: false, done: s.bulk.total } } : {}));
    toast(`${kind === 'fetch' ? 'Fetch all' : 'Pull all'}: ${summarizeJobs(results)}`, results.some((x) => x.status === 'error') ? 'error' : 'info');
    lastLogSig = '';
    summariesAt = 0;
    void get().loadSummaries(true);
    if (get().activeRepo && get().active > 0) void get().refreshAll();
  }
});
