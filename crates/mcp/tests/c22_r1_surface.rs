#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression tests")]
mod support;
use serde_json::json;
use support::*;
#[test]
fn an_agent_with_preferences_permission_can_set_gap_closing() {
    let dir = tmp();
    let h = executor(&dir.join("prefs"));
    let mut options = opts(&dir);
    options.policy = Some(tt_executor::agents::Policy::Autonomous);
    options.scopes =
        Some([tt_registry::Scope::Read, tt_registry::Scope::Ui, tt_registry::Scope::Prefs].into_iter().collect());
    let (mut c, _) = Client::connect(&h, options, json!({}));
    let result = c.run("colour.gapClosing.set", json!({"mode":"small"}));
    c.close().unwrap();
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(
        result["status"], "ok",
        "the persisted paint preference must not require the agent-forbidden app scope: {result}"
    );
}
#[test]
fn record_checkpoint_command_contracts_and_deferred_owners() {
    let entries: Vec<_> = tt_executor::registry()
        .unwrap()
        .entries()
        .filter(|e| e.id().starts_with("colour.") || e.id() == "draw.hitTest.get")
        .map(|e| e.describe())
        .collect();
    assert_eq!(entries.iter().filter(|e| e["status"] == "implemented").count(), 15);
    for e in entries.iter().filter(|e| e["status"] != "implemented") {
        assert!(e["milestone"].as_str().unwrap().starts_with('M'), "{e}");
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/review-c22");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("command-contracts.json"), serde_json::to_vec_pretty(&entries).unwrap()).unwrap();
}
#[test]
fn typed_generic_atomic_sequential_and_script_paints_preserve_commit_contracts() {
    let dir = tmp();
    let h = executor(&dir.join("prefs"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    assert!(
        h.invoke(tt_executor::Invocation::user(
            "agent.typedTools.set",
            json!({"enabled":true,"namespaces":["colour"]})
        ))
        .is_ok()
    );
    let mut options = opts(&dir);
    options.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = Client::connect(&h, options.clone(), json!({}));
    let d = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
    let cols: Vec<_> = h
        .snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
        .collect();
    let add=c.run("draw.stroke.add",json!({"drawingId":d,"kind":"pencil","layer":"new","fit":false,"closed":true,"points":[{"x":1000,"y":1000},{"x":1100,"y":1000},{"x":1100,"y":1100},{"x":1000,"y":1100}]}));
    assert_eq!(add["status"], "ok");
    let zone = add["result"]["zoneIds"][0].clone();
    let paint = c.call("colour_paint_apply", json!({"drawingId":d,"objectIds":[zone],"colourId":cols[1]}));
    assert_eq!(paint["structuredContent"]["status"], "ok", "{paint}");
    let before = h.snapshot().unwrap().unwrap().document_rev();
    let rx = h.subscribe();
    let dry = c.call(
        "run_command",
        json!({"id":"colour.paint.remove","params":{"drawingId":d,"objectIds":[zone]},"dryRun":true}),
    )["structuredContent"]
        .clone();
    assert_eq!(dry["status"], "ok");
    assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), before);
    assert!(rx.try_iter().all(|e| e.name != "draw.drawing.changed"));
    for atomic in [true, false] {
        let rev = h.snapshot().unwrap().unwrap().document_rev();
        let r=c.call("batch",json!({"label":"Review paint","atomic":atomic,"steps":[{"id":"colour.paint.remove","params":{"drawingId":d,"objectIds":[zone]}},{"id":"colour.paint.apply","params":{"drawingId":d,"objectIds":[zone],"colourId":cols[2]}}]}));
        assert_eq!(r["structuredContent"]["status"], "ok", "{r}");
        assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), rev + if atomic { 1 } else { 2 });
        let events: Vec<_> = rx.try_iter().filter(|e| e.name == "draw.drawing.changed").collect();
        assert_eq!(events.len(), if atomic { 1 } else { 2 });
    }
    let script=serde_json::from_value(json!({"label":"Review script","atomic":false,"steps":[{"id":"batch","params":{"atomic":true,"label":"Group","steps":[{"id":"colour.paint.remove","params":{"drawingId":d,"objectIds":[zone]}},{"id":"colour.paintUnpainted.apply","params":{"drawingId":d,"objectIds":[zone],"colourId":cols[3]}}]}}]})).unwrap();
    let r = tt_mcp::script::run(&h, &options, script, None).unwrap();
    assert!(r.ok, "{:?}", r.envelope.error);
    c.close().unwrap();
    drop(h);
    for _ in 0..80 {
        if std::fs::remove_dir_all(&dir).is_ok() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn retain_a_baseline_bundle_for_the_headless_paint_probe() {
    let h = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let e = h.invoke(tt_executor::Invocation::user("doc.export.json", json!({"scope":"project","binaries":"embed"})));
    assert!(e.is_ok());
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/review-c22");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("fixture-bundle.json"), serde_json::to_vec_pretty(&e.result.unwrap()["bundle"]).unwrap())
        .unwrap();
}
