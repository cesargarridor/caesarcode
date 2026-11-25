// Tipos compartidos Rust DTO <-> frontend (PLAN §3.4). Todo camelCase; rutas nativas absolutas.

export interface AppError { kind: string; message: string }

export interface AppInfo { version: string; platform: 'macos' | 'windows' | 'linux'; pathSep: string }
export interface RootInfo { path: string; name: string; exists: boolean }
export interface WorkspaceInfo { root: string; name: string; roots: RootInfo[] }
export interface FsEntry { name: string; path: string; isDir: boolean; isSymlink: boolean; size: number; ext: string | null; module?: string | null; role?: FolderRole | null; compact?: CompactChain | null }
export type FolderRole =
  | 'sourceRoot' | 'testRoot' | 'resourcesRoot' | 'testResourcesRoot' | 'generatedRoot' | 'package' | 'excluded' | 'module';
export interface CompactChain { name: string; path: string }
export type SymbolKind =
  | 'class' | 'abstractClass' | 'interface' | 'enum' | 'record' | 'annotation' | 'exception' | 'object'
  | 'dataClass' | 'sealedClass' | 'kotlinFile' | 'script' | 'unknown';
export type Stereotype =
  | 'springBootApp' | 'main' | 'controller' | 'service' | 'repository' | 'configuration' | 'entity' | 'component' | 'test';
export interface SymbolInfo { kind: SymbolKind; stereotype: Stereotype | null; public: boolean }
export interface FileContent { path: string; text: string; isBinary: boolean; tooLarge: boolean; truncated: boolean }
export type FsEvent =
  | { type: 'changed'; paths: string[] }
  | { type: 'gitChanged'; repo?: string | null }
  | { type: 'rootsChanged' }
  | { type: 'error'; message: string };
export type GitFileStatus = 'M' | 'U' | 'D' | 'A' | 'R';
export interface GitStatus { isRepo: boolean; branch: string | null; files: Record<string, GitFileStatus> }
export interface QuickOpenItem { path: string; relPath: string; root: string; score: number; indices: number[] }
export interface SearchOptions { caseSensitive: boolean; regex: boolean; wholeWord: boolean; include?: string | null; maxResults?: number | null }
export interface SearchMatch {
  path: string; relPath: string; root: string; line: number; column: number;
  preview: string; matchStart: number; matchEnd: number;
}
export interface SearchResult { matches: SearchMatch[]; truncated: boolean; filesSearched: number; cancelled: boolean }

export interface ServiceConfig {
  id: string; name: string; command: string; cwd: string;
  env: Record<string, string>; envFile: string | null; port: number | null; kind: ServiceKind;
  tech?: Tech | null; framework?: string | null;
  /** Raíz del workspace a la que pertenece (ruta absoluta). `id` es el id VISIBLE (`raíz/id` fuera de la primaria). */
  root?: string;
}
export type ServiceKind = 'node' | 'python' | 'rust' | 'maven' | 'gradle' | 'docker' | 'go' | 'java' | 'shell' | 'custom';
export type Tech = 'spring' | 'python' | 'node' | 'go' | 'rust' | 'docker' | 'other';
export type ServiceState = 'stopped' | 'starting' | 'running' | 'stopping' | 'error';
export interface ServiceStatus {
  id: string; name?: string; root?: string; ephemeral?: boolean; state: ServiceState; pid: number | null; exitCode: number | null;
  cpu: number; memoryMb: number; portOpen: boolean; port: number | null; startedAt: number | null; message: string | null;
}

export type RunKind = 'springBoot' | 'python' | 'node' | 'go' | 'rust' | 'shell';
export interface RunConfig {
  id: string; name: string; kind: RunKind; cwd: string; command: string; args: string;
  env: Record<string, string>; envFile: string | null; port: number | null; showInServices: boolean;
}
export interface RunConfigFile { version: number; configs: RunConfig[]; fileRunners: Record<string, string> }
export interface RunTemplate { kind: RunKind; label: string; config: RunConfig }
export interface ResolvedRun { command: string; cwd: string; label: string; runner: string }
export type RunTarget =
  | { type: 'file'; path: string }
  | { type: 'config'; id: string }
  | { type: 'detected'; service: ServiceConfig };
export interface RunStarted { runtimeId: string; label: string; command: string }

export type LogLevel = 'error' | 'warn' | 'info';
export interface LogLine { seq: number; serviceId: string; ts: number; level: LogLevel; text: string }
export type TerminalEvent = { type: 'data'; data: string } | { type: 'exit'; code: number | null };

export type LspState = 'starting' | 'ready' | 'unavailable' | 'crashed';
export interface LspServerStatus { language: string; server: string; state: LspState; message: string | null }
export interface LspDocStatus { language: string | null; server: string | null; available: boolean }
export interface DiagnosticDto {
  fromLine: number; fromChar: number; toLine: number; toChar: number;
  severity: 'error' | 'warning' | 'info' | 'hint'; message: string; source: string | null;
}
export type LspEvent =
  | { type: 'diagnostics'; path: string; diagnostics: DiagnosticDto[] }
  | { type: 'status'; status: LspServerStatus };
