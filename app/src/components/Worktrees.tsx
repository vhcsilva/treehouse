import { useEffect, useMemo, useRef } from "react";
import { panesFor, type PaneSpec } from "../lib/naming";
import type { Feature } from "../lib/types";
import { activeFeatures, activeWorkspace, selectedFeature, useStore } from "../store";
import { mountTerminal } from "../terminals";
import { Icon } from "./Icon";

const CHANGE_COLOR: Record<string, string> = { M: "#FCD34D", A: "#6EE7B7", D: "#FCA5A5", R: "#93C5FD", U: "#FCA5A5", "?": "#6EE7B7" };

function repoBadge(s: ReturnType<typeof useStore.getState>, f: Feature, repo: string) {
  const st = s.status[f.id]?.[repo];
  if (!st) return { text: "", color: "var(--muted)" };
  const parts = [];
  if (st.changes.length) parts.push(`${st.changes.length}M`);
  if (st.ahead) parts.push(`↑${st.ahead}`);
  if (st.behind) parts.push(`↓${st.behind}`);
  return { text: parts.join(" ") || "✓", color: st.changes.length || st.behind ? "#FCD34D" : "#6EE7B7" };
}

export function Explorer() {
  const s = useStore();
  const ws = activeWorkspace(s);
  const features = activeFeatures(s);
  return (
    <aside className="glass explorer" aria-label="Worktrees">
      <div style={{ display: "flex", alignItems: "center", padding: "12px 14px 8px" }}>
        <span className="cap">WORKTREES</span>
        <button className="btn small" style={{ marginLeft: "auto", padding: "2px 8px" }} aria-label="Nova feature" onClick={() => s.openModal("new")}>
          +
        </button>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 2, padding: "4px 8px 12px", overflow: "auto" }}>
        {features.length === 0 && <p className="hint" style={{ padding: "8px 10px" }}>Nenhuma feature ainda. Crie a primeira com “+”.</p>}
        {features.map((f, i) => {
          const sel = f.id === s.selectedId;
          const totalChanges = Object.values(s.status[f.id] ?? {}).reduce((a, r) => a + r.changes.length, 0);
          const item = (
            <button className="tree-item" onClick={() => s.select(f.id)} aria-current={sel ? "page" : undefined} title={`Ctrl+${i + 1}`}>
              <Icon name={sel ? "chevronDown" : "chevronRight"} size={10} stroke={3} />
              <span className="dot" style={{ background: f.color, boxShadow: sel ? "0 0 8px rgba(167,139,250,0.8)" : undefined }} />
              <span style={{ flex: "1 1 auto", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{f.name}</span>
              <span className="mono" style={{ fontSize: 10, color: sel ? "#c4b5fd" : "#FCD34D" }}>
                {sel ? f.jiraKey ?? "" : totalChanges ? `${totalChanges}M` : ""}
              </span>
            </button>
          );
          if (!sel) return <div key={f.id}>{item}</div>;
          return (
            <div key={f.id} className="tree-sel">
              {item}
              {f.repos.map((r) => {
                const b = repoBadge(s, f, r.name);
                return (
                  <div key={r.name} className="tree-repo">
                    <Icon name="folder" size={12} />
                    <span style={{ flex: "1 1 auto" }}>{r.name}</span>
                    <span style={{ color: b.color, fontSize: 11 }}>{b.text}</span>
                  </div>
                );
              })}
              <div style={{ height: 6 }} />
            </div>
          );
        })}
      </div>
      <div className="mono" style={{ marginTop: "auto", padding: "12px 14px", borderTop: "1px solid rgba(255,255,255,0.06)", fontSize: 11, color: "var(--muted)" }}>
        <div>{ws?.worktreesDir}</div>
        <div>{features.length} features ativas</div>
      </div>
    </aside>
  );
}

function FeatureHeader({ f }: { f: Feature }) {
  const s = useStore();
  const rebasing = Object.values(s.status[f.id] ?? {}).some((r) => r.rebasing);
  return (
    <section className="glass" style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: 14, padding: "14px 16px" }}>
      <div style={{ display: "flex", flexDirection: "column", gap: 8, minWidth: 0 }}>
        <h1 style={{ margin: 0, fontSize: 19, letterSpacing: "-0.01em" }}>{f.name}</h1>
        <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
          <span className="chip" style={{ color: "var(--ink)" }}>⎇ {f.branch}</span>
          {[...new Set(f.repos.map((r) => r.base))].map((b) => (
            <span key={b} className="chip">base {b}</span>
          ))}
          {f.jiraKey && (
            <span className="chip" style={{ background: "rgba(59,130,246,0.14)", borderColor: "rgba(96,165,250,0.35)", color: "#BFDBFE" }}>
              {f.jiraKey}
            </span>
          )}
        </div>
      </div>
      <div style={{ flex: "1 1 auto" }} />
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
        <button className="btn primary" onClick={() => void s.openKiro()}>
          <Icon name="code" size={14} /> Abrir no Kiro <kbd className="kbd" style={{ background: "rgba(255,255,255,0.18)", color: "#fff" }}>Ctrl⇧O</kbd>
        </button>
        {rebasing ? (
          <button className="btn danger" onClick={() => void s.abortRebase()}>Abortar rebase</button>
        ) : (
          <button className="btn" onClick={() => void s.updateFromBase()}>
            <Icon name="sync" size={14} /> {s.settings.updateStrategy === "rebase" ? "Rebase" : "Merge"} da base <kbd className="kbd">Ctrl⇧U</kbd>
          </button>
        )}
      </div>
    </section>
  );
}

