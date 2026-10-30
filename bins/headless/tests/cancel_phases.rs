//! C14-R3-03 closure: cancellation is terminal at every phase of a confirmation asked through MCP elicitation, against
//! the real binary over stdio:
//! - before the elicitation is sent (the cancellation in the same write as the call);
//! - while the elicitation waits for an answer;
//! - cancellation, then a late `accept` (the reviewer's case, also in `c14-r3-mcp.py`);
//! - an `accept`, then a cancellation that lands while the answer is being processed (raced repeatedly): the outcome
//!   is always consistent: either nothing happened and the request gets no response, or the confirmed overwrite
//!   happened and is answered. Never an overwrite the client was told nothing about, never a response without it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

fn dir() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let d = std::env::temp_dir().join(format!("tt-c14-cancel-{}-{t}-{n}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Session {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Value>,
    seen: Vec<Value>,
    next: u64,
    root: PathBuf,
}

impl Session {
    fn start() -> Self {
        let root = dir();
        let mut child = Command::new(env!("CARGO_BIN_EXE_toontown-headless"))
            .args(["--mcp-stdio", "--user-dir"])
            .arg(root.join("user"))
            .arg("--allow-path")
            .arg(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { return };
                if tx.send(serde_json::from_str::<Value>(&line).unwrap()).is_err() {
                    return;
                }
            }
        });
        let mut s = Self { child, stdin, rx, seen: Vec::new(), next: 1, root };
        let init = s.request(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": { "elicitation": {} }, "clientInfo": { "name": "phases", "version": "1" } }),
        );
        assert!(init.get("result").is_some(), "{init}");
        s.write(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        s
    }

    fn write(&mut self, v: &Value) {
        writeln!(self.stdin, "{v}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Several messages in one write (their order is the order the server reads them).
    fn write_all(&mut self, vs: &[Value]) {
        let text: String = vs.iter().map(|v| format!("{v}\n")).collect();
        self.stdin.write_all(text.as_bytes()).unwrap();
        self.stdin.flush().unwrap();
    }

    fn send(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next;
        self.next += 1;
        self.write(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        id
    }

    /// The next message matching `pred`, within `timeout`.
    fn find(&mut self, timeout: Duration, pred: impl Fn(&Value) -> bool) -> Option<Value> {
        if let Some(i) = self.seen.iter().position(&pred) {
            return Some(self.seen.remove(i));
        }
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.checked_duration_since(Instant::now())?;
            let v = self.rx.recv_timeout(left).ok()?;
            if pred(&v) {
                return Some(v);
            }
            self.seen.push(v);
        }
    }

    fn response(&mut self, id: u64, timeout: Duration) -> Option<Value> {
        self.find(timeout, |v| v["id"] == id && v.get("method").is_none())
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.send(method, params);
        self.response(id, Duration::from_secs(30)).expect("response")
    }

    fn run(&mut self, command: &str, params: Value) -> Value {
        self.request("tools/call", json!({ "name": "run_command", "arguments": { "id": command, "params": params } }))
            ["result"]["structuredContent"]
            .clone()
    }

    /// Starts an overwriting export of the activity log onto `target` (a confirmation under Assist).
    fn start_overwrite(&mut self, target: &Path) -> u64 {
        self.send(
            "tools/call",
            json!({ "name": "run_command", "arguments": { "id": "agent.activity.export",
                    "params": { "path": target.to_string_lossy(), "onExists": "overwrite" } } }),
        )
    }

    fn cancel(id: u64) -> Value {
        json!({ "jsonrpc": "2.0", "method": "notifications/cancelled", "params": { "requestId": id } })
    }

    fn accept(ask: &Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": ask["id"], "result": { "action": "accept", "content": { "confirm": true } } })
    }

    /// The worker is free: an ordinary call answers.
    fn responsive(&mut self) {
        let id = self.send("tools/call", json!({ "name": "get_state", "arguments": {} }));
        let r = self.response(id, Duration::from_secs(5)).expect("the call thread is free");
        assert_eq!(r["result"]["structuredContent"]["status"], "ok", "{r}");
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn elicitation(v: &Value) -> bool {
    v["method"] == "elicitation/create"
}

#[test]
fn cancellation_before_the_elicitation_is_sent_asks_nothing() {
    let mut s = Session::start();
    let target = s.root.join("old.jsonl");
    std::fs::write(&target, "old").unwrap();
    for round in 0..10 {
        // The call and its cancellation in one write: the cancellation lands before the confirmation is asked (no
        // `elicitation/create` at all) or, when the call thread got there first, while it waits (the ask is
        // withdrawn). Either way nothing acts and nothing answers.
        let id = s.next;
        s.next += 1;
        let call = json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": "run_command", "arguments": { "id": "agent.activity.export",
                "params": { "path": target.to_string_lossy(), "onExists": "overwrite" } } } });
        s.write_all(&[call, Session::cancel(id)]);
        if let Some(ask) = s.find(Duration::from_secs(1), elicitation) {
            let withdrawn = s.find(Duration::from_secs(2), |v| v["method"] == "notifications/cancelled");
            assert!(withdrawn.is_some_and(|w| w["params"]["requestId"] == ask["id"]), "round {round}: withdrawn");
        }
        assert!(s.response(id, Duration::from_millis(300)).is_none(), "round {round}: no response");
        s.responsive();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "old", "round {round}");
    }
}

