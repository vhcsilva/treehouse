//! Testes de ponta a ponta do binário: protocolo no stdio, terminais e daemon.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};

struct Client {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Value>,
    next_id: u64,
    /// Notificações recebidas enquanto se esperava respostas.
    events: Vec<Value>,
}

impl Client {
    fn spawn(args: &[&str], envs: &[(&str, &str)]) -> Self {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_treehouse-agent"));
        cmd.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        for (k, v) in envs {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    if tx.send(v).is_err() {
                        break;
                    }
                }
            }
        });
        Self { child, stdin, rx, next_id: 1, events: vec![] }
    }

    /// Envia sem `id`: o agente executa e não responde.
    fn notify(&mut self, method: &str, params: Value) {
        let msg = json!({"jsonrpc":"2.0","method":method,"params":params});
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let v = self.rx.recv_timeout(left).unwrap_or_else(|_| panic!("sem resposta para {method}"));
            if v.get("id") == Some(&json!(id)) {
                if let Some(err) = v.get("error") {
                    panic!("{method} falhou: {err}");
                }
                return v["result"].clone();
            }
            self.events.push(v);
        }
    }

    /// Espera até a saída acumulada do terminal `id` conter `needle`.
    fn wait_output(&mut self, id: &str, needle: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut acc = String::new();
        let consume = |v: &Value, acc: &mut String| {
            if v["method"] == "term.output" && v["params"]["id"] == id {
                let bytes = B64.decode(v["params"]["data"].as_str().unwrap()).unwrap();
                acc.push_str(&String::from_utf8_lossy(&bytes));
            }
        };
        for v in std::mem::take(&mut self.events) {
            consume(&v, &mut acc);
        }
        while !acc.contains(needle) {
            let left = deadline.saturating_duration_since(Instant::now());
            let v = self
                .rx
                .recv_timeout(left)
                .unwrap_or_else(|_| panic!("'{needle}' não apareceu; saída até agora: {acc:?}"));
            consume(&v, &mut acc);
        }
        acc
    }

    fn wait_event(&mut self, method: &str) -> Value {
        if let Some(pos) = self.events.iter().position(|v| v["method"] == method) {
            return self.events.remove(pos);
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let v = self.rx.recv_timeout(left).unwrap_or_else(|_| panic!("evento {method} não chegou"));
            if v["method"] == method {
                return v;
            }
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn b64(s: &str) -> String {
    B64.encode(s)
}

#[test]
fn stdio_version_and_terminal() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut c = Client::spawn(&["serve"], &[]);
    let v = c.call("agent.version", json!({}));
    assert_eq!(v["protocol"], 1);

    let r = c.call(
        "term.open",
        json!({"id":"f/api/0","cwd":tmp.path(),"shell":"/bin/bash","command":"echo PRONTO_$((40+2))","cols":100,"rows":30}),
    );
    assert_eq!(r["created"], true);
    c.wait_output("f/api/0", "PRONTO_42");

    // Entrada vinda do app.
    c.call("term.write", json!({"id":"f/api/0","data":b64("pwd\r")}));
    c.wait_output("f/api/0", tmp.path().to_str().unwrap());
    c.call("term.resize", json!({"id":"f/api/0","cols":80,"rows":20}));

    // Abrir de novo devolve o mesmo terminal com o histórico.
    let again = c.call("term.open", json!({"id":"f/api/0","cwd":tmp.path(),"shell":"/bin/bash"}));
    assert_eq!(again["created"], false);
    let sb = String::from_utf8(B64.decode(again["scrollback"].as_str().unwrap()).unwrap()).unwrap();
    assert!(sb.contains("PRONTO_42"));

    c.call("term.write", json!({"id":"f/api/0","data":b64("exit 3\r")}));
    let ev = c.wait_event("term.exit");
    assert_eq!(ev["params"]["code"], 3);
    let list = c.call("term.list", json!({}));
    assert_eq!(list[0]["alive"], false);
}

#[test]
fn unknown_method_and_bad_params_are_errors() {
    let mut c = Client::spawn(&["serve"], &[]);
    writeln!(c.stdin, r#"{{"jsonrpc":"2.0","id":7,"method":"nao.existe","params":{{}}}}"#).unwrap();
    writeln!(c.stdin, r#"{{"jsonrpc":"2.0","id":8,"method":"git.status","params":{{"nada":1}}}}"#).unwrap();
    c.stdin.flush().unwrap();
    let mut seen = 0;
    while seen < 2 {
        let v = c.rx.recv_timeout(Duration::from_secs(10)).unwrap();
        match v["id"].as_i64() {
            Some(7) => assert_eq!(v["error"]["code"], -32601),
            Some(8) => assert_eq!(v["error"]["code"], -32602),
            _ => continue,
        }
        seen += 1;
    }
}

#[test]
fn daemon_keeps_terminals_between_connections() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sock = tmp.path().join("agent.sock");
    let state = tmp.path().join("state");
    let envs = [
        ("TREEHOUSE_SOCKET", sock.to_str().unwrap()),
        ("XDG_STATE_HOME", state.to_str().unwrap()),
    ];

    let pid1;
    {
        let mut c1 = Client::spawn(&["connect"], &envs);
        pid1 = c1.call("agent.version", json!({}))["pid"].clone();
        c1.call(
            "term.open",
            json!({"id":"pay/api/0","cwd":tmp.path(),"shell":"/bin/bash","command":"echo VIVO_$((1+1))"}),
        );
        c1.wait_output("pay/api/0", "VIVO_2");
    } // o app "fecha" aqui

    let mut c2 = Client::spawn(&["connect"], &envs);
    let v = c2.call("agent.version", json!({}));
    assert_eq!(v["pid"], pid1, "deveria ser o mesmo daemon");
    let list = c2.call("term.list", json!({}));
    assert_eq!(list[0]["id"], "pay/api/0");
    assert_eq!(list[0]["alive"], true);
    let att = c2.call("term.attach", json!({"id":"pay/api/0"}));
    let sb = String::from_utf8(B64.decode(att["scrollback"].as_str().unwrap()).unwrap()).unwrap();
    assert!(sb.contains("VIVO_2"));

    c2.call("agent.shutdown", json!({}));
    let deadline = Instant::now() + Duration::from_secs(5);
    while sock.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!sock.exists(), "o daemon deveria ter removido o socket ao sair");
}

#[test]
fn keystroke_echo_is_fast_through_the_daemon() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sock = tmp.path().join("agent.sock");
    let state = tmp.path().join("state");
    let envs = [("TREEHOUSE_SOCKET", sock.to_str().unwrap()), ("XDG_STATE_HOME", state.to_str().unwrap())];
    let mut c = Client::spawn(&["connect"], &envs);
    c.call("term.open", json!({"id":"lat","cwd":tmp.path(),"shell":"/bin/sh"}));
    std::thread::sleep(Duration::from_millis(300));
    c.events.clear();
    while c.rx.try_recv().is_ok() {}

    // Cada tecla vai como notificação (sem esperar resposta), como no app.
    let mut worst = Duration::ZERO;
    let mut total = Duration::ZERO;
    for ch in "abcdefghijklmnopqrst".chars() {
        let t0 = Instant::now();
        c.notify("term.write", json!({"id":"lat","data":b64(&ch.to_string())}));
        loop {
            let v = c.rx.recv_timeout(Duration::from_secs(5)).expect("eco não chegou");
            assert!(v.get("id").is_none() || v["id"].is_null(), "notificação não deve gerar resposta: {v}");
            if v["method"] == "term.output" {
                let bytes = B64.decode(v["params"]["data"].as_str().unwrap()).unwrap();
                if String::from_utf8_lossy(&bytes).contains(ch) {
                    break;
                }
            }
        }
        let dt = t0.elapsed();
        worst = worst.max(dt);
        total += dt;
    }
    eprintln!("eco: média {:?}, pior {:?}", total / 20, worst);
    assert!(worst < Duration::from_millis(100), "eco lento: pior {worst:?}");
    c.call("agent.shutdown", json!({}));
}
