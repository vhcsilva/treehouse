import { describe, expect, it } from "vitest";
import { branchName, featureSlug, panesFor, relativePath, slugify, workspaceFileContent } from "./naming";
import type { Feature, Workspace } from "./types";

describe("nomes", () => {
  it("slugify remove acentos e símbolos", () => {
    expect(slugify("Relatório de Conciliação!")).toBe("relatorio-de-conciliacao");
    expect(featureSlug("Checkout com Pix", "PAY-412")).toBe("pay-412");
    expect(featureSlug("Timeout no login", null)).toBe("timeout-no-login");
  });

  it("aplica o padrão de branch com e sem Jira", () => {
    expect(branchName("feature/{jira}-{slug}", "Checkout com Pix", "PAY-412")).toBe("feature/PAY-412-checkout-com-pix");
    expect(branchName("feature/{jira}-{slug}", "Timeout no login", null)).toBe("feature/timeout-no-login");
    expect(branchName("{slug}", "Algo", null)).toBe("algo");
  });

  it("caminhos relativos", () => {
    expect(relativePath("/home/v/wt/pag", "/home/v/wt/pag/pay-412/api")).toBe("pay-412/api");
    expect(relativePath("/home/v/wt/pag", "/home/v/code/api")).toBe("../../code/api");
  });
});

const ws: Workspace = {
  id: "w",
  name: "pagamentos",
  distro: "Ubuntu",
  projectsDir: "~/code",
  worktreesDir: "~/wt",
  branchPattern: "feature/{jira}-{slug}",
  defaultBase: "origin/develop",
  kiroStrategy: "remote",
  repos: [
    { name: "api", path: "/c/api", remote: null, base: "origin/develop", command: "npm run dev", enabled: true },
    { name: "web", path: "/c/web", remote: null, base: "origin/main", command: "", enabled: true },
  ],
};

const feature = (repos: string[]): Feature => ({
  id: "pay-1",
  workspaceId: "w",
  name: "Pix",
  slug: "pay-1",
  color: "#A78BFA",
  branch: "feature/pay-1",
  jiraKey: null,
  root: "/h/wt/pay-1",
  workspaceFile: "/h/wt/pay-1.code-workspace",
  repos: repos.map((r) => ({ name: r, clonePath: `/c/${r}`, path: `/h/wt/pay-1/${r}`, base: "origin/main" })),
  extraTerms: [],
  createdAt: "",
  archivedAt: null,
});

describe("terminais e workspace", () => {
  it("um repo: comando + shell", () => {
    const p = panesFor(feature(["api"]), ws);
    expect(p.map((x) => x.title)).toEqual(["api · npm run dev", "api · shell"]);
    expect(p[0].command).toBe("npm run dev");
  });

  it("terminais extras são fecháveis e mantêm o id ao fechar outro", () => {
    const p = panesFor({ ...feature(["api"]), extraTerms: [0, 2] }, ws);
    const extras = p.filter((x) => x.closable);
    expect(extras.map((x) => x.id)).toEqual(["pay-1/api/10", "pay-1/api/12"]);
    expect(p.filter((x) => !x.closable)).toHaveLength(2);
  });

  it("vários repos: um terminal por repo", () => {
    const p = panesFor(feature(["api", "web"]), ws);
    expect(p.map((x) => x.title)).toEqual(["api · npm run dev", "web · shell"]);
  });

  it("gera o .code-workspace com caminhos relativos e cor", () => {
    const json = JSON.parse(workspaceFileContent(feature(["api", "web"])));
    expect(json.folders).toEqual([
      { name: "api", path: "pay-1/api" },
      { name: "web", path: "pay-1/web" },
    ]);
    expect(json.settings["workbench.colorCustomizations"]["titleBar.activeBackground"]).toBe("#A78BFA");
  });
});
