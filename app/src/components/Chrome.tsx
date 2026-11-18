import { branchesLabel } from "../lib/naming";
import { activeFeatures, activeWorkspace, selectedFeature, useStore, type View } from "../store";
import { Icon, type IconName } from "./Icon";

export function TitleBar() {
  const s = useStore();
  const ws = activeWorkspace(s);
  const f = selectedFeature(s);
  const crumb: Record<View, string> = {
    wt: f ? branchesLabel(f) : "nenhuma feature",
    overview: "visão geral",
    settings: "configurações",
    welcome: "",
  };
  return (
    <header className="glass titlebar">
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <div className="logo">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="#fff" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <circle cx="6" cy="5" r="2" />
            <circle cx="6" cy="19" r="2" />
            <circle cx="18" cy="9" r="2" />
            <path d="M6 7v10M18 11c0 4-6 3-12 6" />
          </svg>
        </div>
        <span style={{ fontWeight: 700, fontSize: 14 }}>Treehouse</span>
      </div>
      {s.workspaces.length > 1 ? (
        <select
          className="inp"
          aria-label="Workspace"
          style={{ padding: "5px 8px", fontSize: 12 }}
          value={ws?.id}
          onChange={(e) => void s.saveSettings({ activeWorkspaceId: e.target.value }).then(() => s.connect(s.workspaces.find((w) => w.id === e.target.value)?.distro))}
        >
          {s.workspaces.map((w) => (
            <option key={w.id} value={w.id}>
              {w.name}
            </option>
          ))}
        </select>
      ) : (
        <span className="chip">{ws?.name}</span>
      )}
      <span className="mono" style={{ fontSize: 12, color: "var(--quiet)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        {crumb[s.view]}
      </span>
      <div style={{ flex: "1 1 auto" }} />
      <button className="search" onClick={() => s.openModal("palette")}>
        <Icon name="search" size={13} />
        Buscar feature ou comando
        <span style={{ marginLeft: "auto", display: "flex", gap: 3 }}>
          <kbd className="kbd">Ctrl</kbd>
          <kbd className="kbd">Shift</kbd>
          <kbd className="kbd">K</kbd>
        </span>
      </button>
      <div style={{ flex: "1 1 auto" }} />
      <button className="btn primary small" onClick={() => s.openModal("new")}>
        <Icon name="plus" size={13} /> Nova feature
      </button>
      <span className={`pill ${s.agent.connected ? "ok" : "bad"}`} title={s.agent.error ?? undefined}>
        <span className="dot" style={{ width: 6, height: 6, background: s.agent.connected ? "#34D399" : "#F87171" }} />
        {s.agent.connected ? `WSL · ${s.agent.distro ?? "padrão"}` : "WSL desconectado"}
      </span>
    </header>
  );
}

export function ActivityBar() {
  const s = useStore();
  const item = (view: View, label: string, icon: IconName) => (
    <button className={`act ${s.view === view ? "on" : ""}`} aria-label={label} title={label} aria-current={s.view === view ? "page" : undefined} onClick={() => s.setView(view)}>
      <Icon name={icon} size={18} />
    </button>
  );
  return (
    <nav className="glass activity" aria-label="Atividades">
      {item("wt", "Worktrees", "branch")}
      {item("overview", "Visão geral", "grid")}
      <div style={{ flex: "1 1 auto" }} />
      {item("settings", "Configurações", "sliders")}
    </nav>
  );
}

export function StatusBar() {
  const s = useStore();
  const f = selectedFeature(s);
  const st = f ? s.status[f.id] : undefined;
  const repos = st ? Object.values(st) : [];
  const changes = repos.reduce((a, r) => a + r.changes.length, 0);
  const ahead = repos.reduce((a, r) => a + r.ahead, 0);
  const behind = repos.reduce((a, r) => a + r.behind, 0);
  return (
    <footer className="glass statusbar">
      {f ? (
        <>
          <span style={{ color: "#c4b5fd" }} title={f.repos.map((r) => `${r.name}: ${r.branch}`).join("\n")}>⎇ {branchesLabel(f)}</span>
          <span>
            ↑{ahead} ↓{behind}
          </span>
          <span>{changes} alterações</span>
          {repos.some((r) => r.rebasing) && <span style={{ color: "var(--warn)" }}>rebase em andamento</span>}
        </>
      ) : (
        <span>{activeFeatures(s).length} features</span>
      )}
      <span style={{ marginLeft: "auto" }}>
        {s.backend?.simulated ? "modo simulado (navegador)" : `agente ${s.agent.version ?? "—"}`}
      </span>
      <span>Ctrl+1…9 troca de feature · Ctrl+Shift+K comandos</span>
    </footer>
  );
}

export function Toasts() {
  const { toasts, dismissToast } = useStore();
  return (
    <div className="toasts" role="status">
      {toasts.map((t) => (
        <div key={t.id} className={`modal toast ${t.kind}`}>
          <div style={{ minWidth: 0, flex: 1 }}>
            <div style={{ fontWeight: 600 }}>{t.title}</div>
            {t.detail && <pre>{t.detail}</pre>}
          </div>
          <button className="link" aria-label="Fechar aviso" onClick={() => dismissToast(t.id)}>
            <Icon name="close" size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}

export function Busy() {
  const busy = useStore((s) => s.busy);
  if (!busy) return null;
  return (
    <div className="busy">
      <div className="modal" style={{ padding: "16px 20px", display: "flex", gap: 12, alignItems: "center" }}>
        <div className="spinner" />
        {busy}
      </div>
    </div>
  );
}
