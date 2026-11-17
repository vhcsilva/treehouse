//! Tudo que o app precisa saber sobre o WSL: listar distros, instalar/atualizar o
//! agente e montar o comando que conecta nele.

use std::path::Path;

use tokio::process::Command;

/// Onde o agente fica instalado dentro do WSL (relativo ao $HOME).
pub const AGENT_REL_PATH: &str = ".local/share/treehouse/bin/treehouse-agent";

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `Command` que não abre janela de console no Windows.
pub fn quiet(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Comando que inicia a ponte com o agente (stdio ↔ daemon).
pub fn connect_command(distro: Option<&str>) -> Command {
    if cfg!(windows) {
        let mut cmd = quiet("wsl.exe");
        if let Some(d) = distro.filter(|d| !d.is_empty()) {
            cmd.args(["-d", d]);
        }
        // `--exec` não passa pelo shell de login: nada de `echo` do .bashrc no protocolo.
        cmd.args([
            "--exec",
            "sh",
            "-c",
            &format!("exec \"$HOME/{AGENT_REL_PATH}\" connect"),
        ]);
        cmd
    } else {
        // Desenvolvimento fora do Windows: usa o agente local.
        let bin = std::env::var("TREEHOUSE_AGENT_BIN").unwrap_or_else(|_| "treehouse-agent".into());
        let mut cmd = quiet(&bin);
        cmd.arg("connect");
        cmd
    }
}

/// Distros instaladas (`wsl.exe -l -q` responde em UTF-16).
pub async fn list_distros() -> Vec<String> {
    if !cfg!(windows) {
        return vec!["local".into()];
    }
    let Ok(out) = quiet("wsl.exe").args(["-l", "-q"]).output().await else {
        return vec![];
    };
    decode_wsl_output(&out.stdout)
        .lines()
        .map(|l| l.trim().trim_matches('\0').to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Mensagens do próprio wsl.exe vêm em UTF-16LE; saídas de programas Linux, em UTF-8.
pub fn decode_wsl_output(bytes: &[u8]) -> String {
    let bom = bytes.starts_with(&[0xff, 0xfe]);
    let bytes = if bom { &bytes[2..] } else { bytes };
    let looks_utf16 = bom || (bytes.len() >= 2 && bytes.iter().skip(1).step_by(2).take(16).all(|b| *b == 0));
    if looks_utf16 {
        let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units).trim_start_matches('\u{feff}').to_string()
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

fn wsl_exec(distro: Option<&str>) -> Command {
    let mut cmd = quiet("wsl.exe");
    if let Some(d) = distro.filter(|d| !d.is_empty()) {
        cmd.args(["-d", d]);
    }
    cmd.arg("--exec");
    cmd
}

/// Versão do agente instalado no WSL, se houver.
pub async fn installed_agent_version(distro: Option<&str>) -> Option<String> {
    if !cfg!(windows) {
        return None;
    }
    let out = wsl_exec(distro)
        .args(["sh", "-c", &format!("\"$HOME/{AGENT_REL_PATH}\" version")])
        .output()
        .await
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Copia o binário empacotado com o app para dentro do WSL.
/// Copia para um arquivo novo e renomeia: funciona mesmo com o daemon antigo rodando.
pub async fn install_agent(distro: Option<&str>, bundled: &Path) -> anyhow::Result<()> {
    let win_path = bundled.to_string_lossy().to_string();
    let out = wsl_exec(distro).args(["wslpath", "-u", &win_path]).output().await?;
    anyhow::ensure!(out.status.success(), "wslpath falhou: {}", decode_wsl_output(&out.stderr));
    let linux_path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let script = format!(
        "set -e; d=\"$HOME/{dir}\"; mkdir -p \"$d\"; cp \"$1\" \"$d/treehouse-agent.new\"; \
         chmod +x \"$d/treehouse-agent.new\"; mv -f \"$d/treehouse-agent.new\" \"$d/treehouse-agent\"",
        dir = AGENT_REL_PATH.trim_end_matches("/treehouse-agent")
    );
    let out = wsl_exec(distro).args(["sh", "-c", &script, "sh", &linux_path]).output().await?;
    anyhow::ensure!(out.status.success(), "falha ao instalar o agente: {}", decode_wsl_output(&out.stderr));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf16_distro_list() {
        let text = "Ubuntu-24.04\r\nDebian\r\n";
        let mut bytes = vec![0xff, 0xfe];
        for u in text.encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        let s = decode_wsl_output(&bytes);
        let names: Vec<&str> = s.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        assert_eq!(names, vec!["Ubuntu-24.04", "Debian"]);
        assert_eq!(decode_wsl_output(b"treehouse-agent 0.1.0"), "treehouse-agent 0.1.0");
    }
}
