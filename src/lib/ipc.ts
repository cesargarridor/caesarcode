import { Channel, invoke } from '@tauri-apps/api/core';
import type {
  AppInfo,
  BlameResult,
  BranchAction,
  CommitDetail,
  DiffTarget,
  FileDiff,
  GitConflictAction,
  GitProgress,
  GitRefs,
  GitResetMode,
  GitSeqAction,
  GitSeqOp,
  LogPage,
  LogQuery,
  OpOutcome,
  RemoteOp,
  RepoInfo,
  RepoSummary,
  ReposResult,
  RootInfo,
  StashAction,
  StatusV2,
  TagAction,
  FileContent,
  TabularData,
  FsEntry,
  FsEvent,
  SymbolInfo,
  CompletionDto,
  GenEvent,
  GeneratorInfo,
  GitStatus,
  HoverDto,
  LocationDto,
  LogLine,
  LspDocStatus,
  LspEvent,
  LspServerStatus,
  ProjectRequest,
  QuickOpenItem,
  RecentProject,
  ResolvedRun,
  RunConfigFile,
  RunStarted,
  RunTarget,
  RunTemplate,
  SearchOptions,
  SearchResult,
  ServiceConfig,
  ServiceStatus,
  SpringCatalog,
  TerminalEvent,
  ToolsReport,
  WorkspaceInfo,
} from './types';

// Wrappers tipados de `invoke`. Cada bloque añade los suyos.
// Para streaming: `const ch = new Channel<T>(); ch.onmessage = fn;` ANTES del invoke.

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>('app_info');
}

// ---- Workspace ----
export const workspaceOpen = (path: string) => invoke<WorkspaceInfo>('workspace_open', { path });
export const workspaceCurrent = () => invoke<WorkspaceInfo | null>('workspace_current');
export const workspaceRoots = () => invoke<RootInfo[]>('workspace_roots');
export const workspaceAddRoot = (path: string) => invoke<WorkspaceInfo>('workspace_add_root', { path });
export const workspaceRemoveRoot = (path: string) => invoke<WorkspaceInfo>('workspace_remove_root', { path });

// ---- Sistema de archivos ----
export const fsReadDir = (path: string) => invoke<FsEntry[]>('fs_read_dir', { path });
export const fsClassify = (paths: string[]) => invoke<[string, SymbolInfo][]>('fs_classify', { paths });
export const fsReadFile = (path: string) => invoke<FileContent>('fs_read_file', { path });
export const externalOpenDialog = () => invoke<string[]>('external_open_dialog');
export const readTabularFile = (path: string, sheet?: string | null, limit?: number) =>
  invoke<TabularData>('read_tabular_file', { path, sheet: sheet ?? null, limit: limit ?? null });
export const mediaAllow = (path: string) => invoke<string>('media_allow', { path });
export const fsWriteFile = (path: string, text: string) => invoke<void>('fs_write_file', { path, text });
export const fsCreateFile = (parent: string, name: string) => invoke<string>('fs_create_file', { parent, name });
export const fsCreateDir = (parent: string, name: string) => invoke<string>('fs_create_dir', { parent, name });
export const fsRename = (path: string, newName: string) => invoke<string>('fs_rename', { path, newName });
export const fsDelete = (path: string) => invoke<void>('fs_delete', { path });

/** Suscripción de slot único: una nueva llamada reemplaza a la anterior en Rust. */
export function fsWatchSubscribe(handler: (ev: FsEvent) => void): Promise<void> {
  const onEvent = new Channel<FsEvent>();
  onEvent.onmessage = handler;
  return invoke<void>('fs_watch_subscribe', { onEvent });
}

// ---- Git / búsqueda ----
// `repo` (toplevel del repo, ver `gitRepos`) es SIEMPRE el primer parámetro: así tsc encuentra cada llamada.
export const gitStatus = (repo: string) => invoke<GitStatus>('git_status', { repo });
export const gitRepos = () => invoke<ReposResult>('git_repos');
export const gitRepoSummary = (repo: string) => invoke<RepoSummary>('git_repo_summary', { repo });
export const gitReposSummary = () => invoke<RepoSummary[]>('git_repos_summary');

// ---- Git avanzado (C1) ----
export const gitRepoInfo = (repo: string) => invoke<RepoInfo>('git_repo_info', { repo });
export const gitStatusV2 = (repo: string) => invoke<StatusV2>('git_status_v2', { repo });
export const gitLog = (repo: string, query: LogQuery) => invoke<LogPage>('git_log', { repo, query });
export const gitRefs = (repo: string) => invoke<GitRefs>('git_refs', { repo });
export const gitCommitDetail = (repo: string, sha: string) => invoke<CommitDetail>('git_commit_detail', { repo, sha });
export const gitDiff = (repo: string, target: DiffTarget, context = 3) => invoke<FileDiff>('git_diff', { repo, target, context });
/** `rev`: sha, `HEAD` o `INDEX`. Rechaza binarios y > 5 MB. */
export const gitFileAt = (repo: string, rev: string, path: string) => invoke<string>('git_file_at', { repo, rev, path });
export const gitStage = (repo: string, paths: string[]) => invoke<OpOutcome>('git_stage', { repo, paths });
export const gitUnstage = (repo: string, paths: string[]) => invoke<OpOutcome>('git_unstage', { repo, paths });
/** `untracked: true` manda los archivos a la papelera (nunca `git clean`). */
export const gitDiscard = (repo: string, paths: string[], untracked: boolean) =>
  invoke<OpOutcome>('git_discard', { repo, paths, untracked });
