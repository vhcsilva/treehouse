// Espelho de crates/protocol/src/lib.rs. Mantenha os dois em sincronia.

export const PROTOCOL_VERSION = 1;

export interface RpcError {
  code: number;
  message: string;
  data?: unknown;
}

export const codes = {
  GIT_FAILED: -32001,
  DIRTY_WORKTREE: -32002,
  NO_SUCH_TERMINAL: -32003,
  PATH_EXISTS: -32004,
} as const;

export interface VersionInfo {
  agentVersion: string;
  protocol: number;
  pid: number;
}

export interface ScannedRepo {
  name: string;
  path: string;
  remote: string | null;
  defaultBase: string | null;
}

export interface RepoSpec {
  name: string;
  clonePath: string;
  base: string;
}

export interface CreatedWorktree {
  repo: string;
  path: string;
  branch: string;
  base: string;
  reusedBranch: boolean;
}

export interface ArchiveRepo {
  clonePath: string;
  path: string;
  branch: string;
}

export interface ArchiveResult {
  removed: string[];
  deletedBranches: string[];
  warnings: string[];
}

export interface FileChange {
  code: "M" | "A" | "D" | "R" | "U" | "?";
  path: string;
  staged: boolean;
}

export interface RepoStatus {
  path: string;
  branch: string | null;
  ahead: number;
  behind: number;
  changes: FileChange[];
  rebasing: boolean;
}

export interface Commit {
  hash: string;
  subject: string;
  when: string;
  inFeature: boolean;
}

export interface RebaseResult {
  ok: boolean;
  conflicts: string[];
  message: string;
}

export interface TermOpenResult {
  created: boolean;
  scrollback: string;
}

export interface TermInfo {
  id: string;
  cwd: string;
  alive: boolean;
  exitCode: number | null;
}

/** Métodos do agente e o formato de parâmetros/resultados de cada um. */
export interface AgentMethods {
  "agent.version": [Record<string, never>, VersionInfo];
  "workspace.scan": [{ dir: string }, ScannedRepo[]];
  "feature.create": [{ root: string; branch: string; repos: RepoSpec[]; fetch?: boolean }, CreatedWorktree[]];
  "feature.addRepo": [{ root: string; branch: string; repo: RepoSpec; fetch?: boolean }, CreatedWorktree];
  "feature.archive": [
    { root: string; repos: ArchiveRepo[]; deleteBranch: boolean; force: boolean; workspaceFile?: string },
    ArchiveResult,
  ];
  "git.status": [{ path: string; base?: string }, RepoStatus];
  "git.log": [{ path: string; base: string; limit?: number }, Commit[]];
  "git.rebase": [
    { path: string; base: string; fetch?: boolean; autostash?: boolean; strategy?: "rebase" | "merge" },
    RebaseResult,
  ];
  "git.rebaseAbort": [{ path: string }, null];
  "fs.writeFile": [{ path: string; content: string }, null];
  "fs.removeFile": [{ path: string }, null];
  "fs.expand": [{ path: string }, string];
  "term.open": [
    { id: string; cwd: string; shell?: string; command?: string; cols: number; rows: number },
    TermOpenResult,
  ];
  "term.attach": [{ id: string }, TermOpenResult];
  "term.write": [{ id: string; data: string }, null];
  "term.resize": [{ id: string; cols: number; rows: number }, null];
  "term.close": [{ id: string }, null];
  "term.list": [Record<string, never>, TermInfo[]];
}

export type Method = keyof AgentMethods;
export type Params<M extends Method> = AgentMethods[M][0];
export type Result<M extends Method> = AgentMethods[M][1];

export interface AgentNotification {
  method: "term.output" | "term.exit" | string;
  params: any;
}
