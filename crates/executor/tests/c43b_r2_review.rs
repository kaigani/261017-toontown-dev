//! Independent round two parity attacks.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, dead_code, reason = "review fixtures")]
mod common;
use common::parity_gate::*;
use serde_json::json;
#[test]
fn d36_owner_matrix() {
    let cat = catalogue();
    for owner in ["M4 C4.4 ", "m4 c4.4", "M4  C4.4", "M4 C4.40", "M04 C4.4", "M12 C12.1"] {
        assert!(
            !deferred_problems(&[(
                "anim.camera.set".into(),
                json!({"owner":owner,"section":"05 \u{00a7}19.1","location":"review"})
            )])
            .is_empty(),
            "D-36 rejects {owner}"
        );
    }
    for milestone in 0..=11 {
        if let Some(owner) =
            cat.iter().find(|o| o.starts_with(&format!("M{milestone} ")) && (milestone != 4 || o.as_str() == "M4 C4.4"))
        {
            assert_eq!(owner_problem(owner, &cat).is_some(), milestone < 4, "D-36 catalogue and passed owner {owner}");
        }
    }
}
#[test]
#[ignore = "known round-two finding; run with --ignored to reproduce"]
fn d35_reads_both_ways() {
    let declared = json!({"surfaces":{"review":{"reads":{"app.state.get":["review.ts"]}}},"plumbing":{}}).to_string();
    assert!(!reads_problems(&json!({}), &declared).is_empty(), "D-35 declared read needs call site");
    let m = json!({"surfaces":["main"],"callSites":[{"command":"app.state.get","file":"review.ts"}]});
    assert!(!reads_problems(&m, r#"{"surfaces":{},"plumbing":{}}"#).is_empty(), "D-35 call site needs declaration");
    assert!(
        !reads_problems(&m, &declared).is_empty(),
        "D-35 a read must belong to a rendered surface, not nonexistent review"
    );
}
#[test]
#[ignore = "known round-two finding; run with --ignored to reproduce"]
fn d36_escaped_duplicate() {
    let raw = r#"{"deferred":{"anim.camera.set":{"owner":"M4 C4.4"},"anim.camera.\u0073et":{"owner":"M4 C4.4"}}}"#;
    assert!(deferred_entries(raw).is_err(), "D-36 duplicate decoded JSON keys must be refused");
}
#[test]
fn d36_whitespace_duplicate() {
    assert!(
        deferred_entries(r#"{"deferred":{"anim.camera.set":{},   "anim.camera.set" : {}}}"#).is_err(),
        "D-36 whitespace cannot hide duplicate"
    );
}
#[test]
fn d35_inert_mention() {
    let mut m = manifest();
    for section in SECTIONS {
        for row in m[*section].as_array_mut().unwrap() {
            if let Some(commands) = row["commands"].as_array_mut() {
                commands.retain(|c| c != "draw.group.create");
            }
        }
    }
    m["uses"].as_array_mut().unwrap().push(json!({"command":"draw.group.create","files":["inert.ts"]}));
    let d = serde_json::from_str(DEFERRED).unwrap();
    assert!(
        pa3(&m, &d).0.iter().any(|s| s.contains("draw.group.create") && s.contains("no state")),
        "D-35 mention binds nothing"
    );
}
#[test]
fn d36_wrong_file_heading_and_unimplemented() {
    assert!(!section_problems("review", "00 \u{00a7}19.1").is_empty(), "D-36 heading must exist in cited file");
    let r = tt_executor::registry().unwrap();
    let entry = r
        .entries()
        .find(|e| !matches!(e.status(), tt_registry::Status::Implemented) && e.spec.kind != tt_registry::Kind::Event)
        .unwrap();
    assert!(
        deferred_problems(&[(
            entry.id().into(),
            json!({"owner":"M4 C4.4","section":"05 \u{00a7}19.1","location":"review"})
        )])
        .iter()
        .any(|p| p.contains("not implemented")),
        "D-36 rejects registered unimplemented command"
    );
}

#[path = "c13_support/mod.rs"]
mod c13_support;
mod selection_attacks {
    use std::collections::{BTreeMap, BTreeSet};

    use serde_json::{Value, json};
    use tt_executor::ExecutorHandle;
    use tt_executor::canvas::{Part, PayloadOptions};
    use tt_render::canvas::{CanvasDrawing, Record, decode};

    const BOB: &str = "scene:#1/element:\"Bob\"";
    use crate::common::*;

    /// Bob's empty cell (exposure: frames 7 and 8 hold no drawing): the table's drawing is created there.
    const FRAME: u32 = 7;
    /// Half the side of an object's cell, drawing units.
    const CELL: f64 = 130.0;

    /// The fixture project saved with a real image for the locked drawing's bitmap node, opened from its package, so the
    /// bitmap is drawn (an in-memory fixture has no image bytes: `render.bitmapMissing`). Returns the executor and the
    /// drawing holding the bitmap.
    fn open_with_image() -> (ExecutorHandle, String) {
        use tt_model::change::PatchOp;
        use tt_model::document::EntryKey;
        use tt_persist::container::{BlobData, Blobs};
        let doc = tt_model::fixture::sample_document(1).unwrap();
        let bob = doc.elements().find(|e| e.name() == "Bob").unwrap();
        // Exposure [[d1, 6], [null, 2], [d2, 12], [d1, 24]]: the third entry is the drawing with the bitmap node.
        let d2 = bob.exposure().as_ref().unwrap()[2].0.clone().unwrap();
        let node = doc.drawing(&d2).unwrap().nodes()[0].id().as_str().to_owned();
        let (w, h) = (32u32, 16u32);
        let px: Vec<u8> = (0..w * h).flat_map(|i| [u8::try_from(i % 251).unwrap(), 90, 200, 255]).collect();
        let png = tt_render::encode_png(&px, w, h).unwrap();
        let entry = format!("banks/{}/{}/{node}.png", doc.bank_of(&d2).unwrap().as_str(), d2.as_str());
        let sha = {
            use sha2::Digest;
            sha2::Sha256::digest(&png).iter().map(|b| format!("{b:02x}")).collect::<String>()
        };
        let ops = vec![PatchOp {
            op: "replace",
            from: None,
            path: "/nodes/0/image".into(),
            range: None,
            value: Some(json!({ "entry": entry, "mediaType": "image/png", "size": png.len(), "sha256": sha })),
            old_value: None,
            elided: false,
        }];
        let (doc, _) = doc.with_patched_entries(&[(EntryKey::Drawing(d2.clone()), ops)]).unwrap();
        let mut blobs = Blobs::default();
        blobs.insert(entry, BlobData::Bytes(std::sync::Arc::new(png)));
        blobs.insert(
            "media/as_01K742SG0G5HPQ0RBF5X4DSR0W.wav".into(),
            BlobData::Bytes(std::sync::Arc::new(vec![0; 352_844])),
        );
        let dir = crate::c13_support::tmp();
        let preserved = tt_persist::load::Preserved::default();
        let input = tt_persist::save::SaveInput {
            doc: &doc,
            preserved: &preserved,
            blobs: &blobs,
            session: None,
            saved_at: "2026-10-01T09:00:00Z".into(),
        };
        let path = dir.join("Parts.animpkg");
        tt_persist::save::save_package(&path, &input, None).unwrap();
        let h = crate::c13_support::spawn(&dir.join("user"));
        crate::c13_support::job_ok(&h, "project.open", json!({ "path": path.to_string_lossy() }));
        (h, d2.as_str().to_owned())
    }

    fn colour(h: &ExecutorHandle, name: &str) -> String {
        let doc = h.snapshot().unwrap().unwrap();
        doc.palettes()
            .flat_map(|p| p.swatches().iter())
            .find(|s| serde_json::to_value(s).unwrap()["name"] == name)
            .map(|s| s.colour_id().as_str().to_owned())
            .unwrap_or_else(|| panic!("no swatch `{name}`"))
    }

    /// How a payload vertex is recognised as an object's.
    #[derive(Debug, Clone)]
    enum Mark {
        /// In the cell around this point, in this colour (any colour when `None`).
        Cell(f64, f64, Option<String>),
        /// The bitmap record of this image.
        Image,
    }

    struct Table {
        h: ExecutorHandle,
        drawing: String,
        ink: String,
        skin: String,
        /// Every object of the drawing: id ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ how its geometry is recognised.
        objects: BTreeMap<String, Mark>,
        /// A zone's ancestors are those of the pencil stroke that closes it.
        zone_of: BTreeMap<String, String>,
    }

    impl Table {
        fn pencil_square(&mut self, x: f64, y: f64, layer: &str) -> (String, String) {
            let r = ok(
                &self.h,
                "draw.stroke.add",
                json!({ "drawingId": self.drawing, "kind": "pencil", "layer": layer, "fit": false, "closed": true,
                    "thickness": 2, "colourId": self.ink,
                    "points": [{"x": x - 60.0, "y": y - 60.0}, {"x": x + 60.0, "y": y - 60.0},
                               {"x": x + 60.0, "y": y + 60.0}, {"x": x - 60.0, "y": y + 60.0}] }),
            );
            let id = r["strokeIds"][0].as_str().unwrap().to_owned();
            self.objects.insert(id.clone(), Mark::Cell(x, y, Some(self.ink.clone())));
            (id, r["layerId"].as_str().unwrap().to_owned())
        }

        fn paint(&mut self, x: f64, y: f64, pencil: &str) -> String {
            let r = ok(
                &self.h,
                "colour.paint.apply",
                json!({ "drawingId": self.drawing, "points": [{"x": x, "y": y}], "colourId": self.skin }),
            );
            let zone = r["painted"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["objectKind"] == "zone")
                .unwrap_or_else(|| panic!("no zone painted at ({x}, {y}): {r}"))["objectId"]
                .as_str()
                .unwrap()
                .to_owned();
            self.objects.insert(zone.clone(), Mark::Cell(x, y, Some(self.skin.clone())));
            self.zone_of.insert(zone.clone(), pencil.to_owned());
            zone
        }

        fn brush(&mut self, x: f64, y: f64, layer: &str) -> String {
            let r = ok(
                &self.h,
                "draw.stroke.add",
                json!({ "drawingId": self.drawing, "kind": "brush", "layer": layer, "fit": false, "colourId": self.ink,
                    "points": [{"x": x - 50.0, "y": y}, {"x": x, "y": y + 10.0}, {"x": x + 50.0, "y": y}] }),
            );
            let id = r["strokeIds"][0].as_str().unwrap().to_owned();
            self.objects.insert(id.clone(), Mark::Cell(x, y, None));
            id
        }

        fn text(&mut self, x: f64, y: f64) -> String {
            let r = ok(
                &self.h,
                "draw.text.add",
                json!({ "drawingId": self.drawing, "position": {"x": x - 20.0, "y": y}, "text": "Tx", "colourId": self.ink }),
            );
            let id = r["textId"].as_str().unwrap().to_owned();
            self.objects.insert(id.clone(), Mark::Cell(x, y, None));
            id
        }

        fn group(&mut self, ids: &[&str]) -> String {
            let r = ok(&self.h, "draw.group.create", json!({ "drawingId": self.drawing, "ids": ids }));
            r["groupId"].as_str().unwrap().to_owned()
        }

        /// Each object's ancestors (the layers and groups holding it), from the drawing as it is now.
        fn ancestors(&self) -> BTreeMap<String, Vec<String>> {
            fn walk(nodes: &Value, above: &[String], out: &mut BTreeMap<String, Vec<String>>) {
                for n in nodes.as_array().into_iter().flatten() {
                    let id = n["id"].as_str().unwrap().to_owned();
                    out.insert(id.clone(), above.to_vec());
                    let mut inner = above.to_vec();
                    inner.push(id);
                    for s in n["strokes"].as_array().into_iter().flatten() {
                        out.insert(s["id"].as_str().unwrap().to_owned(), inner.clone());
                    }
                    walk(&n["children"], &inner, out);
                }
            }
            let d = ok(&self.h, "draw.drawing.get", json!({ "drawingId": self.drawing }));
            let mut out = BTreeMap::new();
            walk(&d["nodes"], &[], &mut out);
            for (zone, pencil) in &self.zone_of {
                // The zone belongs to its pencil's layer: the pencil's ancestors are the zone's.
                let a = out.get(pencil).cloned().unwrap_or_else(|| panic!("pencil `{pencil}` not in the drawing"));
                out.insert(zone.clone(), a);
            }
            out
        }

        /// The payload's points, each with the object it belongs to.
        fn points(&self, part: Part) -> Vec<(f32, f32, String)> {
            let d: CanvasDrawing = decode(
                &self.h.canvas_drawing(None, 1, 0, PayloadOptions { part, ..Default::default() }).unwrap().bytes,
            )
            .unwrap();
            let mut out = Vec::new();
            for r in &d.records {
                match r {
                    Record::Solid { vertices, indices } | Record::Gradient { vertices, indices, .. } => {
                        for i in indices {
                            let v = vertices[*i as usize];
                            let slot = d.slots[usize::from(v.slot)].id.clone();
                            out.push((v.x, v.y, self.owner(f64::from(v.x), f64::from(v.y), &slot)));
                        }
                    }
                    Record::Bitmap { quad, .. } => {
                        let id = self.objects.iter().find(|(_, m)| matches!(m, Mark::Image)).unwrap().0.clone();
                        for p in quad.as_chunks::<2>().0 {
                            out.push((p[0], p[1], id.clone()));
                        }
                    }
                }
            }
            out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
            out
        }

        fn owner(&self, x: f64, y: f64, slot: &str) -> String {
            let found: Vec<&String> = self
                .objects
                .iter()
                .filter(|(_, m)| match m {
                    Mark::Cell(cx, cy, c) => {
                        (x - cx).abs() <= CELL && (y - cy).abs() <= CELL && c.as_ref().is_none_or(|c| c == slot)
                    }
                    Mark::Image => false,
                })
                .map(|(id, _)| id)
                .collect();
            assert_eq!(found.len(), 1, "the vertex ({x}, {y}) in colour `{slot}` belongs to one object: {found:?}");
            found[0].clone()
        }

        /// One cell of the table: select `ids`, then check the two parts against the whole.
        fn cell(&self, name: &str, ids: &[&str]) {
            ok(&self.h, "draw.selection.set", json!({ "drawingId": self.drawing, "by": "ids", "ids": ids }));
            let ancestors = self.ancestors();
            let chosen: BTreeSet<&str> = ids.iter().copied().collect();
            let expected: BTreeSet<String> = self
                .objects
                .keys()
                .filter(|o| {
                    chosen.contains(o.as_str())
                        || ancestors.get(*o).is_some_and(|a| a.iter().any(|x| chosen.contains(x.as_str())))
                })
                .cloned()
                .collect();
            assert!(!expected.is_empty(), "{name}: the cell selects some object");
            let all = self.points(Part::All);
            let selection = self.points(Part::Selection);
            let rest = self.points(Part::Rest);
            // Every object of the drawing is drawn (the fixture is what the table says).
            let drawn: BTreeSet<String> = all.iter().map(|p| p.2.clone()).collect();
            let every: BTreeSet<String> = self.objects.keys().cloned().collect();
            assert_eq!(drawn, every, "{name}: every object has geometry in the whole drawing");
            // selection + rest = all.
            let mut split = selection.clone();
            split.extend(rest.iter().cloned());
            split.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
            assert_eq!(split.len(), all.len(), "{name}: selection + rest has as many points as the whole");
            for (a, b) in all.iter().zip(&split) {
                assert!(
                    (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4 && a.2 == b.2,
                    "{name}: selection + rest differs from the whole at {a:?} / {b:?}"
                );
            }
            // The selected objects' geometry is all in the selection, and only theirs.
            let in_selection: BTreeSet<String> = selection.iter().map(|p| p.2.clone()).collect();
            assert_eq!(in_selection, expected, "{name}: the selection part holds exactly the selected objects");
            let leaked: BTreeSet<&String> = rest.iter().map(|p| &p.2).filter(|o| expected.contains(*o)).collect();
            assert!(leaked.is_empty(), "{name}: selected objects drawn in the rest: {leaked:?}");
        }
    }

    #[test]
    fn r2_selection_parts_extended() {
        let (h, locked) = open_with_image();
        let (ink, skin) = (colour(&h, "Ink"), colour(&h, "Skin"));
        // The bitmap first: pasted into Bob's empty cell, which creates the table's drawing.
        let source = ok(&h, "draw.drawing.get", json!({ "drawingId": locked }));
        let r = ok(&h, "draw.content.add", json!({ "elementId": BOB, "frame": FRAME, "content": source }));
        assert_eq!(r["createdDrawing"], true, "{r}");
        let drawing = r["drawingId"].as_str().unwrap().to_owned();
        let bitmap = r["idMap"].as_object().unwrap().values().next().unwrap().as_str().unwrap().to_owned();
        ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": FRAME }));
        let mut t = Table { h, drawing, ink, skin, objects: BTreeMap::new(), zone_of: BTreeMap::new() };
        t.objects.insert(bitmap.clone(), Mark::Image);

        // Stage 1: every kind alone, on two top-level layers; a text and the bitmap at the top level.
        let (pa, la) = t.pencil_square(-900.0, 600.0, "new");
        let za = t.paint(-900.0, 600.0, &pa);
        let ba = t.brush(-600.0, 600.0, &la);
        let (pb, lb) = t.pencil_square(-300.0, 600.0, "new");
        let zb = t.paint(-300.0, 600.0, &pb);
        let bb = t.brush(300.0, 600.0, &lb);
        let ta = t.text(600.0, 600.0);
        assert_ne!(la, lb, "two layers");
        for (name, ids) in [
            ("pencil alone, first layer", vec![pa.as_str()]),
            ("zone alone, first layer", vec![za.as_str()]),
            ("brush alone, first layer", vec![ba.as_str()]),
            ("pencil alone, second layer", vec![pb.as_str()]),
            ("zone alone, second layer", vec![zb.as_str()]),
            ("brush alone, second layer", vec![bb.as_str()]),
            ("text alone", vec![ta.as_str()]),
            ("bitmap alone", vec![bitmap.as_str()]),
            ("first layer whole", vec![la.as_str()]),
            ("second layer whole", vec![lb.as_str()]),
            ("one object on each layer", vec![pa.as_str(), bb.as_str()]),
        ] {
            t.cell(name, &ids);
        }

        // Stage 2: a group of a pencil and a brush stroke (grouped into a layer of their own, 02 ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§8.4), a text and the
        // bitmap; its zone painted inside the group.
        let (pg, staging) = t.pencil_square(-900.0, 0.0, "new");
        let bg = t.brush(-600.0, 0.0, &staging);
        let tg = t.text(-300.0, 0.0);
        let g1 = t.group(&[&pg, &bg, &tg, &bitmap]);
        let zg = t.paint(-900.0, 0.0, &pg);
        let lg = t.ancestors()[&pg].last().unwrap().clone();
        assert_ne!(lg, g1, "the grouped strokes are in a layer inside the group");
        for (name, ids) in [
            ("group whole", vec![g1.as_str()]),
            ("group partly: its pencil", vec![pg.as_str()]),
            ("group partly: its zone", vec![zg.as_str()]),
            ("group partly: its brush", vec![bg.as_str()]),
            ("group partly: its text", vec![tg.as_str()]),
            ("group partly: its bitmap", vec![bitmap.as_str()]),
            ("group partly: its layer", vec![lg.as_str()]),
            ("group whole and a top-level object", vec![g1.as_str(), ta.as_str()]),
        ] {
            t.cell(name, &ids);
        }

        // Stage 3: that group nested in another, beside a text.
        let tn = t.text(300.0, 0.0);
        let g2 = t.group(&[&g1, &tn]);
        for (name, ids) in [
            ("outer group whole: the nested group's members too", vec![g2.as_str()]),
            ("nested group whole, outer partly", vec![g1.as_str()]),
            ("outer group partly: its own text", vec![tn.as_str()]),
            ("nested member: pencil", vec![pg.as_str()]),
            ("nested member: zone", vec![zg.as_str()]),
            ("nested member: brush", vec![bg.as_str()]),
            ("nested member: text", vec![tg.as_str()]),
            ("nested member: bitmap", vec![bitmap.as_str()]),
            ("nested layer whole", vec![lg.as_str()]),
            ("a nested member and a first-layer zone", vec![zg.as_str(), za.as_str()]),
        ] {
            t.cell(name, &ids);
        }
        let t3 = t.text(900.0, 0.0);
        let g3 = t.group(&[&g2, &t3]);
        t.cell("PV3 three deep inner member", &[&tg]);
        t.cell("PV3 group and its own member", &[&g3, &tg]);
        t.cell("PV3 text and bitmap selected group", &[&g1]);
        ok(&t.h, "edit.undo", json!({}));
        t.cell("PV3 after undo grouping", &[&g2]);
        ok(&t.h, "edit.redo", json!({}));
        t.cell("PV3 after redo grouping", &[&g3]);
    }
}

#[test]
#[ignore = "known round-two finding; run with --ignored to reproduce"]
fn d35_renamed_real_surface() {
    let m = manifest();
    let mut r: serde_json::Value = serde_json::from_str(READS).unwrap();
    let surfaces = r["surfaces"].as_object_mut().unwrap();
    let key = surfaces.keys().next().unwrap().clone();
    let value = surfaces.remove(&key).unwrap();
    surfaces.insert("review.nonexistent.surface".into(), value);
    assert!(!reads_problems(&m, &r.to_string()).is_empty(), "D-35 renamed nonexistent surface must fail too");
}
#[test]
#[ignore = "known round-two finding; run with --ignored to reproduce"]
fn d36_escaped_root_cannot_hide_entries() {
    let raw = r#"{"defer\u0072ed":{"anim.camera.set":{},"anim.camera.set":{}}}"#;
    assert!(deferred_entries(raw).is_err(), "D-36 escaped root must still reject duplicates");
}
