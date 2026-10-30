//! Round-three committed-publication-failure replay variants.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression")]
mod support;
use serde_json::json;
use support::*;
use tt_executor::Invocation;

fn autonomous_opts(dir: &std::path::Path) -> tt_mcp::ServerOptions {
    let mut o = opts(dir);
    o.policy = Some(tt_executor::agents::Policy::Autonomous);
    o
}

fn publication_failure_replay(atomic: bool) {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Replay"}))).is_ok());
    let initial = h.snapshot().unwrap().unwrap();
    let (mut c, _) = Client::connect(&h, autonomous_opts(&dir), json!({}));
    assert_eq!(
        c.run("doc.object.patch", json!({"targets":["scene:#1"],"props":{"meta":{"before":true}}}))["status"],
        "ok"
    );
    let blocked = dir.join("blocked.json");
    std::fs::create_dir(&blocked).unwrap();
    let e = c.call("batch", json!({"label":"committed publication failure", "atomic":atomic, "steps":[
        {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"kept":1}}}},
        {"id":"doc.export.json","params":{"scope":"scenes","refs":["scene:#1"],"path":path(&blocked),"onExists":"overwrite"}}
    ]}));
    let env = &e["structuredContent"];
    assert_eq!(env["status"], "error", "{e}");
    let error = if atomic { &env["error"] } else { &env["error"]["details"]["error"] };
    assert_eq!(error["details"]["reason"], "publish", "{e}");
    let target = dir.join("replay.json");
    let exported = c.run("agent.activity.export", json!({"path":path(&target),"format":"batch"}));
    assert_eq!(exported["status"], "ok", "{exported}");
    let request = tt_mcp::script::load(&target).unwrap();
    let fresh = executor(&dir.join("fresh-user"));
    assert!(fresh.open_document(initial, None, false).is_ok());
    // Remove the blocked directory so replay can publish; only document entries are the oracle.
    std::fs::remove_dir(&blocked).unwrap();
    let outcome = tt_mcp::script::run(&fresh, &autonomous_opts(&dir), request, None).unwrap();
    assert!(outcome.ok, "{:?}", outcome.envelope);
    let canonical = |h: &tt_executor::ExecutorHandle| {
        let d = h.snapshot().unwrap().unwrap();
        d.entry_keys()
            .iter()
            .map(|k| tt_model::canonical::to_canonical_string(&d.entry_raw_json(k).unwrap()).unwrap())
            .collect::<Vec<_>>()
    };
    let actual = canonical(&fresh);
    let expected = canonical(&h);
    c.close().unwrap();
    drop(h);
    drop(fresh);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(actual, expected, "atomic={atomic}: replay lost a committed edit after publication failed");
}

#[test]
fn atomic_publication_failure_keeps_committed_edits_in_script_replay() {
    publication_failure_replay(true);
}

#[test]
fn sequential_publication_failure_keeps_committed_edits_in_script_replay() {
    publication_failure_replay(false);
}

fn atomic_history_replay(undo: bool, checkpoint: bool) {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Replay"}))).is_ok());
    let initial = h.snapshot().unwrap().unwrap();
    let (mut c, _) = Client::connect(&h, autonomous_opts(&dir), json!({}));
    if checkpoint {
        assert_eq!(c.run("checkpoint.create", json!({"name":"before"}))["status"], "ok");
    }
    let e = c.call(
        "batch",
        json!({"label":"two edits", "atomic":true, "steps":[
            {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"first":1}}}},
            {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"second":2}}}}
        ]}),
    );
    assert_eq!(e["structuredContent"]["status"], "ok", "{e}");
    if undo {
        assert_eq!(c.run("edit.undo", json!({}))["status"], "ok");
    }
    if checkpoint {
        assert_eq!(c.run("checkpoint.restore", json!({"name":"before"}))["status"], "ok");
    }
    let target = dir.join("replay.json");
    assert_eq!(c.run("agent.activity.export", json!({"path":path(&target),"format":"batch"}))["status"], "ok");
    let fresh = executor(&dir.join("fresh-user"));
    assert!(fresh.open_document(initial, None, false).is_ok());
    let outcome = tt_mcp::script::run(&fresh, &opts(&dir), tt_mcp::script::load(&target).unwrap(), None).unwrap();
    assert!(outcome.ok, "{:?}", outcome.envelope);
    let expected = h.snapshot().unwrap().unwrap();
    let actual = fresh.snapshot().unwrap().unwrap();
    c.close().unwrap();
    drop(h);
    drop(fresh);
    let _ = std::fs::remove_dir_all(&dir);
    for k in expected.entry_keys() {
        assert_eq!(
            tt_model::canonical::to_canonical_string(&actual.entry_raw_json(&k).unwrap()).unwrap(),
            tt_model::canonical::to_canonical_string(&expected.entry_raw_json(&k).unwrap()).unwrap(),
            "atomic replay must retain canonical entry revisions: {k:?}"
        );
    }
}

#[test]
fn two_edits_in_one_atomic_batch_replay_with_the_same_revision() {
    atomic_history_replay(false, false);
}

#[test]
fn undo_after_an_atomic_batch_replays_the_same_document() {
    atomic_history_replay(true, false);
}

#[test]
fn checkpoint_restore_after_an_atomic_batch_replays_the_same_document() {
    atomic_history_replay(false, true);
}
