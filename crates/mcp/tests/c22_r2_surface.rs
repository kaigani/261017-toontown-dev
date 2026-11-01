#![allow(clippy::unwrap_used, missing_docs, reason = "independent QA variants")]
mod support;
mod topology_oracle;
use serde_json::json;
use support::*;
#[test]
fn prefs_agent_failed_atomic_batch_rolls_back_gap_setting() {
    let dir = tmp();
    let h = executor(&dir.join("prefs"));
    let mut options = opts(&dir);
    options.policy = Some(tt_executor::agents::Policy::Autonomous);
    options.scopes =
        Some([tt_registry::Scope::Read, tt_registry::Scope::Ui, tt_registry::Scope::Prefs].into_iter().collect());
    let (mut c, _) = Client::connect(&h, options, json!({}));
    assert_eq!(c.run("colour.gapClosing.set", json!({"mode":"off"}))["status"], "ok");
    let r=c.call("batch",json!({"atomic":true,"label":"R2 rollback","steps":[{"id":"colour.gapClosing.set","params":{"mode":"large"}},{"id":"colour.gapClosing.set","params":{"mode":"invalid"}}]}));
    assert_ne!(r["structuredContent"]["status"], "ok", "{r}");
    assert_eq!(c.run("colour.gapClosing.get", json!({}))["result"]["mode"], "off");
    c.close().unwrap();
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
fn all_four_deferred_contracts_and_generated_types_name_c23() {
    let registry = tt_executor::registry().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for id in ["colour.fillMapping.get", "colour.fillMapping.set", "colour.selection.apply", "colour.recolour.apply"] {
        let d = registry.entries().find(|e| e.id() == id).unwrap().describe();
        assert_eq!(d["milestone"], "M2 C2.3", "{d}");
    }
    let generated = std::fs::read_to_string(root.join("desktop/src/ipc/generated/commands.ts")).unwrap();
    assert!(generated.contains("colour.fillMapping.get"));
}

#[test]
fn topology_oracle_checks_every_step_of_full_cut_stack_and_reverse_merge() {
    use std::collections::BTreeSet;
    use topology_oracle::Topo;
    assert_eq!(topology_oracle::CUTS, 4);
    let dir = tmp();
    let h = executor(&dir.join("prefs"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut options = opts(&dir);
    options.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = Client::connect(&h, options, json!({}));
    let d = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
    let points = |p: Vec<(f64, f64)>| p.into_iter().map(|(x, y)| json!({"x":x,"y":y})).collect::<Vec<_>>();
    let r=c.run("draw.stroke.add",json!({"drawingId":d,"kind":"pencil","layer":"new","closed":true,"fit":false,"points":points(topology_oracle::square_anchors())}));
    assert_eq!(r["status"], "ok");
    let layer = r["result"]["layerId"].as_str().unwrap();
    let raw = |h: &tt_executor::ExecutorHandle| {
        let doc = h.snapshot().unwrap().unwrap();
        let v = doc.entry_raw_json(&tt_model::document::EntryKey::Drawing(tt_model::Id::parse(d).unwrap())).unwrap();
        v["nodes"].as_array().unwrap().iter().find(|n| n["id"] == layer).unwrap()["strokes"].as_array().unwrap().clone()
    };
    let template = raw(&h)[0].clone();
    let mut topo = Topo::new(template["id"].as_str().unwrap());
    let mut known = BTreeSet::from([template["id"].as_str().unwrap().to_owned()]);
    for k in [3, 1, 0, 2] {
        let mut next = topo.cut(k);
        let r=c.run("draw.stroke.add",json!({"drawingId":d,"kind":"pencil","layer":layer,"fit":false,"points":points(topology_oracle::cut_anchors(k)),"colourId":template["colourId"],"thickness":template["lineStyle"]["thickness"]}));
        assert_eq!(r["status"], "ok", "{r}");
        let actual = raw(&h);
        let ids: Vec<_> = actual.iter().map(|s| s["id"].as_str().unwrap().to_owned()).collect();
        assert!(next.adopt(&ids, &known));
        known.extend(ids);
        topo = next;
        let mut expected: Vec<_> = topo.strokes(&template).into_iter().map(|(_, v)| v).collect();
        let mut actual = actual;
        for v in expected.iter_mut().chain(actual.iter_mut()) {
            v.as_object_mut().unwrap().remove("rev");
        }
        assert_eq!(actual, expected, "cut {k}");
        let read = c.run("draw.drawing.get", json!({"drawingId":d}));
        let zones = read["result"]["nodes"].as_array().unwrap().iter().find(|n| n["id"] == layer).unwrap()["zones"]
            .as_array()
            .unwrap();
        let ids: BTreeSet<String> = zones.iter().map(|z| z["id"].as_str().unwrap().to_owned()).collect();
        let expected: BTreeSet<String> = topo.zones().iter().map(|&(l, r)| topo.zone_id(l, r)).collect();
        assert_eq!(ids, expected);
    }
    for k in [2, 0, 1, 3] {
        let ids = topo.cuts.iter().find(|(i, _)| *i == k).unwrap().1.clone();
        let r = c.run("draw.object.delete", json!({"drawingId":d,"ids":ids}));
        assert_eq!(r["status"], "ok");
        topo = topo.merge(k);
        let read = c.run("draw.drawing.get", json!({"drawingId":d}));
        let zones = read["result"]["nodes"].as_array().unwrap().iter().find(|n| n["id"] == layer).unwrap()["zones"]
            .as_array()
            .unwrap();
        assert_eq!(zones.len(), topo.zones().len());
        for (l, r) in topo.zones() {
            assert!(zones.iter().any(|z| z["id"] == topo.zone_id(l, r)));
        }
        assert_eq!(c.run("edit.undo", json!({}))["status"], "ok");
        assert_eq!(c.run("edit.redo", json!({}))["status"], "ok");
    }
    c.close().unwrap();
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
}
