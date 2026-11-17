import { create } from "zustand";
import { errorMessage, getBackend, isRpcError, type AgentStatus, type Backend } from "./lib/backend";
import { dirname, featureSlug, joinPath, nextColor, workspaceFileContent } from "./lib/naming";
import { codes, type Commit, type RepoStatus } from "./lib/protocol";
import { DEFAULT_SETTINGS, type Feature, type Settings, type Workspace } from "./lib/types";
import { disposeTerminal, disposeTerminals } from "./terminals";

export type View = "wt" | "overview" | "settings" | "welcome";
export type SettingsTab = "ws" | "kiro" | "look";
export type Modal = null | "new" | "archive" | "palette";

export interface Toast {
  id: number;
  title: string;
  detail?: string;
  kind: "info" | "ok" | "error";
}

export interface NewFeatureInput {
  name: string;
  jiraKey: string | null;
  branch: string;
  repos: { name: string; base: string }[];
  openKiro: boolean;
}

interface State {
  backend: Backend | null;
  ready: boolean;
  agent: AgentStatus;
  settings: Settings;
  workspaces: Workspace[];
  features: Feature[];
  selectedId: string | null;
  view: View;
  settingsTab: SettingsTab;
  modal: Modal;
  status: Record<string, Record<string, RepoStatus>>;
  logs: Record<string, Commit[]>;
  busy: string | null;
  toasts: Toast[];

  init(): Promise<void>;
  connect(distro?: string | null): Promise<void>;
  select(id: string): void;
  setView(v: View): void;
  setSettingsTab(t: SettingsTab): void;
  openModal(m: Modal): void;
  toast(title: string, detail?: string, kind?: Toast["kind"]): void;
  dismissToast(id: number): void;
  saveSettings(patch: Partial<Settings>): Promise<void>;
  saveWorkspace(ws: Workspace, makeActive?: boolean): Promise<void>;
  createFeature(input: NewFeatureInput): Promise<boolean>;
  archiveFeature(opts: { deleteBranch: boolean; force: boolean }): Promise<boolean>;
  updateFromBase(autostash?: boolean): Promise<void>;
  abortRebase(): Promise<void>;
  openKiro(feature?: Feature): Promise<void>;
  addTerminal(): Promise<void>;
  closeTerminal(paneId: string): Promise<void>;
  refreshStatus(featureId?: string): Promise<void>;
}

let toastSeq = 1;

export const activeWorkspace = (s: Pick<State, "workspaces" | "settings">) =>
  s.workspaces.find((w) => w.id === s.settings.activeWorkspaceId) ?? s.workspaces[0];

export const activeFeatures = (s: Pick<State, "features" | "workspaces" | "settings">) => {
  const ws = activeWorkspace(s);
  return s.features.filter((f) => f.workspaceId === ws?.id && !f.archivedAt);
};

export const selectedFeature = (s: State) => activeFeatures(s).find((f) => f.id === s.selectedId);

