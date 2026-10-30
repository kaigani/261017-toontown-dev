//! M2 C2.1 obligation 4: both seeded history oracles of the C1.4 review (seeds `0xc120_2026_1019_a571` and
//! `0xc123_2026_1019_b683`) re-run through MCP with real drawing geometry on a saved and reopened geometry fixture.
//! 10 000 random operations each — `draw.stroke.set` geometry/thickness edits, multi-step undo and redo, checkpoint
//! create/restore and dry runs — are checked against an independent oracle of the edited stroke (its persisted anchors
//! computed here from the Bézier list, its line style, its and its drawing's revision) with exact canonical byte checks
//! of every entry every 100 operations, then replayed in a fresh executor, which must reproduce every entry byte for
//! byte, revisions included.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs,
    reason = "oracle"
)]

mod support;

use std::cell::RefCell;
use std::time::Instant;

use serde_json::{Value, json};
use tt_model::document::{Document, EntryKey};

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const PENCIL: &str = "st_01K742SG0R61SYK9GG0HGYVNBT";
const POINTER: &str = "/nodes/0/strokes/1";

struct Protocol {
    handle: tt_executor::ExecutorHandle,
    client: RefCell<Option<support::Client>>,
}

impl Drop for Protocol {
    fn drop(&mut self) {
        if let Some(c) = self.client.get_mut().take() {
            c.close().unwrap();
        }
    }
}

fn run(h: &Protocol, id: &str, mut params: Value) -> Value {
    let dry = params.as_object_mut().and_then(|p| p.remove("dryRun")).unwrap_or(json!(false));
    h.client.borrow_mut().as_mut().unwrap().call("run_command", json!({ "id": id, "params": params, "dryRun": dry }))["structuredContent"].clone()
}

fn ok(h: &Protocol, id: &str, params: Value) -> Value {
    let e = run(h, id, params);
    assert_eq!(e["status"], "ok", "{e}");
    e["result"].clone()
}

fn snapshot(h: &Protocol) -> Document {
    h.handle.snapshot().unwrap().unwrap()
}

fn canonical_entries(doc: &Document) -> Vec<(EntryKey, String)> {
    doc.entry_keys()
        .into_iter()
        .map(|k| {
            let t = tt_model::canonical::to_canonical_string(&doc.entry_raw_json(&k).unwrap()).unwrap();
            (k, t)
        })
        .collect()
}

/// The fixture with brush strokes, a pencil line and text added by commands, saved and reopened through MCP.
fn saved_geometry_fixture() -> Protocol {
    static SAVED: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let path = SAVED.get_or_init(|| {
        let dir = support::tmp();
        let h = support::executor(&dir.join("initial-user"));
        assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
        let call = |id: &str, params: Value| {
            let e = h.invoke(tt_executor::Invocation::user(id, params));
            assert!(e.is_ok(), "{id}: {e:?}");
            e.result.unwrap()
        };
        call("draw.stroke.add", json!({ "drawingId": D1, "points": [{"x": 0, "y": 0, "pressure": 0.2}, {"x": 40, "y": 25, "pressure": 1}, {"x": 80, "y": 5, "pressure": 0.4}], "pen": {"tipShape": "flat", "minSize": 1, "maxSize": 8, "tipAngle": 30} }));
        call("draw.stroke.add", json!({ "drawingId": D1, "kind": "pencil", "points": [{"x": -40, "y": 0}, {"x": -20, "y": 30}, {"x": 0, "y": 35}, {"x": 20, "y": 30}] }));
        call("draw.text.add", json!({ "drawingId": D1, "position": {"x": -60, "y": 120}, "text": "Oracle\nfixture" }));
        let path = dir.join("Geometry.animpkg");
        let e = call("project.saveAs", json!({ "path": path }));
        let done = call("job.wait", json!({ "jobId": e["jobId"], "timeoutMs": 60000 }));
        assert_eq!(done["state"], "succeeded");
        call("project.close", json!({}));
        path
    });
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    let mut opts = support::opts(path.parent().unwrap());
    opts.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (c, _) = support::Client::connect(&h, opts, json!({}));
    let p = Protocol { handle: h, client: RefCell::new(Some(c)) };
    let e = ok(&p, "project.open", json!({ "path": path, "readOnly": true }));
    let done = ok(&p, "job.wait", json!({ "jobId": e["jobId"], "timeoutMs": 60000 }));
    assert_eq!(done["state"], "succeeded", "{done}");
    p
}

