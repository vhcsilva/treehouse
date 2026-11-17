import type { Feature, Workspace } from "./types";

/** Cores das features: distintas entre si e legíveis sobre o fundo escuro. */
export const FEATURE_COLORS = ["#A78BFA", "#38BDF8", "#F472B6", "#34D399", "#FBBF24", "#FB923C", "#2DD4BF", "#C084FC"];

export function slugify(text: string): string {
  return text
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48);
}

/** Pasta e id da feature: a chave do Jira quando houver, senão o nome. */
export function featureSlug(name: string, jiraKey: string | null): string {
  return slugify(jiraKey || name) || "feature";
}

/** Aplica o padrão de nome de branch, ex.: `feature/{jira}-{slug}`. */
export function branchName(pattern: string, name: string, jiraKey: string | null): string {
  const slug = slugify(name);
  let b = pattern.replace("{jira}", jiraKey ?? "").replace("{slug}", slug);
  // Sem chave do Jira, `feature/{jira}-{slug}` viraria `feature/-slug`.
  b = b.replace(/\/-+/g, "/").replace(/-{2,}/g, "-").replace(/-+$/g, "");
  return b || `feature/${slug}`;
}

export function nextColor(used: string[]): string {
  return FEATURE_COLORS.find((c) => !used.includes(c)) ?? FEATURE_COLORS[used.length % FEATURE_COLORS.length];
}

export function joinPath(...parts: string[]): string {
  return parts
    .filter(Boolean)
    .join("/")
    .replace(/\/{2,}/g, "/");
}

export function dirname(p: string): string {
  const i = p.replace(/\/+$/, "").lastIndexOf("/");
  return i <= 0 ? "/" : p.slice(0, i);
}

/** Caminho de `to` relativo à pasta `fromDir` (os dois absolutos). */
export function relativePath(fromDir: string, to: string): string {
  const a = fromDir.split("/").filter(Boolean);
  const b = to.split("/").filter(Boolean);
  let i = 0;
  while (i < a.length && i < b.length && a[i] === b[i]) i++;
  const up = a.slice(i).map(() => "..");
  return [...up, ...b.slice(i)].join("/") || ".";
}

/** Conteúdo do `.code-workspace` que o Kiro abre para a feature. */
export function workspaceFileContent(feature: Pick<Feature, "name" | "color" | "repos" | "workspaceFile">): string {
  const dir = dirname(feature.workspaceFile);
  const body = {
    folders: feature.repos.map((r) => ({ name: r.name, path: relativePath(dir, r.path) })),
    settings: {
      "window.title": `${feature.name} — \${activeEditorShort}\${separator}\${rootName}`,
      "workbench.colorCustomizations": {
        "titleBar.activeBackground": feature.color,
        "titleBar.activeForeground": "#0B0B10",
        "titleBar.inactiveBackground": `${feature.color}99`,
        "titleBar.inactiveForeground": "#0B0B10CC",
        "statusBar.background": feature.color,
        "statusBar.foreground": "#0B0B10",
      },
    },
  };
  return JSON.stringify(body, null, 2) + "\n";
}

/** Id estável de um terminal: `<feature>/<repo>/<n>`. */
export function terminalId(featureId: string, repo: string, n: number): string {
  return `${featureId}/${repo}/${n}`;
}

export interface PaneSpec {
  id: string;
  repo: string;
  title: string;
  cwd: string;
  command?: string;
  /** Terminais extras podem ser fechados (botão × ou ao sair do shell). */
  closable?: boolean;
}

/**
 * Terminais de uma feature: com um repo, “comando do repo + shell”; com vários,
 * um por repo; mais os extras abertos com “+”.
 */
export function panesFor(feature: Feature, ws: Workspace | undefined): PaneSpec[] {
  const cmdOf = (repo: string) => ws?.repos.find((r) => r.name === repo)?.command?.trim() || undefined;
  const panes: PaneSpec[] = [];
  if (feature.repos.length === 1) {
    const r = feature.repos[0];
    const cmd = cmdOf(r.name);
    panes.push({ id: terminalId(feature.id, r.name, 0), repo: r.name, title: `${r.name} · ${cmd ?? "shell"}`, cwd: r.path, command: cmd });
    if (cmd) panes.push({ id: terminalId(feature.id, r.name, 1), repo: r.name, title: `${r.name} · shell`, cwd: r.path });
  } else {
    for (const r of feature.repos) {
      const cmd = cmdOf(r.name);
      panes.push({ id: terminalId(feature.id, r.name, 0), repo: r.name, title: `${r.name} · ${cmd ?? "shell"}`, cwd: r.path, command: cmd });
    }
  }
  const first = feature.repos[0];
  for (const n of feature.extraTerms) {
    panes.push({ id: terminalId(feature.id, first.name, 10 + n), repo: first.name, title: `${first.name} · shell ${n + 2}`, cwd: first.path, closable: true });
  }
  return panes;
}
