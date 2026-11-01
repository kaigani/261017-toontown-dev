//! Final independent import, recovery and mapping variants. No product mutations outside commands.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "independent QA")]
mod c13_support;
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const M: &str = "col_01K9YYYYYYYYYYYYYYYYYYYYYY";
fn drawing(h: &ExecutorHandle, id: &str) -> Value {
    h.snapshot()
        .unwrap()
        .unwrap()
        .entry_raw_json(&tt_model::document::EntryKey::Drawing(tt_model::Id::parse(id).unwrap()))
        .unwrap()
}
fn box_zone(h: &ExecutorHandle) -> Value {
    ok(
        h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"invisible","layer":"new","fit":false,"closed":true,"points":[{"x":1000,"y":1000},{"x":1100,"y":1000},{"x":1100,"y":1050},{"x":1000,"y":1050}]}),
    )
}
fn layer(h: &ExecutorHandle, id: &Value) -> Value {
    ok(h, "draw.drawing.get", json!({"drawingId":D}))["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == *id)
        .unwrap()
        .clone()
}
fn opaque() -> Value {
    json!({"org.review.payload":{"type":"swatch","rev":777,"nested":[{"type":"foreign","rev":888}],"flag":true}})
}

#[test]
fn import_revision_reset_preserves_opaque_meta_and_ext_for_both_id_policies() {
    let mut observations = Vec::new();
    for policy in ["preserve", "new"] {
        let src = fixture();
        ok(
            &src,
            "doc.object.patch",
            json!({"targets":[D],"props":{"ext":opaque(),"meta":{"record":{"type":"foreign","rev":999}}}}),
        );
        let expected = drawing(&src, D);
        let b = ok(&src, "doc.export.json", json!({"scope":"project","binaries":"embed"}))["bundle"].clone();
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        ok(&h, "project.create", json!({"name":"R3 target"}));
        let r = ok(&h, "doc.import.json", json!({"source":b,"idPolicy":policy}));
        let id = if policy == "preserve" {
            D.to_owned()
        } else {
            r["created"]["drawings"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .find(|id| drawing(&h, id)["ext"]["org.review.payload"].is_object())
                .unwrap()
                .to_owned()
        };
        let got = drawing(&h, &id);
        eprintln!("{policy}: imported ext={} meta={}", got["ext"], got["meta"]);
        observations.push((
            policy.to_owned(),
            got["ext"].clone(),
            expected["ext"].clone(),
            got["meta"].clone(),
            expected["meta"].clone(),
        ));
    }
    for (policy, ext, want_ext, meta, want_meta) in observations {
        assert_eq!(ext, want_ext, "{policy}: opaque extension fields are not model revisions");
        assert_eq!(meta, want_meta, "{policy}: opaque metadata is not a model revision");
    }
}

fn mixed_document() -> tt_model::document::Document {
    let mut p = tt_model::fixture::sample_parts(1).unwrap();
    let cols =
        p.palettes.iter().flat_map(|p| p.swatches()).map(|s| s.colour_id().as_str().to_owned()).collect::<Vec<_>>();
    let donor = serde_json::to_value(p.drawings.iter().find(|d| d.id().as_str() == D).unwrap()).unwrap();
    for i in 0..2 {
        let mut d = serde_json::to_value(&p.drawings[i]).unwrap();
        let mut n = donor["nodes"][0].clone();
        n["id"] = json!(format!("nd_r3_{i}"));
        let points = [(1000, 1000), (1000, 1010), (1100, 1010), (1100, 1000)];
        n["strokes"] = json!(
            (0..4)
                .map(|k| {
                    let mut s = donor["nodes"][0]["strokes"][1].clone();
                    s["id"] = json!(format!("st_r3_{i}_{k}"));
                    s["kind"] = json!("pencil");
                    s["closed"] = json!(false);
                    let (a, b) = (points[k], points[(k + 1) % 4]);
                    s["points"] = json!([[a.0, a.1, 0, 0, 0, 0], [b.0, b.1, 0, 0, 0, 0]]);
                    s["sides"] =
                        json!({"left":null,"right":{"colourId":cols[if i==0 || k%2==1 {2}else{1}],"matrix":null}});
                    s
                })
                .collect::<Vec<_>>()
        );
        d["nodes"] = json!([n]);
        p.drawings[i] = tt_model::object::from_json(&d).unwrap();
    }
    tt_model::document::Document::from_parts(p).unwrap()
}
#[test]
fn multi_drawing_import_repairs_only_inconsistent_drawing_and_undoes_everything() {
    let dir = c13_support::tmp();
    let src = c13_support::spawn(&dir.join("report-source"));
    assert!(src.open_document(mixed_document(), None, false).is_ok());
    c13_support::job_ok(&src, "project.saveAs", json!({"path":dir.join("mixed.animpkg")}));
    let loaded = c13_support::spawn(&dir.join("report-load"));
    c13_support::job_ok(&loaded, "project.open", json!({"path":dir.join("mixed.animpkg")}));
    let report = ok(&loaded, "doc.loadReport.get", json!({}));
    let load_issue =
        report["issues"].as_array().unwrap().iter().find(|i| i["code"] == "drawing.fillInconsistent").unwrap();
    assert_eq!(load_issue["details"]["zones"], 1);
    assert!(report["repairedCount"].as_u64().unwrap() >= 1);
    assert_eq!(load_issue["severity"], "warning");
    eprintln!("load issue shape={load_issue}");
    for policy in ["preserve", "new"] {
        let src = ExecutorHandle::spawn(Default::default()).unwrap();
        assert!(src.open_document(mixed_document(), None, false).is_ok());
        let bundle = ok(&src, "doc.export.json", json!({"scope":"project","binaries":"embed"}))["bundle"].clone();
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        ok(&h, "project.create", json!({"name":"multi"}));
        let before = h.snapshot().unwrap().unwrap().entry_keys();
        let rev = rev(&h);
        let r = ok(&h, "doc.import.json", json!({"source":bundle,"idPolicy":policy}));
        let issues = r["issues"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["code"] == "drawing.fillInconsistent")
            .collect::<Vec<_>>();
        assert_eq!(issues.len(), 1, "{r}");
        let issue = issues[0];
        assert_eq!(issue["zones"], 1);
        assert!(issue["layerId"].is_string());
        assert!(issue["message"].is_string());
        let d = drawing(&h, issue["objectId"].as_str().unwrap());
        let s = d["nodes"][0]["strokes"].as_array().unwrap();
        assert_eq!(s.len(), 4);
        assert!(s.iter().all(|x| x["sides"]["right"] == s[0]["sides"]["right"]));
        assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), rev + 1);
        eprintln!("{policy}: import issue={issue}");
        ok(&h, "edit.undo", json!({}));
        assert_eq!(h.snapshot().unwrap().unwrap().entry_keys(), before);
        ok(&h, "edit.redo", json!({}));
        assert_eq!(
            drawing(&h, issue["objectId"].as_str().unwrap())["nodes"][0]["strokes"].as_array().unwrap().len(),
            4
        );
    }
    drop((src, loaded));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn texture_click_uses_palette_first_style_and_drag_and_tile_off_stay_unchanged() {
    for tile in [false, true] {
        let mut p = tt_model::fixture::sample_parts(1).unwrap();
        let mut pal = serde_json::to_value(&p.palettes[0]).unwrap();
        let c = pal["swatches"][0]["colourId"].as_str().unwrap().to_owned();
        // Deliberately reverse palette style order relative to key order in the values map.
        pal["styles"].as_array_mut().unwrap().sort_by(|a, b| b["id"].as_str().cmp(&a["id"].as_str()));
        let styles = pal["styles"].as_array().unwrap().clone();
        assert!(styles.len() >= 2);
        let mut values = serde_json::Map::new();
        for (i, s) in styles.iter().enumerate() {
            values.insert(s["id"].as_str().unwrap().into(),json!({"image":"textures/r3.png","tile":tile,"width":if i==0 {17}else{31},"height":if i==0 {9}else{23}}));
        }
        pal["swatches"][0]["kind"] = json!("texture");
        pal["swatches"][0]["values"] = json!(values);
        p.palettes[0] = tt_model::object::from_json(&pal).unwrap();
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        assert!(h.open_document(tt_model::document::Document::from_parts(p).unwrap(), None, false).is_ok());
        let a = box_zone(&h);
        ok(&h, "colour.paint.apply", json!({"drawingId":D,"objectIds":[a["zoneIds"][0]],"colourId":c}));
        let got = layer(&h, &a["layerId"])["zones"][0]["textureMatrix"].clone();
        eprintln!("tile={tile}, first style={}, matrix={got}", styles[0]["id"]);
        ok(
            &h,
            "colour.paint.apply",
            json!({"drawingId":D,"objectIds":[a["zoneIds"][0]],"colourId":c,"mapping":{"from":{"x":1000,"y":1000},"to":{"x":1030,"y":1040}}}),
        );
        assert_eq!(layer(&h, &a["layerId"])["zones"][0]["textureMatrix"], json!([30, 40, -40, 30, 1000, 1000]));
        assert_eq!(got, if tile { json!([17, 0, 0, -9, 1000, 1050]) } else { json!([100, 0, 0, 50, 1000, 1000]) });
    }
}

#[test]
fn load_and_import_recovery_keep_nested_extensions_meta_deformable_and_path_protection() {
    let dir = c13_support::tmp();
    let mut p = tt_model::fixture::sample_parts(1).unwrap();
    let i = p.drawings.iter().position(|d| d.id().as_str() == D).unwrap();
    let mut d = serde_json::to_value(&p.drawings[i]).unwrap();
    d["nodes"][0]["strokes"][1]["colourId"] = json!(M);
    let ext = json!({"dev.toontown.paint":{"protected":true},"org.review":{"nested":{"arr":[1,{"more":[null,true,"kept"],"type":"foreign","rev":321}]}}});
    let meta = json!({"note":{"text":"full copy","numbers":[1,2,3]}});
    d["colourRecovery"] = json!([{"colourId":M,"paletteId":"pal_r3","paletteName":"Kept","styles":[{"styleId":"sty_r3","styleName":"Default"}],"swatch":{"colourId":M,"type":"swatch","rev":7,"name":"Protected","kind":"solid","deformable":true,"values":{"sty_r3":{"rgba":"#102030FF"}},"ext":ext,"meta":meta}}]);
    p.drawings[i] = tt_model::object::from_json(&d).unwrap();
    let src = c13_support::spawn(&dir.join("src"));
    assert!(src.open_document(tt_model::document::Document::from_parts(p).unwrap(), None, false).is_ok());
    c13_support::job_ok(&src, "project.saveAs", json!({"path":dir.join("r3.animpkg")}));
    let bundle = ok(&src, "doc.export.json", json!({"scope":"project","binaries":"embed"}))["bundle"].clone();
    for import in [false, true] {
        let h = c13_support::spawn(&dir.join(if import { "import" } else { "load" }));
        if import {
            ok(&h, "project.create", json!({"name":"r3"}));
            ok(&h, "doc.import.json", json!({"source":bundle,"idPolicy":"preserve"}));
        } else {
            c13_support::job_ok(&h, "project.open", json!({"path":dir.join("r3.animpkg")}));
        }
        let doc = h.snapshot().unwrap().unwrap();
        let s = doc.palettes().flat_map(|p| p.swatches()).find(|s| s.colour_id().as_str() == M).unwrap();
        let sw = serde_json::to_value(s).unwrap();
        assert_eq!(sw["ext"], ext);
        assert_eq!(sw["meta"], meta);
        assert_eq!(sw["deformable"], true);
        let colour = doc
            .palettes()
            .flat_map(|p| p.swatches())
            .find(|s| s.colour_id().as_str() != M)
            .unwrap()
            .colour_id()
            .as_str()
            .to_owned();
        drop(doc);
        // The fixture pencil is targeted by a path through its first centreline anchor.
        let raw = drawing(&h, D);
        let pt = &raw["nodes"][0]["strokes"][1]["points"][0];
        let x = pt[0].as_f64().unwrap();
        let y = pt[1].as_f64().unwrap();
        let r = run(
            &h,
            "colour.paint.apply",
            json!({"drawingId":D,"path":[{"x":x-1.,"y":y},{"x":x+1.,"y":y}],"colourId":colour}),
        );
        assert_eq!(r.error.as_ref().unwrap().code, "colour.protected", "{r:?}");
        assert_eq!(drawing(&h, D)["nodes"][0]["strokes"][1]["colourId"], M);
    }
    drop(src);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn pasted_piece_event_reports_all_new_pieces_and_removed_original_once() {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let i = parts.drawings.iter().position(|d| d.id().as_str() == D).unwrap();
    let mut d = serde_json::to_value(&parts.drawings[i]).unwrap();
    let mut existing = d["nodes"][0]["strokes"][1].clone();
    existing["points"] = json!([[-10, 0, 0, 0, 0, 0], [10, 0, 0, 0, 0, 0]]);
    let old = existing["id"].clone();
    let mut pasted = existing.clone();
    pasted["id"] = json!("st_r3_paste");
    pasted["points"] = json!([[0, -10, 0, 0, 0, 0], [0, 10, 0, 0, 0, 0]]);
    d["nodes"][0]["strokes"] = json!([existing]);
    d["nodes"].as_array_mut().unwrap().truncate(1);
    parts.drawings[i] = tt_model::object::from_json(&d).unwrap();
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(tt_model::document::Document::from_parts(parts).unwrap(), None, false).is_ok());
    ok(
        &h,
        "edit.clipboard.set",
        json!({"content":{"kind":"drawingObjects","sourceContext":"drawing","itemCount":1,"items":[pasted],"options":null}}),
    );
    let rx = h.subscribe();
    let r = ok(&h, "edit.paste", json!({"context":"drawing","destination":{"drawingId":D}}));
    let events = rx.try_iter().filter(|e| e.name == "draw.drawing.changed").collect::<Vec<_>>();
    assert_eq!(events.len(), 1);
    let raw = drawing(&h, D);
    let strokes = raw["nodes"][0]["strokes"].as_array().unwrap();
    assert_eq!(strokes.len(), 4);
    assert_eq!(r["createdIds"].as_array().unwrap().len(), 2);
    let event = &events[0].payload;
    assert_eq!(event["createdIds"].as_array().unwrap().len(), 4);
    assert_eq!(event["removedIds"], json!([old]));
    for s in strokes {
        assert!(event["createdIds"].as_array().unwrap().contains(&s["id"]));
    }
    eprintln!("pasted piece event={event}; paste's own created IDs={}", r["createdIds"]);
    ok(&h, "edit.undo", json!({}));
    assert_eq!(drawing(&h, D)["nodes"][0]["strokes"].as_array().unwrap().len(), 1);
    ok(&h, "edit.redo", json!({}));
    assert_eq!(drawing(&h, D)["nodes"][0]["strokes"].as_array().unwrap().len(), 4);
}
