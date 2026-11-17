//! Atende uma conexão JSON-RPC (stdio ou socket do daemon).

use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc, Notify};
use treehouse_protocol::*;

use crate::error::{AgentError, Result};
use crate::{git, term::TermManager};

pub struct State {
    pub terms: TermManager,
    pub events: broadcast::Sender<Notification>,
    pub shutdown: Notify,
}

impl State {
    pub fn new() -> Arc<Self> {
        let (events, _) = broadcast::channel(16 * 1024);
        Arc::new(Self { terms: TermManager::new(events.clone()), events, shutdown: Notify::new() })
    }
}

pub fn version_info() -> VersionInfo {
    VersionInfo {
        agent_version: env!("CARGO_PKG_VERSION").to_string(),
        protocol: PROTOCOL_VERSION,
        pid: std::process::id(),
    }
}

/// Atende uma conexão até o cliente fechar.
pub async fn serve_connection<R, W>(state: Arc<State>, reader: R, mut writer: W)
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Saída: respostas + notificações, uma por linha.
    let mut events = state.events.subscribe();
    let writer_task = tokio::spawn(async move {
        loop {
            let line = tokio::select! {
                Some(l) = rx.recv() => l,
                ev = events.recv() => match ev {
                    Ok(n) => serde_json::to_string(&n).unwrap_or_default(),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                },
                else => break,
            };
            let mut buf = line.into_bytes();
            buf.push(b'\n');
            if writer.write_all(&buf).await.is_err() || writer.flush().await.is_err()
            {
                break;
            }
        }
    });

    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let state = state.clone();
        let tx = tx.clone();
        // Cada pedido roda em paralelo: um `git fetch` lento não trava o terminal.
        tokio::spawn(async move {
            let resp = match serde_json::from_str::<Request>(&line) {
                Ok(req) if req.id.is_null() => {
                    // Notificação: executa e não responde (erros vão para o log).
                    let method = req.method.clone();
                    if let Err(e) = dispatch(&state, req).await {
                        eprintln!("{method}: {e}");
                    }
                    return;
                }
                Ok(req) => {
                    let id = req.id.clone();
                    match dispatch(&state, req).await {
                        Ok(v) => Response::ok(id, v),
                        Err(e) => Response::err(id, e.into()),
                    }
                }
                Err(e) => Response::err(
                    Value::Null,
                    RpcError { code: codes::PARSE_ERROR, message: e.to_string(), data: None },
                ),
            };
            let _ = tx.send(serde_json::to_string(&resp).unwrap_or_default());
        });
    }
    drop(tx);
    writer_task.abort();
}

fn parse<T: DeserializeOwned>(v: Value) -> Result<T> {
    serde_json::from_value(v).map_err(|e| AgentError::invalid(e.to_string()))
}

async fn blocking<T, F>(f: F) -> Result<Value>
where
    T: serde::Serialize + Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let out = tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AgentError::internal(e.to_string()))??;
    Ok(serde_json::to_value(out).unwrap_or(Value::Null))
}

pub async fn dispatch(state: &Arc<State>, req: Request) -> Result<Value> {
    use methods::*;
    let p = req.params;
    match req.method.as_str() {
        VERSION => Ok(serde_json::to_value(version_info()).unwrap()),
        SHUTDOWN => {
            state.shutdown.notify_waiters();
            Ok(Value::Null)
        }
        WORKSPACE_SCAN => {
            let p: ScanParams = parse(p)?;
            blocking(move || git::scan(&p)).await
        }
        FEATURE_CREATE => {
            let p: CreateFeatureParams = parse(p)?;
            blocking(move || git::create_feature(&p)).await
        }
        FEATURE_ADD_REPO => {
            let p: AddRepoParams = parse(p)?;
            blocking(move || git::add_repo(&p)).await
        }
        FEATURE_ARCHIVE => {
            let p: ArchiveParams = parse(p)?;
            // Terminais abertos dentro da feature são encerrados antes.
            let root = crate::paths::expand(&p.root).to_string_lossy().into_owned();
            for t in state.terms.list() {
                if t.cwd.starts_with(&root) {
                    let _ = state.terms.close(&TermIdParams { id: t.id });
                }
            }
            blocking(move || git::archive(&p)).await
        }
        WORKTREE_LIST => {
            let p: WorktreeListParams = parse(p)?;
            blocking(move || git::worktree_list(&p)).await
        }
        GIT_STATUS => {
            let p: StatusParams = parse(p)?;
            blocking(move || git::status(&p)).await
        }
        GIT_LOG => {
            let p: LogParams = parse(p)?;
            blocking(move || git::log(&p)).await
        }
        GIT_REBASE => {
            let p: RebaseParams = parse(p)?;
            blocking(move || git::rebase(&p)).await
        }
        GIT_REBASE_ABORT => {
            let p: PathParams = parse(p)?;
            blocking(move || git::rebase_abort(&p)).await
        }
        FS_WRITE => {
            let p: WriteFileParams = parse(p)?;
            blocking(move || {
                let path = crate::paths::expand(&p.path);
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(&path, p.content)?;
                Ok(())
            })
            .await
        }
        FS_REMOVE => {
            let p: PathParams = parse(p)?;
            blocking(move || {
                let _ = std::fs::remove_file(crate::paths::expand(&p.path));
                Ok(())
            })
            .await
        }
        FS_EXPAND => {
            let p: PathParams = parse(p)?;
            Ok(Value::String(crate::paths::expand(&p.path).to_string_lossy().into_owned()))
        }
        GH_PRS => {
            let p: GhPrsParams = parse(p)?;
            blocking(move || git::gh_prs(&p)).await
        }
        TERM_OPEN => Ok(serde_json::to_value(state.terms.open(&parse(p)?)?).unwrap()),
        TERM_ATTACH => Ok(serde_json::to_value(state.terms.attach(&parse(p)?)?).unwrap()),
        TERM_WRITE => state.terms.write(&parse(p)?).map(|_| Value::Null),
        TERM_RESIZE => state.terms.resize(&parse(p)?).map(|_| Value::Null),
        TERM_CLOSE => state.terms.close(&parse(p)?).map(|_| Value::Null),
        TERM_LIST => Ok(serde_json::to_value(state.terms.list()).unwrap()),
        other => Err(AgentError::new(codes::METHOD_NOT_FOUND, format!("método desconhecido: {other}"))),
    }
}
