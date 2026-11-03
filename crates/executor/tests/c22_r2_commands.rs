#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "independent QA variants")]
mod c13_support;
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Invocation, InvokeOptions, Origin};
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const M: &str = "col_01K9YYYYYYYYYYYYYYYYYYYYYY";
fn rect(h: &ExecutorHandle, layer: &str, x: f64, y: f64, w: f64, hg: f64) -> Value {
    ok(
        h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"invisible","layer":layer,"fit":false,"closed":true,
        "points":[{"x":x,"y":y},{"x":x+w,"y":y},{"x":x+w,"y":y+hg},{"x":x,"y":y+hg}]}),
    )
}
fn read(h: &ExecutorHandle, l: &str) -> Value {
    ok(h, "draw.drawing.get", json!({"drawingId":D}))["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == l)
        .unwrap()
        .clone()
}
fn cols(h: &ExecutorHandle) -> Vec<String> {
    h.snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
        .collect()
}
fn inconsistent() -> tt_model::document::Document {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let colors: Vec<_> =
        parts.palettes.iter().flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned())).collect();
    let i = parts.drawings.iter().position(|d| d.id().as_str() == D).unwrap();
    let mut d = serde_json::to_value(&parts.drawings[i]).unwrap();
    let template = d["nodes"][0]["strokes"][1].clone();
    // Clockwise rectangle: right sides are internal. Two long blue edges beat two short red edges.
    let points = [(1000., 1000.), (1000., 1010.), (1100., 1010.), (1100., 1000.)];
    d["nodes"][0]["strokes"] = json!(
        (0..4)
            .map(|k| {
                let mut s = template.clone();
                s["id"] = json!(format!("st_r2_side{k}"));
                s["kind"] = json!("pencil");
                s["closed"] = json!(false);
                let (a, b) = (points[k], points[(k + 1) % 4]);
                s["points"] = json!([[a.0, a.1, 0, 0, 0, 0], [b.0, b.1, 0, 0, 0, 0]]);
                s["sides"] = json!({"left":null,"right":{"colourId":colors[if k%2==0 {1}else{2}],"matrix":null}});
                s
            })
            .collect::<Vec<_>>()
    );
    parts.drawings[i] = tt_model::object::from_json(&d).unwrap();
    tt_model::document::Document::from_parts(parts).unwrap()
}
fn sides(h: &ExecutorHandle) -> Vec<Value> {
    let doc = h.snapshot().unwrap().unwrap();
    let d = doc.entry_raw_json(&tt_model::document::EntryKey::Drawing(tt_model::Id::parse(D).unwrap())).unwrap();
    d["nodes"][0]["strokes"].as_array().unwrap().iter().map(|s| s["sides"]["right"]["colourId"].clone()).collect()
}
#[test]
fn json_import_repairs_clockwise_side_disagreement_without_renoding() {
    let source = ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(source.open_document(inconsistent(), None, false).is_ok());
    let bundle = ok(&source, "doc.export.json", json!({"scope":"project","binaries":"embed"}))["bundle"].clone();
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    ok(&h, "project.create", json!({"name":"R2"}));
    ok(&h, "doc.import.json", json!({"source":bundle,"idPolicy":"preserve"}));
    let got = sides(&h);
    eprintln!("imported right side fills {got:?}");
    assert!(got.iter().all(|s| s == &got[1]), "import must repair inconsistent sides to longest boundary: {got:?}");
}
#[test]
fn loading_clockwise_sides_is_deterministic_and_a_base() {
    let dir = c13_support::tmp();
    let package = dir.join("clockwise.animpkg");
    let h = c13_support::spawn(&dir.join("source"));
    assert!(h.open_document(inconsistent(), None, false).is_ok());
    c13_support::job_ok(&h, "project.saveAs", json!({"path":package}));
    drop(h);
    let mut observed = Vec::new();
    for k in 0..2 {
        let h = c13_support::spawn(&dir.join(format!("load{k}")));
        c13_support::job_ok(&h, "project.open", json!({"path":package}));
        let report = ok(&h, "doc.loadReport.get", json!({}));
        assert!(report["issues"].as_array().unwrap().iter().any(|i| i["code"] == "drawing.fillInconsistent"));
        let got = sides(&h);
        assert!(got.iter().all(|s| s == &got[1]), "{got:?}");
        observed.push(got);
        assert!(!run(&h, "edit.undo", json!({})).is_ok(), "repair must be an unundoable base");
    }
    assert_eq!(observed[0], observed[1]);
    drop(package);
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
fn path_reentry_across_two_layers_is_one_transaction_and_respects_only_painted() {
    let h = fixture();
    let a = rect(&h, "new", 1000., 1000., 10., 10.);
    let b = rect(&h, "new", 1010., 1000., 10., 10.);
    let colors = cols(&h);
    ok(&h, "colour.paint.apply", json!({"drawingId":D,"objectIds":[a["zoneIds"][0]],"colourId":colors[1]}));
    let rx = h.subscribe();
    let rev = h.snapshot().unwrap().unwrap().document_rev();
    let r = ok(
        &h,
        "colour.paint.apply",
        json!({"drawingId":D,"path":[{"x":999,"y":1005},{"x":1021,"y":1005},{"x":999,"y":1005}],"onlyPainted":true,"colourId":colors[2]}),
    );
    assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), rev + 1);
    assert_eq!(r["painted"].as_array().unwrap().iter().filter(|p| p["objectKind"] == "zone").count(), 1);
    assert_eq!(rx.try_iter().filter(|e| e.name == "draw.drawing.changed").count(), 1);
    assert_eq!(read(&h, a["layerId"].as_str().unwrap())["zones"][0]["colourId"], colors[2]);
    assert!(read(&h, b["layerId"].as_str().unwrap())["zones"][0]["colourId"].is_null());
    ok(&h, "edit.undo", json!({}));
    ok(&h, "edit.redo", json!({}));
    assert_eq!(read(&h, a["layerId"].as_str().unwrap())["zones"][0]["colourId"], colors[2]);
    ok(
        &h,
        "doc.object.patch",
        json!({"targets":[colors[2]],"props":{"ext":{"dev.toontown.paint":{"protected":true}}}}),
    );
    let r = ok(
        &h,
        "colour.paint.apply",
        json!({"drawingId":D,"path":[{"x":999,"y":1005},{"x":1021,"y":1005}],"colourId":colors[1]}),
    );
    assert!(r["skipped"].as_array().unwrap().iter().any(|s| s["reason"] == "colour.protected"));
    assert_eq!(read(&h, a["layerId"].as_str().unwrap())["zones"][0]["colourId"], colors[2]);
    ok(
        &h,
        "colour.paint.apply",
        json!({"drawingId":D,"path":[{"x":999,"y":1005},{"x":1021,"y":1005}],"colourId":colors[1],"respectProtected":false}),
    );
    assert_eq!(read(&h, a["layerId"].as_str().unwrap())["zones"][0]["colourId"], colors[1]);
}
#[test]
fn gap_closing_preserves_id_point_line_and_text_targets_and_undo_redo() {
    let h = fixture();
    let a = rect(&h, "new", 1000., 1000., 10., 10.);
    let lid = a["layerId"].as_str().unwrap();
    let b = rect(&h, lid, 1020., 1000., 10., 10.);
    let line = ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"pencil","layer":lid,"fit":false,"points":[{"x":1200,"y":1000},{"x":1210,"y":1000}]}),
    );
    let text = ok(&h, "draw.text.add", json!({"drawingId":D,"text":"QA","position":{"x":1300,"y":1000}}));
    ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"pencil","layer":lid,"fit":false,"points":[{"x":2000,"y":2100},{"x":2000,"y":2000},{"x":2100,"y":2000},{"x":2100,"y":2100},{"x":2051,"y":2100}]}),
    );
    ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"pencil","layer":lid,"fit":false,"points":[{"x":2049,"y":2100},{"x":2000,"y":2100}]}),
    );
    let colors = cols(&h);
    ok(&h, "colour.gapClosing.set", json!({"mode":"small"}));
    let before = h.snapshot().unwrap().unwrap().document_rev();
    let refused =
        run(&h, "colour.paint.apply", json!({"drawingId":D,"points":[{"x":2050,"y":2050}],"gapClosing":"off"}));
    assert!(!refused.is_ok(), "command off overrides prefs small");
    assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), before);
    let dry = h.invoke(Invocation {
        command: "colour.paint.apply".into(),
        params: json!({"drawingId":D,"points":[{"x":2050,"y":2050}],"gapClosing":"small"}),
        options: InvokeOptions { dry_run: true, ..Default::default() },
        origin: Origin::User,
    });
    assert!(dry.is_ok());
    assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), before);
    ok(&h, "colour.gapClosing.set", json!({"mode":"off"}));
    let rx = h.subscribe();
    let r = ok(
        &h,
        "colour.paint.apply",
        json!({"drawingId":D,"objectIds":[a["zoneIds"][0],line["strokeIds"][0],text["textId"]],"points":[{"x":1025,"y":1005},{"x":2050,"y":2050}],"gapClosing":"small","colourId":colors[2]}),
    );
    eprintln!("mixed gap targets {r}");
    assert_eq!(r["painted"].as_array().unwrap().len(), 5);
    assert_eq!(rx.try_iter().filter(|e| e.name == "draw.drawing.changed").count(), 1);
    assert!(!r["addedStrokeIds"].as_array().unwrap().is_empty());
    assert!(r["createdIds"].as_array().unwrap().iter().any(|v| v.as_str().unwrap().starts_with("zn_")));
    assert_eq!(read(&h, lid)["zones"].as_array().unwrap().iter().filter(|z| z["colourId"] == colors[2]).count(), 3);
    ok(&h, "edit.undo", json!({}));
    ok(&h, "edit.redo", json!({}));
    assert_eq!(read(&h, lid)["zones"].as_array().unwrap().iter().filter(|z| z["colourId"] == colors[2]).count(), 3);
    assert!(!b["zoneIds"][0].is_null());
}
#[test]
fn pencil_solid_keeps_matrix_click_gradient_defaults_and_invisible_rejects_matrix() {
    let h = fixture();
    let a = ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"pencil","layer":"new","fit":false,"points":[{"x":1000,"y":1000},{"x":1010,"y":1010}]}),
    );
    let id = a["strokeIds"][0].as_str().unwrap();
    let doc = h.snapshot().unwrap().unwrap();
    let gradient = doc
        .palettes()
        .flat_map(|p| p.swatches())
        .find(|s| *s.kind() == tt_model::entities::palette::SwatchKind::LinearGradient)
        .unwrap()
        .colour_id()
        .as_str()
        .to_owned();
    let solid = doc
        .palettes()
        .flat_map(|p| p.swatches())
        .find(|s| *s.kind() == tt_model::entities::palette::SwatchKind::Solid)
        .unwrap()
        .colour_id()
        .as_str()
        .to_owned();
    drop(doc);
    ok(&h, "colour.paint.apply", json!({"drawingId":D,"objectIds":[id],"colourId":gradient}));
    let raw = |h: &ExecutorHandle| {
        let doc = h.snapshot().unwrap().unwrap();
        let d = doc.entry_raw_json(&tt_model::document::EntryKey::Drawing(tt_model::Id::parse(D).unwrap())).unwrap();
        d["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|n| n["strokes"].as_array().into_iter().flatten())
            .find(|s| s["id"] == id)
            .unwrap()
            .clone()
    };
    // Fixture pencil thickness is 2, so its box expands the centerline by 1 on all sides.
    let before = raw(&h);
    assert_eq!(before["fillMatrix"], json!([12, 0, 0, 12, 999, 999]));
    ok(&h, "colour.paint.apply", json!({"drawingId":D,"objectIds":[id],"colourId":solid}));
    assert_eq!(raw(&h)["fillMatrix"], before["fillMatrix"]);
    let r = run(
        &h,
        "doc.object.patch",
        json!({"targets":[format!("{D}/{id}")],"ops":[{"op":"replace","path":"/kind","value":"invisible"},{"op":"replace","path":"/colourId","value":null},{"op":"replace","path":"/lineStyle","value":null}]}),
    );
    assert!(!r.is_ok(), "invisible cannot retain the pencil matrix");
}
#[test]
fn saving_repeated_missing_usage_writes_exactly_one_placeholder() {
    let dir = c13_support::tmp();
    let h = c13_support::spawn(&dir.join("prefs"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let a = rect(&h, "new", 1000., 1000., 10., 10.);
    let id = a["strokeIds"][0].as_str().unwrap();
    ok(
        &h,
        "doc.object.patch",
        json!({"targets":[format!("{D}/{id}")],"ops":[{"op":"replace","path":"/sides","value":{"left":{"colourId":M,"matrix":null},"right":{"colourId":M,"matrix":null}}}]}),
    );
    c13_support::job_ok(&h, "project.saveAs", json!({"path":dir.join("repeat.animpkg")}));
    let doc = h.snapshot().unwrap().unwrap();
    let d = doc.drawing(&tt_model::Id::parse(D).unwrap()).unwrap();
    let entries: Vec<_> = d.colour_recovery().iter().filter(|e| e.colour_id.as_str() == M).collect();
    assert_eq!(entries.len(), 1);
    let e = serde_json::to_value(entries[0]).unwrap();
    assert_eq!(e["paletteName"], "Recovered");
    assert_eq!(e["swatch"]["name"], "Deleted colour");
    drop(doc);
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
fn gap_preference_dry_run_writes_nothing() {
    let h = fixture();
    ok(&h, "colour.gapClosing.set", json!({"mode":"off"}));
    let r = h.invoke(Invocation {
        command: "colour.gapClosing.set".into(),
        params: json!({"mode":"large"}),
        options: InvokeOptions { dry_run: true, ..Default::default() },
        origin: Origin::User,
    });
    assert!(r.is_ok());
    assert_eq!(ok(&h, "colour.gapClosing.get", json!({}))["mode"], "off");
}

#[test]
fn backup_and_recovery_folder_open_recover_missing_colour() {
    use tt_persist::save::BackupSink;
    for recovered in [false, true] {
        let dir = c13_support::tmp();
        let user = dir.join("user");
        let mut parts = tt_model::fixture::sample_parts(1).unwrap();
        let i = parts.drawings.iter().position(|d| d.id().as_str() == D).unwrap();
        let mut d = serde_json::to_value(&parts.drawings[i]).unwrap();
        d["nodes"][0]["strokes"][1]["colourId"] = json!(M);
        parts.drawings[i] = tt_model::object::from_json(&d).unwrap();
        let doc = tt_model::document::Document::from_parts(parts).unwrap();
        let input = tt_persist::save::SaveInput {
            doc: &doc,
            preserved: &Default::default(),
            blobs: &Default::default(),
            session: None,
            saved_at: "2000-01-01T00:00:00.000Z".into(),
        };
        let h = c13_support::spawn(&user);
        if recovered {
            let info = tt_persist::recovery::RecoveryInfo {
                recovery_id: "rc_1234abcd".into(),
                project_id: doc.project().id().as_str().into(),
                project_name: "R2".into(),
                original_path: None,
                saved_at: input.saved_at.clone(),
                document_rev: doc.document_rev(),
                app_version: "0.0.1".into(),
                process_id: std::process::id(),
                host: tt_persist::host::host(),
                heartbeat_at: input.saved_at.clone(),
            };
            tt_persist::recovery::write(&user.join("Recovery"), &info, &input).unwrap();
            c13_support::job_ok(&h, "project.recovery.open", json!({"recoveryId":"rc_1234abcd"}));
        } else {
            let package = dir.join("raw.animpkg");
            tt_persist::save::save(&package, tt_persist::container::Form::Package, &input, None, false).unwrap();
            let backups = tt_persist::backup::Backups {
                folder: user.join("Backups").join(doc.project().id().as_str()),
                count: 3,
                original_path: package.to_string_lossy().into_owned(),
                project_name: "R2".into(),
            };
            let staged = backups.stage_package(&package).unwrap().unwrap();
            let id = backups.commit(staged).unwrap();
            ok(&h, "project.backup.open", json!({"backupId":id}));
        }
        assert!(
            !ok(&h, "colour.missing.list", json!({}))["missing"].as_array().unwrap().iter().any(|c| c["colourId"] == M)
        );
        let report = ok(&h, "doc.loadReport.get", json!({}));
        assert!(
            report["issues"].as_array().unwrap().iter().any(|i| i["code"] == "colour.placeholderCreated"),
            "{report}"
        );
        let doc = h.snapshot().unwrap().unwrap();
        assert!(
            doc.palettes().any(|p| p.name() == "Recovered" && p.swatches().iter().any(|s| s.colour_id().as_str() == M))
        );
        drop(doc);
        drop(h);
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn recovery_preserves_full_swatch_extensions_including_paint_protection() {
    let h = fixture();
    let ext = json!({"dev.toontown.paint":{"protected":true},"org.review.swatch":{"tag":"retain"}});
    let copy = json!([{"colourId":M,"paletteId":"pal_r2","paletteName":"Recovered","styles":[{"styleId":"sty_r2","styleName":"Default"}],
        "swatch":{"colourId":M,"type":"swatch","rev":3,"name":"Protected recovered","kind":"solid","deformable":false,"values":{"sty_r2":{"rgba":"#102030FF"}},"ext":ext}}]);
    let content = json!({"nodes":[{"type":"layer","id":"l_r2","layerKind":"pencil","strokes":[{"id":"s_r2","kind":"pencil","closed":false,"colourId":M,"lineStyle":{"thickness":2,"lineType":"solid"},"bezier":[{"x":1000,"y":1000},{"x":1000,"y":1000},{"x":1010,"y":1000},{"x":1010,"y":1000}]}]}],"colourRecovery":copy});
    ok(&h, "draw.content.add", json!({"drawingId":D,"content":content}));
    let doc = h.snapshot().unwrap().unwrap();
    let swatch = doc.palettes().flat_map(|p| p.swatches()).find(|s| s.colour_id().as_str() == M).unwrap();
    let raw = serde_json::to_value(swatch).unwrap();
    eprintln!("recovered full swatch {raw}");
    assert_eq!(raw["ext"], ext, "10 4.11 full swatch copy and SI-061 paint protection must survive recovery");
}

#[test]
fn tiled_texture_click_uses_native_pixel_dimensions() {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let palette = &parts.palettes[0];
    let colour = palette.swatches()[0].colour_id().as_str().to_owned();
    let values: serde_json::Map<String, Value> = palette
        .styles()
        .iter()
        .map(|s| {
            (s.id().as_str().to_owned(), json!({"image":"textures/r2-native.png","tile":true,"width":16,"height":8}))
        })
        .collect();
    let mut pal = serde_json::to_value(palette).unwrap();
    pal["swatches"][0]["kind"] = json!("texture");
    pal["swatches"][0]["values"] = json!(values);
    parts.palettes[0] = tt_model::object::from_json(&pal).unwrap();
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(tt_model::document::Document::from_parts(parts).unwrap(), None, false).is_ok());
    let r = rect(&h, "new", 1000., 1000., 100., 100.);
    ok(&h, "colour.paint.apply", json!({"drawingId":D,"objectIds":[r["zoneIds"][0]],"colourId":colour}));
    let layer = read(&h, r["layerId"].as_str().unwrap());
    let m = &layer["zones"][0]["textureMatrix"];
    eprintln!("16x8 tiled texture click on 100x100 box: {m}");
    assert_eq!(m[0].as_f64().unwrap().abs(), 16., "03 7.2 tile-on means native size, not box stretch");
    assert_eq!(m[3].as_f64().unwrap().abs(), 8.);
}

#[test]
fn conflicting_recovery_copies_choose_the_first_using_drawing_deterministically() {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let donor =
        serde_json::to_value(parts.drawings.iter().find(|d| d.id().as_str() == D).unwrap()).unwrap()["nodes"].clone();
    let mut indices: Vec<_> = (0..parts.drawings.len()).collect();
    indices.sort_by_key(|&i| parts.drawings[i].id().as_str().to_owned());
    let mut users = 0;
    for i in indices {
        let mut d = serde_json::to_value(&parts.drawings[i]).unwrap();
        if !d["nodes"][0]["strokes"].is_array() {
            if d["name"] == "10" {
                d["nodes"] = donor.clone();
            } else {
                continue;
            }
        }
        d["nodes"][0]["strokes"][1]["colourId"] = json!(M);
        let rgba = if users == 0 { "#102030FF" } else { "#A0B0C0FF" };
        d["colourRecovery"] = json!([{"colourId":M,"paletteId":"pal_r2","paletteName":"Recovered","styles":[{"styleId":"sty_r2","styleName":"Default"}],
            "swatch":{"colourId":M,"type":"swatch","rev":3,"name":"Copy","kind":"solid","deformable":false,"values":{"sty_r2":{"rgba":rgba}}}}]);
        parts.drawings[i] = tt_model::object::from_json(&d).unwrap();
        users += 1;
        if users == 2 {
            break;
        }
    }
    assert_eq!(users, 2);
    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let dir = c13_support::tmp();
    let package = dir.join("conflict.animpkg");
    let input = tt_persist::save::SaveInput {
        doc: &doc,
        preserved: &Default::default(),
        blobs: &Default::default(),
        session: None,
        saved_at: "2000-01-01T00:00:00.000Z".into(),
    };
    tt_persist::save::save(&package, tt_persist::container::Form::Package, &input, None, false).unwrap();
    for k in 0..2 {
        let h = c13_support::spawn(&dir.join(format!("user{k}")));
        c13_support::job_ok(&h, "project.open", json!({"path":package}));
        let doc = h.snapshot().unwrap().unwrap();
        let found: Vec<_> = doc.palettes().flat_map(|p| p.swatches()).filter(|s| s.colour_id().as_str() == M).collect();
        assert_eq!(found.len(), 1);
        let v = serde_json::to_value(found[0]).unwrap();
        assert!(v["values"].as_object().unwrap().values().all(|v| v["rgba"] == "#102030FF"), "{v}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn paste_splits_existing_stroke_reports_pieces_and_cut_copy_piece_survives_redo() {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let i = parts.drawings.iter().position(|d| d.id().as_str() == D).unwrap();
    let mut d = serde_json::to_value(&parts.drawings[i]).unwrap();
    let mut existing = d["nodes"][0]["strokes"][1].clone();
    existing["points"] = json!([[-10, 0, 0, 0, 0, 0], [10, 0, 0, 0, 0, 0]]);
    let mut pasted = existing.clone();
    pasted["id"] = json!("st_r2_clip");
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
    let r = ok(&h, "edit.paste", json!({"context":"drawing","destination":{"drawingId":D}}));
    eprintln!("paste into crossing layer {r}");
    let get = |h: &ExecutorHandle| {
        ok(h, "draw.drawing.get", json!({"drawingId":D}))["nodes"][0]["strokes"].as_array().unwrap().clone()
    };
    let strokes = get(&h);
    assert_eq!(strokes.len(), 4);
    assert_eq!(r["createdIds"].as_array().unwrap().len(), 2, "paste should name the pasted line's pieces");
    for id in r["createdIds"].as_array().unwrap() {
        let id = id.as_str().unwrap().rsplit('/').next().unwrap();
        assert!(strokes.iter().any(|s| s["id"] == id));
    }
    ok(&h, "edit.undo", json!({}));
    assert_eq!(get(&h).len(), 1);
    ok(&h, "edit.redo", json!({}));
    assert_eq!(get(&h).len(), 4);
    let piece = strokes[0]["id"].as_str().unwrap();
    ok(&h, "edit.copy", json!({"context":"drawing","objectIds":[format!("{D}/{piece}")]}));
    ok(&h, "edit.cut", json!({"context":"drawing","objectIds":[format!("{D}/{piece}")]}));
    assert_eq!(get(&h).len(), 3);
    ok(&h, "edit.undo", json!({}));
    ok(&h, "edit.redo", json!({}));
    assert_eq!(get(&h).len(), 3);
}