export interface TextEditDto { fromLine: number; fromChar: number; toLine: number; toChar: number; newText: string }
export interface CompletionDto {
  label: string; kind: string | null; detail: string | null; documentation: string | null;
  insertText: string; isSnippet: boolean; sortText: string | null; filterText: string | null; textEdit: TextEditDto | null;
}
export interface HoverDto { markdown: string }
export interface LocationDto { path: string; line: number; character: number; endLine: number; endCharacter: number }

// ---- Proyectos (Bloque B) ----
export interface RecentProject {
  path: string; name: string; lastOpened: number; kind: string | null; pinned: boolean; exists: boolean;
  /** Raíces adjuntas persistidas además de la primaria. */
  extraRoots?: number;
}
export interface ToolInfo { id: string; path: string | null; version: string | null }
export interface JdkInfo { home: string; version: string; major: number; vendor: string | null }
export interface ToolsReport { tools: ToolInfo[]; jdks: JdkInfo[]; online: boolean | null }
export type GeneratorStrategy = 'local' | 'cli' | 'http' | 'httpWithFallback';
export interface GeneratorInfo {
  id: string; label: string; group: string; strategy: GeneratorStrategy;
  requires: string[]; available: boolean; missing: string[]; offlineFallback: boolean;
}
export type BuildSystem = 'intellij' | 'maven' | 'gradleKotlin' | 'gradleGroovy';
export interface CommonOpts { name: string; location: string; git: boolean }
export interface JvmOpts {
  build: BuildSystem; javaVersion: number; jdkHome: string | null;
  groupId: string; artifactId: string; packageName: string; sampleCode: boolean;
}
export interface ArchetypeRef { group: string; artifact: string; version: string }
export interface MicroserviceSpec { name: string; port: number; dependencies: string[]; layers: boolean }
export interface InfraSpec { port: number }
export interface SpringRequest {
  groupId: string; basePackage: string; javaVersion: number; bootVersion: string | null; build: BuildSystem;
  services: MicroserviceSpec[]; gateway: InfraSpec | null; discovery: InfraSpec | null; configServer: InfraSpec | null;
  dockerCompose: boolean; dockerfiles: boolean; preferOnline: boolean; single: boolean;
}
export type ProjectRequest =
  | { generator: 'java' | 'kotlin' | 'groovy' | 'javaFx' | 'jakartaEe' | 'ktor'; common: CommonOpts; jvm: JvmOpts }
  | { generator: 'empty' | 'html'; common: CommonOpts }
  | { generator: 'mavenArchetype'; common: CommonOpts; jvm: JvmOpts; archetype: ArchetypeRef }
  | { generator: 'quarkus'; common: CommonOpts; jvm: JvmOpts; extensions: string[] }
  | { generator: 'micronaut'; common: CommonOpts; jvm: JvmOpts; features: string[] }
  | { generator: 'springMicroservices'; common: CommonOpts; spring: SpringRequest }
  | { generator: 'express' | 'node' | 'react'; common: CommonOpts; typescript: boolean }
  | { generator: 'vue'; common: CommonOpts; typescript: boolean; router: boolean; pinia: boolean }
  | { generator: 'vite'; common: CommonOpts; template: string }
  | { generator: 'angular'; common: CommonOpts; routing: boolean; style: string }
  | { generator: 'python'; common: CommonOpts; venv: boolean }
  | { generator: 'go'; common: CommonOpts; module: string }
  | { generator: 'rust'; common: CommonOpts; bin: boolean };
export type GenEvent =
  | { type: 'step'; label: string }
  | { type: 'output'; line: string; stream: 'stdout' | 'stderr' }
  | { type: 'warning'; message: string }
  | { type: 'done'; root: string; runConfigs: number }
  | { type: 'failed'; message: string };
export interface SpringDep { id: string; name: string; description: string; offline: boolean }
export interface SpringDepGroup { name: string; items: SpringDep[] }
export interface SpringCatalog {
  bootVersions: string[]; defaultBoot: string; javaVersions: number[]; dependencies: SpringDepGroup[]; source: 'online' | 'offline';
}

// ---- Git (Bloque C1) — espejo de src-tauri/src/git/* ----
export type GitRepoState = 'clean' | 'merging' | 'rebasing' | 'cherryPicking' | 'reverting' | 'bisecting';
export interface RepoInfo {
  isRepo: boolean; top: string | null; head: string | null; branch: string | null; detached: boolean;
  upstream: string | null; ahead: number; behind: number; state: GitRepoState; gitVersion: string | null;
  /** `true` si el status usa `--untracked-files=normal` por lentitud del repo. */
  untrackedLimited: boolean;
}
export type EntryKind = 'ordinary' | 'renamed' | 'copied' | 'unmerged' | 'untracked' | 'ignored';
export interface StatusEntry {
  /** Relativa al top del repo, con `/`. */
  path: string; origPath: string | null;
  /** Letra de estado ('M','A','D','R','C','T','?','U') o ' ' si no hay cambio en esa columna. */
  index: string; worktree: string; kind: EntryKind;
  /** "UU", "AA", "DU"... solo en conflictos. */
  conflict: string | null;
}
export interface StatusV2 {
  info: RepoInfo; staged: StatusEntry[]; unstaged: StatusEntry[]; untracked: StatusEntry[]; conflicted: StatusEntry[];
}

