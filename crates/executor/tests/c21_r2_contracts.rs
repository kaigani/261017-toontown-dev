//! Round-2 independent contract variants.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
mod c13_support;
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::{InvokeOptions, Status};
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const S: &str = "st_01K742SG0R61SYK9GG0HGYVNBT";
const T: &str = "nd_01K742SG0WFAQJFKAZKV4CD56C";
const BOB: &str = "scene:#1/element:\"Bob\"";

fn result_contract(r: &Value) {
    assert_eq!(r["drawingId"], D, "{r}");
    assert!(r["rev"].is_u64(), "{r}");
    for k in ["createdIds", "modifiedIds", "removedIds"] {
        assert!(r[k].is_array(), "{k}: {r}");
    }
}

#[test]
fn setter_addressing_revision_and_result_matrix() {
    for mode in 0..4 {
        for (cmd, key, id, edit) in [
            ("draw.stroke.set", "strokeId", S, json!({"thickness":3})),
            ("draw.text.set", "textId", T, json!({"text":"round two"})),
            ("draw.textFormat.set", "textId", T, json!({"bold":true})),
        ] {
            let h = fixture();
            ok(&h, "app.context.set", json!({"elementId":BOB,"frame":1}));
            let mut p = edit;
            p[key] = json!(if mode == 2 { format!("{D}/{id}") } else { id.into() });
            match mode {
                0 => p["drawingId"] = json!(D),
                1 => {
                    p["elementId"] = json!(BOB);
                    p["frame"] = json!(1);
                }
                _ => {}
            }
            p["expectRev"] = json!(obj_rev(&h, D) + 100);
            assert_eq!(err(&h, cmd, p.clone()).code, "drawing.staleRevision");
            p["expectRev"] = json!(obj_rev(&h, D));
            result_contract(&ok(&h, cmd, p.clone()));
            p[key] = json!(format!("dr_01K742SG0SQC4H28B4EHWWD0C1/{id}"));
            p["drawingId"] = json!(D);
            assert_eq!(err(&h, cmd, p).code, "arg.invalid");
        }
    }
}

#[test]
fn paragraph_patch_variants_are_atomic() {
    for text in ["one\ntwo", "one\n", ""] {
        let h = fixture();
        let r = ok(&h, "draw.text.add", json!({"drawingId":D,"position":{"x":0,"y":0},"text":text}));
        let key = format!("{D}/{}", r["textId"].as_str().unwrap());
        let before = get(&h, &key);
        let original = before["paragraphs"].as_array().unwrap();
        let mut variants = Vec::new();
        if !text.is_empty() {
            variants.push(json!([]));
        }
        let mut shifted = original.clone();
        shifted[0]["start"] = json!(1);
        variants.push(json!(shifted));
        let mut too_long = original.clone();
        too_long[0]["length"] = json!(999);
        variants.push(json!(too_long));
        if original.len() > 1 {
            let mut reversed = original.clone();
            reversed.reverse();
            variants.push(json!(reversed));
            let mut overlap = original.clone();
            overlap[1]["start"] = json!(0);
            variants.push(json!(overlap));
        }
        for paragraphs in variants {
            let revision = rev(&h);
            assert_eq!(
                run(
                    &h,
                    "doc.object.patch",
                    json!({"targets":[key],"ops":[{"op":"replace","path":"/paragraphs","value":paragraphs}]})
                )
                .status,
                Status::Error
            );
            assert_eq!(rev(&h), revision);
            assert_eq!(get(&h, &key), before);
        }
    }
}

