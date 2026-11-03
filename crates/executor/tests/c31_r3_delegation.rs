//! Field/event equivalence for every implemented writable delegated property and RFC write form.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
const B: &str = "scene:\"Intro\"/element:\"Bob\"";
const D: &str = "scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"";
fn scrub(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.remove("createdAt");
            for x in m.values_mut() {
                scrub(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(scrub),
        _ => {}
    }
}
#[test]
fn all_delegated_properties_props_add_replace_copy_move_test_match_owner() {
    for (target, field, value, command, p) in [
        (B, "name", json!("AG928"), "element.rename", json!({"elementId":B,"name":"AG928"})),
        (B, "note", json!("note928"), "element.note.set", json!({"elementId":B,"text":"note928"})),
        (B, "markerColour", json!("#139287FF"), "element.colour.set", json!({"elementIds":[B],"marker":"#139287FF"})),
        (
            B,
            "backgroundColour",
            json!("#917253FF"),
            "element.colour.set",
            json!({"elementIds":[B],"background":"#917253FF"}),
        ),
        (
            B,
            "visibleInScene",
            json!(false),
            "element.visibility.set",
            json!({"elementIds":[B],"scope":"scene","visible":false}),
        ),
        (
            B,
            "visibleInXsheet",
            json!(false),
            "element.visibility.set",
            json!({"elementIds":[B],"scope":"xsheet","visible":false}),
        ),
        (B, "locked", json!(true), "element.lock.set", json!({"elementIds":[B],"locked":true})),
        (D, "name", json!("Drawing928"), "element.drawing.rename", json!({"drawingId":D,"name":"Drawing928"})),
        (D, "locked", json!(true), "element.drawingLock.set", json!({"drawingIds":[D],"locked":true})),
    ] {
        for form in ["props", "add", "replace", "copy", "move", "test"] {
            let a = fixture();
            let b = fixture();
            let key = get(&a, target)["id"].as_str().unwrap().to_owned();
            for h in [&a, &b] {
                ok(h, "element.current.set", json!({"elementId":B}));
                ok(h, "doc.object.patch", json!({"targets":[target],"props":{"meta":{"source928":value}}}));
            }
            let ra = a.subscribe();
            let rb = b.subscribe();
            if form == "move" {
                let r = batch(
                    &a,
                    json!({"label":"Direct928","atomic":true,"steps":[
                        {"id":"doc.object.patch","params":{"targets":[target],"ops":[{"op":"remove","path":"/meta/source928"}]}},
                        {"id":command,"params":p}
                    ]}),
                );
                assert_eq!(r.status, tt_executor::Status::Ok, "{r:?}");
            } else if form != "test" {
                ok(&a, command, p.clone());
            }
            let patch = match form {
                "props" => json!({"targets":[target],"props":{field:value}}),
                "copy" | "move" => {
                    json!({"targets":[target],"ops":[{"op":form,"from":"/meta/source928","path":format!("/{field}")}]})
                }
                "test" => {
                    json!({"targets":[target],"ops":[{"op":"test","path":format!("/{field}"),"value":get(&b,target)[field]}]})
                }
                _ => json!({"targets":[target],"ops":[{"op":form,"path":format!("/{field}"),"value":value}]}),
            };
            ok(&b, "doc.object.patch", patch);
            let mut expected = get(&a, &key);
            let mut actual = get(&b, &key);
            scrub(&mut expected);
            scrub(&mut actual);
            assert_eq!(actual, expected, "{field} {form}");
            assert_eq!(
                ok(&a, "cell.selection.get", json!({})),
                ok(&b, "cell.selection.get", json!({})),
                "{field} {form}"
            );
            let events = |r: std::sync::mpsc::Receiver<tt_executor::Event>| -> Vec<Value> {
                r.try_iter()
                    .filter(|e| e.name.starts_with("element.") || e.name == "cell.selection.changed")
                    .map(|e| json!({"name":e.name,"payload":e.payload}))
                    .collect()
            };
            assert_eq!(events(ra), events(rb), "{field} {form}");
        }
    }
}
