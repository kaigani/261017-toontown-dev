//! A scripted MCP client over OS pipes for the protocol tests: requests, notifications, and answers to the server's
//! own requests (elicitation, roots).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, dead_code, reason = "test helpers")]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, ExecutorOptions, Maintenance, PersistOptions, UserDirs};
use tt_mcp::ServerOptions;

pub fn tmp() -> PathBuf {
    let d = std::env::temp_dir().join(format!("tt-c14-mcp-{}", tt_persist::fsx::random_token(6)));
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub fn executor(dirs: &Path) -> ExecutorHandle {
    ExecutorHandle::spawn_with_options(ExecutorOptions {
        config: ExecutorConfig { budget: Duration::from_secs(60) },
        persist: PersistOptions {
            user_dirs: Some(UserDirs::under(dirs)),
            maintenance: Maintenance { tick: Duration::ZERO, ..Maintenance::default() },
            migrations: tt_persist::migrate::CHAIN,
        },
    })
    .unwrap()
}

type Responder = Box<dyn FnMut(&str, &Value) -> Value + Send>;

pub struct Client {
    writer: std::io::PipeWriter,
    rx: Receiver<Value>,
    next: u64,
    /// Notifications received so far (in order).
    pub notes: Vec<Value>,
    /// Requests the server made (method, params).
    pub asked: Vec<(String, Value)>,
    responder: Responder,
    server: Option<JoinHandle<Result<(), String>>>,
}

impl Client {
    pub fn start(h: &ExecutorHandle, opts: ServerOptions) -> Self {
        let (in_r, in_w) = std::io::pipe().unwrap();
        let (out_r, out_w) = std::io::pipe().unwrap();
        let handle = h.clone();
        let server = std::thread::spawn(move || tt_mcp::serve(handle, opts, BufReader::new(in_r), out_w));
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(out_r).lines() {
                let Ok(line) = line else { return };
                let v: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("not JSON ({e}): {line}"));
                if tx.send(v).is_err() {
                    return;
                }
            }
        });
        Self {
            writer: in_w,
            rx,
            next: 1,
            notes: Vec::new(),
            asked: Vec::new(),
            responder: Box::new(|_, _| json!({})),
            server: Some(server),
        }
    }

    /// Starts and initializes with the given client capabilities.
    pub fn connect(h: &ExecutorHandle, opts: ServerOptions, caps: Value) -> (Self, Value) {
        let mut c = Self::start(h, opts);
        let init = c.request(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": caps, "clientInfo": { "name": "c14-test", "version": "1" } }),
        );
        c.notify("notifications/initialized", json!({}));
        (c, init)
    }

    pub fn respond_with(&mut self, f: impl FnMut(&str, &Value) -> Value + Send + 'static) {
        self.responder = Box::new(f);
    }

    pub fn raw(&mut self, line: &str) {
        writeln!(self.writer, "{line}").unwrap();
        self.writer.flush().unwrap();
    }

    pub fn notify(&mut self, method: &str, params: Value) {
        let m = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        self.raw(&m.to_string());
    }

    /// Sends a request and returns its id without waiting.
    pub fn send(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next;
        self.next += 1;
        let m = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        self.raw(&m.to_string());
        id
    }

    /// The next message, answering server requests and keeping notifications.
    fn pump(&mut self, timeout: Duration) -> Option<Value> {
        let v = match self.rx.recv_timeout(timeout) {
            Ok(v) => v,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return None,
        };
        if let (Some(method), Some(id)) = (v.get("method").and_then(Value::as_str), v.get("id")) {
            let params = v.get("params").cloned().unwrap_or(Value::Null);
            self.asked.push((method.to_owned(), params.clone()));
            let result = (self.responder)(method, &params);
            let m = json!({ "jsonrpc": "2.0", "id": id, "result": result });
            self.raw(&m.to_string());
            return self.pump(timeout);
        }
        if v.get("method").is_some() {
            self.notes.push(v.clone());
        }
        Some(v)
    }

    /// The next message of any kind (`None` after `timeout`).
    pub fn next(&mut self, timeout: Duration) -> Option<Value> {
        self.pump(timeout)
    }

    /// The response to request `id` (the full JSON-RPC message).
    pub fn wait(&mut self, id: u64, timeout: Duration) -> Option<Value> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            let v = self.pump(left)?;
            if v.get("id").and_then(Value::as_u64) == Some(id) && v.get("method").is_none() {
                return Some(v);
            }
        }
    }

    /// Sends and waits (30 s): the full response message.
    pub fn message(&mut self, method: &str, params: Value) -> Value {
        let id = self.send(method, params);
        self.wait(id, Duration::from_secs(30)).unwrap_or_else(|| panic!("no response to {method}"))
    }

    /// Sends and waits: the result (panics on a JSON-RPC error).
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let m = self.message(method, params);
        assert!(m.get("error").is_none(), "{method}: {m}");
        m["result"].clone()
    }

    /// `tools/call`: the tool result.
    pub fn call(&mut self, tool: &str, args: Value) -> Value {
        self.request("tools/call", json!({ "name": tool, "arguments": args }))
    }

    /// `tools/call` of `run_command`: the envelope (structured content).
    pub fn run(&mut self, id: &str, params: Value) -> Value {
        self.call("run_command", json!({ "id": id, "params": params }))["structuredContent"].clone()
    }

    /// Collects notifications for `d`.
    pub fn drain(&mut self, d: Duration) {
        let deadline = Instant::now() + d;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            if self.pump(left).is_none() {
                break;
            }
        }
    }

    pub fn notes_of(&self, method: &str) -> Vec<Value> {
        self.notes.iter().filter(|n| n["method"] == method).cloned().collect()
    }

    /// Closes the client's side and waits for the server to finish.
    pub fn close(mut self) -> Result<(), String> {
        let server = self.server.take();
        drop(self);
        server.map_or(Ok(()), |s| s.join().unwrap())
    }

    pub fn server_finished(&self) -> bool {
        self.server.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

/// An agent session option set rooted at `dir`.
pub fn opts(dir: &Path) -> ServerOptions {
    ServerOptions { roots: vec![dir.to_owned()], pause_wait: Duration::from_millis(400), ..ServerOptions::default() }
}

pub fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}
