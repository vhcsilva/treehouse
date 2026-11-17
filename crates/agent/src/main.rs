use treehouse_agent::{daemon, paths, server};

const USAGE: &str = "\
treehouse-agent — agente do Treehouse para o WSL

uso:
  treehouse-agent connect     liga o daemon se preciso e conecta o stdio a ele (usado pelo app)
  treehouse-agent daemon      roda o daemon em primeiro plano
  treehouse-agent serve       atende no stdio, sem daemon (desenvolvimento)
  treehouse-agent version     mostra a versão
";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    let socket = paths::socket_path();
    match cmd.as_str() {
        "connect" => daemon::run_connect(&socket).await,
        "daemon" => daemon::run_daemon(&socket).await,
        "serve" => daemon::run_stdio().await,
        "version" | "--version" | "-V" => {
            let v = server::version_info();
            println!("treehouse-agent {} (protocolo {})", v.agent_version, v.protocol);
            Ok(())
        }
        _ => {
            eprint!("{USAGE}");
            std::process::exit(2);
        }
    }
}