#[test]
fn mixed_patch_variants_and_failed_batch_have_no_effect() {
    let h = fixture();
    let r = ok(&h, "draw.stroke.add", json!({"drawingId":D,"points":[{"x":0,"y":0},{"x":20,"y":0}]}));
    let key = format!("{D}/{}", r["strokeIds"][0].as_str().unwrap());
    for ops in [
        json!([{"op":"replace","path":"/points/0/0","value":123},{"op":"replace","path":"/spine/pressure/0/1","value":0.1}]),
        json!([{"op":"replace","path":"/points/0/0","value":123},{"op":"replace","path":"/spine/points/0/0","value":2}]),
    ] {
        let before = get(&h, &key);
        let revision = rev(&h);
        let e = err(&h, "doc.object.patch", json!({"targets":[key],"ops":ops}));
        assert_eq!(e.details["pointers"], json!(["/points", "/spine"]));
        assert_eq!(get(&h, &key), before);
        assert_eq!(rev(&h), revision);
        let e = batch(
            &h,
            json!({"label":"round two","steps":[{"id":"draw.pivot.set","params":{"drawingId":D,"point":{"x":9,"y":2}}},{"id":"doc.object.patch","params":{"targets":[key],"ops":ops}}]}),
        );
        assert_eq!(e.status, Status::Error);
        assert_eq!(rev(&h), revision);
    }
    let before = get(&h, &key);
    ok(
        &h,
        "doc.object.patch",
        json!({"targets":[key],"ops":[{"op":"replace","path":"/spine/pressure/0/1","value":0.1}]}),
    );
    assert_ne!(get(&h, &key)["points"], before["points"]);
    let revision = rev(&h);
    assert_eq!(err(&h,"doc.object.patch",json!({"targets":[key],"ops":[{"op":"replace","path":"/points/0/0","value":123},{"op":"remove","path":"/spine"}]})).code,"doc.schemaViolation");
    assert_eq!(rev(&h), revision);
    ok(
        &h,
        "doc.object.patch",
        json!({"targets":[key],"ops":[{"op":"replace","path":"/points/0/0","value":123},{"op":"replace","path":"/spine","value":null}]}),
    );
    assert!(get(&h, &key)["spine"].is_null());
}

#[test]
fn events_exclude_dry_run_and_failures_and_sequential_batch_has_two() {
    let h = fixture();
    let rx = h.subscribe();
    let params = json!({"drawingId":D,"point":{"x":15,"y":16}});
    let e = with(&h, "draw.pivot.set", params.clone(), InvokeOptions { dry_run: true, ..Default::default() });
    assert!(e.is_ok(), "{e:?}");
    assert!(!rx.try_iter().any(|e| e.name == "draw.drawing.changed"));
    let e = batch(
        &h,
        json!({"label":"round two","steps":[{"id":"draw.pivot.set","params":params},{"id":"draw.stroke.set","params":{"drawingId":D,"strokeId":"absent","thickness":4}}]}),
    );
    assert_eq!(e.status, Status::Error);
    assert!(!rx.try_iter().any(|e| e.name == "draw.drawing.changed"));
    let e = batch(
        &h,
        json!({"label":"round two sequential","atomic":false,"steps":[{"id":"draw.pivot.set","params":params},{"id":"draw.pivot.set","params":{"drawingId":D,"point":{"x":18,"y":19}}}]}),
    );
    assert!(e.is_ok(), "{e:?}");
    let events: Vec<_> = rx.try_iter().filter(|e| e.name == "draw.drawing.changed").collect();
    assert_eq!(events.len(), 2, "{events:?}");
    for e in events {
        assert_eq!(e.payload["drawingId"], D);
        assert!(e.payload["rev"].is_u64());
    }
}

#[test]
fn unrelated_format_survives_nfc_in_command_path() {
    let h = fixture();
    let r = ok(&h, "draw.text.add", json!({"drawingId":D,"position":{"x":0,"y":0},"text":"\u{0995}\u{09c7}Z"}));
    let id = r["textId"].as_str().unwrap();
    let key = format!("{D}/{id}");
    ok(&h, "draw.textFormat.set", json!({"drawingId":D,"textId":id,"start":2,"length":1,"bold":true}));
    ok(&h, "draw.text.set", json!({"drawingId":D,"textId":id,"insert":{"at":2,"text":"\u{09be}"}}));
    let object = get(&h, &key);
    eprintln!("NFC command readback: {object}");
    assert_eq!(object["text"], "\u{0995}\u{09cb}Z");
    assert!(
        object["runs"].as_array().unwrap().iter().any(|r| r["bold"] == true
            && r["start"].as_u64().unwrap() <= 2
            && r["start"].as_u64().unwrap() + r["length"].as_u64().unwrap() > 2),
        "02 3.7: unaffected Z loses bold: {object}"
    );
}

