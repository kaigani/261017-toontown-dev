//! Final-round command-path pressure, refusal and Unicode variants.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
mod c13_support;
mod common;
use common::*;
use serde_json::json;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

#[test]
fn deleting_a_separator_keeps_formats_on_reordered_hebrew_points() {
    let h = fixture();
    let result =
        ok(&h, "draw.text.add", json!({"drawingId":D,"position":{"x":0,"y":0},"text":"\u{5e9}\u{5c1}Q\u{5b8}Z"}));
    let id = result["textId"].as_str().unwrap();
    ok(&h, "draw.textFormat.set", json!({"drawingId":D,"textId":id,"start":0,"length":5,"size":10}));
    ok(&h, "draw.textFormat.set", json!({"drawingId":D,"textId":id,"start":1,"length":1,"size":20}));
    ok(&h, "draw.textFormat.set", json!({"drawingId":D,"textId":id,"start":3,"length":1,"size":30}));
    ok(&h, "draw.text.set", json!({"drawingId":D,"textId":id,"delete":{"start":2,"length":1}}));
    let object = get(&h, &format!("{D}/{id}"));
    assert_eq!(object["text"], "\u{5e9}\u{5b8}\u{5c1}Z");
    let sizes: Vec<f64> = object["runs"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|r| std::iter::repeat_n(r["size"].as_f64().unwrap(), r["length"].as_u64().unwrap() as usize))
        .collect();
    assert_eq!(sizes, vec![10.0, 30.0, 20.0, 10.0], "02 3.7: normalization must not reformat the moved SHIN DOT");
}

#[test]
fn complete_command_accepted_pressure_worst_case_is_measured() {
    let h = fixture();
    let pressure: Vec<_> = (0..99_000).map(|i| json!({"t":f64::from(i)/98_999.0,"p":i%2})).collect();
    let started = std::time::Instant::now();
    ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"brush","pressure":pressure,
        "pen":{"tipShape":"round","minSize":0,"maxSize":100},
        "points":[{"x":0,"y":0},{"x":100,"y":0}]}),
    );
    eprintln!("complete accepted draw.stroke.add {:?}", started.elapsed());
    assert!(started.elapsed() < std::time::Duration::from_secs(30));
}

#[test]
fn stationary_pressure_dab_is_not_refused_as_zero_area() {
    let h = fixture();
    let e = run(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"brush","smoothness":0,
        "pen":{"tipShape":"round","minSize":0,"maxSize":100},
        "points":[{"x":0,"y":0,"pressure":0},{"x":0,"y":0,"pressure":1},{"x":0,"y":0,"pressure":0}]}),
    );
    assert!(e.is_ok(), "stationary maximum must produce a radius-50 dab: {e:?}");
}

#[test]
fn over_limit_brush_refuses_atomically_and_worker_accepts_next_edit() {
    let h = fixture();
    let before = get(&h, D);
    let revision = rev(&h);
    let pressure: Vec<_> = (0..90_000).map(|i| json!({"t":f64::from(i)/89_999.0,"p":i % 2})).collect();
    let started = std::time::Instant::now();
    let error = err(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"kind":"brush",
        "pen":{"tipShape":"round","minSize":0,"maxSize":0.05},"pressure":pressure,
        "points":[{"x":0,"y":0},{"x":1_000_000,"y":0}]}),
    );
    eprintln!("complete draw.stroke.add refusal {:?}: {error:?}", started.elapsed());
    assert_eq!(error.code, "limit.exceeded");
    assert!(error.message.contains("1200000000") || error.message.contains("100000"), "{error:?}");
    assert_eq!(rev(&h), revision);
    assert_eq!(get(&h, D), before);
    ok(&h, "draw.pivot.set", json!({"drawingId":D,"point":{"x":2,"y":3}}));
}

#[test]
fn nfc_script_corpus_edits_formats_content_and_package_round_trip() {
    let dir = c13_support::tmp();
    let h = c13_support::spawn(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut saved = Vec::new();
    for (base, insert, expected) in [
        ("\u{995}\u{9c7}", "\u{9be}", "\u{995}\u{9cb}"),
        ("\u{915}", "\u{93c}", "\u{915}\u{93c}"),
        ("\u{ac00}", "\u{11a8}", "\u{ac01}"),
        ("a\u{301}", "\u{323}\u{327}", "\u{1ea1}\u{327}\u{301}"),
        ("\u{5e9}\u{5c1}", "\u{5b8}", "\u{5e9}\u{5b8}\u{5c1}"),
        ("\u{1f469}", "\u{200d}\u{1f3a8}\u{fe0f}", "\u{1f469}\u{200d}\u{1f3a8}\u{fe0f}"),
    ] {
        for mode in 0..3 {
            let suffix = if mode == 1 { "Q" } else { "" };
            let result = ok(
                &h,
                "draw.text.add",
                json!({"drawingId":D,"position":{"x":0,"y":0},"text":format!("{base}{suffix}Z\nR")}),
            );
            let id = result["textId"].as_str().unwrap();
            let key = format!("{D}/{id}");
            let obj = get(&h, &key);
            let n = obj["text"].as_str().unwrap().chars().count();
            ok(&h, "draw.textFormat.set", json!({"drawingId":D,"textId":id,"start":n-3,"length":1,"bold":true}));
            let at = n - 3 - suffix.len();
            if mode == 0 {
                ok(&h, "draw.text.set", json!({"drawingId":D,"textId":id,"insert":{"at":at,"text":insert}}));
            } else if mode == 1 {
                ok(&h, "draw.text.set", json!({"drawingId":D,"textId":id,"insert":{"at":at+1,"text":insert}}));
                ok(&h, "draw.text.set", json!({"drawingId":D,"textId":id,"delete":{"start":at,"length":1}}));
            } else {
                ok(&h, "draw.text.set", json!({"drawingId":D,"textId":id,"text":format!("{expected}Z\nR")}));
            }
            let object = get(&h, &key);
            assert_eq!(object["text"], format!("{expected}Z\nR"));
            if mode < 2 {
                let zi = object["text"].as_str().unwrap().chars().count() - 3;
                assert!(
                    object["runs"].as_array().unwrap().iter().any(|r| r["bold"] == true
                        && r["start"].as_u64().unwrap() <= zi as u64
                        && r["start"].as_u64().unwrap() + r["length"].as_u64().unwrap() > zi as u64),
                    "{object}"
                );
            }
            let read = ok(&h, "draw.drawing.get", json!({"drawingId":D}));
            let node = read["nodes"].as_array().unwrap().iter().find(|v| v["id"] == id).unwrap();
            let copy = ok(&h, "draw.content.add", json!({"drawingId":D,"content":{"nodes":[node]}}));
            let copy_id = copy["idMap"][id].as_str().unwrap();
            let copied = get(&h, &format!("{D}/{copy_id}"));
            for field in ["text", "runs", "paragraphs"] {
                assert_eq!(copied[field], object[field]);
            }
            saved.push((key, object));
        }
    }
    let path = dir.join("NFC.animpkg");
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
