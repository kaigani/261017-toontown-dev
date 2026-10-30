//! Round-three committed-publication-failure replay variants.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression")]
mod support;
use serde_json::json;
use support::*;
use tt_executor::Invocation;

fn publication_failure_replay(atomic: bool) {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.invoke(Invocation::user("project.create", json!({"name":"Replay"}))).is_ok());
    let initial = h.snapshot().unwrap().unwrap();
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let blocked = dir.join("blocked.json");
    std::fs::create_dir(&blocked).unwrap();
    let e = c.call("batch", json!({"label":"committed publication failure", "atomic":atomic, "steps":[
        {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"kept":1}}}},
        {"id":"doc.export.json","params":{"scope":"scenes","refs":["scene:#1"],"path":path(&blocked),"onExists":"overwrite"}}
    ]}));
    let env = &e["structuredContent"];
    assert_eq!(env["status"], "error", "{e}");
    assert_eq!(env["error"]["details"]["reason"], "publish", "{e}");
    let target = dir.join("replay.json");
    let exported = c.run("agent.activity.export", json!({"path":path(&target),"format":"batch"}));
    assert_eq!(exported["status"], "ok", "{exported}");
    let request = tt_mcp::script::load(&target).unwrap();
    let fresh = executor(&dir.join("fresh-user"));
    assert!(fresh.open_document(initial, None, false).is_ok());
    // Remove the blocked directory so replay can publish; only document entries are the oracle.
    std::fs::remove_dir(&blocked).unwrap();
    let outcome = tt_mcp::script::run(&fresh, &opts(&dir), request, None).unwrap();
    assert!(outcome.ok, "{:?}", outcome.envelope);
    let canonical = |h: &tt_executor::ExecutorHandle| {
        let d = h.snapshot().unwrap().unwrap();
        d.entry_keys().iter().map(|k| tt_model::canonical::to_canonical_string(&d.entry_raw_json(k).unwrap()).unwrap()).collect::<Vec<_>>()
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

