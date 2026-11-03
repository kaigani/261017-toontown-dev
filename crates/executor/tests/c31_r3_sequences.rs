//! Independent frame-list/note oracle through commands and persistence: 4096 seeded edit sequences.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod c13_support;
mod common;
use c13_support::{job_ok, tmp};
use common::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn next(s: &mut u64, n: usize) -> usize {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s as usize % n
}
fn canonical(f: &[Option<String>]) -> Value {
    let end = f.iter().rposition(Option::is_some).map_or(0, |i| i + 1);
    let mut out: Vec<Value> = vec![];
    for d in &f[..end] {
        if let Some(r) = out.last_mut().filter(|r| r[0] == json!(d)) {
            r[1] = json!(r[1].as_u64().unwrap() + 1);
        } else {
            out.push(json!([d, 1]));
        }
    }
    json!(out)
}
fn insert(f: &mut Vec<Option<String>>, at: usize, values: Vec<Option<String>>) {
    if f.len() < at {
        f.resize(at, None);
    }
    f.splice(at..at, values);
}
#[test]
fn independent_sequences_4096_persist_canonical_runs_and_notes() {
    let h = fixture();
    let source = "scene:\"Intro\"/element:\"Bob\"";
    let bank = get(&h, source)["drawingBankId"].clone();
    let clone = ok(&h, "element.clone", json!({"elementId":source,"name":"OracleClone928"}))["elementId"].clone();
    let clone = clone.as_str().unwrap().to_owned();
    assert_eq!(get(&h, &clone)["drawingBankId"], bank);
    let drawings = ok(&h, "element.drawing.list", json!({"elementId":source}))["drawings"].as_array().unwrap().clone();
    let ids: Vec<String> = drawings.iter().map(|d| d["id"].as_str().unwrap().to_owned()).collect();
    let names: Vec<String> = drawings.iter().map(|d| d["name"].as_str().unwrap().to_owned()).collect();
    let dir = tmp();
    let mut seed = 0x313940u64;
    for sequence in 0..4096 {
        let e = if sequence % 2 == 0 { source } else { &clone };
        ok(&h, "doc.object.patch", json!({"targets":[e],"props":{"exposure":[],"cellNotes":{}}}));
        let mut frames: Vec<Option<String>> = vec![];
        let mut notes: BTreeMap<usize, String> = BTreeMap::new();
        for step in 0..8 {
            let at = next(&mut seed, 16) + 1;
            let count = next(&mut seed, 3) + 1;
            let d = next(&mut seed, ids.len());
            match next(&mut seed, 6) {
                0 => {
                    ok(&h, "cell.label.set", json!({"elementIds":[e],"from":at,"to":at+count-1,"drawing":names[d]}));
                    if frames.len() < at + count - 1 {
                        frames.resize(at + count - 1, None);
                    }
                    for f in &mut frames[at - 1..at + count - 1] {
                        *f = Some(ids[d].clone());
                    }
                }
                1 => {
                    ok(&h, "cell.range.add", json!({"elementIds":[e],"frame":at,"position":"before","count":count}));
                    insert(&mut frames, at - 1, vec![None; count]);
                    notes = notes.into_iter().map(|(f, t)| (if f >= at { f + count } else { f }, t)).collect();
                }
                2 => {
                    ok(&h, "cell.range.delete", json!({"elementIds":[e],"from":at,"to":at+count-1,"shift":true}));
                    let end = (at - 1 + count).min(frames.len());
                    if at - 1 < end {
                        frames.drain(at - 1..end);
                    }
                    notes = notes
                        .into_iter()
                        .filter_map(|(f, t)| {
                            if f >= at && f < at + count {
                                None
                            } else {
                                Some((if f >= at + count { f - count } else { f }, t))
                            }
                        })
                        .collect();
                }
                3 => {
                    let text = format!("s{sequence}-t{step}-928");
                    ok(&h, "cell.note.set", json!({"elementId":e,"frame":at,"text":text}));
                    notes.insert(at, text);
                }
                4 => {
                    // Explicit insert cycle of one cell: original range followed by one copy.
                    ok(&h, "cell.cycle.create", json!({"elementId":e,"from":at,"to":at,"cycles":2}));
                    let value = frames.get(at - 1).cloned().unwrap_or(None);
                    insert(&mut frames, at, vec![value]);
                    notes = notes.into_iter().map(|(f, t)| (if f > at { f + 1 } else { f }, t)).collect();
                }
                _ => {
                    ok(
                        &h,
                        "cell.columnate.apply",
                        json!({"elementId":e,"frame":at,"firstDrawing":names[d],"lastDrawing":names[d],"exposeEach":count,"exposeLast":count,"loopMode":"none","placement":"insert"}),
                    );
                    insert(&mut frames, at - 1, vec![Some(ids[d].clone()); count]);
                    notes = notes.into_iter().map(|(f, t)| (if f >= at { f + count } else { f }, t)).collect();
                }
            }
            let actual = get(&h, e);
            assert_eq!(
                serde_json::to_vec(&actual["exposure"]).unwrap(),
                serde_json::to_vec(&canonical(&frames)).unwrap(),
                "sequence {sequence} step {step}"
            );
            let expected: BTreeMap<String, String> = notes.iter().map(|(f, t)| (f.to_string(), t.clone())).collect();
            let got = actual.get("cellNotes").cloned().unwrap_or(json!({}));
            assert_eq!(got, json!(expected), "notes sequence {sequence} step {step}");
        }
        let path = dir.join(format!("sequence-{sequence}.animpkg"));
        job_ok(&h, "project.saveAs", json!({"path":path}));
        ok(&h, "project.close", json!({"onUnsaved":"discard"}));
        job_ok(&h, "project.open", json!({"path":path}));
        assert_eq!(
            serde_json::to_vec(&get(&h, e)["exposure"]).unwrap(),
            serde_json::to_vec(&canonical(&frames)).unwrap(),
            "saved sequence {sequence}"
        );
        if sequence % 512 == 0 {
            println!("persisted sequence {sequence}");
        }
    }
}
