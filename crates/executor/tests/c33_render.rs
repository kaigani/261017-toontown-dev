//! M3 C3.3 (builder; readiness items 4, 10): `render.frame.run` and `render.limits.get` — sizes, backgrounds, pixel
//! read-back against the painted colours, inline and file output with `onConflict`, the inline limit, determinism
//! across repeats, undo/redo and save → open, query purity, compositions, JPEG, mixed atomic batches, cancellation.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod c33_support;
mod common;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use c33_support::*;
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, Status};

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A cleared fixture with one painted square of half-side 100 du at the origin: (handle, element, colour value).
fn one_square() -> (ExecutorHandle, String, [u8; 4]) {
    let h = fixture();
    clear(&h);
    let (c, rgba) = solids(&h, 1)[0].clone();
    let id = square(&h, "Square", 100.0, &c);
    (h, id, rgba)
}

#[test]
fn sizes_scale_and_limits() {
    let _t = turn();
    let (h, _, _) = one_square();
    let r = ok(&h, "render.frame.run", json!({ "frame": 1, "scale": 0.25 }));
    assert_eq!((r["width"].as_u64(), r["height"].as_u64()), (Some(480), Some(270)), "the camera size times the scale");
    let r = ok(&h, "render.frame.run", json!({ "frame": 1, "width": 300 }));
    assert_eq!((r["width"].as_u64(), r["height"].as_u64()), (Some(300), Some(169)), "the camera aspect");
    let r = ok(&h, "render.frame.run", json!({ "frame": 1, "width": 200, "height": 200 }));
    assert_eq!((r["width"].as_u64(), r["height"].as_u64()), (Some(200), Some(200)));
    assert_eq!(err(&h, "render.frame.run", json!({ "width": 8192, "scale": 2 })).code, "limit.exceeded");
    assert_eq!(err(&h, "render.frame.run", json!({ "scale": 9 })).code, "arg.invalid");
    let l = ok(&h, "render.limits.get", json!({}));
    assert_eq!(
        l,
        json!({ "maxInlinePixels": 2048, "maxInlineBytes": 8_388_608, "maxWidth": 8192, "maxHeight": 8192, "formats": ["png", "jpeg", "tga", "bmp"] })
    );
    // Over the inline limit: a typed refusal with the limits, before anything is written.
    let e = err(&h, "render.frame.run", json!({ "frame": 1, "width": 3000 }));
    assert_eq!(e.code, "render.inlineTooLarge");
    assert_eq!(e.details["maxInlinePixels"], 2048);
}

#[test]
fn pixels_show_the_painted_colour_on_the_background() {
    let _t = turn();
    let (h, _, rgba) = one_square();
    // 1920 x 1080 scaled 0.25: the square (±100 du = ±1 field) spans pixels 200..280 x 95..175.
    let img = render(&h, json!({ "frame": 1, "scale": 0.25 }));
    assert_eq!(img.at(240, 135), rgba, "the centre is the painted colour");
    assert_eq!(img.at(205, 100), rgba);
    assert_eq!(img.at(10, 10), [255, 255, 255, 255], "the scene background (white)");
    assert_eq!(img.at(195, 135), [255, 255, 255, 255], "just outside the square");
    let t = render(&h, json!({ "frame": 1, "scale": 0.25, "background": "transparent" }));
    assert_eq!(t.at(10, 10)[3], 0, "transparent");
    assert_eq!(t.at(240, 135), rgba);
    let c = render(&h, json!({ "frame": 1, "scale": 0.25, "background": "#102030FF" }));
    assert_eq!(c.at(10, 10), [0x10, 0x20, 0x30, 0xFF]);
    // Every result carries the image facts (09 §11.1 item 3).
    assert_eq!(img.result["mimeType"], "image/png");
    assert_eq!(img.result["bytes"].as_u64().unwrap() as usize, img.png.len());
    assert_eq!(img.result["sha256"].as_str().unwrap().len(), 64);
    assert_eq!(img.result["documentRevision"].as_u64().unwrap(), rev(&h));
}