/** Stage (`cached`), unstage (`cached`+`reverse`) o descartar (`reverse`) un parche/hunk. */
export const gitApplyPatch = (repo: string, patch: string, cached: boolean, reverse: boolean) =>
  invoke<OpOutcome>('git_apply_patch', { repo, patch, cached, reverse });
export const gitCommit = (repo: string, message: string, amend = false, signOff = false) =>
  invoke<OpOutcome>('git_commit', { repo, message, amend, signOff });
export const gitBranch = (repo: string, action: BranchAction) => invoke<OpOutcome>('git_branch', { repo, action });
export const gitCheckoutCommit = (repo: string, sha: string) => invoke<OpOutcome>('git_checkout_commit', { repo, sha });
export const gitMerge = (repo: string, name: string, noFf = false, squash = false) =>
  invoke<OpOutcome>('git_merge', { repo, name, noFf, squash });
export const gitRebase = (repo: string, onto: string) => invoke<OpOutcome>('git_rebase', { repo, onto });
export const gitCherryPick = (repo: string, shas: string[]) => invoke<OpOutcome>('git_cherry_pick', { repo, shas });
export const gitRevert = (repo: string, sha: string) => invoke<OpOutcome>('git_revert', { repo, sha });
/** `hard` exige `confirm: true` (la UI debe haber pedido confirmación). */
export const gitReset = (repo: string, sha: string, mode: GitResetMode, confirm = false) =>
  invoke<OpOutcome>('git_reset', { repo, sha, mode, confirm });
export const gitSequence = (repo: string, op: GitSeqOp, action: GitSeqAction) => invoke<OpOutcome>('git_sequence', { repo, op, action });
export const gitConflict = (repo: string, path: string, action: GitConflictAction) =>
  invoke<OpOutcome>('git_conflict', { repo, path, action });
export const gitStash = (repo: string, action: StashAction) => invoke<OpOutcome>('git_stash', { repo, action });
export const gitTag = (repo: string, action: TagAction) => invoke<OpOutcome>('git_tag', { repo, action });
/** `git init` en una carpeta (no lleva `repo`: aún no hay repo). */
export const gitInit = (path: string) => invoke<OpOutcome>('git_init', { path });
export const gitBlame = (repo: string, path: string, rev?: string | null) =>
  invoke<BlameResult>('git_blame', { repo, path, rev: rev ?? null });
export const gitFileHistory = (repo: string, path: string, skip: number, limit: number) =>
  invoke<LogPage>('git_file_history', { repo, path, skip, limit });
export const gitCancel = (jobId: string) => invoke<boolean>('git_cancel', { jobId });
/** fetch/pull/push con progreso por Channel (termina siempre con `done` o `failed`). Rechaza con el error si falla. */
export function gitRemote(repo: string, op: RemoteOp, jobId: string, handler: (ev: GitProgress) => void): Promise<OpOutcome> {
  const onProgress = new Channel<GitProgress>();
  onProgress.onmessage = handler;
  return invoke<OpOutcome>('git_remote', { repo, op, jobId, onProgress });
}
export const quickOpen = (query: string, limit: number) => invoke<QuickOpenItem[]>('quick_open', { query, limit });
export const searchGlobal = (query: string, options: SearchOptions) =>
  invoke<SearchResult>('search_global', { query, options });

// ---- Microservicios ----
// `root`: raíz del workspace (ruta absoluta); `undefined` = primaria. `servicesLoad` devuelve ids VISIBLES
// (`raíz/id` fuera de la primaria) y `root`; `servicesSave` acepta ids visibles o crudos.
export const servicesLoad = (root?: string) => invoke<ServiceConfig[]>('services_load', { root: root ?? null });
export const servicesSave = (root: string | undefined, services: ServiceConfig[]) =>
  invoke<void>('services_save', { root: root ?? null, services });
export const servicesDetect = (root?: string) => invoke<ServiceConfig[]>('services_detect', { root: root ?? null });
export const runConfigsLoad = (root?: string) => invoke<RunConfigFile>('run_configs_load', { root: root ?? null });
export const runConfigsSave = (root: string | undefined, file: RunConfigFile) =>
  invoke<void>('run_configs_save', { root: root ?? null, file });
