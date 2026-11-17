mod bridge;
mod db;
mod kiro;
mod wsl;

use std::sync::{Arc, Mutex};

use serde_json::Value;
use tauri::{Manager, State};
use treehouse_protocol::RpcError;

use bridge::{AgentStatus, Bridge};
use db::Db;

struct AppState {
    db: Mutex<Db>,
    bridge: Arc<Bridge>,
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

// ---------------------------------------------------------------------------
// Agente
// ---------------------------------------------------------------------------

/// Repassa uma chamada JSON-RPC ao agente. O erro volta como objeto `RpcError`.
#[tauri::command]
async fn agent_call(state: State<'_, AppState>, method: String, params: Value) -> Result<Value, RpcError> {
    state.bridge.call(&method, params).await
}

#[tauri::command]
fn agent_status(state: State<'_, AppState>) -> AgentStatus {
    state.bridge.status()
}

/// Escolhe a distro, instala/atualiza o agente se preciso e conecta.
#[tauri::command]
async fn agent_start(app: tauri::AppHandle, state: State<'_, AppState>, distro: Option<String>) -> CmdResult<Value> {
    let bridge = state.bridge.clone();
    bridge.set_distro(distro.clone()).await;
    if cfg!(windows) {
        let want = format!("treehouse-agent {}", env!("CARGO_PKG_VERSION"));
        let have = wsl::installed_agent_version(distro.as_deref()).await;
        if !have.as_deref().unwrap_or("").starts_with(&want) {
            let bundled = app
                .path()
                .resolve("resources/agent/treehouse-agent", tauri::path::BaseDirectory::Resource)
                .map_err(err)?;
            if !bundled.exists() {
                return Err(format!(
                    "o binário do agente não veio no instalador ({}). Veja o README.",
                    bundled.display()
                ));
            }
            wsl::install_agent(distro.as_deref(), &bundled).await.map_err(err)?;
        }
    }
    bridge.handshake().await.map_err(|e| e.message)
}

#[tauri::command]
async fn list_distros() -> Vec<String> {
    wsl::list_distros().await
}

// ---------------------------------------------------------------------------
// Kiro e janela
// ---------------------------------------------------------------------------

#[tauri::command]
async fn kiro_open(command: String, strategy: kiro::Strategy, distro: String, path: String) -> CmdResult<()> {
    kiro::open(&command, strategy, &distro, &path).await.map_err(err)
}

#[tauri::command]
fn set_window_effect(window: tauri::WebviewWindow, effect: String) -> CmdResult<()> {
    use tauri::utils::config::WindowEffectsConfig;
    use tauri::window::Effect;
    let effects = match effect.as_str() {
        "mica" => Some(WindowEffectsConfig { effects: vec![Effect::Mica], ..Default::default() }),
        "acrylic" => Some(WindowEffectsConfig { effects: vec![Effect::Acrylic], ..Default::default() }),
        _ => None,
    };
    window.set_effects(effects).map_err(err)
}

// ---------------------------------------------------------------------------
// Banco local
// ---------------------------------------------------------------------------

#[tauri::command]
fn db_list(state: State<'_, AppState>, collection: String) -> CmdResult<Vec<Value>> {
    state.db.lock().unwrap().list(&collection).map_err(err)
}

#[tauri::command]
fn db_put(state: State<'_, AppState>, collection: String, id: String, value: Value) -> CmdResult<()> {
    state.db.lock().unwrap().put(&collection, &id, &value).map_err(err)
}

#[tauri::command]
fn db_delete(state: State<'_, AppState>, collection: String, id: String) -> CmdResult<()> {
    state.db.lock().unwrap().delete(&collection, &id).map_err(err)
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let db = Db::open(&dir.join("treehouse.db"))?;
            let bridge = Bridge::new(app.handle().clone());
            app.manage(AppState { db: Mutex::new(db), bridge });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            agent_call,
            agent_status,
            agent_start,
            list_distros,
            kiro_open,
            set_window_effect,
            db_list,
            db_put,
            db_delete
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Treehouse");
}
