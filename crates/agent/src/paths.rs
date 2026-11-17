use std::path::PathBuf;

/// Expande `~` e `~/…` para o diretório do usuário.
pub fn expand(p: &str) -> PathBuf {
    if p == "~" {
        return home();
    }
    if let Some(rest) = p.strip_prefix("~/") {
        return home().join(rest);
    }
    PathBuf::from(p)
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

/// Caminho do socket do daemon.
pub fn socket_path() -> PathBuf {
    if let Some(p) = std::env::var_os("TREEHOUSE_SOCKET") {
        return PathBuf::from(p);
    }
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        let dir = PathBuf::from(dir);
        if dir.is_dir() {
            return dir.join("treehouse.sock");
        }
    }
    // SAFETY: getuid não tem pré-condições.
    let uid = unsafe { libc::getuid() };
    std::env::temp_dir().join(format!("treehouse-{uid}.sock"))
}

/// Pasta de logs do agente.
pub fn state_dir() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"));
    base.join("treehouse")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_tilde() {
        let h = home();
        assert_eq!(expand("~"), h);
        assert_eq!(expand("~/wt/x"), h.join("wt/x"));
        assert_eq!(expand("/abs/path"), PathBuf::from("/abs/path"));
        assert_eq!(expand("~user/x"), PathBuf::from("~user/x"));
    }
}
