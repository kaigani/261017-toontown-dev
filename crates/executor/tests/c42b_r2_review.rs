//! Round-two adversarial executor review.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tt_executor::ExecutorHandle;
fn ready() -> ExecutorHandle {
    let h = fixture();
    ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":1}));
    h
}
fn canonical(h: &ExecutorHandle) -> Vec<u8> {
    let d = h.snapshot().unwrap().unwrap();
    let entries: Vec<_> =
        d.entry_keys().iter().map(|k| tt_persist::save::entry_text(&d, &Default::default(), k).unwrap()).collect();
    serde_json::to_vec(&entries).unwrap()
}
fn content(h: &ExecutorHandle) -> Value {
    let d = h.snapshot().unwrap().unwrap();
    let mut v: Value = serde_json::from_slice(&canonical(h)).unwrap();
    fn strip(v: &mut Value) {
        match v {
            Value::Object(m) => {
                m.remove("rev");
                for v in m.values_mut() {
                    strip(v)
                }
            }
            Value::Array(a) => {
                for v in a {
                    strip(v)
                }
            }
            _ => {}
        }
    }
    let a = v.as_array_mut().unwrap();
    for x in a {
        let mut obj: Value = serde_json::from_str(x.as_str().unwrap()).unwrap();
        strip(&mut obj);
        *x = obj;
    }
    drop(d);
    v
}
fn stroke(h: &ExecutorHandle, n: u32) {
    ok(
        h,
        "draw.stroke.add",
        json!({"kind":"pencil","points":[{"x":300+n,"y":100},{"x":320+n,"y":120}],"fit":false,"layer":"new"}),
    );
}
#[test]
fn fifty_append_undo_redo_cycles() {
    let h = ready();
    for i in 0..50 {
        let before = content(&h);
        stroke(&h, i);
        let after = content(&h);
        ok(&h, "edit.undo", json!({}));
        assert_eq!(content(&h), before, "undo cycle {i}");
        ok(&h, "edit.redo", json!({}));
        assert_eq!(content(&h), after, "redo cycle {i}");
    }
}
#[test]
fn seeded_script_profile_receipt() {
    let h = ready();
    ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":9}));
    ok(&h, "element.drawingLock.set", json!({"locked":false}));
    let seed = 0xc42b_0201u64;
    let mut rng = seed;
    let rx = h.subscribe();
    let mut events = Vec::new();
    eprintln!("seed={seed:#x}");
    for i in 0..320 {
        if i % 16 == 0 {
            ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":if i%32==0{1}else{9}}));
        }
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        let x = 300 + (rng % 100) as u32;
        match rng % 8 {
            0 => {
                ok(&h, "draw.rectangle.add", json!({"from":{"x":x,"y":100},"to":{"x":x+20,"y":130},"layer":"new"}));
            }
            1 => {
                ok(&h, "draw.ellipse.add", json!({"from":{"x":x,"y":100},"to":{"x":x+20,"y":130},"layer":"new"}));
            }
            2 => {
                ok(
                    &h,
                    "draw.stroke.add",
                    json!({"kind":"brush","points":[{"x":x,"y":100,"pressure":0.2},{"x":x+10,"y":120,"pressure":0.8}],"fit":false,"layer":"top"}),
                );
            }
            3 => {
                ok(&h, "draw.selection.set", json!({"by":"all"}));
                ok(&h, "draw.transform.apply", json!({"translate":{"x":1,"y":-1}}));
            }
            4 => {
                ok(&h, "edit.undo", json!({}));
                ok(&h, "edit.redo", json!({}));
            }
            6 => {
                let before = canonical(&h);
                let e = run(&h, "draw.erase.apply", json!({"path":[{"x":1000,"y":1000},{"x":1010,"y":1010}]}));
                if let Some(e) = e.error {
                    assert!(["drawing.nothingToDo", "colour.zoneNotClosed"].contains(&e.code.as_str()));
                    assert_eq!(canonical(&h), before);
                }
            }
            7 => {
                let before = canonical(&h);
                let e = run(&h, "colour.paint.apply", json!({"points":[{"x":310,"y":110}]}));
                if let Some(e) = e.error {
                    assert!(["drawing.nothingToDo", "colour.zoneNotClosed"].contains(&e.code.as_str()));
                    assert_eq!(canonical(&h), before);
                }
            }
            _ => stroke(&h, i % 100),
        }
        events.extend(rx.try_iter());
    }
    let bytes = canonical(&h);
    let hash: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reviews/evidence/M4-C4.2b-r2");
    std::fs::create_dir_all(&root).unwrap();
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    std::fs::write(root.join(format!("A2-{profile}.sha256")), format!("{hash}\n")).unwrap();
    let other = root.join(if cfg!(debug_assertions) { "A2-release.sha256" } else { "A2-debug.sha256" });
    if other.exists() {
        assert_eq!(std::fs::read_to_string(other).unwrap().trim(), hash);
    }
    let event_bytes = serde_json::to_vec(&events).unwrap();
    let event_hash: String = Sha256::digest(&event_bytes).iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(root.join(format!("A2-{profile}-events.sha256")), format!("{event_hash}\n")).unwrap();
    let other = root.join(if cfg!(debug_assertions) { "A2-release-events.sha256" } else { "A2-debug-events.sha256" });
    if other.exists() {
        assert_eq!(std::fs::read_to_string(other).unwrap().trim(), event_hash);
    }
    if !cfg!(debug_assertions) {
        let doc = h.snapshot().unwrap().unwrap();
        let input = tt_persist::save::SaveInput {
            doc: &doc,
            preserved: &Default::default(),
            blobs: &Default::default(),
            session: None,
            saved_at: "2026-11-14T00:00:00Z".into(),
        };
        tt_persist::save::save_folder(&root.join("release-project"), &input, None, Default::default()).unwrap();
    } else if root.join("release-project").exists() {
        let loaded = tt_persist::load::load(&root.join("release-project"), &Default::default()).unwrap();
        let entries: Vec<_> = loaded
            .doc
            .entry_keys()
            .iter()
            .map(|k| tt_persist::save::entry_text(&loaded.doc, &loaded.preserved, k).unwrap())
            .collect();
        assert_eq!(serde_json::to_vec(&entries).unwrap(), bytes, "release saved project reopened in debug");
    }
}
#[test]
fn locked_and_invalid_append_leave_content_unchanged() {
    let h = ready();
    let before = canonical(&h);
    err(&h, "draw.stroke.add", json!({"kind":"pencil","points":[{"x":0,"y":0,"pressure":-1}]}));
    assert_eq!(canonical(&h), before);
    ok(&h, "element.drawingLock.set", json!({"locked":true}));
    let before = canonical(&h);
    err(&h, "draw.stroke.add", json!({"kind":"pencil","points":[{"x":1,"y":1},{"x":2,"y":2}]}));
    assert_eq!(canonical(&h), before);
}