#[test]
fn cancellation_while_waiting_and_before_a_late_accept_authorises_nothing() {
    for late_accept in [false, true] {
        let mut s = Session::start();
        let target = s.root.join("old.jsonl");
        std::fs::write(&target, "old").unwrap();
        let id = s.start_overwrite(&target);
        let ask = s.find(Duration::from_secs(10), elicitation).expect("elicitation/create");
        if late_accept {
            // Cancellation first, the answer right behind it, in one write (the reviewer's ordering).
            s.write_all(&[Session::cancel(id), Session::accept(&ask)]);
        } else {
            s.write(&Session::cancel(id));
            let withdrawn = s.find(Duration::from_secs(2), |v| v["method"] == "notifications/cancelled");
            assert!(withdrawn.is_some_and(|w| w["params"]["requestId"] == ask["id"]));
        }
        assert!(s.response(id, Duration::from_millis(800)).is_none(), "late_accept {late_accept}: no response");
        s.responsive();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "old", "late_accept {late_accept}: nothing written");
        // The token was not consumed: the activity log has no successful export.
        let log = s.run("agent.activity.list", json!({ "limit": 100 }));
        let exports: Vec<&Value> =
            log["result"].as_array().unwrap().iter().filter(|e| e["commandId"] == "agent.activity.export").collect();
        assert!(exports.iter().all(|e| e["status"] != "ok"), "{exports:?}");
    }
}

#[test]
fn an_accept_racing_a_cancellation_always_ends_consistently() {
    let mut s = Session::start();
    let mut outcomes = [0u32; 2];
    for round in 0..20 {
        let target = s.root.join(format!("old-{round}.jsonl"));
        std::fs::write(&target, "old").unwrap();
        let id = s.start_overwrite(&target);
        let ask = s.find(Duration::from_secs(10), elicitation).expect("elicitation/create");
        // The answer first, the cancellation right behind it: the cancellation lands while the answer is processed.
        s.write_all(&[Session::accept(&ask), Session::cancel(id)]);
        let answered = s.response(id, Duration::from_secs(2));
        s.responsive();
        let written = std::fs::read_to_string(&target).unwrap() != "old";
        match answered {
            Some(r) => {
                assert!(written, "round {round}: answered but nothing written: {r}");
                assert_eq!(r["result"]["structuredContent"]["status"], "ok", "round {round}: {r}");
                outcomes[1] += 1;
            }
            None => {
                assert!(!written, "round {round}: the file was overwritten but the request was dropped silently");
                outcomes[0] += 1;
            }
        }
    }
    println!("dropped before acting: {}, acted and answered: {}", outcomes[0], outcomes[1]);
}