#[test]
fn renders_are_deterministic_across_repeats_history_and_save_open() {
    let _t = turn();
    let (h, id, _) = one_square();
    ok(&h, "anim.transform.set", json!({ "nodeIds": [id], "frame": 1, "rotation": 10 }));
    ok(&h, "anim.transform.set", json!({ "nodeIds": [id], "frame": 13, "rotation": 50, "position": { "x": 2 } }));
    let params = json!({ "frame": 7, "scale": 0.25 });
    let a = render(&h, params.clone());
    let b = render(&h, params.clone());
    assert_eq!(a.png, b.png, "byte-identical PNG (09 §11.1 item 2)");
    assert_eq!(a.result["sha256"], b.result["sha256"]);
    ok(&h, "edit.undo", json!({}));
    assert_ne!(render(&h, params.clone()).png, a.png, "the undone key changes the frame");
    ok(&h, "edit.redo", json!({}));
    assert_eq!(render(&h, params.clone()).png, a.png, "redo restores the same bytes");
    let dir = c13_support::tmp();
    let path = dir.join("render.animpkg");
    c13_support::job_ok(&h, "project.saveAs", json!({ "path": path.to_string_lossy() }));
    ok(&h, "project.close", json!({ "onUnsaved": "discard" }));
    c13_support::job_ok(&h, "project.open", json!({ "path": path.to_string_lossy() }));
    assert_eq!(render(&h, params).png, a.png, "save and open keep the frame");
}

#[test]
fn a_render_is_a_pure_read() {
    let _t = turn();
    let (h, _, _) = one_square();
    let rev0 = rev(&h);
    let app = ok(&h, "app.state.get", json!({}));
    let history = ok(&h, "edit.history.get", json!({}));
    let rx = h.subscribe();
    render(&h, json!({ "frame": 12, "scale": 0.1 }));
    ok(&h, "inspect.frame.describe", json!({ "frame": 12 }));
    ok(&h, "inspect.contactSheet.render", json!({ "frames": [1, 2], "cellWidth": 32 }));
    assert_eq!(rev(&h), rev0);
    assert_eq!(ok(&h, "app.state.get", json!({})), app, "the current frame and selection stay");
    assert_eq!(ok(&h, "edit.history.get", json!({})), history);
    assert!(rx.try_iter().next().is_none(), "no events");
}

#[test]
fn file_output_and_conflicts() {
    let _t = turn();
    let (h, _, _) = one_square();
    let dir = c13_support::tmp();
    let path = dir.join("f.png");
    let inline = render(&h, json!({ "frame": 1, "scale": 0.1 }));
    let r = ok(
        &h,
        "render.frame.run",
        json!({ "frame": 1, "scale": 0.1, "output": "file", "path": path.to_string_lossy() }),
    );
    assert_eq!(r["path"].as_str(), Some(path.to_string_lossy().as_ref()));
    assert!(r.get("data").is_none());
    assert_eq!(std::fs::read(&path).unwrap(), inline.png, "the file is the inline image");
    let e = err(
        &h,
        "render.frame.run",
        json!({ "frame": 1, "scale": 0.1, "output": "file", "path": path.to_string_lossy() }),
    );
    assert_eq!(e.code, "export.fileExists");
    assert_eq!(e.details["paths"][0].as_str(), Some(path.to_string_lossy().as_ref()));
    let r = ok(
        &h,
        "render.frame.run",
        json!({ "frame": 1, "scale": 0.1, "output": "file", "path": path.to_string_lossy(), "onConflict": "rename" }),
    );
    assert!(r["path"].as_str().unwrap().ends_with("f (2).png"), "{r}");
    ok(
        &h,
        "render.frame.run",
        json!({ "frame": 2, "scale": 0.2, "output": "file", "path": path.to_string_lossy(), "onConflict": "overwrite" }),
    );
    assert_ne!(std::fs::read(&path).unwrap(), inline.png, "overwritten");
    assert_eq!(err(&h, "render.frame.run", json!({ "output": "file" })).code, "arg.invalid", "a file needs a path");
    assert_eq!(err(&h, "render.frame.run", json!({ "output": "file", "path": "relative.png" })).code, "fs.invalidPath");
}

