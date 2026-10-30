//! The built binary in its agent run modes (11 §3): `--mcp-stdio` over real process pipes, and `--run` batch
//! scripts with their exit statuses and JSON-lines log.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use serde_json::{Value, json};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_toontown-headless"))
}

fn tmp() -> PathBuf {
    let d = std::env::temp_dir().join(format!("tt-c14-headless-{}-{}", std::process::id(), rand()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A per-process unique suffix: the clock alone repeats when two tests start in the same tick (it did on Windows).
fn rand() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64;
    (t % 1_000_000_007) * 1000 + n
}

fn p(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn script(dir: &Path, name: &str, v: &Value) -> PathBuf {
    let f = dir.join(name);
    std::fs::write(&f, serde_json::to_vec_pretty(v).unwrap()).unwrap();
    f
}

#[test]
fn batch_scripts_exit_zero_or_five_and_log_json_lines() {
    let dir = tmp();
    let user = dir.join("user");
    let pkg = dir.join("Batch.animpkg");
    let ok_script = script(
        &dir,
        "create.json",
        &json!({ "label": "Create", "atomic": false, "steps": [
            { "id": "project.create", "params": { "name": "Batch", "path": p(&pkg) }, "as": "made" },
            { "id": "doc.object.patch", "params": { "targets": ["$made.result.sceneId"], "props": { "meta": { "by": "script" } } } },
            { "id": "project.save", "params": {} },
        ] }),
    );
    let log = dir.join("log.jsonl");
    let out = bin()
        .args(["--headless", "--run"])
        .arg(&ok_script)
        .arg("--out")
        .arg(&log)
        .arg("--allow-path")
        .arg(&dir)
        .arg("--user-dir")
        .arg(&user)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {} stdout: {}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let env: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(env["status"], "ok");
    let lines: Vec<Value> =
        std::fs::read_to_string(&log).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines[0]["record"], "start");
    assert_eq!(lines.iter().filter(|l| l["record"] == "step").count(), 3);
    assert_eq!(lines.last().unwrap()["record"], "end");
    assert_eq!(lines.last().unwrap()["ok"], true);
    assert!(pkg.exists());
    // A failing step: exit 5, the log says why.
    let bad = script(
        &dir,
        "bad.json",
        &json!({ "label": "Bad", "steps": [ { "id": "doc.object.patch", "params": { "targets": ["el_missing"], "props": {} } } ] }),
    );
    let log2 = dir.join("bad.jsonl");
    let out = bin()
        .args(["--run"])
        .arg(&bad)
        .arg("--project")
        .arg(&pkg)
        .arg("--out")
        .arg(&log2)
        .arg("--user-dir")
        .arg(&user)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(5), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let end: Value = serde_json::from_str(std::fs::read_to_string(&log2).unwrap().lines().last().unwrap()).unwrap();
    assert_eq!(end["ok"], false);
    assert!(end["error"]["code"].is_string(), "{end}");
    // Not a batch: usage error (2).
    let junk = dir.join("junk.json");
    std::fs::write(&junk, b"[1,2,3]").unwrap();
    let out = bin().arg("--run").arg(&junk).arg("--user-dir").arg(&user).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    // Writes outside the roots are refused in batch mode too.
    let outside = script(
        &dir,
        "outside.json",
        &json!({ "label": "Outside", "atomic": false, "steps": [
            { "id": "project.create", "params": { "name": "O", "path": p(&std::env::temp_dir().join("tt-c14-outside-never.animpkg")) } } ] }),
    );
    let out = bin().arg("--run").arg(&outside).arg("--user-dir").arg(&user).output().unwrap();
    assert_eq!(out.status.code(), Some(5));
    assert!(String::from_utf8_lossy(&out.stdout).contains("fs.outsideRoots"));
    let _ = std::fs::remove_dir_all(&dir);
}

struct Stdio2 {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Value>,
    next: u64,
}

impl Stdio2 {
    fn start(args: &[std::ffi::OsString]) -> Self {
        let mut cmd = bin();
        cmd.args(args);
        let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { return };
                let v: Value = serde_json::from_str(&line)
                    .unwrap_or_else(|e| panic!("stdout carries only JSON-RPC ({e}): {line}"));
                if tx.send(v).is_err() {
                    return;
                }
            }
        });
        Self { child, stdin, rx, next: 1 }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        writeln!(self.stdin, "{}", json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).unwrap();
        self.stdin.flush().unwrap();
        loop {
            let v = self.rx.recv_timeout(Duration::from_secs(30)).expect("response");
            if v["id"] == id && v.get("method").is_none() {
                assert!(v.get("error").is_none(), "{v}");
                return v["result"].clone();
            }
        }
    }
}

#[test]
fn mcp_stdio_serves_the_opened_project_and_exits_when_the_client_leaves() {
    let dir = tmp();
    let user = dir.join("user");
    let pkg = dir.join("Served.animpkg");
    let create = script(
        &dir,
        "create.json",
        &json!({ "label": "Create", "atomic": false, "steps": [ { "id": "project.create", "params": { "name": "Served", "path": p(&pkg) } } ] }),
    );
    let out =
        bin().arg("--run").arg(&create).arg("--allow-path").arg(&dir).arg("--user-dir").arg(&user).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stdout));
    let args: Vec<std::ffi::OsString> = ["--headless", "--mcp-stdio", "--typed-tools", "doc", "--policy", "assist"]
        .iter()
        .map(Into::into)
        .chain(["--project".into(), pkg.clone().into_os_string(), "--user-dir".into(), user.clone().into_os_string()])
        .collect();
    let mut s = Stdio2::start(&args);
    // A line that is not JSON: the server answers a parse error and keeps serving.
    writeln!(s.stdin, "not json").unwrap();
    let parse = s.rx.recv_timeout(Duration::from_secs(30)).unwrap();
    assert_eq!(parse["error"]["code"], -32700, "{parse}");
    let init = s.request(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "headless-test", "version": "1" } }),
    );
    assert_eq!(init["protocolVersion"], "2025-06-18");
    assert_eq!(init["_meta"]["toontown"]["policy"], "assist");
    let state = s.request("tools/call", json!({ "name": "get_state", "arguments": {} }));
    assert_eq!(state["structuredContent"]["result"]["project"]["name"], "Served", "{state}");
    let tools = s.request("tools/list", json!({}));
    assert!(tools["tools"].as_array().unwrap().iter().any(|t| t["name"] == "doc_object_get"));
    let project = s.request("resources/read", json!({ "uri": "app://project" }));
    assert!(project["contents"][0]["text"].as_str().unwrap().contains("Served"));
    drop(s.stdin);
    let status = s.child.wait().unwrap();
    assert_eq!(status.code(), Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}
