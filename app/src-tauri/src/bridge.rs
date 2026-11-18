//! Conexão com o `treehouse-agent`: JSON-RPC, uma mensagem por linha.
//! Respostas resolvem a chamada pendente; notificações viram eventos para a interface.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::{oneshot, Mutex};
use treehouse_protocol::{methods, RpcError, PROTOCOL_VERSION};

use crate::wsl;

pub const EVENT_AGENT: &str = "agent-event";
pub const EVENT_STATUS: &str = "agent-status";

type Pending = Arc<std::sync::Mutex<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>>;

struct Conn {
    stdin: ChildStdin,
    child: Child,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub connected: bool,
    pub distro: Option<String>,
    pub version: Option<String>,
    pub error: Option<String>,
}

pub struct Bridge {
    app: AppHandle,
    conn: Mutex<Option<Conn>>,
    pending: Pending,
    next_id: AtomicU64,
    distro: std::sync::Mutex<Option<String>>,
    status: std::sync::Mutex<AgentStatus>,
}

impl Bridge {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            conn: Mutex::new(None),
            pending: Arc::new(std::sync::Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            distro: std::sync::Mutex::new(None),
            status: std::sync::Mutex::new(AgentStatus::default()),
        })
    }

    pub fn status(&self) -> AgentStatus {
        self.status.lock().unwrap().clone()
    }

    fn set_status(&self, f: impl FnOnce(&mut AgentStatus)) {
        let snapshot = {
            let mut s = self.status.lock().unwrap();
            f(&mut s);
            s.clone()
        };
        let _ = self.app.emit(EVENT_STATUS, snapshot);
    }

    pub async fn set_distro(self: &Arc<Self>, distro: Option<String>) {
        *self.distro.lock().unwrap() = distro;
        self.disconnect().await;
    }

    pub async fn disconnect(&self) {
        if let Some(mut c) = self.conn.lock().await.take() {
            let _ = c.child.kill().await;
        }
        self.fail_pending("conexão reiniciada");
        self.set_status(|s| s.connected = false);
    }

    fn fail_pending(&self, why: &str) {
        let waiters: Vec<_> = self.pending.lock().unwrap().drain().collect();
        for (_, tx) in waiters {
            let _ = tx.send(Err(RpcError { code: -32000, message: why.into(), data: None }));
        }
    }

    async fn connect(self: &Arc<Self>) -> Result<(), RpcError> {
        let distro = self.distro.lock().unwrap().clone();
        let mut cmd = wsl::connect_command(distro.as_deref());
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| {
            let msg = format!("não foi possível iniciar o agente: {e}");
            self.set_status(|s| {
                s.connected = false;
                s.error = Some(msg.clone());
            });
            RpcError { code: -32000, message: msg, data: None }
        })?;
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        *self.conn.lock().await = Some(Conn { stdin, child });

        let this = self.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                this.handle_line(&line);
            }
            // A ponte caiu (WSL desligado, agente encerrado…).
            this.conn.lock().await.take();
            this.fail_pending("o agente desconectou");
            this.set_status(|s| s.connected = false);
        });

        self.set_status(|s| {
            s.connected = true;
            s.distro = distro.clone();
            s.error = None;
        });
        Ok(())
    }

    fn handle_line(&self, line: &str) {
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        if let Some(id) = v.get("id").and_then(Value::as_u64) {
            if let Some(tx) = self.pending.lock().unwrap().remove(&id) {
                let res = match v.get("error") {
                    Some(e) if !e.is_null() => Err(serde_json::from_value(e.clone()).unwrap_or(RpcError {
                        code: -32603,
                        message: e.to_string(),
                        data: None,
                    })),
                    _ => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                };
                let _ = tx.send(res);
            }
        } else if v.get("method").is_some() {
            let _ = self.app.emit(EVENT_AGENT, v);
        }
    }

    /// Chama um método do agente, conectando se preciso.
    pub async fn call(self: &Arc<Self>, method: &str, params: Value) -> Result<Value, RpcError> {
        if self.conn.lock().await.is_none() {
            self.connect().await?;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string() + "\n";
        {
            let mut guard = self.conn.lock().await;
            let Some(conn) = guard.as_mut() else {
                self.pending.lock().unwrap().remove(&id);
                return Err(RpcError { code: -32000, message: "agente desconectado".into(), data: None });
            };
            if let Err(e) = conn.stdin.write_all(msg.as_bytes()).await {
                self.pending.lock().unwrap().remove(&id);
                return Err(RpcError { code: -32000, message: e.to_string(), data: None });
            }
            let _ = conn.stdin.flush().await;
        }
        // `git fetch` pode demorar; terminais respondem na hora.
        let timeout = if method.starts_with("term.") { 15 } else { 180 };
        match tokio::time::timeout(Duration::from_secs(timeout), rx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => Err(RpcError { code: -32000, message: "conexão encerrada".into(), data: None }),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(RpcError { code: -32000, message: format!("{method}: tempo esgotado"), data: None })
            }
        }
    }

    /// Envia sem esperar resposta (teclas do terminal, redimensionar).
    /// Nada de registro pendente nem ida e volta: a tecla segue na hora.
    pub async fn notify(self: &Arc<Self>, method: &str, params: Value) -> Result<(), RpcError> {
        if self.conn.lock().await.is_none() {
            self.connect().await?;
        }
        let msg = json!({"jsonrpc": "2.0", "method": method, "params": params}).to_string() + "\n";
        let mut guard = self.conn.lock().await;
        let Some(conn) = guard.as_mut() else {
            return Err(RpcError { code: -32000, message: "agente desconectado".into(), data: None });
        };
        conn.stdin
            .write_all(msg.as_bytes())
            .await
            .map_err(|e| RpcError { code: -32000, message: e.to_string(), data: None })?;
        let _ = conn.stdin.flush().await;
        Ok(())
    }

    /// Pede para o daemon atual sair; a próxima chamada sobe o binário instalado.
    pub async fn restart_daemon(self: &Arc<Self>) {
        let _ = self.call(methods::SHUTDOWN, json!({})).await;
        self.disconnect().await;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    /// Conecta, confere a versão e troca o daemon se ele for de outra versão do app.
    pub async fn handshake(self: &Arc<Self>) -> Result<Value, RpcError> {
        let v = self.call(methods::VERSION, json!({})).await?;
        let protocol = v.get("protocol").and_then(Value::as_u64).unwrap_or(0) as u32;
        let agent_version = v.get("agentVersion").and_then(Value::as_str).unwrap_or("").to_string();
        if protocol != PROTOCOL_VERSION || agent_version != env!("CARGO_PKG_VERSION") {
            // Daemon antigo: pede para sair e reconecta (o `connect` sobe o binário novo).
            self.restart_daemon().await;
            let v2 = self.call(methods::VERSION, json!({})).await?;
            self.set_status(|s| s.version = v2.get("agentVersion").and_then(Value::as_str).map(String::from));
            return Ok(v2);
        }
        self.set_status(|s| s.version = Some(agent_version));
        Ok(v)
    }
}