#[test]
fn compositions_and_formats() {
    let _t = turn();
    let (h, id, rgba) = one_square();
    let c = solids(&h, 2)[1].clone();
    let other = square(&h, "Other", 50.0, &c.0);
    ok(&h, "anim.transform.set", json!({ "nodeIds": [other], "target": "static", "position": { "x": 4 } }));
    // `activeView` renders the Drawing view since M4 C4.2 (SI-107); with the Camera view active it waits for M4 C4.3.
    ok(&h, "view.type.set", json!({ "type": "camera" }));
    for (comp, owner) in [("xsheet", "M5 C5.1"), ("activeView", "M4 C4.3")] {
        let e = err(&h, "render.frame.run", json!({ "composition": comp }));
        assert_eq!((e.code.as_str(), e.details["owner"].as_str()), ("render.compositionUnavailable", Some(owner)));
    }
    // `element`: only the given elements, through the camera.
    let only = render(&h, json!({ "frame": 1, "scale": 0.25, "composition": "element", "elementIds": [other] }));
    assert_eq!(only.at(240, 135), [255, 255, 255, 255], "the square is not in the composition");
    let both = render(&h, json!({ "frame": 1, "scale": 0.25 }));
    assert_eq!(both.at(240, 135), rgba);
    assert_eq!(only.at(400, 135), both.at(400, 135), "the other square is drawn alike");
    assert_eq!(err(&h, "render.frame.run", json!({ "composition": "element" })).code, "arg.invalid");
    // `element` with a drawing: drawing space at the camera's table-plane scale (1200 du across the camera width).
    let d = drawing_at(&h, &id, 1);
    let dr = render(&h, json!({ "frame": 1, "composition": "element", "drawingId": d, "scale": 0.25 }));
    // The drawing's bounds (its outline stroke's width included) at 1920 / 1200 px per du, scaled 0.25.
    let full = ok(&h, "inspect.frame.describe", json!({ "frame": 1, "detail": "full" }));
    let bw = full["elements"].as_array().unwrap().iter().find(|e| e["elementId"] == id.as_str()).unwrap()["drawingBounds"]["width"]
        .as_f64()
        .unwrap();
    assert_eq!(dr.w, (bw * 1920.0 / 1200.0 * 0.25).round() as u32, "bounds {bw} du");
    assert_eq!(dr.w, dr.h, "a square");
    assert_eq!(dr.at(40, 40), rgba);
    // JPEG: decodable, the same size.
    let j = ok(&h, "render.frame.run", json!({ "frame": 1, "scale": 0.25, "format": "jpeg", "jpegQuality": 95 }));
    assert_eq!(j["mimeType"], "image/jpeg");
    let bytes = tt_persist::b64::decode(j["data"].as_str().unwrap()).unwrap();
    let dec = tt_io::image::decode(&bytes, Some("jpg"), &tt_io::image::LIMITS).unwrap();
    assert_eq!((dec.width, dec.height), (480, 270));
}

#[test]
fn a_render_inside_an_atomic_batch_sees_the_staged_edit() {
    let _t = turn();
    let (h, id, _) = one_square();
    let env = batch(
        &h,
        json!({ "label": "pose and look", "atomic": true, "steps": [
            { "id": "anim.transform.set", "params": { "nodeIds": [id], "target": "static", "rotation": 33 } },
            { "id": "render.frame.run", "params": { "frame": 1, "scale": 0.25 } } ] }),
    );
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let staged: Value = env.steps.as_ref().unwrap()[1].result.clone().unwrap();
    let after = ok(&h, "render.frame.run", json!({ "frame": 1, "scale": 0.25 }));
    assert_eq!(staged["sha256"], after["sha256"], "the batch's render is the committed state's");
    // A failing step after a render rolls back the edit; the render changed nothing to roll back.
    let rev0 = rev(&h);
    let env = batch(
        &h,
        json!({ "label": "bad", "atomic": true, "steps": [
            { "id": "anim.transform.set", "params": { "nodeIds": [id], "target": "static", "rotation": 1 } },
            { "id": "render.frame.run", "params": { "frame": 1, "scale": 0.25 } },
            { "id": "render.frame.run", "params": { "composition": "xsheet" } } ] }),
    );
    assert_ne!(env.status, Status::Ok);
    assert_eq!(rev(&h), rev0);
}

#[test]
fn a_render_past_the_callers_budget_is_cancelled_and_frees_the_worker() {
    let _t = turn();
    let budget = Duration::from_millis(400);
    let h = fixture_with(ExecutorConfig { budget });
    clear(&h);
    let (c, _) = solids(&h, 1)[0].clone();
    for i in 0..6 {
        square(&h, &format!("Many {i}"), 40.0, &c);
    }
    let rev0 = rev(&h);
    tt_vector::work::hooks::set_spin_micros(200_000);
    let t = Instant::now();
    let e = run(&h, "render.frame.run", json!({ "frame": 1, "scale": 0.25 }));
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(e.error.as_ref().map(|x| x.code.as_str()), Some("command.timeout"), "{e:?}");
    assert!(t.elapsed() < budget * 4, "{:?}", t.elapsed());
    assert!(run(&h, "doc.state.get", json!({})).is_ok(), "the worker answers");
    assert_eq!(rev(&h), rev0);
}

