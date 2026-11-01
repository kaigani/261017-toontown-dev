//! MCP final-round topology, stale IDs, event deltas and saved-project readback.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "independent QA")]
mod support;
use serde_json::{Value, json};
use support::*;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
fn run(c: &mut Client, id: &str, p: Value) -> Value {
    let r = c.run(id, p);
    assert_eq!(r["status"], "ok", "{id}: {r}");
    r["result"].clone()
}
fn zones(c: &mut Client, layer: &Value) -> Value {
    run(c, "draw.drawing.get", json!({"drawingId":D}))["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == *layer)
        .unwrap()["zones"]
        .clone()
}

#[test]
fn e2_split_stale_fresh_ids_events_inspect_and_saved_readback() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut o = opts(&dir);
    o.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = Client::connect(&h, o, json!({}));
    let colours = h
        .snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches())
        .map(|s| s.colour_id().as_str().to_owned())
        .collect::<Vec<_>>();
    let r = run(
        &mut c,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"pencil","layer":"new","fit":false,"closed":true,"points":[{"x":1000,"y":1000},{"x":1100,"y":1000},{"x":1100,"y":1100},{"x":1000,"y":1100}]}),
    );
    let layer = r["layerId"].clone();
    let old = r["zoneIds"][0].clone();
    run(&mut c, "colour.paint.apply", json!({"drawingId":D,"objectIds":[old],"colourId":colours[1]}));
    let rx = h.subscribe();
    let cut = run(
        &mut c,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"invisible","layer":layer,"fit":false,"points":[{"x":1040,"y":990},{"x":1040,"y":1110}]}),
    );
    let events = rx.try_iter().filter(|e| e.name == "draw.drawing.changed").collect::<Vec<_>>();
    assert_eq!(events.len(), 1);
    for k in ["createdIds", "removedIds"] {
        let ids = |v: &Value| {
            v.as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_owned())
                .collect::<std::collections::BTreeSet<_>>()
        };
        assert_eq!(ids(&events[0].payload[k]), ids(&cut[k]), "{k}");
    }
    assert!(cut["removedIds"].as_array().unwrap().contains(&old));
    let z = zones(&mut c, &layer);
    assert_eq!(z.as_array().unwrap().len(), 2);
    assert!(z.as_array().unwrap().iter().all(|z| z["colourId"] == colours[1]));
    let before = h.snapshot().unwrap().unwrap().document_rev();
    let stale = c.run("colour.paint.apply", json!({"drawingId":D,"objectIds":[old],"colourId":colours[2]}));
    assert_eq!(stale["error"]["code"], "colour.staleZoneId");
    assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), before);
    let ids = z.as_array().unwrap().iter().map(|z| z["id"].clone()).collect::<Vec<_>>();
    run(&mut c, "colour.paint.apply", json!({"drawingId":D,"objectIds":ids,"colourId":colours[2]}));
    assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), before + 1);
    assert_eq!(rx.try_iter().filter(|e| e.name == "draw.drawing.changed").count(), 1);
    let painted = zones(&mut c, &layer);
    assert!(painted.as_array().unwrap().iter().all(|z| z["colourId"] == colours[2]));
    run(&mut c, "edit.undo", json!({}));
    assert_eq!(zones(&mut c, &layer), z);
    run(&mut c, "edit.redo", json!({}));
    assert_eq!(zones(&mut c, &layer), painted);
    let resource = c.request("resources/read", json!({"uri":format!("app://drawing/{D}")}));
    let content = resource["contents"][0]["text"].as_str().unwrap();
    assert!(content.contains(cut["strokeIds"][0].as_str().unwrap()));
    assert!(content.contains(&colours[2]));
    let saved = run(&mut c, "project.saveAs", json!({"path":dir.join("r3.animpkg")}));
    let done = run(&mut c, "job.wait", json!({"jobId":saved["jobId"],"timeoutMs":60000}));
    assert_eq!(done["state"], "succeeded", "{done}");
    c.close().unwrap();
    drop(h);
    let h = executor(&dir.join("reopen"));
    let mut o = opts(&dir);
    o.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = Client::connect(&h, o, json!({}));
    let opened = run(&mut c, "project.open", json!({"path":dir.join("r3.animpkg")}));
    let done = run(&mut c, "job.wait", json!({"jobId":opened["jobId"],"timeoutMs":60000}));
    assert_eq!(done["state"], "succeeded", "{done}");
    assert_eq!(zones(&mut c, &layer), painted);
    let report = run(&mut c, "doc.loadReport.get", json!({}));
    assert!(!report["issues"].as_array().unwrap().iter().any(|i| i["code"] == "drawing.fillInconsistent"));
    eprintln!("split event {}; saved zones {}", events[0].payload, painted);
    c.close().unwrap();
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
}
