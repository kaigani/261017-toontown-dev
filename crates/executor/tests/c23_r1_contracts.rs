//! Independent five-command mutation and staged-file contracts.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod c13_support;
mod common;
use common::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use tt_executor::{ExecutorHandle, InvokeOptions};
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
fn normalized(h: &ExecutorHandle) -> Value {
    fn walk(v: &mut Value) {
        match v {
            Value::Object(m) => {
                m.shift_remove("rev");
                for x in m.values_mut() {
                    walk(x);
                }
            }
            Value::Array(a) => a.iter_mut().for_each(walk),
            _ => {}
        }
    }
    let mut d = ok(h, "draw.drawing.get", json!({"drawingId":D}));
    walk(&mut d);
    d
}
fn ids(v: &Value) -> BTreeSet<String> {
    fn walk(v: &Value, o: &mut BTreeSet<String>) {
        match v {
            Value::Object(m) => {
                if let Some(s) = m.get("id").and_then(Value::as_str) {
                    o.insert(s.to_owned());
                }
                for x in m.values() {
                    walk(x, o);
                }
            }
            Value::Array(a) => {
                for x in a {
                    walk(x, o)
                }
            }
            _ => {}
        }
    }
    let mut o = BTreeSet::new();
    walk(v, &mut o);
    o
}
fn set(v: &Value) -> BTreeSet<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_owned()).collect()
}
fn setup(h: &ExecutorHandle, i: usize) -> (&'static str, Value) {
    let r = ok(
        h,
        "draw.rectangle.add",
        json!({"drawingId":D,"from":{"x":1000,"y":1000},"to":{"x":1100,"y":1060},"layer":"new"}),
    );
    // Resolve a gradient by kind, rather than rely on sample swatch order.
    let c = h
        .snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter())
        .find(|s| format!("{:?}", s.kind()) == "LinearGradient")
        .unwrap()
        .colour_id()
        .to_string();
    ok(h, "colour.paint.apply", json!({"drawingId":D,"objectIds":[r["zoneIds"][0]],"colourId":c}));
    match i {
        0 => (
            "draw.rectangle.add",
            json!({"drawingId":D,"from":{"x":1200,"y":1000},"to":{"x":1250,"y":1050},"layer":"new"}),
        ),
        1 => ("draw.transform.apply", json!({"drawingId":D,"ids":[r["layerId"]],"translate":{"x":20,"y":5}})),
        2 => ("draw.arrange.apply", json!({"drawingId":D,"ids":[r["layerId"]],"to":"back"})),
        3 => (
            "colour.fillMapping.set",
            json!({"drawingId":D,"objectIds":[r["zoneIds"][0]],"mapping":{"linear":{"start":{"x":1020,"y":1000},"end":{"x":1080,"y":1050}}}}),
        ),
        _ => (
            "colour.selection.apply",
            json!({"drawingId":D,"objectIds":[r["zoneIds"][0]],"colourId":"col_01K742SG059XGSNZ6M9MACZ2ZY"}),
        ),
    }
}
#[test]
fn five_independent_mutations_are_atomic_reversible_and_persistent() {
    for i in 0..5 {
        let h = fixture();
        let (cmd, p) = setup(&h, i);
        let before = normalized(&h);
        let start = rev(&h);
        let dry = with(&h, cmd, p.clone(), InvokeOptions { dry_run: true, ..Default::default() });
        assert!(dry.is_ok(), "{cmd}: {dry:?}");
        assert_eq!((rev(&h), normalized(&h)), (start, before.clone()));
        let mut stale = InvokeOptions::default();
        stale.expect_rev.insert(D.to_owned(), 0);
        let refused = with(&h, cmd, p.clone(), stale);
        assert_eq!(refused.error.unwrap().code, "rev.conflict", "{cmd}: stale envelope revision");
        assert_eq!((rev(&h), normalized(&h)), (start, before.clone()));
        let failed = batch(
            &h,
            json!({"label":"review rollback","steps":[{"id":cmd,"params":p},{"id":"draw.transform.apply","params":{"drawingId":D,"ids":["st_missing"],"translate":{"x":1,"y":0}}}]}),
        );
        assert!(!failed.is_ok());
        assert_eq!((rev(&h), normalized(&h)), (start, before.clone()));
        let rx = h.subscribe();
        let r = ok(&h, cmd, p.clone());
        let after = normalized(&h);
        assert_eq!(rev(&h), start + 1);
        let events: Vec<_> = rx.try_iter().filter(|e| e.name == "draw.drawing.changed").collect();
        assert_eq!(events.len(), 1);
        let (a, b) = (ids(&before), ids(&after));
        let made: BTreeSet<_> = b.difference(&a).cloned().collect();
        let gone: BTreeSet<_> = a.difference(&b).cloned().collect();
        assert_eq!(set(&events[0].payload["createdIds"]), made, "{cmd}");
        assert_eq!(set(&events[0].payload["removedIds"]), gone, "{cmd}");
        if r.get("createdIds").is_some() {
            assert_eq!(set(&r["createdIds"]), made);
            assert_eq!(set(&r["removedIds"]), gone);
        }
        ok(&h, "edit.undo", json!({}));
        assert_eq!(serde_json::to_vec(&normalized(&h)).unwrap(), serde_json::to_vec(&before).unwrap());
        ok(&h, "edit.redo", json!({}));
        assert_eq!(serde_json::to_vec(&normalized(&h)).unwrap(), serde_json::to_vec(&after).unwrap());
        let dir = c13_support::tmp();
        let package = dir.join("roundtrip.animpkg");
        c13_support::job_ok(&h, "project.saveAs", json!({"path":package}));
        drop(h);
        let h2 = c13_support::spawn(&dir.join("user"));
        c13_support::job_ok(&h2, "project.open", json!({"path":package}));
        assert_eq!(normalized(&h2), after);
        drop(h2);
        std::fs::remove_dir_all(dir).unwrap();
        let h = fixture();
        let (cmd, p) = setup(&h, i);
        ok(&h, "draw.lock.set", json!({"drawingId":D,"locked":true}));
        let start = rev(&h);
        assert_eq!(err(&h, cmd, p).code, "drawing.locked");
        assert_eq!(rev(&h), start);
    }
}
#[test]
fn transform_stale_revision_and_render_file_staging() {
    let h = fixture();
    let (_, mut p) = setup(&h, 1);
    p["expectRev"] = json!(0);
    let start = rev(&h);
    assert_eq!(err(&h, "draw.transform.apply", p).code, "drawing.staleRevision");
    assert_eq!(rev(&h), start);
    let dir = c13_support::tmp();
    let path = dir.join("render.png");
    let p = json!({"drawingId":D,"width":32,"path":path});
    let r = with(&h, "draw.drawing.render", p.clone(), InvokeOptions { dry_run: true, ..Default::default() });
    assert!(r.is_ok());
    assert!(!path.exists());
    let r = batch(
        &h,
        json!({"label":"rollback output","steps":[{"id":"draw.drawing.render","params":p},{"id":"draw.transform.apply","params":{"drawingId":D,"ids":["st_missing"]}}]}),
    );
    assert!(!r.is_ok());
    assert!(!path.exists());
    let r = h.invoke(tt_executor::Invocation {
        command: "draw.drawing.render".into(),
        params: p,
        options: Default::default(),
        origin: agent(&[tt_registry::Scope::Read]),
    });
    assert!(matches!(r.error.unwrap().code.as_str(), "permission.denied" | "fs.outsideRoots"));
    assert!(!path.exists());
    std::fs::remove_dir_all(dir).unwrap();
}
