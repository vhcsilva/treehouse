// Instâncias do xterm.js, uma por terminal do agente. Elas sobrevivem à troca de
// feature: só o elemento sai da tela, e a saída continua chegando.

import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { errorMessage, type Backend } from "./lib/backend";
import type { PaneSpec } from "./lib/naming";

interface Instance {
  term: Terminal;
  fit: FitAddon;
  el: HTMLDivElement;
  opened: boolean;
  started: boolean;
  /** O processo saiu; Enter abre um shell novo no mesmo lugar. */
  exited: boolean;
  spec: PaneSpec;
  opts: { fontSize: number; shell: string };
}

const instances = new Map<string, Instance>();
let backend: Backend | null = null;
/** Chamado quando o shell de um terminal fechável sai (o painel deve sumir). */
let onClosableExit: (id: string) => void = () => {};

export function setClosableExitHandler(fn: (id: string) => void) {
  onClosableExit = fn;
}

const b64ToBytes = (b64: string) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
const textToB64 = (text: string) => {
  const bytes = new TextEncoder().encode(text);
  let bin = "";
  bytes.forEach((b) => (bin += String.fromCharCode(b)));
  return btoa(bin);
};

export function bindTerminalEvents(b: Backend) {
  backend = b;
  b.onEvent((n) => {
    const inst = instances.get(n.params?.id);
    if (!inst) return;
    if (n.method === "term.output") inst.term.write(b64ToBytes(n.params.data));
    if (n.method === "term.exit") {
      inst.started = false;
      if (inst.spec.closable) {
        onClosableExit(inst.spec.id);
        return;
      }
      inst.exited = true;
      const code = n.params.code != null ? `: código ${n.params.code}` : "";
      inst.term.write(`\r\n\x1b[2m[processo encerrado${code}] pressione Enter para abrir um shell novo\x1b[0m\r\n`);
    }
  });
}

/** Atalhos do app passam pelo terminal: Ctrl+Shift+<tecla> e Ctrl+1…9. */
const isAppShortcut = (e: KeyboardEvent) => e.ctrlKey && (e.shiftKey || /^[1-9]$/.test(e.key));

function start(i: Instance) {
  if (!backend || i.started) return;
  // Ao reabrir depois de um `exit`, só o shell: o comando do repo não roda de novo sem pedir.
  const command = i.exited ? undefined : i.spec.command;
  i.started = true;
  i.exited = false;
  backend
    .call("term.open", {
      id: i.spec.id,
      cwd: i.spec.cwd,
      command,
      shell: i.opts.shell || undefined,
      cols: i.term.cols,
      rows: i.term.rows,
    })
    .then((res) => {
      if (!res.created && res.scrollback) {
        i.term.reset();
        i.term.write(b64ToBytes(res.scrollback));
      }
    })
    .catch((e) => {
      i.started = false;
      i.term.write(`\x1b[31mNão foi possível abrir o terminal: ${errorMessage(e)}\x1b[0m\r\n`);
    });
}

function create(spec: PaneSpec, opts: { fontSize: number; shell: string }): Instance {
  const fontSize = opts.fontSize;
  const term = new Terminal({
    fontFamily: "'JetBrains Mono', 'Cascadia Mono', Consolas, monospace",
    fontSize,
    lineHeight: 1.25,
    cursorBlink: true,
    allowTransparency: true,
    scrollback: 10000,
    theme: {
      background: "#00000000",
      foreground: "#E8E9F0",
      cursor: "#C4B5FD",
      selectionBackground: "#8B5CF655",
      black: "#1B1D27",
      brightBlack: "#6E7186",
      red: "#FCA5A5",
      green: "#6EE7B7",
      yellow: "#FCD34D",
      blue: "#93C5FD",
      magenta: "#C4B5FD",
      cyan: "#67E8F9",
      white: "#E8E9F0",
    },
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  term.attachCustomKeyEventHandler((e) => !isAppShortcut(e));
  const el = document.createElement("div");
  el.className = "xterm-host";
  const inst: Instance = { term, fit, el, opened: false, started: false, exited: false, spec, opts };
  term.onData((data) => {
    if (inst.exited) {
      if (data.includes("\r")) {
        term.reset();
        start(inst);
      }
      return;
    }
    void backend?.call("term.write", { id: spec.id, data: textToB64(data) }).catch(() => {});
  });
  return inst;
}

/** Coloca o terminal `spec` dentro de `container`. Devolve a função que o tira de lá. */
export function mountTerminal(container: HTMLElement, spec: PaneSpec, opts: { fontSize: number; shell: string }): () => void {
  let inst = instances.get(spec.id);
  if (!inst) {
    inst = create(spec, opts);
    instances.set(spec.id, inst);
  }
  inst.spec = spec;
  inst.opts = opts;
  const i = inst;
  container.appendChild(i.el);
  if (!i.opened) {
    i.term.open(i.el);
    i.opened = true;
  }
  const fitNow = () => {
    try {
      i.fit.fit();
    } catch {
      /* elemento ainda sem tamanho */
    }
  };
  fitNow();

  if (!i.started && !i.exited) start(i);

  let last = "";
  const ro = new ResizeObserver(() => {
    fitNow();
    const key = `${i.term.cols}x${i.term.rows}`;
    if (key !== last && i.started) {
      last = key;
      void backend?.call("term.resize", { id: spec.id, cols: i.term.cols, rows: i.term.rows }).catch(() => {});
    }
  });
  ro.observe(container);

  return () => {
    ro.disconnect();
    if (i.el.parentElement === container) container.removeChild(i.el);
  };
}

export function focusTerminal(id: string) {
  instances.get(id)?.term.focus();
}

/** Fecha e descarta um terminal (botão × ou shell que saiu). */
export function disposeTerminal(id: string) {
  const inst = instances.get(id);
  instances.delete(id);
  inst?.term.dispose();
  inst?.el.remove();
  void backend?.call("term.close", { id }).catch(() => {});
}

/** Fecha e descarta os terminais cujo id começa com `prefix` (ex.: ao arquivar). */
export function disposeTerminals(prefix: string) {
  for (const [id, inst] of [...instances]) {
    if (!id.startsWith(prefix)) continue;
    inst.term.dispose();
    instances.delete(id);
    void backend?.call("term.close", { id }).catch(() => {});
  }
}

export function setTerminalFontSize(size: number) {
  for (const inst of instances.values()) {
    inst.term.options.fontSize = size;
    try {
      inst.fit.fit();
    } catch {
      /* oculto */
    }
  }
}
