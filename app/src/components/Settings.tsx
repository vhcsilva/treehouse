import { useEffect, useState } from "react";
import { errorMessage } from "../lib/backend";
import { slugify } from "../lib/naming";
import type { KiroStrategy, RepoConfig, WindowEffect, Workspace } from "../lib/types";
import { activeWorkspace, useStore, type SettingsTab } from "../store";
import { setTerminalFontSize } from "../terminals";

const blankWorkspace = (): Workspace => ({
  id: "",
  name: "",
  distro: "",
  projectsDir: "~/code",
  worktreesDir: "~/wt",
  branchPattern: "feature/{jira}-{slug}",
  defaultBase: "origin/main",
  kiroStrategy: "remote",
  repos: [],
});

function WorkspaceForm({ initial, submitLabel, onSaved }: { initial: Workspace; submitLabel: string; onSaved?: () => void }) {
  const s = useStore();
  const [ws, setWs] = useState<Workspace>(initial);
  const [distros, setDistros] = useState<string[]>([]);
  const [scanning, setScanning] = useState(false);
  const set = (patch: Partial<Workspace>) => setWs((w) => ({ ...w, ...patch }));
  const setRepo = (name: string, patch: Partial<RepoConfig>) => set({ repos: ws.repos.map((r) => (r.name === name ? { ...r, ...patch } : r)) });

  useEffect(() => setWs(initial), [initial]);
  useEffect(() => {
    void s.backend?.listDistros().then((d) => {
      setDistros(d);
      setWs((w) => (w.distro ? w : { ...w, distro: d[0] ?? "" }));
    });
  }, [s.backend]);

  const scan = async () => {
    if (!s.backend) return;
    setScanning(true);
    try {
      if (s.agent.distro !== ws.distro || !s.agent.connected) await s.backend.start(ws.distro || null);
      const found = await s.backend.call("workspace.scan", { dir: ws.projectsDir });
      const repos: RepoConfig[] = found.map((f) => {
        const prev = ws.repos.find((r) => r.name === f.name);
        return prev ?? { name: f.name, path: f.path, remote: f.remote, base: f.defaultBase ?? ws.defaultBase, command: "", enabled: true };
      });
      const name = ws.name || ws.projectsDir.split("/").filter(Boolean).pop() || "workspace";
      set({ repos, name, worktreesDir: ws.worktreesDir === "~/wt" ? `~/wt/${slugify(name)}` : ws.worktreesDir });
      s.toast(`${found.length} repositórios encontrados`, found.map((f) => f.name).join(", "), "ok");
    } catch (e) {
      s.toast("Não foi possível escanear", errorMessage(e), "error");
    } finally {
      setScanning(false);
    }
  };

  const save = async () => {
    const id = ws.id || `ws-${slugify(ws.name) || Date.now()}`;
    await s.saveWorkspace({ ...ws, id }, !ws.id);
    s.toast("Workspace salvo", ws.name, "ok");
    onSaved?.();
  };
  const valid = ws.name.trim() && ws.worktreesDir.trim() && ws.repos.some((r) => r.enabled);

  return (
    <section className="glass" style={{ padding: 18, display: "flex", flexDirection: "column", gap: 16 }}>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0,1fr))", gap: 14 }}>
        <label className="fld">
          Nome
          <input className="inp" value={ws.name} onChange={(e) => set({ name: e.target.value })} placeholder="pagamentos" />
        </label>
        <label className="fld">
          Distribuição WSL
          <select className="inp" value={ws.distro} onChange={(e) => set({ distro: e.target.value })}>
            {distros.map((d) => (
              <option key={d}>{d}</option>
            ))}
          </select>
        </label>
        <label className="fld">
          Pasta dos projetos (no WSL)
          <div style={{ display: "flex", gap: 8 }}>
            <input className="inp mono" style={{ flex: "1 1 auto" }} value={ws.projectsDir} onChange={(e) => set({ projectsDir: e.target.value })} />
            <button type="button" className="btn" disabled={scanning} onClick={() => void scan()}>
              {scanning ? "Escaneando…" : "Escanear"}
            </button>
          </div>
        </label>
        <label className="fld">
          Onde criar os worktrees
          <input className="inp mono" value={ws.worktreesDir} onChange={(e) => set({ worktreesDir: e.target.value })} />
        </label>
        <label className="fld">
          Padrão de nome da branch
          <input className="inp mono" value={ws.branchPattern} onChange={(e) => set({ branchPattern: e.target.value })} />
        </label>
        <label className="fld">
          Branch base padrão
          <input className="inp mono" value={ws.defaultBase} onChange={(e) => set({ defaultBase: e.target.value })} />
        </label>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        <span className="hint">Repositórios do workspace. A branch base usada para criar e atualizar pode ser diferente em cada um.</span>
        <div style={{ overflowX: "auto", border: "1px solid rgba(255,255,255,0.08)", borderRadius: 10 }}>
          <div style={{ minWidth: 720 }}>
            <div className="table-row" style={{ borderTop: 0, background: "rgba(255,255,255,0.03)" }}>
              <span className="cap">INCLUIR</span>
              <span className="cap">REPO</span>
              <span className="cap">REMOTO</span>
              <span className="cap">BRANCH BASE</span>
              <span className="cap">COMANDO AO ABRIR</span>
            </div>
            {ws.repos.map((r) => (
              <div key={r.name} className="table-row">
                <input type="checkbox" aria-label={`Incluir ${r.name}`} checked={r.enabled} onChange={(e) => setRepo(r.name, { enabled: e.target.checked })} />
                <span style={{ fontWeight: 600 }}>{r.name}</span>
                <span className="mono" style={{ fontSize: 11.5, color: "var(--muted)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={r.remote ?? r.path}>
                  {r.remote ?? r.path}
                </span>
                <input className="inp mono" aria-label={`Branch base de ${r.name}`} value={r.base} onChange={(e) => setRepo(r.name, { base: e.target.value })} style={{ padding: "6px 8px" }} />
                <input className="inp mono" aria-label={`Comando ao abrir ${r.name}`} value={r.command} placeholder="nenhum" onChange={(e) => setRepo(r.name, { command: e.target.value })} style={{ padding: "6px 8px" }} />
              </div>
            ))}
            {ws.repos.length === 0 && <p className="hint" style={{ padding: 12, margin: 0 }}>Clique em Escanear para encontrar os repositórios git da pasta.</p>}
          </div>
        </div>
      </div>
      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <button className="btn primary" disabled={!valid} onClick={() => void save()}>
          {submitLabel}
        </button>
      </div>
    </section>
  );
}

