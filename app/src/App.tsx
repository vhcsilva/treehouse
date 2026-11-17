import { useEffect } from "react";
import { ActivityBar, Busy, StatusBar, TitleBar, Toasts } from "./components/Chrome";
import { ArchiveModal, NewFeatureModal, Palette } from "./components/Modals";
import { SettingsView, WelcomeView } from "./components/Settings";
import { OverviewView, WorktreesView } from "./components/Worktrees";
import { activeFeatures, useStore } from "./store";
import { bindTerminalEvents, setClosableExitHandler } from "./terminals";

function useShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useStore.getState();
      if (!e.ctrlKey || e.altKey || s.view === "welcome") return;
      const key = e.key.toLowerCase();
      if (!e.shiftKey && /^[1-9]$/.test(key)) {
        const f = activeFeatures(s)[Number(key) - 1];
        if (f) {
          e.preventDefault();
          s.select(f.id);
        }
        return;
      }
      if (!e.shiftKey && key === ",") {
        e.preventDefault();
        s.setView("settings");
        return;
      }
      if (!e.shiftKey) return;
      const actions: Record<string, () => void> = {
        k: () => s.openModal(s.modal === "palette" ? null : "palette"),
        n: () => s.openModal("new"),
        o: () => void s.openKiro(),
        u: () => void s.updateFromBase(false),
      };
      if (actions[key]) {
        e.preventDefault();
        actions[key]();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);
}

/** Atualiza o status do git da feature aberta a cada 5 s (e ao voltar para a janela). */
function useStatusPolling() {
  const selectedId = useStore((s) => s.selectedId);
  const connected = useStore((s) => s.agent.connected);
  useEffect(() => {
    if (!connected) return;
    const tick = () => document.visibilityState === "visible" && void useStore.getState().refreshStatus();
    tick();
    const t = setInterval(tick, 5000);
    window.addEventListener("focus", tick);
    return () => {
      clearInterval(t);
      window.removeEventListener("focus", tick);
    };
  }, [selectedId, connected]);
}

let initialized = false;

export default function App() {
  const s = useStore();
  useEffect(() => {
    if (initialized) return; // StrictMode roda efeitos duas vezes em desenvolvimento
    initialized = true;
    void s.init().then(() => {
      const b = useStore.getState().backend;
      if (b) bindTerminalEvents(b);
      setClosableExitHandler((id) => void useStore.getState().closeTerminal(id));
      document.body.classList.toggle("solid", useStore.getState().settings.effect === "none");
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useShortcuts();
  useStatusPolling();

  if (!s.ready) return null;
  return (
    <>
      <div className="glow a" />
      <div className="glow b" />
      <div className="glow c" />
      <div className="grid-bg" />
      <div className="app">
        {s.view === "welcome" ? (
          <WelcomeView />
        ) : (
          <>
            <TitleBar />
            <div className="row">
              <ActivityBar />
              {s.view === "wt" && <WorktreesView />}
              {s.view === "overview" && <OverviewView />}
              {s.view === "settings" && <SettingsView />}
            </div>
            <StatusBar />
          </>
        )}
      </div>
      {s.modal === "new" && <NewFeatureModal />}
      {s.modal === "archive" && <ArchiveModal />}
      {s.modal === "palette" && <Palette />}
      <Toasts />
      <Busy />
    </>
  );
}