/// The oracle's state of the stroke: its Bézier list and thickness.
type State = (Vec<(f64, f64)>, f64);

/// The persisted anchors of an open 4-point Bézier list (10 §4.9: handles are offsets), computed independently.
fn anchors(b: &[(f64, f64)]) -> Value {
    let (p0, p1, p2, p3) = (b[0], b[1], b[2], b[3]);
    json!([[p0.0, p0.1, 0, 0, p1.0 - p0.0, p1.1 - p0.1], [p3.0, p3.1, p2.0 - p3.0, p2.1 - p3.1, 0, 0]])
}

/// A committed content change: the next revision stamps the stroke and its drawing (REV-02/03).
fn change(expected_rev: &mut u64, stroke_rev: &mut u64, drawing_rev: &mut u64) {
    *expected_rev += 1;
    *stroke_rev = *expected_rev;
    *drawing_rev = *expected_rev;
}

fn oracle(seed_value: u64, name: &str) {
    let start = Instant::now();
    let h = saved_geometry_fixture();
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [format!("{D1}/{PENCIL}")], "props": { "meta": { "seedBase": true } } }),
    );
    let baseline = snapshot(&h);
    let key = EntryKey::Drawing(tt_model::Id::parse(D1).unwrap());
    let drawing_base = baseline.entry_raw_json(&key).unwrap();
    let stroke_base = drawing_base.pointer(POINTER).unwrap().clone();
    assert_eq!(stroke_base["id"], PENCIL);
    let pts = stroke_base["points"].as_array().unwrap();
    let to_list = |p: &Value| -> Vec<(f64, f64)> {
        let a = p[0].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>();
        let b = p[1].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>();
        vec![(a[0], a[1]), (a[0] + a[4], a[1] + a[5]), (b[0] + b[2], b[1] + b[3]), (b[0], b[1])]
    };
    let mut current: State =
        (to_list(&Value::Array(pts.clone())), stroke_base["lineStyle"]["thickness"].as_f64().unwrap());
    let mut history: Vec<(State, State)> = Vec::new();
    let mut position = 0;
    let mut expected_rev = baseline.document_rev();
    let mut stroke_rev = stroke_base["rev"].as_u64().unwrap();
    let mut drawing_rev = drawing_base["rev"].as_u64().unwrap();
    let mut seed = seed_value;
    let mut commands: Vec<(String, Value)> = Vec::new();
    let mut checkpoint = current.clone();
    let mut have_checkpoint = false;
    let mut counts = [0_u64; 6];
    for i in 0..10_000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let choice = seed % 100;
        let (id, params) = if choice < 55 {
            let c = |k: u64| ((seed >> k) % 4001) as f64 / 20.0 - 100.0;
            let next: State =
                (vec![(c(1), c(5)), (c(9), c(13)), (c(17), c(21)), (c(25), c(29))], ((seed >> 33) % 40) as f64 / 4.0);
            if next != current {
                history.truncate(position);
                history.push((current.clone(), next.clone()));
                position = history.len();
                if history.len() > 1000 {
                    history.remove(0);
                    position -= 1;
                }
                current = next.clone();
                change(&mut expected_rev, &mut stroke_rev, &mut drawing_rev);
            }
            counts[0] += 1;
            let bezier: Vec<Value> = next.0.iter().map(|(x, y)| json!({ "x": x, "y": y })).collect();
            ("draw.stroke.set", json!({ "drawingId": D1, "strokeId": PENCIL, "bezier": bezier, "thickness": next.1 }))
        } else if choice < 75 && position > 0 {
            let n = ((seed >> 8) % 3 + 1).min(position as u64) as usize;
            for _ in 0..n {
                position -= 1;
                current = history[position].0.clone();
                change(&mut expected_rev, &mut stroke_rev, &mut drawing_rev);
            }
            counts[1] += n as u64;
            ("edit.undo", json!({ "steps": n }))
        } else if choice < 88 && position < history.len() {
            let n = ((seed >> 8) % 3 + 1).min((history.len() - position) as u64) as usize;
            for _ in 0..n {
                current = history[position].1.clone();
                position += 1;
                change(&mut expected_rev, &mut stroke_rev, &mut drawing_rev);
            }
            counts[2] += n as u64;
            ("edit.redo", json!({ "steps": n }))
        } else if choice < 91 && !have_checkpoint {
            checkpoint = current.clone();
            have_checkpoint = true;
            counts[3] += 1;
            ("checkpoint.create", json!({ "name": "random-oracle" }))
        } else if choice < 95 && have_checkpoint {
            if checkpoint != current {
                history.truncate(position);
                history.push((current.clone(), checkpoint.clone()));
                position = history.len();
                if history.len() > 1000 {
                    history.remove(0);
                    position -= 1;
                }
                current = checkpoint.clone();
                change(&mut expected_rev, &mut stroke_rev, &mut drawing_rev);
            }
            counts[4] += 1;
            ("checkpoint.restore", json!({ "name": "random-oracle" }))
        } else {
            counts[5] += 1;
            ("draw.stroke.set", json!({ "drawingId": D1, "strokeId": PENCIL, "thickness": 99, "dryRun": true }))
        };
        let env = run(&h, id, params.clone());
        assert!(env["status"] == "ok", "seed {seed_value:x}, operation {i}: {id} {params}: {env:?}");
        commands.push((id.into(), params));
        assert_eq!(snapshot(&h).document_rev(), expected_rev, "seed {seed_value:x}, operation {i}");
        if i % 100 == 0 || i == 9999 {
            let snap = snapshot(&h);
            for k in baseline.entry_keys() {
                let mut want = baseline.entry_raw_json(&k).unwrap();
                if k == key {
                    let s = want.pointer_mut(POINTER).unwrap();
                    s["points"] = anchors(&current.0);
                    s["lineStyle"]["thickness"] = json!(current.1);
                    s["rev"] = json!(stroke_rev);
                    want["rev"] = json!(drawing_rev);
                }
                assert_eq!(
                    tt_model::canonical::to_canonical_string(&snap.entry_raw_json(&k).unwrap()).unwrap(),
                    tt_model::canonical::to_canonical_string(&want).unwrap(),
                    "seed {seed_value:x}, operation {i}, {k:?}"
                );
            }
        }
    }
    let replay = saved_geometry_fixture();
    ok(
        &replay,
        "doc.object.patch",
        json!({ "targets": [format!("{D1}/{PENCIL}")], "props": { "meta": { "seedBase": true } } }),
    );
    for (i, (id, params)) in commands.iter().enumerate() {
        let env = run(&replay, id, params.clone());
        assert!(env["status"] == "ok", "replay {i}: {env:?}");
    }
    assert_eq!(
        canonical_entries(&snapshot(&h)),
        canonical_entries(&snapshot(&replay)),
        "fresh replay is byte identical, revisions included"
    );
    let receipt = json!({ "seed": format!("0x{seed_value:x}"), "operations": 10000, "replayOperations": 10000,
        "strokeSetUndoRedoCreateRestoreDryRun": counts, "documentRev": expected_rev, "durationSeconds": start.elapsed().as_secs_f64(),
        "oracle": "independent stroke geometry/thickness/revision stack; exact canonical all-entry byte checks every 100 operations; full fresh executor replay" });
    println!("{name}: {receipt}");
}

#[test]
#[ignore = "10 000-operation oracle (about 5 minutes per seed in a debug build on the Windows runner); CI runs both             seeds in release in the headless job: `cargo test --release -p tt-mcp --test c21_history -- --ignored`             (M2 C2.1 obligation 4)"]
fn geometry_oracle_seed_a571_matches_and_replays() {
    oracle(0xc120_2026_1019_a571, "geometry-history-r1");
}

#[test]
#[ignore = "10 000-operation oracle (about 5 minutes per seed in a debug build on the Windows runner); CI runs both             seeds in release in the headless job: `cargo test --release -p tt-mcp --test c21_history -- --ignored`             (M2 C2.1 obligation 4)"]
fn geometry_oracle_seed_b683_matches_and_replays() {
    oracle(0xc123_2026_1019_b683, "geometry-history-r3");
}
