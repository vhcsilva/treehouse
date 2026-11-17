import type { AgentNotification, Method, Params, Result, RpcError } from "./protocol";
import type { KiroStrategy, WindowEffect } from "./types";

export interface AgentStatus {
  connected: boolean;
  distro: string | null;
  version: string | null;
  error: string | null;
}

export type Collection = "workspaces" | "features" | "settings";

/** O que a interface precisa do lado nativo. Há uma versão Tauri e uma simulada (navegador). */
export interface Backend {
  readonly simulated: boolean;
  call<M extends Method>(method: M, params: Params<M>): Promise<Result<M>>;
  /** Envia sem esperar resposta (teclas do terminal, redimensionar). */
  notify<M extends Method>(method: M, params: Params<M>): void;
  onEvent(cb: (n: AgentNotification) => void): () => void;
  onStatus(cb: (s: AgentStatus) => void): () => void;
  start(distro: string | null): Promise<void>;
  listDistros(): Promise<string[]>;
  kiroOpen(command: string, strategy: KiroStrategy, distro: string, path: string): Promise<void>;
  setWindowEffect(effect: WindowEffect): Promise<void>;
  dbList<T>(collection: Collection): Promise<T[]>;
  dbPut(collection: Collection, id: string, value: unknown): Promise<void>;
  dbDelete(collection: Collection, id: string): Promise<void>;
}

export function isRpcError(e: unknown): e is RpcError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}

export function errorMessage(e: unknown): string {
  if (isRpcError(e)) return e.data ? `${e.message}: ${String(e.data)}` : e.message;
  if (e instanceof Error) return e.message;
  return String(e);
}

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function tauriBackend(): Promise<Backend> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  const unlistenLater = (p: Promise<() => void>) => () => void p.then((u) => u());
  return {
    simulated: false,
    call: (method, params) => invoke("agent_call", { method, params }),
    notify: (method, params) => {
      void invoke("agent_notify", { method, params }).catch(() => {});
    },
    onEvent: (cb) => unlistenLater(listen<AgentNotification>("agent-event", (e) => cb(e.payload))),
    onStatus: (cb) => unlistenLater(listen<AgentStatus>("agent-status", (e) => cb(e.payload))),
    start: async (distro) => {
      await invoke("agent_start", { distro });
    },
    listDistros: () => invoke("list_distros"),
    kiroOpen: (command, strategy, distro, path) => invoke("kiro_open", { command, strategy, distro, path }),
    setWindowEffect: (effect) => invoke("set_window_effect", { effect }),
    dbList: (collection) => invoke("db_list", { collection }),
    dbPut: (collection, id, value) => invoke("db_put", { collection, id, value }),
    dbDelete: (collection, id) => invoke("db_delete", { collection, id }),
  };
}

let instance: Promise<Backend> | null = null;

export function getBackend(): Promise<Backend> {
  if (!instance) {
    instance = isTauri ? tauriBackend() : import("./mock").then((m) => m.mockBackend());
  }
  return instance;
}
