// Teste de integração: as chamadas que a interface faz, contra o agente real.
// Roda só com TREEHOUSE_AGENT_BIN apontando para o binário (ex.: target/debug/treehouse-agent).

import { spawn, execFileSync, type ChildProcessWithoutNullStreams } from "node:child_process";
import { mkdtempSync, readFileSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import type { Method, Params, Result } from "./protocol";
import { workspaceFileContent } from "./naming";
import type { Feature } from "./types";

const BIN = process.env.TREEHOUSE_AGENT_BIN;

class Agent {
  private proc: ChildProcessWithoutNullStreams;
  private buf = "";
  private id = 1;
  private waiting = new Map<number, (v: any) => void>();
  readonly events: any[] = [];
  constructor(bin: string) {
    this.proc = spawn(bin, ["serve"]);
    this.proc.stdout.on("data", (d) => {
      this.buf += d.toString();
      let i;
      while ((i = this.buf.indexOf("\n")) >= 0) {
        const msg = JSON.parse(this.buf.slice(0, i));
        this.buf = this.buf.slice(i + 1);
        if (msg.id != null) this.waiting.get(msg.id)?.(msg);
        else this.events.push(msg);
      }
    });
  }
  call<M extends Method>(method: M, params: Params<M>): Promise<Result<M>> {
    const id = this.id++;
    return new Promise((resolve, reject) => {
      this.waiting.set(id, (m) => (m.error ? reject(m.error) : resolve(m.result)));
      this.proc.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
    });
  }
  close() {
    this.proc.kill();
  }
}

const git = (cwd: string, ...args: string[]) => execFileSync("git", ["-C", cwd, ...args], { encoding: "utf8" });

describe.skipIf(!BIN)("interface ↔ agente real", () => {
  let agent: Agent;
  let tmp: string;
  let clone: string;

  beforeAll(() => {
    process.env.GIT_AUTHOR_NAME = process.env.GIT_COMMITTER_NAME = "Teste";
    process.env.GIT_AUTHOR_EMAIL = process.env.GIT_COMMITTER_EMAIL = "t@example.com";
    tmp = mkdtempSync(join(tmpdir(), "th-"));
    const seed = join(tmp, "seed");
    mkdirSync(seed);
    git(seed, "init", "-q", "-b", "main");
    writeFileSync(join(seed, "README.md"), "oi\n");
    git(seed, "add", ".");
    git(seed, "commit", "-q", "-m", "inicial");
    git(seed, "branch", "develop");
    execFileSync("git", ["clone", "-q", "--bare", seed, join(tmp, "api.git")]);
    mkdirSync(join(tmp, "code"));
    clone = join(tmp, "code", "api");
    execFileSync("git", ["clone", "-q", join(tmp, "api.git"), clone]);
    agent = new Agent(BIN!);
  });
  afterAll(() => agent?.close());

  it("escaneia, cria, mostra status, abre terminal e arquiva", async () => {
    const scanned = await agent.call("workspace.scan", { dir: join(tmp, "code") });
    expect(scanned.map((r) => r.name)).toEqual(["api"]);

    const root = join(tmp, "wt", "pay-1");
    const created = await agent.call("feature.create", {
      root,
      branch: "feature/PAY-1-teste",
      repos: [{ name: "api", clonePath: clone, base: "origin/develop" }],
    });
    expect(created[0].path).toBe(join(root, "api"));

    const feature: Feature = {
      id: "pay-1", workspaceId: "w", name: "Teste", slug: "pay-1", color: "#A78BFA", branch: "feature/PAY-1-teste",
      jiraKey: "PAY-1", root, workspaceFile: `${root}.code-workspace`,
      repos: [{ name: "api", clonePath: clone, path: created[0].path, base: "origin/develop" }],
      extraTerminals: 0, createdAt: "", archivedAt: null,
    };
    await agent.call("fs.writeFile", { path: feature.workspaceFile, content: workspaceFileContent(feature) });
    expect(JSON.parse(readFileSync(feature.workspaceFile, "utf8")).folders[0].path).toBe("pay-1/api");

    writeFileSync(join(created[0].path, "novo.ts"), "x");
    const st = await agent.call("git.status", { path: created[0].path, base: "origin/develop" });
    expect(st.branch).toBe("feature/PAY-1-teste");
    expect(st.changes).toEqual([{ code: "?", path: "novo.ts", staged: false }]);

    const term = await agent.call("term.open", { id: "pay-1/api/0", cwd: created[0].path, shell: "/bin/bash", command: "echo OK_$((20+1))", cols: 80, rows: 24 });
    expect(term.created).toBe(true);
    await new Promise((r) => setTimeout(r, 800));
    const out = agent.events
      .filter((e) => e.method === "term.output" && e.params.id === "pay-1/api/0")
      .map((e) => Buffer.from(e.params.data, "base64").toString())
      .join("");
    expect(out).toContain("OK_21");

    await expect(
      agent.call("feature.archive", { root, repos: [{ clonePath: clone, path: created[0].path, branch: feature.branch }], deleteBranch: true, force: false, workspaceFile: feature.workspaceFile }),
    ).rejects.toMatchObject({ code: -32002 });
    const res = await agent.call("feature.archive", {
      root,
      repos: [{ clonePath: clone, path: created[0].path, branch: feature.branch }],
      deleteBranch: true,
      force: true,
      workspaceFile: feature.workspaceFile,
    });
    expect(res.deletedBranches).toEqual(["feature/PAY-1-teste"]);
    expect(existsSync(root)).toBe(false);
    expect(existsSync(feature.workspaceFile)).toBe(false);
    const terms = await agent.call("term.list", {});
    expect(terms.find((t) => t.id === "pay-1/api/0")).toBeUndefined();
  });
});