function Pane({ spec }: { spec: PaneSpec }) {
  const ref = useRef<HTMLDivElement>(null);
  const fontSize = useStore((s) => s.settings.fontSize);
  const shell = useStore((s) => s.settings.shell);
  const ready = useStore((s) => s.agent.connected);
  const closeTerminal = useStore((s) => s.closeTerminal);
  useEffect(() => {
    if (!ref.current || !ready) return;
    return mountTerminal(ref.current, spec, { fontSize, shell });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [spec.id, ready]);
  return (
    <div className="pane">
      <div className="pane-head">
        <span className="dot" style={{ width: 6, height: 6, background: spec.command ? "#6EE7B7" : "#C4B5FD" }} />
        <span style={{ color: "var(--ink)" }}>{spec.title}</span>
        <span style={{ color: "var(--dim)", marginLeft: "auto", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{spec.cwd.split("/").slice(-2).join("/")}</span>
        {spec.closable && (
          <button className="link pane-close" aria-label={`Fechar ${spec.title}`} title="Fechar terminal" onClick={() => void closeTerminal(spec.id)}>
            <Icon name="close" size={12} />
          </button>
        )}
      </div>
      <div className="pane-body" ref={ref}>
        {!ready && <p className="hint">Conectando ao WSL…</p>}
      </div>
    </div>
  );
}

function Terminals({ f }: { f: Feature }) {
  const s = useStore();
  const ws = activeWorkspace(s);
  const panes = useMemo(() => panesFor(f, ws), [f, ws]);
  return (
    <section className="terms" aria-label="Terminais">
      <div style={{ display: "flex", alignItems: "center", gap: 4, padding: "6px 8px", borderBottom: "1px solid rgba(255,255,255,0.07)" }}>
        <span className="cap" style={{ padding: "0 6px" }}>TERMINAL</span>
        <span className="mono" style={{ fontSize: 11, color: "var(--dim)" }}>{f.slug}</span>
        <div style={{ flex: "1 1 auto" }} />
        <span className="hint" style={{ fontSize: 11 }}>os processos continuam rodando ao trocar de feature</span>
        <button className="link" aria-label="Novo terminal" title="Novo terminal" onClick={() => void s.addTerminal()} style={{ color: "var(--muted)", fontSize: 15 }}>
          +
        </button>
      </div>
      <div className="panes">
        {panes.map((p) => (
          <Pane key={p.id} spec={p} />
        ))}
      </div>
    </section>
  );
}

function GitPanel({ f }: { f: Feature }) {
  const s = useStore();
  const st = s.status[f.id] ?? {};
  const log = s.logs[f.id] ?? [];
  const changes = Object.entries(st).flatMap(([repo, r]) => r.changes.map((c) => ({ ...c, repo })));
  return (
    <aside className="glass gitpanel" aria-label="Git">
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        <div style={{ display: "flex", alignItems: "center" }}>
          <span className="cap">ALTERAÇÕES</span>
          <span className="mono" style={{ marginLeft: "auto", fontSize: 11, padding: "1px 7px", borderRadius: 999, background: "rgba(251,191,36,0.14)", color: "#FCD34D" }}>
            {changes.length}
          </span>
        </div>
        {changes.slice(0, 40).map((c) => (
          <div key={`${c.repo}/${c.path}`} className="change">
            <span style={{ color: CHANGE_COLOR[c.code], width: 12, fontWeight: 600 }}>{c.code}</span>
            <span title={`${c.repo}/${c.path}`}>{f.repos.length > 1 ? `${c.repo}/${c.path}` : c.path}</span>
          </div>
        ))}
        {changes.length === 0 && <span style={{ fontSize: 12, color: "#6EE7B7" }}>Árvore limpa</span>}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <span className="cap" style={{ marginBottom: 6 }}>HISTÓRICO · {f.repos[0].name}</span>
        {log.map((k) => (
          <div key={k.hash} className="commit">
            <div className="rail">
              <i style={{ background: k.inFeature ? f.color : "rgba(255,255,255,0.12)" }} />
              <b style={{ borderColor: k.inFeature ? f.color : "#5E6275" }} />
              <i style={{ background: k.inFeature ? f.color : "rgba(255,255,255,0.12)" }} />
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 1, padding: "5px 0", minWidth: 0 }}>
              <span style={{ fontSize: 12, color: k.inFeature ? "var(--ink)" : "var(--muted)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{k.subject}</span>
              <span className="mono" style={{ fontSize: 10.5, color: "var(--dim)" }}>
                {k.hash} · {k.when}
              </span>
            </div>
          </div>
        ))}
      </div>
      <button className="btn danger small" style={{ marginTop: "auto", justifyContent: "center" }} onClick={() => s.openModal("archive")}>
        Arquivar feature
      </button>
    </aside>
  );
}

export function WorktreesView() {
  const s = useStore();
  const f = selectedFeature(s);
  return (
    <>
      <Explorer />
      <div className="col" style={{ flex: "1 1 auto" }}>
        {f ? (
          <>
            <FeatureHeader f={f} />
            <div className="row">
              <Terminals f={f} />
              <GitPanel f={f} />
            </div>
          </>
        ) : (
          <div className="glass empty">
            <div>
              <p style={{ fontSize: 15, color: "var(--ink)" }}>Nenhuma feature selecionada</p>
              <button className="btn primary" onClick={() => s.openModal("new")}>
                <Icon name="plus" size={14} /> Nova feature
              </button>
            </div>
          </div>
        )}
      </div>
    </>
  );
}

export function OverviewView() {
  const s = useStore();
  const features = activeFeatures(s);
  const all = features.map((f) => Object.values(s.status[f.id] ?? {}));
  const stats = [
    { label: "features", value: features.length, color: "var(--ink)" },
    { label: "com alterações", value: all.filter((r) => r.some((x) => x.changes.length)).length, color: "#FCD34D" },
    { label: "desatualizadas", value: all.filter((r) => r.some((x) => x.behind)).length, color: "#FCD34D" },
  ];
  useEffect(() => {
    features.forEach((f) => void s.refreshStatus(f.id));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [features.length]);
  return (
    <main className="col" style={{ flex: "1 1 auto", overflow: "auto" }}>
      <div className="glass" style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: 24, padding: "16px 18px" }}>
        <div>
          <h1 style={{ margin: 0, fontSize: 19 }}>Visão geral</h1>
          <span className="hint">Todas as features do workspace {activeWorkspace(s)?.name}</span>
        </div>
        <div style={{ flex: "1 1 auto" }} />
        {stats.map((st) => (
          <div key={st.label}>
            <div style={{ fontSize: 22, fontWeight: 700, color: st.color }}>{st.value}</div>
            <div className="hint">{st.label}</div>
          </div>
        ))}
      </div>
      <div className="cards">
        {features.map((f) => {
          const repos = Object.values(s.status[f.id] ?? {});
          const changes = repos.reduce((a, r) => a + r.changes.length, 0);
          const behind = repos.reduce((a, r) => a + r.behind, 0);
          return (
            <div key={f.id} className="glass card">
              <div className="halo" />
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span className="dot" style={{ background: f.color, boxShadow: "0 0 10px rgba(167,139,250,0.8)" }} />
                {f.jiraKey ? <span className="chip">{f.jiraKey}</span> : <span className="hint">sem card</span>}
                <span style={{ marginLeft: "auto", fontSize: 12, color: changes || behind ? "#FCD34D" : "#6EE7B7" }}>
                  {changes ? `${changes} alterações` : behind ? `${behind} atrás da base` : "em dia"}
                </span>
              </div>
              <h2 style={{ margin: 0, fontSize: 17 }}>{f.name}</h2>
              <span className="mono" style={{ fontSize: 11, color: "var(--muted)" }}>{f.branch}</span>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                {f.repos.map((r) => (
                  <span key={r.name} className="chip">{r.name}</span>
                ))}
              </div>
              <button className="btn" style={{ justifyContent: "center" }} onClick={() => s.select(f.id)}>
                Entrar no contexto
              </button>
            </div>
          );
        })}
        <button
          onClick={() => s.openModal("new")}
          style={{ minHeight: 220, background: "rgba(255,255,255,0.02)", border: "1.5px dashed rgba(255,255,255,0.16)", borderRadius: 12, color: "var(--quiet)", display: "grid", placeItems: "center" }}
        >
          <span style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <Icon name="plus" /> Nova feature
          </span>
        </button>
      </div>
    </main>
  );
}
