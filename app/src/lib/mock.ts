// Backend simulado para rodar a interface no navegador (`pnpm dev`) sem Tauri nem WSL.
// Não toca em nada de verdade: serve para ver e testar as telas.

import type { Backend, AgentStatus, Collection } from "./backend";
import type { AgentNotification, Method, Params, RepoStatus, Result } from "./protocol";
import type { Feature, Settings, Workspace } from "./types";
import { DEFAULT_SETTINGS } from "./types";

const HOME = "/home/dev";
const enc = (s: string) => btoa(unescape(encodeURIComponent(s)));
const dec = (s: string) => decodeURIComponent(escape(atob(s)));

const demoWorkspace: Workspace = {
  id: "ws-pagamentos",
  name: "pagamentos",
  distro: "Ubuntu-24.04",
  projectsDir: "~/code/pagamentos",
  worktreesDir: "~/wt/pagamentos",
  branchPattern: "feature/{jira}-{slug}",
  defaultBase: "origin/develop",
  kiroStrategy: "remote",
  repos: [
    { name: "api", path: `${HOME}/code/pagamentos/api`, remote: "git@github.com:sua-org/pagamentos-api.git", base: "origin/develop", command: "npm run dev", enabled: true },
    { name: "web", path: `${HOME}/code/pagamentos/web`, remote: "git@github.com:sua-org/pagamentos-web.git", base: "origin/main", command: "pnpm dev", enabled: true },
    { name: "infra", path: `${HOME}/code/pagamentos/infra`, remote: "git@github.com:sua-org/pagamentos-infra.git", base: "origin/main", command: "", enabled: true },
  ],
};

function demoFeature(id: string, name: string, jira: string | null, color: string, repos: string[]): Feature {
  const root = `${HOME}/wt/pagamentos/${id}`;
  return {
    id,
    workspaceId: demoWorkspace.id,
    name,
    slug: id,
    color,
    branch: `feature/${jira ? jira + "-" : ""}${id}`,
    jiraKey: jira,
    root,
    workspaceFile: `${root}.code-workspace`,
    repos: repos.map((r) => ({
      name: r,
      clonePath: `${HOME}/code/pagamentos/${r}`,
      path: `${root}/${r}`,
      base: demoWorkspace.repos.find((x) => x.name === r)!.base,
    })),
    extraTerms: [],
    createdAt: new Date().toISOString(),
    archivedAt: null,
  };
}

const db: Record<Collection, Map<string, unknown>> = {
  workspaces: new Map([[demoWorkspace.id, demoWorkspace]]),
  features: new Map<string, unknown>([
    ["pay-412", demoFeature("pay-412", "Checkout com Pix", "PAY-412", "#A78BFA", ["api", "web"])],
    ["pay-398", demoFeature("pay-398", "Webhook de estorno", "PAY-398", "#38BDF8", ["api"])],
    ["timeout-no-login", demoFeature("timeout-no-login", "Timeout no login", null, "#F472B6", ["web"])],
  ]),
  settings: new Map<string, unknown>([["app", { ...DEFAULT_SETTINGS, activeWorkspaceId: demoWorkspace.id } satisfies Settings]]),
};

if (new URLSearchParams(location.search).has("vazio")) {
  db.workspaces.clear();
  db.features.clear();
  db.settings.clear();
}

const listeners = new Set<(n: AgentNotification) => void>();
const emit = (n: AgentNotification) => listeners.forEach((l) => l(n));

const fakeStatus = (path: string): RepoStatus => {
  const seed = [...path].reduce((a, c) => a + c.charCodeAt(0), 0);
  const changes: RepoStatus["changes"] =
    seed % 3 === 0
      ? []
      : [
          { code: "M", path: "src/payments/pix.service.ts", staged: false },
          { code: "M", path: "src/payments/pix.controller.ts", staged: false },
          { code: "?", path: "src/payments/pix.mapper.ts", staged: false },
        ].slice(0, (seed % 3) + 1) as RepoStatus["changes"];
  return { path, branch: "feature/x", ahead: seed % 4, behind: seed % 5 === 0 ? 5 : 0, changes, rebasing: false };
};

const terms = new Map<string, { cwd: string; line: string; out: string }>();
const prompt = (cwd: string) => `\x1b[38;5;141m❯\x1b[0m \x1b[2m${cwd.replace(HOME, "~")}\x1b[0m $ `;

function termOut(id: string, text: string) {
  const t = terms.get(id);
  if (t) t.out += text;
  emit({ method: "term.output", params: { id, data: enc(text) } });
}

