import { useEffect, useMemo, useRef, useState } from "react";
import { branchName, featureSlug, joinPath } from "../lib/naming";
import { activeFeatures, activeWorkspace, selectedFeature, useStore } from "../store";
import { Icon } from "./Icon";

function useEscape(onClose: () => void) {
  useEffect(() => {
    const h = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [onClose]);
}

export function NewFeatureModal() {
  const s = useStore();
  const ws = activeWorkspace(s);
  const close = () => s.openModal(null);
  useEscape(close);
  const repos = ws?.repos.filter((r) => r.enabled) ?? [];
  const [name, setName] = useState("");
  const [jira, setJira] = useState("");
  const [branch, setBranch] = useState("");
  const [branchEdited, setBranchEdited] = useState(false);
  const [chosen, setChosen] = useState<Record<string, boolean>>(() => (repos[0] ? { [repos[0].name]: true } : {}));
  const [bases, setBases] = useState<Record<string, string>>(() => Object.fromEntries(repos.map((r) => [r.name, r.base])));
  const [openKiro, setOpenKiro] = useState(true);
  const nameRef = useRef<HTMLInputElement>(null);
  useEffect(() => nameRef.current?.focus(), []);

  const jiraKey = jira.trim().toUpperCase() || null;
  const autoBranch = ws ? branchName(ws.branchPattern, name || "nova-feature", jiraKey) : "";
  const finalBranch = branchEdited ? branch : autoBranch;
  const selected = repos.filter((r) => chosen[r.name]);
  const slug = featureSlug(name, jiraKey);
  const root = ws ? joinPath(ws.worktreesDir, slug) : "";
  const preview = useMemo(
    () =>
      selected.length === 0
        ? "# escolha ao menos um repositório"
        : selected
            .map((r) => `git -C ${r.path} worktree add ${joinPath(root, r.name)} -b ${finalBranch} ${bases[r.name] ?? r.base}`)
            .join("\n") + `\n${openKiro ? `kiro ${root}.code-workspace` : ""}`,
    [selected, root, finalBranch, bases, openKiro],
  );
  const valid = name.trim().length > 0 && selected.length > 0 && finalBranch.length > 0;

  const submit = async () => {
    if (!valid) return;
    await s.createFeature({
      name: name.trim(),
      jiraKey,
      branch: finalBranch,
      repos: selected.map((r) => ({ name: r.name, base: bases[r.name] ?? r.base })),
      openKiro,
    });
  };

  return (
    <div className="scrim" style={{ paddingTop: 50 }} onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <form
        role="dialog"
        aria-label="Nova feature"
        className="modal"
        style={{ width: "100%", maxWidth: 640, padding: 24, display: "flex", flexDirection: "column", gap: 16 }}
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <div style={{ display: "flex", alignItems: "center" }}>
          <h2 style={{ margin: 0, fontSize: 18 }}>Nova feature</h2>
          <button type="button" className="link" aria-label="Fechar" style={{ marginLeft: "auto", color: "var(--quiet)" }} onClick={close}>
            <Icon name="close" />
          </button>
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "minmax(0,2fr) minmax(0,1fr)", gap: 12 }}>
          <label className="fld">
            Nome
            <input ref={nameRef} className="inp" value={name} onChange={(e) => setName(e.target.value)} placeholder="Checkout com Pix" />
          </label>
          <label className="fld">
            Card do Jira (opcional)
            <input className="inp mono" value={jira} onChange={(e) => setJira(e.target.value)} placeholder="PAY-412" />
          </label>
        </div>
        <label className="fld">
          Branch
          <input
            className="inp mono"
            value={finalBranch}
            onChange={(e) => {
              setBranch(e.target.value);
              setBranchEdited(true);
            }}
          />
        </label>
        <div className="fld">
          Repositórios
          <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
            {repos.map((r) => (
              <button
                type="button"
                key={r.name}
                className={`repo-toggle ${chosen[r.name] ? "on" : ""}`}
                aria-pressed={!!chosen[r.name]}
                onClick={() => setChosen((c) => ({ ...c, [r.name]: !c[r.name] }))}
              >
                {chosen[r.name] ? "✓ " : ""}
                {r.name}
              </button>
            ))}
          </div>
          <span className="hint">Normalmente um só. Marque mais de um quando a feature atravessar repositórios.</span>
        </div>
        {selected.length > 0 && (
          <div style={{ display: "grid", gridTemplateColumns: `repeat(${Math.min(selected.length, 3)}, minmax(0,1fr))`, gap: 12 }}>
            {selected.map((r) => (
              <label key={r.name} className="fld">
                Criar {r.name} a partir de
                <input className="inp mono" value={bases[r.name] ?? r.base} onChange={(e) => setBases((b) => ({ ...b, [r.name]: e.target.value }))} />
              </label>
            ))}
          </div>
        )}
        <label className="chk">
          <input type="checkbox" checked={openKiro} onChange={(e) => setOpenKiro(e.target.checked)} /> Abrir no Kiro ao criar
        </label>
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <span className="cap">VAI EXECUTAR</span>
          <pre className="preview">{preview}</pre>
        </div>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button type="button" className="btn" onClick={close}>
            Cancelar
          </button>
          <button type="submit" className="btn primary" disabled={!valid || !!s.busy}>
            Criar feature
          </button>
        </div>
      </form>
    </div>
  );
}