export type GitRefKind = 'head' | 'local' | 'remote' | 'tag' | 'stash';
export interface RefLabel { name: string; kind: GitRefKind }
export interface CommitRow {
  sha: string; parents: string[]; author: string; email: string; time: number; subject: string; refs: RefLabel[];
}
export interface LogPage { commits: CommitRow[]; hasMore: boolean }
export interface LogQuery {
  skip: number; /** ≤ 2000 */ limit: number; all: boolean; path?: string | null; search?: string | null; author?: string | null;
}

export interface BranchRef {
  name: string; fullName: string; sha: string; upstream: string | null; ahead: number; behind: number; isHead: boolean;
}
export interface TagRef { name: string; sha: string; annotated: boolean; message: string | null }
export interface StashRef { index: number; sha: string; message: string; time: number }
export interface RemoteInfo { name: string; fetchUrl: string }
export interface GitRefs { local: BranchRef[]; remote: BranchRef[]; tags: TagRef[]; stashes: StashRef[]; remotes: RemoteInfo[] }

export interface ChangedFile {
  path: string; origPath: string | null; status: string /* A M D R C T */; additions: number | null; deletions: number | null;
}
export interface CommitDetail { row: CommitRow; body: string; committer: string; commitTime: number; files: ChangedFile[] }

export interface DiffLine { kind: ' ' | '+' | '-' | '\\'; text: string; oldNo: number | null; newNo: number | null }
export interface DiffHunk {
  header: string; oldStart: number; oldLines: number; newStart: number; newLines: number; lines: DiffLine[];
}
export interface FileDiff {
  path: string; oldPath: string | null; binary: boolean; hunks: DiffHunk[];
  /** `diff --git…` hasta el primer `@@`: cabecera + hunk.header + líneas = parche para `gitApplyPatch`. */
  header: string; tooLarge: boolean;
}
export type DiffTarget =
  | { type: 'worktree'; path: string }
  | { type: 'index'; path: string }
  | { type: 'commit'; sha: string; path: string }
  | { type: 'range'; from: string; to: string; path: string };

export interface BlameCommit { author: string; time: number; summary: string }
export interface BlameResult { commits: Record<string, BlameCommit>; lines: [number, string][] }

export interface OpOutcome { ok: boolean; conflicts: string[]; message: string }

export type BranchAction =
  | { type: 'create'; name: string; start?: string | null; checkout: boolean }
  | { type: 'delete'; name: string; force: boolean }
  | { type: 'rename'; from: string; to: string }
  | { type: 'checkout'; name: string }
  | { type: 'checkoutRemote'; remoteBranch: string }
  | { type: 'setUpstream'; name: string; upstream: string };
export type GitResetMode = 'soft' | 'mixed' | 'hard';
export type GitSeqOp = 'merge' | 'rebase' | 'cherryPick' | 'revert';
export type GitSeqAction = 'abort' | 'continue' | 'skip';
export type GitConflictAction = 'ours' | 'theirs' | 'markResolved';
export type StashAction =
  | { type: 'push'; message?: string | null; includeUntracked: boolean; keepIndex: boolean }
  | { type: 'pop'; index: number }
  | { type: 'apply'; index: number }
  | { type: 'drop'; index: number };
export type TagAction =
  | { type: 'create'; name: string; sha: string; message?: string | null }
  | { type: 'delete'; name: string }
  | { type: 'push'; remote: string; name: string };
export type RemoteOp =
  | { type: 'fetch'; remote?: string | null; prune: boolean }
  | { type: 'pull'; rebase: boolean; ffOnly?: boolean }
  | { type: 'push'; remote?: string | null; branch?: string | null; forceWithLease: boolean; setUpstream: boolean; tags: boolean };
export type GitProgress =
  | { type: 'line'; text: string }
  | { type: 'percent'; phase: string; pct: number }
  | { type: 'done' }
  | { type: 'failed'; message: string };

// ---- Multi-repo (Bloque C de PLAN5) — espejo de src-tauri/src/git/repos.rs ----
export interface RepoEntry {
  /** Toplevel canónico del repo (identidad). */
  top: string; name: string;
  /** Raíz del workspace que lo contiene. */
  root: string; depth: number; nested: boolean; submodule: boolean;
}
export interface ReposResult { repos: RepoEntry[]; truncated: boolean }
export interface RepoSummary {
  top: string; name: string; branch: string | null; detached: boolean; upstream: string | null;
  ahead: number; behind: number; changed: number; staged: number; untracked: number; conflicted: number;
  /** `merging | rebasing | cherry-picking | reverting | bisecting`; `null` si está limpio. */
  state: string | null;
  error: string | null;
}

export interface TabularData {
  sheets: string[];
  sheet: string | null;
  headers: string[];
  rows: string[][];
  totalRows: number;
  fileSize: number;
  delimiter: string | null;
}
