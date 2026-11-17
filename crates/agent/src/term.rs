//! Terminais (PTYs) que vivem no agente. Trocar de feature no app só troca qual
//! terminal é mostrado; os processos continuam rodando aqui.

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use tokio::sync::broadcast;
use treehouse_protocol::*;

use crate::error::{AgentError, Result};
use crate::paths::expand;

/// Quanto de saída cada terminal guarda para redesenhar a tela ao reconectar.
pub const SCROLLBACK_BYTES: usize = 2 * 1024 * 1024;

struct Term {
    cwd: String,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    scrollback: Mutex<VecDeque<u8>>,
    /// `None` = ainda rodando; `Some(code)` = terminou.
    exit: Mutex<Option<Option<i32>>>,
}

pub struct TermManager {
    terms: Mutex<HashMap<String, Arc<Term>>>,
    events: broadcast::Sender<Notification>,
}

impl TermManager {
    pub fn new(events: broadcast::Sender<Notification>) -> Self {
        Self { terms: Mutex::new(HashMap::new()), events }
    }

    pub fn open(&self, p: &TermOpenParams) -> Result<TermOpenResult> {
        if let Some(t) = self.get(&p.id) {
            if t.exit.lock().unwrap().is_none() {
                let _ = t.master.lock().unwrap().resize(size(p.cols, p.rows));
                return Ok(TermOpenResult { created: false, scrollback: snapshot(&t) });
            }
            self.terms.lock().unwrap().remove(&p.id);
        }

        let cwd = expand(&p.cwd);
        if !cwd.is_dir() {
            return Err(AgentError::invalid(format!("pasta não encontrada: {}", cwd.display())));
        }
        let shell = p
            .shell
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| std::env::var("SHELL").ok().filter(|s| !s.is_empty()))
            .or_else(login_shell)
            .unwrap_or_else(|| "/bin/bash".into());

        let pair = native_pty_system()
            .openpty(size(p.cols, p.rows))
            .map_err(|e| AgentError::internal(format!("openpty: {e}")))?;
        let mut cmd = CommandBuilder::new(&shell);
        cmd.arg("-l");
        cmd.cwd(&cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TREEHOUSE_TERM", &p.id);
        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| AgentError::internal(format!("não foi possível iniciar {shell}: {e}")))?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(|e| AgentError::internal(e.to_string()))?;
        let writer = pair.master.take_writer().map_err(|e| AgentError::internal(e.to_string()))?;
        let killer = child.clone_killer();

        let term = Arc::new(Term {
            cwd: cwd.to_string_lossy().into_owned(),
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            killer: Mutex::new(killer),
            scrollback: Mutex::new(VecDeque::new()),
            exit: Mutex::new(None),
        });
        self.terms.lock().unwrap().insert(p.id.clone(), term.clone());

        // Leitura da saída: guarda no histórico e avisa os clientes conectados.
        {
            let term = term.clone();
            let events = self.events.clone();
            let id = p.id.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 16 * 1024];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            push_scrollback(&term, &buf[..n]);
                            let _ = events.send(Notification::new(
                                methods::EV_TERM_OUTPUT,
                                TermOutputEvent { id: id.clone(), data: B64.encode(&buf[..n]) },
                            ));
                        }
                    }
                }
            });
        }
        // Espera o processo terminar.
        {
            let term = term.clone();
            let events = self.events.clone();
            let id = p.id.clone();
            std::thread::spawn(move || {
                let code = child.wait().ok().map(|s| s.exit_code() as i32);
                *term.exit.lock().unwrap() = Some(code);
                let _ = events.send(Notification::new(methods::EV_TERM_EXIT, TermExitEvent { id, code }));
            });
        }

        if let Some(command) = p.command.as_ref().filter(|c| !c.trim().is_empty()) {
            let mut w = term.writer.lock().unwrap();
            let _ = w.write_all(format!("{command}\r").as_bytes());
            let _ = w.flush();
        }
        Ok(TermOpenResult { created: true, scrollback: String::new() })
    }

    pub fn attach(&self, p: &TermIdParams) -> Result<TermOpenResult> {
        let t = self.require(&p.id)?;
        Ok(TermOpenResult { created: false, scrollback: snapshot(&t) })
    }

    pub fn write(&self, p: &TermWriteParams) -> Result<()> {
        let t = self.require(&p.id)?;
        let data = B64.decode(&p.data).map_err(|e| AgentError::invalid(format!("base64 inválido: {e}")))?;
        let mut w = t.writer.lock().unwrap();
        w.write_all(&data)?;
        w.flush()?;
        Ok(())
    }

    pub fn resize(&self, p: &TermResizeParams) -> Result<()> {
        let t = self.require(&p.id)?;
        let res = t.master.lock().unwrap().resize(size(p.cols, p.rows));
        res.map_err(|e| AgentError::internal(e.to_string()))
    }

    pub fn close(&self, p: &TermIdParams) -> Result<()> {
        if let Some(t) = self.terms.lock().unwrap().remove(&p.id) {
            let _ = t.killer.lock().unwrap().kill();
        }
        Ok(())
    }

    /// Fecha todos os terminais cujo id começa com `prefix` (ex.: ao arquivar uma feature).
    pub fn close_prefix(&self, prefix: &str) -> usize {
        let mut terms = self.terms.lock().unwrap();
        let ids: Vec<String> = terms.keys().filter(|k| k.starts_with(prefix)).cloned().collect();
        for id in &ids {
            if let Some(t) = terms.remove(id) {
                let _ = t.killer.lock().unwrap().kill();
            }
        }
        ids.len()
    }

    pub fn list(&self) -> Vec<TermInfo> {
        let mut v: Vec<TermInfo> = self
            .terms
            .lock()
            .unwrap()
            .iter()
            .map(|(id, t)| {
                let exit = *t.exit.lock().unwrap();
                TermInfo { id: id.clone(), cwd: t.cwd.clone(), alive: exit.is_none(), exit_code: exit.flatten() }
            })
            .collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    pub fn running(&self) -> usize {
        self.list().iter().filter(|t| t.alive).count()
    }

    fn get(&self, id: &str) -> Option<Arc<Term>> {
        self.terms.lock().unwrap().get(id).cloned()
    }

    fn require(&self, id: &str) -> Result<Arc<Term>> {
        self.get(id)
            .ok_or_else(|| AgentError::new(codes::NO_SUCH_TERMINAL, format!("terminal inexistente: {id}")))
    }
}

/// Shell do usuário em /etc/passwd (o `wsl.exe --exec` não define `$SHELL`).
fn login_shell() -> Option<String> {
    // SAFETY: getpwuid devolve um ponteiro para dados estáticos da libc; copiamos na hora.
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if pw.is_null() || (*pw).pw_shell.is_null() {
            return None;
        }
        let s = std::ffi::CStr::from_ptr((*pw).pw_shell).to_string_lossy().into_owned();
        (!s.is_empty()).then_some(s)
    }
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows: rows.max(2), cols: cols.max(10), pixel_width: 0, pixel_height: 0 }
}

fn push_scrollback(t: &Term, data: &[u8]) {
    let mut sb = t.scrollback.lock().unwrap();
    sb.extend(data);
    let excess = sb.len().saturating_sub(SCROLLBACK_BYTES);
    if excess > 0 {
        sb.drain(..excess);
    }
}

fn snapshot(t: &Term) -> String {
    let sb = t.scrollback.lock().unwrap();
    let (a, b) = sb.as_slices();
    let mut v = Vec::with_capacity(sb.len());
    v.extend_from_slice(a);
    v.extend_from_slice(b);
    B64.encode(v)
}