#[test]
fn unicode_ranges_round_trip_through_content_and_package() {
    let dir = c13_support::tmp();
    let h = c13_support::spawn(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut saved = Vec::new();
    for (input, expected) in [
        ("e\u{301}", "\u{e9}"),
        ("\u{1100}\u{1161}\u{11a8}", "\u{ac01}"),
        (
            "\u{1f469}\u{200d}\u{1f3a8}\u{fe0f}\n\u{5d0}\u{5d1}\u{5d2}\n",
            "\u{1f469}\u{200d}\u{1f3a8}\u{fe0f}\n\u{5d0}\u{5d1}\u{5d2}\n",
        ),
        ("\n\u{301}x", "\n\u{301}x"),
        ("stable text", "stable text"),
    ] {
        let r = ok(&h, "draw.text.add", json!({"drawingId":D,"position":{"x":0,"y":0},"text":input}));
        let id = r["textId"].as_str().unwrap();
        let key = format!("{D}/{id}");
        let object = get(&h, &key);
        assert_eq!(object["text"], expected);
        let n = expected.chars().count() as u64;
        assert_eq!(object["runs"].as_array().unwrap().iter().map(|r| r["length"].as_u64().unwrap()).sum::<u64>(), n);
        assert_eq!(object["paragraphs"].as_array().unwrap().len(), expected.chars().filter(|c| *c == '\n').count() + 1);
        let read = ok(&h, "draw.drawing.get", json!({"drawingId":D}));
        let node = read["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).unwrap();
        let inserted = ok(&h, "draw.content.add", json!({"drawingId":D,"content":{"nodes":[node]}}));
        let copy = inserted["idMap"][id].as_str().unwrap();
        assert_eq!(get(&h, &format!("{D}/{copy}"))["text"], expected);
        saved.push((key, object));
    }
    let path = dir.join("Unicode.animpkg");
    c13_support::job_ok(&h, "project.saveAs", json!({"path":path}));
    ok(&h, "project.close", json!({}));
    c13_support::job_ok(&h, "project.open", json!({"path":path}));
    for (key, before) in saved {
        let after = get(&h, &key);
        for field in ["text", "runs", "paragraphs"] {
            assert_eq!(after[field], before[field]);
        }
    }
    ok(&h, "project.close", json!({}));
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn malformed_paragraphs_in_package_fail_the_load_gate() {
    use tt_persist::zip::{Method, ZipArchive, ZipWriter};
    let dir = c13_support::tmp();
    let h = c13_support::spawn(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let path = dir.join("Paragraphs.animpkg");
    c13_support::job_ok(&h, "project.saveAs", json!({"path":path}));
    ok(&h, "project.close", json!({}));
    let zip = ZipArchive::open(&path, Some(tt_persist::manifest::PACKAGE_MIMETYPE), Default::default()).unwrap();
    let mut entries = std::collections::BTreeMap::new();
    for e in zip.entries() {
        entries.insert(e.name.clone(), zip.read(&e.name, 64 * 1024 * 1024).unwrap());
    }
    drop(zip);
    let drawing = entries.keys().find(|p| p.ends_with(&format!("/{D}.json"))).unwrap().clone();
    let mut d: Value = serde_json::from_slice(&entries[&drawing]).unwrap();
    let node = d["nodes"].as_array_mut().unwrap().iter_mut().find(|n| n["id"] == T).unwrap();
    node["paragraphs"] = json!([]);
    let bytes = serde_json::to_vec(&d).unwrap();
    let mut manifest: Value = serde_json::from_slice(&entries["manifest.json"]).unwrap();
    let item = manifest["entries"].as_array_mut().unwrap().iter_mut().find(|e| e["path"] == drawing).unwrap();
    item["size"] = json!(bytes.len());
    item["sha256"] = json!(tt_persist::fsx::sha256(&bytes));
    entries.insert(drawing, bytes);
    entries.insert("manifest.json".into(), serde_json::to_vec(&manifest).unwrap());
    let mut out = ZipWriter::new(std::fs::File::create(&path).unwrap());
    out.add_bytes("mimetype", &entries.remove("mimetype").unwrap(), Method::Stored).unwrap();
    for (name, bytes) in entries {
        out.add_bytes(&name, &bytes, Method::Deflate).unwrap();
    }
    out.finish().unwrap();
    let mut repaired = Vec::new();
    for _ in 0..2 {
        let result = c13_support::job_ok(&h, "project.open", json!({"path":path}));
        assert!(result.to_string().contains("paragraphsCover"), "{result}");
        let object = get(&h, D);
        assert_eq!(object["nodes"], json!([]));
        assert!(
            object["ext"]["app.unreadable"].get(format!("banks/db_01K742SG0JW3TVZ32SQC03AMY1/{D}.json")).is_some(),
            "{object}"
        );
        repaired.push(object);
        ok(&h, "project.close", json!({"onUnsaved":"discard"}));
    }
    assert_eq!(repaired[0], repaired[1], "LOAD-03 deterministic repair with raw content retained");
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn remaining_draw_commands_common_addressing_and_results() {
    for mode in 0..3 {
        let h = fixture();
        ok(&h, "app.context.set", json!({"elementId":BOB,"frame":1}));
        let address = |mut p: Value| {
            match mode {
                0 => p["drawingId"] = json!(D),
                1 => {
                    p["elementId"] = json!(BOB);
                    p["frame"] = json!(1);
                }
                _ => {}
            }
            p
        };
        let read = ok(&h, "draw.drawing.get", address(json!({})));
        assert_eq!(read["drawingId"], D);
        for (cmd, params) in [
            ("draw.stroke.add", json!({"kind":"pencil","points":[{"x":1,"y":2},{"x":4,"y":5}]})),
            ("draw.text.add", json!({"position":{"x":1,"y":2},"text":"matrix"})),
            ("draw.content.add", json!({"content":{"nodes":[read["nodes"][2]]}})),
        ] {
            result_contract(&ok(&h, cmd, address(params)));
        }
        for (cmd, params) in
            [("draw.pivot.set", json!({"point":{"x":3,"y":4}})), ("draw.group.create", json!({"ids":[S]}))]
        {
            let mut p = address(params);
            p["expectRev"] = json!(obj_rev(&h, D) + 100);
            assert_eq!(err(&h, cmd, p.clone()).code, "drawing.staleRevision");
            p["expectRev"] = json!(obj_rev(&h, D));
            let r = ok(&h, cmd, p);
            result_contract(&r);
            if cmd == "draw.group.create" {
                let mut ungroup = address(json!({"groupIds":[r["groupId"]],"expectRev":obj_rev(&h,D)+100}));
                assert_eq!(err(&h, "draw.group.delete", ungroup.clone()).code, "drawing.staleRevision");
                ungroup["expectRev"] = json!(obj_rev(&h, D));
                result_contract(&ok(&h, "draw.group.delete", ungroup));
            }
        }
        let mut del = address(json!({"ids":[S],"expectRev":obj_rev(&h,D)+100}));
        assert_eq!(err(&h, "draw.object.delete", del.clone()).code, "drawing.staleRevision");
        del["expectRev"] = json!(obj_rev(&h, D));
        result_contract(&ok(&h, "draw.object.delete", del));
    }
}
