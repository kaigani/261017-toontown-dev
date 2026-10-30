//! The audit log (11 §11) through the real MCP path: exactly one entry per tool call (one per batch), and the entries
//! of a fixed script reproduce the committed fixture `tests/fixtures/audit-golden.jsonl` once volatile fields are
//! normalised (times, durations, session ids, generated object ids, temporary paths). `TT_BLESS=1` rewrites it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod support;

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};
use support::*;

/// Replaces generated ids (`xx_` + 26 Crockford characters) by `<id1>`, `<id2>`… in order of first appearance, and
/// the temporary folder by `<dir>`.
fn normalise(text: &str, dir: &Path) -> String {
    let dir_json = serde_json::to_string(&path(dir)).unwrap();
    let dir_json = &dir_json[1..dir_json.len() - 1];
    // Separators too: the fixture is shared by Windows and macOS.
    let mut s = text.replace(dir_json, "<dir>").replace("\\\\", "/");
    let mut ids: BTreeMap<String, String> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i + 29 <= b.len() {
        let cand = &s[i..i + 29];
        let ok = cand.as_bytes()[2] == b'_'
            && cand.as_bytes()[..2].iter().all(u8::is_ascii_lowercase)
            && cand.as_bytes()[3..].iter().all(|c| c.is_ascii_digit() || c.is_ascii_uppercase());
        if ok && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
            if !ids.contains_key(cand) {
                order.push(cand.to_owned());
                ids.insert(cand.to_owned(), format!("<id{}>", order.len()));
            }
            i += 29;
        } else {
            i += 1;
        }
    }
    for (k, v) in &ids {
        s = s.replace(k, v);
    }
    s
}

fn entries(c: &mut Client) -> Vec<Value> {
    let text = c.request("resources/read", json!({ "uri": "app://activity" }))["contents"][0]["text"]
        .as_str()
        .unwrap()
        .to_owned();
    text.lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

#[test]
fn one_entry_per_call_and_reproducible_from_the_committed_fixture() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let env = c.run("project.create", json!({ "name": "Audit", "path": path(&dir.join("Audit.animpkg")) }));
    let scene = env["result"]["sceneId"].as_str().unwrap().to_owned();
    let mut calls = 1;
    let script: Vec<(&str, Value)> = vec![
        ("get_state", json!({ "include": ["project"] })),
        ("get_object", json!({ "ref": scene })),
        ("set_properties", json!({ "targets": [scene], "props": { "meta": { "audit": 1 } } })),
        ("set_properties", json!({ "targets": [scene], "props": { "meta": { "audit": 2 } }, "dryRun": true })),
        (
            "batch",
            json!({ "label": "Pair", "steps": [
            { "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "b": 1 } } } },
            { "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "b": 2 } } } },
        ] }),
        ),
        ("undo", json!({})),
        ("redo", json!({})),
        ("run_command", json!({ "id": "doc.object.patch", "params": { "targets": ["el_missing"], "props": {} } })),
        ("run_command", json!({ "id": "agent.server.set", "params": { "enabled": false } })),
        ("describe_command", json!({ "id": "doc.diff" })),
        ("list_commands", json!({ "namespace": "edit" })),
        ("get_docs", json!({ "topic": "errors" })),
        ("pointer", json!({})),
        ("checkpoint", json!({ "action": "create", "name": "audit" })),
        (
            "run_command",
            json!({ "id": "project.saveAs", "params": { "path": path(&dir.join("Audit.animpkg")), "onExists": "overwrite" } }),
        ),
    ];
    for (tool, args) in &script {
        c.call(tool, args.clone());
        calls += 1;
    }
    // Resource reads are not tool calls and are not audited.
    c.request("resources/read", json!({ "uri": "app://state" }));
    let log = entries(&mut c);
    assert_eq!(log.len(), calls, "exactly one entry per tool call: {log:#?}");
    assert_eq!(log.iter().filter(|e| e["tool"] == "batch").count(), 1);
    let seqs: Vec<u64> = log.iter().map(|e| e["seq"].as_u64().unwrap()).collect();
    assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1), "{seqs:?}");
    // Normalised form.
    let mut lines = Vec::new();
    for e in &log {
        let mut e = e.clone();
        let m = e.as_object_mut().unwrap();
        for volatile in ["time", "durationMs", "sessionId"] {
            assert!(m.remove(volatile).is_some() || volatile == "sessionId", "{volatile}");
        }
        lines.push(serde_json::to_string(&e).unwrap());
    }
    let actual = normalise(&(lines.join("\n") + "\n"), &dir);
    let golden_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audit-golden.jsonl");
    if std::env::var_os("TT_BLESS").is_some() {
        std::fs::create_dir_all(golden_path.parent().unwrap()).unwrap();
        std::fs::write(&golden_path, &actual).unwrap();
    }
    let golden = std::fs::read_to_string(&golden_path).unwrap().replace("\r\n", "\n");
    assert_eq!(actual, golden, "the audit log no longer reproduces the committed fixture (TT_BLESS=1 to update)");
    c.close().unwrap();
    // The persisted log beside the project: one line per call as well.
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn persisted_activity_log_beside_the_project() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let pkg = dir.join("Persist.animpkg");
    let env = c.run("project.create", json!({ "name": "Persist", "path": path(&pkg) }));
    let scene = env["result"]["sceneId"].as_str().unwrap().to_owned();
    let user = tt_executor::Invocation {
        command: "agent.activity.persist.set".into(),
        params: json!({ "enabled": true }),
        options: tt_executor::InvokeOptions::default(),
        origin: tt_executor::Origin::User,
    };
    assert!(h.invoke(user).is_ok());
    for i in 0..3 {
        c.call("set_properties", json!({ "targets": [scene], "props": { "meta": { "i": i } } }));
    }
    let file = dir.join("agent-activity.jsonl");
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    assert_eq!(text.lines().count(), 3, "{}: {text}", file.display());
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