export function ArchiveModal() {
  const s = useStore();
  const f = selectedFeature(s);
  const close = () => s.openModal(null);
  useEscape(close);
  const [deleteBranch, setDeleteBranch] = useState(false);
  const [force, setForce] = useState(false);
  if (!f) return null;
  const changes = Object.values(s.status[f.id] ?? {}).reduce((a, r) => a + r.changes.length, 0);
  return (
    <div className="scrim" style={{ paddingTop: 140 }} onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div role="alertdialog" aria-label="Arquivar feature" className="modal" style={{ width: "100%", maxWidth: 540, padding: 24, display: "flex", flexDirection: "column", gap: 14 }}>
        <h2 style={{ margin: 0, fontSize: 18 }}>Arquivar “{f.name}”?</h2>
        {changes > 0 && (
          <div style={{ display: "flex", flexDirection: "column", gap: 8, padding: 12, borderRadius: 10, background: "rgba(251,191,36,0.1)", border: "1px solid rgba(251,191,36,0.35)", color: "#FDE68A" }}>
            <span>⚠ Há {changes} arquivos com alterações não commitadas.</span>
            <label className="chk" style={{ color: "#FDE68A" }}>
              <input type="checkbox" checked={force} onChange={(e) => setForce(e.target.checked)} /> Descartar as alterações e arquivar mesmo assim
            </label>
          </div>
        )}
        <span style={{ color: "var(--ink-2)" }}>Isso vai:</span>
        <ul style={{ margin: 0, paddingLeft: 18, display: "flex", flexDirection: "column", gap: 6, color: "var(--ink-2)" }}>
          <li>encerrar os terminais e processos da feature</li>
          <li>
            remover {f.repos.length > 1 ? "as worktrees" : "a worktree"} em <span className="mono" style={{ fontSize: 12 }}>{f.root}</span>
          </li>
          <li>manter a branch no remoto intacta</li>
        </ul>
        <label className="chk">
          <input type="checkbox" checked={deleteBranch} onChange={(e) => setDeleteBranch(e.target.checked)} /> Também apagar a branch local{" "}
          <span className="mono" style={{ fontSize: 11, color: "var(--muted)" }}>{f.branch}</span>
        </label>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button className="btn" onClick={close}>
            Cancelar
          </button>
          <button className="btn danger" disabled={changes > 0 && !force} onClick={() => void s.archiveFeature({ deleteBranch, force })}>
            Arquivar
          </button>
        </div>
      </div>
    </div>
  );
}

interface PaletteItem {
  id: string;
  label: string;
  hint?: string;
  color?: string;
  run: () => void;
}

