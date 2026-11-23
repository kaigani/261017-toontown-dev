//! M5 C5.1a round 1 (builder; the observation on E-18's evidence): the comparison behind
//! `reviews/evidence/M5-C5.1a/e18-event-stream.json`, rerunnable. It replays the seeded script of
//! `c42b_r2_review::seeded_script_profile_receipt` (copied here: the reviewer's file stays as it is, and this test
//! writes nothing), hashes the event stream as that test does, and proves that the new stream is the old one with only
//! `timeline.currentFrame.changed` events added: removing them gives exactly the hash committed before M5.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]
mod common;

use common::*;
use serde_json::json;
use sha2::{Digest, Sha256};
use tt_executor::ExecutorHandle;

/// The committed receipts: before M5 (`m4`) and after ruling E-18.
const OLD: &str = "39ed54c599dece98a21f1cc65a3910679a5fdd7d86ab76150d83a39e36cb0d1b";
const NEW: &str = "e65adfa722c2a55f6f5d47c6938b527227459a3f101a0f2996c9aaac5eed9b42";

fn canonical(h: &ExecutorHandle) -> Vec<u8> {
    let d = h.snapshot().unwrap().unwrap();
    let entries: Vec<_> =
        d.entry_keys().iter().map(|k| tt_persist::save::entry_text(&d, &Default::default(), k).unwrap()).collect();
    serde_json::to_vec(&entries).unwrap()
}

fn stroke(h: &ExecutorHandle, n: u32) {
    ok(
        h,
        "draw.stroke.add",
        json!({"kind":"pencil","points":[{"x":300+n,"y":100},{"x":320+n,"y":120}],"fit":false,"layer":"new"}),
    );
}

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn the_new_event_stream_is_the_old_one_with_only_frame_events_added() {
    // The reviewer's script, step for step.
    let h = fixture();
    ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":1}));
    ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":9}));
    ok(&h, "element.drawingLock.set", json!({"locked":false}));
    let mut rng = 0xc42b_0201u64;
    let rx = h.subscribe();
    let mut events = Vec::new();
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
    // The stream as the reviewer's test hashes it: the committed new receipt.
    assert_eq!(sha(&serde_json::to_vec(&events).unwrap()), NEW, "the committed E-18 receipt");
    // The added events: one `timeline.currentFrame.changed` per frame change of the script (every 16th step, frames 1
    // and 9 alternating from 1), each a command change of the current scene.
    let added: Vec<_> = events.iter().filter(|e| e.name == "timeline.currentFrame.changed").collect();
    assert_eq!(added.len(), 20, "{added:?}");
    for (k, e) in added.iter().enumerate() {
        assert_eq!(e.payload["source"], "command");
        assert_eq!(e.payload["frame"], if k % 2 == 0 { 1 } else { 9 }, "{k}: {e:?}");
    }
    // Without them: the receipt committed before M5, so every other event is present, in order, with its payload.
    let without: Vec<_> = events.into_iter().filter(|e| e.name != "timeline.currentFrame.changed").collect();
    assert_eq!(without.len(), 1113);
    assert_eq!(sha(&serde_json::to_vec(&without).unwrap()), OLD, "the receipt committed before M5");
}
