//! Abrir uma feature no Kiro.

use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Strategy {
    /// Extensão Open Remote - WSL: `--file-uri vscode-remote://wsl+<distro>/…`.
    Remote,
    /// Caminho de rede `\\wsl.localhost\<distro>\…`.
    Unc,
}

/// Argumentos para o executável do Kiro, a partir do caminho Linux do alvo
/// (um `.code-workspace` ou uma pasta).
pub fn args(strategy: Strategy, distro: &str, linux_path: &str) -> Vec<String> {
    let is_workspace = linux_path.ends_with(".code-workspace");
    match strategy {
        Strategy::Remote => {
            let flag = if is_workspace { "--file-uri" } else { "--folder-uri" };
            vec![flag.into(), format!("vscode-remote://wsl+{distro}{}", encode_path(linux_path))]
        }
        Strategy::Unc => vec![format!(r"\\wsl.localhost\{distro}{}", linux_path.replace('/', "\\"))],
    }
}

/// Codifica o que não pode ir cru numa URI (espaços, acentos), preservando as barras.
fn encode_path(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    for b in p.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub async fn open(command: &str, strategy: Strategy, distro: &str, linux_path: &str) -> anyhow::Result<()> {
    let a = args(strategy, distro, linux_path);
    if cfg!(windows) {
        // O `kiro` do PATH é um .cmd: precisa passar pelo cmd.exe.
        let mut cmd = crate::wsl::quiet("cmd.exe");
        cmd.arg("/C").arg(command).args(&a);
        cmd.spawn()?;
    } else {
        crate::wsl::quiet(command).arg(linux_path).spawn()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_uses_file_uri_for_workspaces() {
        assert_eq!(
            args(Strategy::Remote, "Ubuntu-24.04", "/home/vitor/wt/pagamentos/pay-412.code-workspace"),
            vec![
                "--file-uri".to_string(),
                "vscode-remote://wsl+Ubuntu-24.04/home/vitor/wt/pagamentos/pay-412.code-workspace".to_string()
            ]
        );
        assert_eq!(args(Strategy::Remote, "Ubuntu", "/home/x/a b")[1], "vscode-remote://wsl+Ubuntu/home/x/a%20b");
        assert_eq!(args(Strategy::Remote, "Ubuntu", "/home/x/dir")[0], "--folder-uri");
    }

    #[test]
    fn unc_path() {
        assert_eq!(
            args(Strategy::Unc, "Ubuntu", "/home/x/w.code-workspace"),
            vec![r"\\wsl.localhost\Ubuntu\home\x\w.code-workspace".to_string()]
        );
    }
}