export const useStore = create<State>((set, get) => {
  const be = () => {
    const b = get().backend;
    if (!b) throw new Error("backend não iniciado");
    return b;
  };
  const putFeature = async (f: Feature) => {
    await be().dbPut("features", f.id, f);
    set((s) => ({ features: s.features.some((x) => x.id === f.id) ? s.features.map((x) => (x.id === f.id ? f : x)) : [...s.features, f] }));
  };

  return {
    backend: null,
    ready: false,
    agent: { connected: false, distro: null, version: null, error: null },
    settings: DEFAULT_SETTINGS,
    workspaces: [],
    features: [],
    selectedId: null,
    view: "wt",
    settingsTab: "ws",
    modal: null,
    status: {},
    logs: {},
    busy: null,
    toasts: [],

    async init() {
      const backend = await getBackend();
      backend.onStatus((agent) => set({ agent }));
      const [workspaces, rawFeatures, settingsDocs] = await Promise.all([
        backend.dbList<Workspace>("workspaces"),
        backend.dbList<Feature>("features"),
        backend.dbList<Settings>("settings"),
      ]);
      const features = rawFeatures.map((f) => ({
        ...f,
        extraTerms: f.extraTerms ?? Array.from({ length: f.extraTerminals ?? 0 }, (_, i) => i),
      }));
      const settings = { ...DEFAULT_SETTINGS, ...(settingsDocs[0] ?? {}) };
      set({ backend, workspaces, features, settings, ready: true, view: workspaces.length ? "wt" : "welcome" });
      const first = activeFeatures(get())[0];
      if (first) set({ selectedId: first.id });
      document.documentElement.style.setProperty("--accent", settings.accent);
      document.documentElement.style.setProperty("--blur", `${settings.blur}px`);
      if (!backend.simulated) void backend.setWindowEffect(settings.effect).catch(() => {});
      const ws = activeWorkspace(get());
      if (ws) await get().connect(ws.distro);
    },

    async connect(distro) {
      try {
        await be().start(distro ?? null);
        await get().refreshStatus();
      } catch (e) {
        get().toast("Não foi possível conectar ao WSL", errorMessage(e), "error");
      }
    },

    select(id) {
      set({ selectedId: id, view: "wt" });
      void get().refreshStatus(id);
    },
    setView: (view) => set({ view, modal: null }),
    setSettingsTab: (settingsTab) => set({ settingsTab }),
    openModal: (modal) => set({ modal }),

    toast(title, detail, kind = "info") {
      const id = toastSeq++;
      set((s) => ({ toasts: [...s.toasts.slice(-3), { id, title, detail, kind }] }));
      setTimeout(() => get().dismissToast(id), kind === "error" ? 12000 : 6000);
    },
    dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),

    async saveSettings(patch) {
      const settings = { ...get().settings, ...patch };
      await be().dbPut("settings", "app", settings);
      set({ settings });
      document.documentElement.style.setProperty("--accent", settings.accent);
      document.documentElement.style.setProperty("--blur", `${settings.blur}px`);
      if (patch.effect && !be().simulated) await be().setWindowEffect(patch.effect).catch(() => {});
    },

    async saveWorkspace(ws, makeActive = false) {
      const prev = activeWorkspace(get());
      await be().dbPut("workspaces", ws.id, ws);
      set((s) => ({ workspaces: s.workspaces.some((w) => w.id === ws.id) ? s.workspaces.map((w) => (w.id === ws.id ? ws : w)) : [...s.workspaces, ws] }));
      if (makeActive || !get().settings.activeWorkspaceId) await get().saveSettings({ activeWorkspaceId: ws.id });
      if (prev?.distro !== ws.distro || makeActive) await get().connect(ws.distro);
    },

    async createFeature(input) {
      const ws = activeWorkspace(get());
      if (!ws) return false;
      const slug = featureSlug(input.name, input.jiraKey);
      if (get().features.some((f) => f.id === slug && !f.archivedAt)) {
        get().toast("Já existe uma feature com esse nome", slug, "error");
        return false;
      }
      set({ busy: "Criando worktrees…" });
      try {
        const wtDir = await be().call("fs.expand", { path: ws.worktreesDir });
        const root = joinPath(wtDir, slug);
        const created = await be().call("feature.create", {
          root,
          branch: input.branch,
          repos: input.repos.map((r) => ({ name: r.name, base: r.base, clonePath: ws.repos.find((x) => x.name === r.name)!.path })),
        });
        const feature: Feature = {
          id: slug,
          workspaceId: ws.id,
          name: input.name,
          slug,
          color: nextColor(activeFeatures(get()).map((f) => f.color)),
          branch: input.branch,
          jiraKey: input.jiraKey,
          root,
          workspaceFile: `${root}.code-workspace`,
          repos: created.map((c) => ({
            name: c.repo,
            path: c.path,
            base: c.base,
            clonePath: ws.repos.find((x) => x.name === c.repo)!.path,
          })),
          extraTerms: [],
          createdAt: new Date().toISOString(),
          archivedAt: null,
        };
        await be().call("fs.writeFile", { path: feature.workspaceFile, content: workspaceFileContent(feature) });
        await putFeature(feature);
        set({ selectedId: feature.id, view: "wt", modal: null });
        const reused = created.filter((c) => c.reusedBranch).map((c) => c.repo);
        get().toast(
          `Feature “${feature.name}” criada`,
          reused.length ? `Branch já existia em: ${reused.join(", ")}` : created.map((c) => c.path).join("\n"),
          "ok",
        );
        if (input.openKiro) await get().openKiro(feature);
        void get().refreshStatus(feature.id);
        return true;
      } catch (e) {
        get().toast("Não foi possível criar a feature", errorMessage(e), "error");
        return false;
      } finally {
        set({ busy: null });
      }
    },

    async archiveFeature({ deleteBranch, force }) {
      const f = selectedFeature(get());
      if (!f) return false;
      set({ busy: "Arquivando…" });
      try {
        const res = await be().call("feature.archive", {
          root: f.root,
          repos: f.repos.map((r) => ({ clonePath: r.clonePath, path: r.path, branch: f.branch })),
          deleteBranch,
          force,
          workspaceFile: f.workspaceFile,
        });
        disposeTerminals(`${f.id}/`);
        await putFeature({ ...f, archivedAt: new Date().toISOString() });
        const next = activeFeatures(get())[0];
        set({ selectedId: next?.id ?? null, modal: null });
        get().toast(`“${f.name}” arquivada`, [...res.removed, ...res.warnings].join("\n"), "ok");
        return true;
      } catch (e) {
        const dirty = isRpcError(e) && e.code === codes.DIRTY_WORKTREE;
        get().toast(dirty ? "Há alterações não commitadas" : "Não foi possível arquivar", errorMessage(e), "error");
        return false;
      } finally {
        set({ busy: null });
      }
    },

    async updateFromBase(autostash = false) {
      const f = selectedFeature(get());
      if (!f) return;
      set({ busy: "Atualizando da base…" });
      const strategy = get().settings.updateStrategy;
      try {
        for (const r of f.repos) {
          const res = await be().call("git.rebase", { path: r.path, base: r.base, autostash, strategy });
          if (!res.ok) {
            get().toast(`Conflito em ${r.name}`, `${res.conflicts.join("\n")}\nResolva no Kiro ou aborte.`, "error");
            return;
          }
        }
        get().toast(`${strategy === "rebase" ? "Rebase" : "Merge"} concluído`, f.repos.map((r) => `${r.name} ← ${r.base}`).join("\n"), "ok");
      } catch (e) {
        const dirty = isRpcError(e) && e.code === codes.DIRTY_WORKTREE;
        get().toast(dirty ? "Há alterações não commitadas" : "Falha ao atualizar", dirty ? "Faça commit ou use “Atualizar com stash” na paleta (Ctrl+Shift+K)." : errorMessage(e), "error");
      } finally {
        set({ busy: null });
        void get().refreshStatus(f.id);
      }
    },

    async abortRebase() {
      const f = selectedFeature(get());
      if (!f) return;
      for (const r of f.repos) await be().call("git.rebaseAbort", { path: r.path }).catch(() => {});
      get().toast("Rebase abortado", undefined, "ok");
      void get().refreshStatus(f.id);
    },

    async openKiro(feature) {
      const f = feature ?? selectedFeature(get());
      const ws = activeWorkspace(get());
      if (!f || !ws) return;
      try {
        await be().kiroOpen(get().settings.kiroCommand, ws.kiroStrategy, ws.distro, f.workspaceFile);
        get().toast("Abrindo no Kiro", f.workspaceFile);
      } catch (e) {
        get().toast("Não foi possível abrir o Kiro", errorMessage(e), "error");
      }
    },

    async addTerminal() {
      const f = selectedFeature(get());
      if (!f || f.extraTerms.length >= 4) return;
      const next = f.extraTerms.length ? Math.max(...f.extraTerms) + 1 : 0;
      await putFeature({ ...f, extraTerms: [...f.extraTerms, next] });
    },

    async closeTerminal(paneId) {
      disposeTerminal(paneId);
      // O terminal pode ser de outra feature (o shell saiu enquanto ela estava escondida).
      const f = get().features.find((x) => paneId.startsWith(`${x.id}/`));
      if (!f) return;
      const n = Number(paneId.split("/").pop()) - 10;
      if (f.extraTerms.includes(n)) await putFeature({ ...f, extraTerms: f.extraTerms.filter((x) => x !== n) });
    },

    async refreshStatus(featureId) {
      const id = featureId ?? get().selectedId;
      const f = activeFeatures(get()).find((x) => x.id === id);
      if (!f || !get().agent.connected) return;
      try {
        const entries = await Promise.all(f.repos.map(async (r) => [r.name, await be().call("git.status", { path: r.path, base: r.base })] as const));
        const log = await be().call("git.log", { path: f.repos[0].path, base: f.repos[0].base, limit: 8 });
        set((s) => ({ status: { ...s.status, [f.id]: Object.fromEntries(entries) }, logs: { ...s.logs, [f.id]: log } }));
      } catch {
        // Worktree removida por fora, WSL desligado… o próximo ciclo tenta de novo.
      }
    },
  };
});

/** Pasta pai de um caminho, exportado para telas que mostram onde as coisas ficam. */
export { dirname };
