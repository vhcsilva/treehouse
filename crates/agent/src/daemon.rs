//! Daemon persistente + ponte `connect`.
//!
//! O app roda `wsl.exe -d <distro> -- treehouse-agent connect`. O `connect` liga o
//! daemon se preciso e repassa stdin/stdout para o socket Unix dele. Assim os
//! terminais sobrevivem ao fechamento do app.

use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::{UnixListener, UnixStream};

use crate::paths;
use crate::server::{serve_connection, State};

pub async fn run_daemon(socket: &Path) -> anyhow::Result<()> {
    // Um socket que sobrou de um daemon morto é removido; um vivo impede a subida.
    if socket.exists() {
        if UnixStream::connect(socket).await.is_ok() {
            anyhow::bail!("já existe um daemon em {}", socket.display());
        }
        let _ = std::fs::remove_file(socket);
    }
    if let Some(dir) = socket.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let listener = UnixListener::bind(socket)?;
    // Só o próprio usuário pode falar com o daemon.
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;

    let state = State::new();
    eprintln!("treehouse-agent daemon {} ouvindo em {}", env!("CARGO_PKG_VERSION"), socket.display());
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                let (r, w) = stream.into_split();
                tokio::spawn(serve_connection(state.clone(), r, w));
            }
            _ = state.shutdown.notified() => {
                eprintln!("encerrando a pedido do app");
                tokio::time::sleep(Duration::from_millis(100)).await;
                break;
            }
        }
    }
    let _ = std::fs::remove_file(socket);
    Ok(())
}

/// Garante que o daemon está rodando e devolve uma conexão com ele.
pub async fn ensure_daemon(socket: &Path) -> anyhow::Result<UnixStream> {
    if let Ok(s) = UnixStream::connect(socket).await {
        return Ok(s);
    }
    spawn_daemon(socket)?;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Ok(s) = UnixStream::connect(socket).await {
            return Ok(s);
        }
    }
    anyhow::bail!("o daemon não respondeu em {}", socket.display())
}

fn spawn_daemon(socket: &Path) -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let log_dir = paths::state_dir();
    std::fs::create_dir_all(&log_dir)?;
    let log = std::fs::OpenOptions::new().create(true).append(true).open(log_dir.join("agent.log"))?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("daemon")
        .env("TREEHOUSE_SOCKET", socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(log));
    // SAFETY: setsid só cria uma nova sessão; desliga o daemon do terminal do `connect`.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    cmd.spawn()?;
    Ok(())
}

/// Ponte stdio ↔ socket do daemon.
pub async fn run_connect(socket: &Path) -> anyhow::Result<()> {
    let stream = ensure_daemon(socket).await?;
    let (mut sr, mut sw) = stream.into_split();
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let up = tokio::spawn(async move {
        let _ = tokio::io::copy(&mut stdin, &mut sw).await;
    });
    let down = tokio::spawn(async move {
        let _ = tokio::io::copy(&mut sr, &mut stdout).await;
    });
    tokio::select! {
        _ = up => {}
        _ = down => {}
    }
    Ok(())
}

/// Atende direto no stdio, sem daemon (desenvolvimento e testes).
pub async fn run_stdio() -> anyhow::Result<()> {
    let state: Arc<State> = State::new();
    serve_connection(state, tokio::io::stdin(), tokio::io::stdout()).await;
    Ok(())
}
