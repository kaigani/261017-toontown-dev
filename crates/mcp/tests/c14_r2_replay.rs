//! Round-two replay through actual batch-script mode into a fresh executor.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression")]
mod support;

use serde_json::{Value, json};
use support::*;
use tt_executor::Invocation;

fn canonical(h: &tt_executor::ExecutorHandle) -> Vec<String> {
    let doc = h.snapshot().unwrap().unwrap();
    doc.entry_keys()
        .iter()
        .map(|k| tt_model::canonical::to_canonical_string(&doc.entry_raw_json(k).unwrap()).unwrap())
        .collect()
}

fn replay_session(partial: bool, retry: bool) {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name": "Replay"}))).is_ok());
    let initial = h.snapshot().unwrap().unwrap();
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let patch = |n| json!({"id": "doc.object.patch", "params": {"targets": ["scene:#1"], "props": {"meta": {"n": n}}}});
    if partial {
        let e = c.call(
            "batch",
            json!({"label": "partial", "atomic": false,
            "steps": [patch(1), {"id": "unknown.command", "params": {}}]}),
        );
        assert_eq!(e["structuredContent"]["status"], "error");
    } else {
        for n in [1, 2] {
            let p = patch(n);
            assert_eq!(c.run("doc.object.patch", p["params"].clone())["status"], "ok");
        }
        if retry {
            let args = json!({"id": "edit.undo", "params": {}, "requestId": "undo-once"});
            let first = c.call("run_command", args.clone());
            assert_eq!(first["structuredContent"]["status"], "ok");
            assert_eq!(first, c.call("run_command", args));
        }
    }
    // Include discovery and a dry run: neither may become a replay edit.
    c.call("list_commands", json!({"filter": "doc.*"}));
    let preview = patch(99);
    c.call("run_command", json!({"id": "doc.object.patch", "params": preview["params"], "dryRun": true}));
    let target = dir.join("replay.json");
    let e = c.run("agent.activity.export", json!({"path": path(&target), "format": "batch"}));
    assert_eq!(e["status"], "ok", "{e}");
    let request = tt_mcp::script::load(&target).unwrap();
    let fresh = executor(&dir.join("replay-user"));
    assert!(fresh.open_document(initial, None, false).is_ok());
    let outcome = tt_mcp::script::run(&fresh, &opts(&dir), request, None).unwrap();
    assert!(outcome.ok, "{:?}", outcome.envelope);
    let expected = canonical(&h);
    let actual = canonical(&fresh);
    c.close().unwrap();
    drop(h);
    drop(fresh);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(actual, expected, "partial={partial}, retry={retry}: canonical entries differ after script replay");
}

#[test]
fn ordinary_edits_dry_runs_and_discovery_replay_canonically() {
    replay_session(false, false);
}

#[test]
fn partially_committed_batch_replays_canonically() {
    replay_session(true, false);
}

#[test]
fn idempotent_retry_replays_canonically() {
    replay_session(false, true);
}