function runFake(id: string, cmd: string) {
  const t = terms.get(id)!;
  let out = "";
  if (cmd === "exit") {
    termOut(id, "logout\r\n");
    terms.delete(id);
    emit({ method: "term.exit", params: { id, code: 0 } });
    return;
  }
  if (cmd === "pwd") out = t.cwd + "\r\n";
  else if (cmd.startsWith("npm run dev")) out = "\r\n> api@2.4.0 dev\r\n[nest] Listening on \x1b[34mhttp://localhost:3012\x1b[0m\r\n";
  else if (cmd.startsWith("pnpm dev")) out = "\r\n  VITE v6.2.0  ready in 412 ms\r\n  ➜  Local: \x1b[34mhttp://localhost:5112/\x1b[0m\r\n";
  else if (cmd === "git status") out = "On branch feature/x\r\nChanges not staged for commit:\r\n\t\x1b[31mmodified:   src/payments/pix.service.ts\x1b[0m\r\n";
  else if (cmd) out = `(simulado) ${cmd}\r\n`;
  termOut(id, out + prompt(t.cwd));
}

async function call<M extends Method>(method: M, p: Params<M>): Promise<Result<M>> {
  await new Promise((r) => setTimeout(r, 120));
  const params = p as any;
  const r = (v: unknown) => v as Result<M>;
  switch (method) {
    case "agent.version":
      return r({ agentVersion: "0.2.0", protocol: 2, pid: 4242 });
    case "fs.expand":
      return r(String(params.path).replace(/^~/, HOME));
    case "workspace.scan":
      return r(
        demoWorkspace.repos.map((x) => ({ name: x.name, path: x.path, remote: x.remote, defaultBase: x.name === "api" ? "origin/develop" : "origin/main" })),
      );
    case "feature.create":
      await new Promise((r2) => setTimeout(r2, 600));
      return r(
        params.repos.map((x: any) => ({
          repo: x.name,
          path: `${params.root}/${x.name}`,
          branch: params.branch,
          base: x.base,
          reusedBranch: false,
        })),
      );
    case "feature.addRepo":
      return r({ repo: params.repo.name, path: `${params.root}/${params.repo.name}`, branch: params.branch, base: params.repo.base, reusedBranch: false });
    case "feature.archive":
      return r({ removed: params.repos.map((x: any) => x.path), deletedBranches: params.deleteBranch ? [params.repos[0]?.branch] : [], warnings: [] });
    case "git.status":
      return r(fakeStatus(params.path));
    case "git.log":
      return r([
        { hash: "e41b9c2", subject: "feat(pix): gera QR code dinâmico", when: "40 minutes ago", inFeature: true },
        { hash: "7a0d3f1", subject: "feat(pix): cria PixService", when: "3 hours ago", inFeature: true },
        { hash: "2c99e10", subject: "Merge PR #226 develop", when: "1 day ago", inFeature: false },
        { hash: "b81e4a7", subject: "fix: arredondamento de taxas", when: "1 day ago", inFeature: false },
      ]);
    case "git.rebase":
      return r({ ok: true, conflicts: [], message: "Successfully rebased and updated." });
    case "git.rebaseAbort":
    case "fs.writeFile":
    case "fs.removeFile":
    case "term.resize":
      return r(null);
    case "term.open": {
      const existing = terms.get(params.id);
      if (existing) return r({ created: false, scrollback: enc(existing.out) });
      terms.set(params.id, { cwd: params.cwd, line: "", out: "" });
      setTimeout(() => {
        termOut(params.id, prompt(params.cwd));
        if (params.command) {
          termOut(params.id, params.command);
          runFake(params.id, params.command);
        }
      }, 50);
      return r({ created: true, scrollback: "" });
    }
    case "term.attach": {
      const t = terms.get(params.id);
      return r({ created: false, scrollback: enc(t?.out ?? "") });
    }
    case "term.write": {
      const t = terms.get(params.id);
      if (!t) return r(null);
      for (const ch of dec(params.data)) {
        if (ch === "\r") {
          termOut(params.id, "\r\n");
          const cmd = t.line.trim();
          t.line = "";
          runFake(params.id, cmd);
        } else if (ch === "\x7f") {
          if (t.line) {
            t.line = t.line.slice(0, -1);
            termOut(params.id, "\b \b");
          }
        } else if (ch >= " ") {
          t.line += ch;
          termOut(params.id, ch);
        }
      }
      return r(null);
    }
    case "term.close":
      terms.delete(params.id);
      return r(null);
    case "term.list":
      return r([...terms.entries()].map(([id, t]) => ({ id, cwd: t.cwd, alive: true, exitCode: null })));
  }
  throw { code: -32601, message: `método desconhecido: ${method}` };
}

export function mockBackend(): Backend {
  const status: AgentStatus = { connected: true, distro: "Ubuntu-24.04", version: "0.2.0 (simulado)", error: null };
  return {
    simulated: true,
    call,
    notify: (method, params) => void call(method, params).catch(() => {}),
    onEvent: (cb) => {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    onStatus: (cb) => {
      setTimeout(() => cb(status), 0);
      return () => {};
    },
    start: async () => {},
    listDistros: async () => ["Ubuntu-24.04", "Debian"],
    kiroOpen: async () => {},
    setWindowEffect: async () => {},
    dbList: async <T,>(c: Collection) => [...db[c].values()] as T[],
    dbPut: async (c, id, v) => {
      db[c].set(id, structuredClone(v));
    },
    dbDelete: async (c, id) => {
      db[c].delete(id);
    },
  };
}
