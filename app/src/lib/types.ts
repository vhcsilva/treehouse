// Documentos guardados no SQLite local (coleções do app/src-tauri/src/db.rs).

export type KiroStrategy = "remote" | "unc";
export type WindowEffect = "mica" | "acrylic" | "none";
export type TerminalLayout = "grid" | "columns" | "tabs";

export interface RepoConfig {
  name: string;
  /** Caminho do clone principal no WSL. */
  path: string;
  remote: string | null;
  /** Branch base para criar e atualizar, ex.: `origin/develop`. */
  base: string;
  /** Comando que roda no primeiro terminal do repo, ex.: `npm run dev`. */
  command: string;
  enabled: boolean;
}

export interface Workspace {
  id: string;
  name: string;
  distro: string;
  projectsDir: string;
  worktreesDir: string;
  /** Ex.: `feature/{jira}-{slug}`. */
  branchPattern: string;
  defaultBase: string;
  kiroStrategy: KiroStrategy;
  repos: RepoConfig[];
}

export interface FeatureRepo {
  name: string;
  clonePath: string;
  /** Caminho absoluto da worktree. */
  path: string;
  base: string;
}

export interface Feature {
  id: string;
  workspaceId: string;
  name: string;
  slug: string;
  color: string;
  branch: string;
  jiraKey: string | null;
  /** Pasta da feature (absoluta), contém uma subpasta por repo. */
  root: string;
  /** `.code-workspace` (absoluto) aberto no Kiro. */
  workspaceFile: string;
  repos: FeatureRepo[];
  /** Índices dos terminais extras abertos com “+” (cada um vira o id `<feature>/<repo>/<10+n>`). */
  extraTerms: number[];
  /** Formato antigo (só a contagem); convertido em `extraTerms` ao carregar. */
  extraTerminals?: number;
  createdAt: string;
  archivedAt: string | null;
}

export interface Settings {
  id: "app";
  activeWorkspaceId: string | null;
  kiroCommand: string;
  shell: string;
  fontSize: number;
  updateStrategy: "rebase" | "merge";
  terminalLayout: TerminalLayout;
  effect: WindowEffect;
  blur: number;
  accent: string;
}

export const DEFAULT_SETTINGS: Settings = {
  id: "app",
  activeWorkspaceId: null,
  kiroCommand: "kiro",
  shell: "",
  fontSize: 13,
  updateStrategy: "rebase",
  terminalLayout: "grid",
  effect: "mica",
  blur: 20,
  accent: "#8B5CF6",
};