export const runTemplates = () => invoke<RunTemplate[]>('run_templates');
export const runDetect = (root?: string) => invoke<ServiceConfig[]>('run_detect', { root: root ?? null });
export const runResolveFile = (path: string) => invoke<ResolvedRun>('run_resolve_file', { path });
export const runStart = (target: RunTarget, root?: string) => invoke<RunStarted>('run_start', { target, root: root ?? null });
export const servicesStatus = () => invoke<ServiceStatus[]>('services_status');
export const serviceStart = (id: string) => invoke<void>('service_start', { id });
export const serviceStop = (id: string) => invoke<void>('service_stop', { id });
export const serviceRestart = (id: string) => invoke<void>('service_restart', { id });
export const servicesStartAll = () => invoke<void>('services_start_all');
export const servicesStopAll = () => invoke<void>('services_stop_all');
export const serviceLogs = (id: string | null, limit: number) => invoke<LogLine[]>('service_logs', { id, limit });
export const serviceClearLogs = (id: string | null) => invoke<void>('service_clear_logs', { id });
export const portKill = (port: number) => invoke<number[]>('port_kill', { port });

/** Suscripción de slot único (reemplaza a la anterior en Rust). */
export function servicesSubscribe(
  onLog: (lines: LogLine[]) => void,
  onStatus: (statuses: ServiceStatus[]) => void,
): Promise<void> {
  const logCh = new Channel<LogLine[]>();
  logCh.onmessage = onLog;
  const statusCh = new Channel<ServiceStatus[]>();
  statusCh.onmessage = onStatus;
  return invoke<void>('services_subscribe', { onLog: logCh, onStatus: statusCh });
}

// ---- Terminal ----
export function terminalOpen(
  cwd: string | null,
  cols: number,
  rows: number,
  handler: (ev: TerminalEvent) => void,
): Promise<string> {
  const onEvent = new Channel<TerminalEvent>();
  onEvent.onmessage = handler;
  return invoke<string>('terminal_open', { cwd, cols, rows, onEvent });
}
export const terminalWrite = (id: string, data: string) => invoke<void>('terminal_write', { id, data });
export const terminalResize = (id: string, cols: number, rows: number) =>
  invoke<void>('terminal_resize', { id, cols, rows });
export const terminalClose = (id: string) => invoke<void>('terminal_close', { id });

// ---- LSP ----
/** Suscripción de slot único (reemplaza a la anterior en Rust). */
export function lspSubscribe(handler: (ev: LspEvent) => void): Promise<void> {
  const onEvent = new Channel<LspEvent>();
  onEvent.onmessage = handler;
  return invoke<void>('lsp_subscribe', { onEvent });
}
export const lspDidOpen = (path: string, text: string) => invoke<LspDocStatus>('lsp_did_open', { path, text });
export const lspDidChange = (path: string, version: number, text: string) =>
  invoke<void>('lsp_did_change', { path, version, text });
export const lspDidSave = (path: string, text: string) => invoke<void>('lsp_did_save', { path, text });
export const lspDidClose = (path: string) => invoke<void>('lsp_did_close', { path });
export const lspCompletion = (path: string, line: number, character: number) =>
  invoke<CompletionDto[]>('lsp_completion', { path, line, character });
export const lspHover = (path: string, line: number, character: number) =>
  invoke<HoverDto | null>('lsp_hover', { path, line, character });
export const lspDefinition = (path: string, line: number, character: number) =>
  invoke<LocationDto[]>('lsp_definition', { path, line, character });
export const lspStatus = () => invoke<LspServerStatus[]>('lsp_status');

// ---- Proyectos ----
export const projectsRecentList = () => invoke<RecentProject[]>('projects_recent_list');
export const projectsRecentRemove = (path: string) => invoke<void>('projects_recent_remove', { path });
export const projectsRecentPin = (path: string, pinned: boolean) => invoke<void>('projects_recent_pin', { path, pinned });
export const projectsToolsDetect = (checkOnline: boolean) => invoke<ToolsReport>('projects_tools_detect', { checkOnline });
export const projectGenerators = () => invoke<GeneratorInfo[]>('project_generators');
/** `online = false` devuelve al instante el catálogo offline; `true` consulta start.spring.io (timeout 6 s). */
export const springCatalog = (online: boolean) => invoke<SpringCatalog>('spring_catalog', { online });
export const projectCancel = (jobId: string) => invoke<void>('project_cancel', { jobId });
/** Crea un proyecto emitiendo eventos por Channel. Resuelve con la raíz; rechaza con el error. */
export function projectCreate(request: ProjectRequest, jobId: string, handler: (ev: GenEvent) => void): Promise<string> {
  const onEvent = new Channel<GenEvent>();
  onEvent.onmessage = handler;
  return invoke<string>('project_create', { request, jobId, onEvent });
}