export function Palette() {
  const s = useStore();
  const close = () => s.openModal(null);
  useEscape(close);
  const [q, setQ] = useState("");
  const [idx, setIdx] = useState(0);
  const features = activeFeatures(s);
  const items: PaletteItem[] = [
    ...features.map((f, i) => ({ id: f.id, label: f.name, hint: `${f.branch} · Ctrl ${i + 1}`, color: f.color, run: () => s.select(f.id) })),
    { id: "c-new", label: "Nova feature", hint: "Ctrl Shift N", run: () => s.openModal("new") },
    { id: "c-kiro", label: "Abrir a feature atual no Kiro", hint: "Ctrl Shift O", run: () => void s.openKiro() },
    { id: "c-up", label: "Atualizar da base", hint: "Ctrl Shift U", run: () => void s.updateFromBase(false) },
    { id: "c-up-stash", label: "Atualizar da base com stash automático", run: () => void s.updateFromBase(true) },
    { id: "c-abort", label: "Abortar rebase em andamento", run: () => void s.abortRebase() },
    { id: "c-term", label: "Novo terminal na feature atual", run: () => void s.addTerminal() },
    { id: "c-archive", label: "Arquivar a feature atual", run: () => s.openModal("archive") },
    { id: "c-overview", label: "Visão geral", run: () => s.setView("overview") },
    { id: "c-latency", label: "Medir latência com o WSL", run: () => void s.measureLatency() },
    { id: "c-settings", label: "Configurações", hint: "Ctrl ,", run: () => s.setView("settings") },
  ];
  const norm = (t: string) => t.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  const shown = items.filter((i) => norm(`${i.label} ${i.hint ?? ""}`).includes(norm(q)));
  const pick = (i: PaletteItem | undefined) => {
    if (!i) return;
    if (!["c-new", "c-archive"].includes(i.id)) close();
    i.run();
  };
  return (
    <div className="scrim" style={{ paddingTop: 110 }} onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div role="dialog" aria-label="Paleta de comandos" className="modal" style={{ width: "100%", maxWidth: 620, overflow: "hidden" }}>
        <label style={{ display: "flex", alignItems: "center", gap: 10, padding: "14px 16px", borderBottom: "1px solid rgba(255,255,255,0.08)" }}>
          <span className="mono" style={{ color: "#a78bfa" }}>❯</span>
          <input
            autoFocus
            aria-label="Buscar"
            value={q}
            onChange={(e) => {
              setQ(e.target.value);
              setIdx(0);
            }}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") setIdx((i) => Math.min(i + 1, shown.length - 1));
              if (e.key === "ArrowUp") setIdx((i) => Math.max(i - 1, 0));
              if (e.key === "Enter") pick(shown[idx]);
            }}
            placeholder="Feature ou comando…"
            style={{ flex: "1 1 auto", background: "transparent", border: 0, color: "var(--ink)", fontSize: 15, outline: "none" }}
          />
          <kbd className="kbd">Esc</kbd>
        </label>
        <div style={{ padding: 8, display: "flex", flexDirection: "column", gap: 2, maxHeight: 420, overflow: "auto" }}>
          {shown.map((i, n) => (
            <button
              key={i.id}
              onMouseEnter={() => setIdx(n)}
              onClick={() => pick(i)}
              style={{ display: "flex", alignItems: "center", gap: 10, background: n === idx ? "rgba(139,92,246,0.18)" : "transparent", border: 0, borderRadius: 8, padding: "9px 10px", color: "var(--ink)", fontSize: 13, textAlign: "left" }}
            >
              {i.color && <span className="dot" style={{ background: i.color }} />}
              <span style={{ flex: "1 1 auto" }}>{i.label}</span>
              {i.hint && <span className="mono" style={{ fontSize: 11, color: "var(--muted)" }}>{i.hint}</span>}
            </button>
          ))}
          {shown.length === 0 && <p className="hint" style={{ padding: 10 }}>Nada encontrado.</p>}
        </div>
      </div>
    </div>
  );
}
