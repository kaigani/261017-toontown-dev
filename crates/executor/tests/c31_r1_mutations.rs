//! Every C3.1 action/toggle's first declaration example through independent common-contract checks.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs,
    reason = "review tests"
)]
mod c13_support;
mod common;
use common::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tt_executor::{ExecutorHandle, InvokeOptions, Status};
fn content(h: &ExecutorHandle) -> BTreeMap<String, Value> {
    let doc = h.snapshot().unwrap().unwrap();
    doc.entry_keys()
        .iter()
        .flat_map(|k| tt_model::change::own_objects(&doc, k).unwrap())
        .map(|o| (o.key, o.json))
        .collect()
}
fn fixture_isolated() -> (ExecutorHandle, std::path::PathBuf) {
    let dir = c13_support::tmp();
    let h = c13_support::spawn(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    // Isolate clipboard preparation from the command under test.
    ok(&h, "cell.clipboard.set", json!({"elementIds":["scene:\"Intro\"/element:\"Bob\""],"from":1,"to":6}));
    (h, dir)
}
#[test]
fn all_c31_mutation_examples_obey_staging_revision_event_history_and_roundtrip() {
    let registry = tt_executor::registry().unwrap();
    let mut count = 0;
    for e in registry.entries().filter(|e| {
        tt_registry::owner_of(e.id()) == "M3 C3.1"
            && matches!(e.spec.kind, tt_registry::Kind::Action | tt_registry::Kind::Toggle)
    }) {
        let Some(c) = e.command else { panic!("undeclared {}", e.id()) };
        let params: Value = serde_json::from_str(c.decl.examples[0].params).unwrap();
        let (h, dir) = fixture_isolated();
        let before = content(&h);
        let r = rev(&h);
        let history = ok(&h, "edit.history.get", json!({}))["position"].clone();
        let rx = h.subscribe();
        let dry = with(&h, e.id(), params.clone(), InvokeOptions { dry_run: true, ..Default::default() });
        assert_eq!(content(&h), before, "{} dry run document", e.id());
        assert_eq!(rev(&h), r);
        assert_eq!(ok(&h, "edit.history.get", json!({}))["position"], history);
        assert_eq!(rx.try_iter().filter(|e| e.name == "doc.changed").count(), 0);
        if e.id() == "element.mediaLength.set" && dry.status != Status::Ok {
            assert_eq!(dry.error.unwrap().details["owner"], "M8 C8.3");
            count += 1;
            continue;
        }
        assert_eq!(dry.status, Status::Ok, "{} dry: {dry:?}", e.id());
        let applied = run(&h, e.id(), params.clone());
        assert_eq!(applied.status, Status::Ok, "{} apply: {applied:?}", e.id());
        let after = content(&h);
        let changed = after != before;
        assert_eq!(rev(&h), r + u64::from(changed), "{} revision", e.id());
        assert_eq!(
            rx.try_iter().filter(|e| e.name == "doc.changed").count(),
            usize::from(changed),
            "{} doc event",
            e.id()
        );
        if changed && e.id() != "element.collapsed.set" {
            assert_eq!(
                ok(&h, "edit.history.get", json!({}))["position"].as_u64(),
                Some(history.as_u64().unwrap() + 1),
                "{} history",
                e.id()
            );
            ok(&h, "edit.undo", json!({}));
            assert_eq!(content(&h), before, "{} undo", e.id());
            ok(&h, "edit.redo", json!({}));
            assert_eq!(content(&h), after, "{} redo", e.id());
        }
        if changed {
            let stale = InvokeOptions {
                expect_rev: [("scene:\"Intro\"".into(), 0)].into_iter().collect(),
                ..Default::default()
            };
            let before_failure = content(&h);
            assert_eq!(with(&h, e.id(), params.clone(), stale).error.unwrap().code, "rev.conflict");
            assert_eq!(content(&h), before_failure);
        }
        // Every action's staged mutation, followed by a guaranteed failure, rolls back as one group.
        let (rollback, _) = fixture_isolated();
        let baseline = content(&rollback);
        let result = batch(
            &rollback,
            json!({"label":"Review rollback","atomic":true,"steps":[{"id":e.id(),"params":params},{"id":"element.rename","params":{"elementId":"scene:\"Intro\"/element:\"Bob\"","name":"bad.name"}}]}),
        );
        assert_ne!(result.status, Status::Ok);
        assert_eq!(content(&rollback), baseline, "{} atomic rollback", e.id());
        // Full own-view content survives save/open, including hierarchy and drawing payloads.
        let path = dir.join("Roundtrip.animpkg");
        c13_support::job_ok(&h, "project.saveAs", json!({"path":path}));
        let fresh = c13_support::spawn(&dir.join("reopen-user"));
        c13_support::job_ok(&fresh, "project.open", json!({"path":path}));
        assert_eq!(content(&fresh), content(&h), "{} save/open", e.id());
        println!("C31 contract {}: changed={changed}", e.id());
        count += 1;
    }
    assert!(count >= 35, "coverage unexpectedly fell: {count}");
    println!("C31 mutation examples checked: {count}");
}