#[test]
fn append_other_drawing_edit_and_symmetry() {
    let h = ready();
    let before = content(&h);
    stroke(&h, 1);
    let appended = content(&h);
    let d = h.snapshot().unwrap().unwrap();
    let other = d.drawings().find(|x| x.name() == "2").unwrap().id().as_str().to_owned();
    drop(d);
    ok(&h, "doc.object.patch", json!({"targets":[other],"props":{"name":"second"}}));
    ok(&h, "edit.undo", json!({}));
    assert_eq!(content(&h), appended);
    ok(&h, "edit.undo", json!({}));
    assert_eq!(content(&h), before);
    ok(&h, "tool.symmetry.set", json!({"mode":"kaleidoscope","mirrors":4,"angle":0}));
    let before = content(&h);
    let r = ok(
        &h,
        "draw.stroke.add",
        json!({"kind":"pencil","points":[{"x":300,"y":100},{"x":320,"y":120}],"symmetry":true}),
    );
    assert_eq!(r["strokeIds"].as_array().unwrap().len(), 8);
    ok(&h, "edit.undo", json!({}));
    assert_eq!(content(&h), before);
}
#[test]
fn atomic_appends_to_two_layers_or_drawings() {
    for two_drawings in [false, true] {
        let h = ready();
        ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":9}));
        ok(&h, "element.drawingLock.set", json!({"locked":false}));
        ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":1}));
        let doc = h.snapshot().unwrap().unwrap();
        let a = doc.drawings().find(|d| d.name() == "1").unwrap().id().as_str().to_owned();
        let b = if two_drawings {
            doc.drawings().find(|d| d.name() == "2").unwrap().id().as_str().to_owned()
        } else {
            a.clone()
        };
        drop(doc);
        let before = content(&h);
        let steps:Vec<_>=[a,b].into_iter().map(|id|json!({"id":"draw.stroke.add","params":{"drawingId":id,"kind":"pencil","points":[{"x":300,"y":100},{"x":320,"y":120}],"layer":"new"}})).collect();
        let e = batch(&h, json!({"label":"two appends","steps":steps,"atomic":true}));
        assert!(e.is_ok(), "{e:?}");
        let after = content(&h);
        assert_ne!(after, before);
        ok(&h, "edit.undo", json!({}));
        assert_eq!(content(&h), before);
        ok(&h, "edit.redo", json!({}));
        assert_eq!(content(&h), after);
    }
}
