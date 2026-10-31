//! Round-2 independent contract variants.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
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
    let r = ok(&h, "draw.text.add", json!({"drawingId":D,"position":{"x":0,"y":0},"text":"\u{09c7}Z"}));
    let id = r["textId"].as_str().unwrap();
    let key = format!("{D}/{id}");
    ok(&h, "draw.textFormat.set", json!({"drawingId":D,"textId":id,"start":1,"length":1,"bold":true}));
    ok(&h, "draw.text.set", json!({"drawingId":D,"textId":id,"insert":{"at":1,"text":"\u{09be}"}}));
    let object = get(&h, &key);
    eprintln!("NFC command readback: {object}");
    assert_eq!(object["text"], "\u{09cb}Z");
    assert!(
        object["runs"].as_array().unwrap().iter().any(|r| r["bold"] == true
            && r["start"].as_u64().unwrap() <= 1
            && r["start"].as_u64().unwrap() + r["length"].as_u64().unwrap() > 1),
        "02 3.7: unaffected Z loses bold: {object}"
    );
}