/// Readiness item 10: an edit's `anim.changed` scope is exactly where renders change — frames outside a keyed edit's
/// range render the same bytes before and after it, frames inside it differ, and all-frame edits (loop, camera, path,
/// segment) change frames anywhere.
#[test]
fn invalidation_scope_matches_the_frames_whose_renders_change() {
    let _t = turn();
    let (h, id, _) = one_square();
    for (frame, x) in [(6, -3.0), (12, 0.0), (24, 3.0)] {
        ok(&h, "anim.transform.set", json!({ "nodeIds": [id], "frame": frame, "position": { "x": x } }));
    }
    let frames = [3, 8, 15, 20, 30];
    let shots = |h: &ExecutorHandle| -> Vec<String> {
        frames
            .iter()
            .map(|f| {
                ok(h, "render.frame.run", json!({ "frame": f, "scale": 0.1 }))["sha256"].as_str().unwrap().to_owned()
            })
            .collect()
    };
    let changed = |h: &ExecutorHandle, cmd: &str, params: Value| -> Value {
        let rx = h.subscribe();
        ok(h, cmd, params);
        rx.try_iter().find(|e| e.name == "anim.changed").map(|e| e.payload).unwrap_or(Value::Null)
    };
    // A key at 18 between the keys 12 and 24 of the motion path: the event widens to two keyframes on each side (a TCB
    // tangent reaches one key further, ADR-0019 decision 12), so it says 6-24, and exactly the frames inside change.
    let before = shots(&h);
    let ev =
        changed(&h, "anim.transform.set", json!({ "nodeIds": [id], "frame": 18, "position": { "x": 1.0, "y": 1.5 } }));
    assert_eq!(ev["frames"], json!({ "from": 6.0, "to": 24.0 }), "{ev}");
    let after = shots(&h);
    for (i, f) in frames.iter().enumerate() {
        let inside = (6..=24).contains(f);
        assert_eq!(before[i] != after[i], inside, "frame {f}: changed iff inside the event's range");
    }
    // A looping peg above the square, keyed so that a loop changes what later frames show.
    let peg = ok(&h, "element.create", json!({ "type": "peg", "name": "Looper", "sceneId": INTRO }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    ok(&h, "element.parent.set", json!({ "elementIds": [id], "parentId": peg }));
    ok(&h, "element.timing.set", json!({ "elementIds": [peg], "duration": 24 }));
    ok(&h, "anim.transform.set", json!({ "nodeIds": [peg], "frame": 1, "rotation": 0 }));
    ok(&h, "anim.transform.set", json!({ "nodeIds": [peg], "frame": 24, "rotation": 40 }));
    // All-frame edits: the event has no range and a frame outside every key range changes.
    let cases = [
        ("anim.segment.set", json!({ "nodeIds": [id], "frame": 6, "constant": true })),
        ("anim.loop.set", json!({ "nodeId": peg, "loops": 2 })),
        ("anim.camera.set", json!({ "cameraId": CAMERA, "roll": 10 })),
        (
            "anim.path.set",
            json!({ "nodeId": id, "points": [{ "kind": "key", "frame": 1, "x": -2, "y": 1, "z": 0 }, { "kind": "key", "frame": 30, "x": 2, "y": -1, "z": 0 }] }),
        ),
    ];
    for (cmd, params) in cases {
        let before = shots(&h);
        let ev = changed(&h, cmd, params);
        assert!(ev.is_object(), "{cmd}: one anim.changed");
        let after = shots(&h);
        let changed_frames: Vec<u32> =
            frames.iter().zip(before.iter().zip(&after)).filter(|(_, (a, b))| a != b).map(|(f, _)| *f).collect();
        match ev.get("frames") {
            None => assert!(!changed_frames.is_empty(), "{cmd}: an all-frame change changes some frame"),
            Some(r) => {
                let (lo, hi) = (r["from"].as_f64().unwrap(), r["to"].as_f64().unwrap());
                assert!(
                    changed_frames.iter().all(|f| f64::from(*f) >= lo && f64::from(*f) <= hi),
                    "{cmd}: {changed_frames:?} outside {r}"
                );
            }
        }
    }
}