export function WelcomeView() {
  const initial = useState(blankWorkspace)[0];
  return (
    <main style={{ flex: 1, overflow: "auto", display: "flex", justifyContent: "center", padding: "40px 16px" }}>
      <div style={{ width: "100%", maxWidth: 920, display: "flex", flexDirection: "column", gap: 18 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <div className="logo" style={{ width: 40, height: 40, borderRadius: 10 }}>
            <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#fff" strokeWidth="2.2" strokeLinecap="round" aria-hidden="true">
              <circle cx="6" cy="5" r="2" />
              <circle cx="6" cy="19" r="2" />
              <circle cx="18" cy="9" r="2" />
              <path d="M6 7v10M18 11c0 4-6 3-12 6" />
            </svg>
          </div>
          <div>
            <h1 style={{ margin: 0, fontSize: 22 }}>Bem-vindo ao Treehouse</h1>
            <span className="hint">Escolha a distro, aponte a pasta dos projetos no WSL e escaneie. Dá para mudar tudo depois em Configurações.</span>
          </div>
        </div>
        <WorkspaceForm initial={initial} submitLabel="Começar" />
      </div>
    </main>
  );
}

export function SettingsView() {
  const s = useStore();
  const ws = activeWorkspace(s);
  const [adding, setAdding] = useState(false);
  const tabs: [SettingsTab, string][] = [
    ["ws", "Workspaces e repositórios"],
    ["kiro", "Kiro e terminal"],
    ["look", "Aparência"],
  ];
  const effects: [WindowEffect, string, string][] = [
    ["mica", "Vidro do Windows (Mica)", "Desfoca o papel de parede atrás da janela"],
    ["acrylic", "Vidro do Windows (Acrylic)", "Desfoca as janelas atrás; mais pesado"],
    ["none", "Sólido", "Sem transparência, mais leve para a GPU"],
  ];
  return (
    <>
      <aside className="glass" style={{ flex: "0 0 240px", padding: "12px 8px", display: "flex", flexDirection: "column", gap: 2, alignSelf: "flex-start" }}>
        <span className="cap" style={{ padding: "6px 10px 10px" }}>CONFIGURAÇÕES</span>
        {tabs.map(([id, label]) => (
          <button
            key={id}
            onClick={() => s.setSettingsTab(id)}
            aria-current={s.settingsTab === id ? "page" : undefined}
            className="tree-item"
            style={s.settingsTab === id ? { background: "rgba(139,92,246,0.18)", borderColor: "rgba(167,139,250,0.3)", color: "#f3f1ff", fontWeight: 600 } : undefined}
          >
            {label}
          </button>
        ))}
      </aside>
      <main className="col" style={{ flex: "1 1 auto", maxWidth: 1000, overflow: "auto" }}>
        {s.settingsTab === "ws" && (
          <>
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <h1 style={{ margin: 0, fontSize: 19 }}>Workspaces e repositórios</h1>
              <div style={{ flex: 1 }} />
              <button className="btn small" onClick={() => setAdding((a) => !a)}>
                {adding ? "Cancelar" : "+ Adicionar workspace"}
              </button>
            </div>
            {adding ? (
              <WorkspaceForm initial={blankWorkspace()} submitLabel="Criar workspace" onSaved={() => setAdding(false)} />
            ) : ws ? (
              <WorkspaceForm initial={ws} submitLabel="Salvar" />
            ) : null}
          </>
        )}
        {s.settingsTab === "kiro" && (
          <>
            <h1 style={{ margin: 0, fontSize: 19 }}>Kiro e terminal</h1>
            <section className="glass" style={{ padding: 18, display: "flex", flexDirection: "column", gap: 14 }}>
              <span style={{ fontWeight: 600, fontSize: 15 }}>Kiro</span>
              <label className="fld">
                Comando para abrir (no Windows)
                <input className="inp mono" value={s.settings.kiroCommand} onChange={(e) => void s.saveSettings({ kiroCommand: e.target.value })} />
              </label>
              {ws && (
                <label className="fld">
                  Como abrir as pastas do WSL ({ws.name})
                  <select className="inp" value={ws.kiroStrategy} onChange={(e) => void s.saveWorkspace({ ...ws, kiroStrategy: e.target.value as KiroStrategy })}>
                    <option value="remote">Integração com o WSL (Open Remote - WSL)</option>
                    <option value="unc">Caminho de rede \\wsl.localhost</option>
                  </select>
                </label>
              )}
              <span className="hint">Cada feature abre o próprio .code-workspace: uma janela por feature, com a cor dela na barra de título.</span>
            </section>
            <section className="glass" style={{ padding: 18, display: "flex", flexDirection: "column", gap: 14 }}>
              <span style={{ fontWeight: 600, fontSize: 15 }}>Terminal e git</span>
              <div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0,1fr))", gap: 14 }}>
                <label className="fld">
                  Shell (vazio = o seu shell padrão)
                  <input className="inp mono" value={s.settings.shell} placeholder="/bin/zsh" onChange={(e) => void s.saveSettings({ shell: e.target.value })} />
                </label>
                <label className="fld">
                  Tamanho da fonte
                  <input
                    className="inp"
                    type="number"
                    min={9}
                    max={24}
                    value={s.settings.fontSize}
                    onChange={(e) => {
                      const v = Number(e.target.value) || 13;
                      void s.saveSettings({ fontSize: v });
                      setTerminalFontSize(v);
                    }}
                  />
                </label>
                <label className="fld">
                  Atualizar da base com
                  <select className="inp" value={s.settings.updateStrategy} onChange={(e) => void s.saveSettings({ updateStrategy: e.target.value as "rebase" | "merge" })}>
                    <option value="rebase">rebase (padrão)</option>
                    <option value="merge">merge</option>
                  </select>
                </label>
              </div>
            </section>
            <section className="glass" style={{ padding: 18, display: "flex", flexDirection: "column", gap: 10 }}>
              <span style={{ fontWeight: 600, fontSize: 15 }}>Atalhos</span>
              {[
                ["Trocar de feature", "Ctrl 1…9"],
                ["Paleta de comandos", "Ctrl Shift K"],
                ["Nova feature", "Ctrl Shift N"],
                ["Abrir no Kiro", "Ctrl Shift O"],
                ["Atualizar da base", "Ctrl Shift U"],
                ["Configurações", "Ctrl ,"],
              ].map(([l, k]) => (
                <div key={l} style={{ display: "flex", alignItems: "center" }}>
                  <span style={{ flex: 1, color: "var(--ink-2)" }}>{l}</span>
                  <kbd className="kbd" style={{ fontSize: 11, padding: "3px 8px" }}>{k}</kbd>
                </div>
              ))}
              <span className="hint">Os atalhos usam Shift para não brigar com o shell (Ctrl+K e Ctrl+U apagam a linha no terminal).</span>
            </section>
          </>
        )}
        {s.settingsTab === "look" && (
          <>
            <h1 style={{ margin: 0, fontSize: 19 }}>Aparência</h1>
            <section className="glass" style={{ padding: 18, display: "flex", flexDirection: "column", gap: 16 }}>
              <div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0,1fr))", gap: 12 }}>
                {effects.map(([id, label, desc]) => (
                  <button
                    key={id}
                    aria-pressed={s.settings.effect === id}
                    onClick={() => {
                      void s.saveSettings({ effect: id });
                      document.body.classList.toggle("solid", id === "none");
                    }}
                    style={{
                      textAlign: "left",
                      display: "flex",
                      flexDirection: "column",
                      gap: 8,
                      padding: 12,
                      borderRadius: 10,
                      color: "var(--ink)",
                      background: s.settings.effect === id ? "rgba(139,92,246,0.16)" : "rgba(255,255,255,0.03)",
                      border: `1px solid ${s.settings.effect === id ? "rgba(167,139,250,0.45)" : "rgba(255,255,255,0.1)"}`,
                    }}
                  >
                    <span style={{ height: 56, borderRadius: 7, border: "1px solid rgba(255,255,255,0.12)", background: id === "none" ? "#14151C" : "linear-gradient(135deg, rgba(124,58,237,0.55), rgba(139,92,246,0.25))" }} />
                    <span style={{ fontWeight: 600 }}>{label}</span>
                    <span className="hint">{desc}</span>
                  </button>
                ))}
              </div>
              <label className="fld">
                Intensidade do desfoque dos painéis ({s.settings.blur}px)
                <input type="range" min={0} max={40} value={s.settings.blur} style={{ accentColor: "#8B5CF6" }} onChange={(e) => void s.saveSettings({ blur: Number(e.target.value) })} />
              </label>
              <div className="fld">
                Cor de destaque
                <div style={{ display: "flex", gap: 10 }}>
                  {["#8B5CF6", "#0EA5E9", "#10B981", "#F59E0B"].map((c) => (
                    <button
                      key={c}
                      aria-label={`Destaque ${c}`}
                      aria-pressed={s.settings.accent === c}
                      onClick={() => void s.saveSettings({ accent: c })}
                      style={{ width: 26, height: 26, borderRadius: "50%", background: c, border: 0, outline: s.settings.accent === c ? "2px solid #fff" : "none", outlineOffset: 2 }}
                    />
                  ))}
                </div>
              </div>
            </section>
          </>
        )}
      </main>
    </>
  );
}
