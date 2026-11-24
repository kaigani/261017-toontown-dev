//! M5 C5.1b, plan §6.2: the Timeline's kernel budgets on the workstation (release, 30 samples each), printed as one JSON
//! receipt. The scene is the plan's, 1,000 rows over 9,999 frames and 200 pegs keyed every 4 frames, keyed where the
//! measured views can show keys (frames 1-400; see the build).
//! Run: `cargo run --release -p tt-executor --example c51b_budgets > c51b-budgets.json`.
//!
//! - `timeline.rows.list` for a 120-frame window (bound: p95 <= 20 ms);
//! - `xsheet.exposures.get` for 60 rows x 200 frames (bound: p95 <= 5 ms);
//! - a release to its committed state through the timing-gesture interpreter, for a bar move, a row reorder and a cell
//!   move (bound: p95 <= 50 ms), each undone before the next sample.
//!
//! The window's budgets (the work per frame while scrolling, zooming and dragging; click to the selection drawn; playback
//! presentation) are the window's to measure; this tool measures what the kernel answers.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    missing_docs,
    reason = "measurement tool"
)]

use std::time::Instant;

use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, Invocation, Origin};

const SAMPLES: usize = 30;

fn ok(h: &ExecutorHandle, command: &str, params: Value) -> Value {
    let env = h.invoke(Invocation::user(command, params.clone()));
    assert!(env.error.is_none(), "{command} {params}: {:?}", env.error);
    env.result.unwrap_or(Value::Null)
}

/// A command that may answer with a job (`project.save`, `.saveAs`, `.open`): its job waited for.
fn ok_waited(h: &ExecutorHandle, command: &str, params: Value) -> Value {
    let out = ok(h, command, params);
    match out.get("jobId").and_then(Value::as_str) {
        Some(job) => ok(h, "job.wait", json!({"jobId": job, "timeoutMs": 600_000})),
        None => out,
    }
}

fn summary(mut ms: Vec<f64>, bound: f64) -> Value {
    ms.sort_by(f64::total_cmp);
    let at = |q: f64| ms[((ms.len() as f64 - 1.0) * q).round() as usize];
    let p95 = at(0.95);
    json!({ "samples": ms.len(), "p50Ms": at(0.5), "p95Ms": p95, "maxMs": ms[ms.len() - 1], "boundP95Ms": bound,
            "withinBound": p95 <= bound })
}

fn time(f: impl FnOnce()) -> f64 {
    let t = Instant::now();
    f();
    t.elapsed().as_secs_f64() * 1e3
}

fn main() {
    let built = Instant::now();
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: std::time::Duration::from_secs(600) }).unwrap();
    ok(&h, "project.create", json!({"name": "Budgets"}));
    // Saved and opened again now and then while it is built: each build step is an undo step holding the curves as
    // they were, so 2,500 key additions in one history grow with their square (20 GB before this). Opening the saved
    // project starts a new history; the document is the same.
    // `TT_BUDGETS_SCENE=<package>`: the scene is opened from there when it exists, else built and kept there (a rerun
    // skips the build); without it, built in a temporary package removed at the end.
    let kept = std::env::var_os("TT_BUDGETS_SCENE").map(std::path::PathBuf::from);
    let package = kept
        .clone()
        .unwrap_or_else(|| std::env::temp_dir().join(format!("tt-c51b-budgets-{}.animpkg", std::process::id())));
    let path = package.to_string_lossy().into_owned();
    let reuse = kept.is_some() && package.exists();
    if reuse {
        ok(&h, "project.close", json!({"onUnsaved": "discard"}));
        ok_waited(&h, "project.open", json!({"path": path, "onUnsaved": "fail"}));
    } else {
        ok_waited(&h, "project.saveAs", json!({"path": path, "onExists": "overwrite"}));
    }
    // Saved, the save's job waited for, then opened (an open that saves first reads the file before its own save:
    // merge note §5).
    let reopen = || {
        ok_waited(&h, "project.save", json!({}));
        // Closed first: an open of the project's own path finds its lock held and opens it read-only.
        ok(&h, "project.close", json!({"onUnsaved": "fail"}));
        ok_waited(&h, "project.open", json!({"path": path, "onUnsaved": "fail"}));
    };
    if !reuse {
        // 800 drawings exposed over the whole scene (the first anchors its 9,999 frames), then 200 pegs.
        let mut drawings = Vec::new();
        for batch in 0..9 {
            let n = if batch == 8 { 8 } else { 99 };
            let made = ok(&h, "element.create", json!({"type": "drawing", "count": n, "name": format!("D{batch}")}));
            drawings.extend(made["elementIds"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()));
        }
        for (i, id) in drawings.iter().enumerate() {
            let to = if i == 0 { 9999 } else { 200 + (i % 40) * 5 };
            ok(&h, "element.drawing.create", json!({"elementId": id, "name": "1", "exposeFrom": 1, "exposeTo": to}));
        }
        reopen();
        // 200 pegs keyed every 4 frames where the measured views can show keys: frames 1-400 (a 120-frame window, and the
        // frames in view at the zoom levels the window budgets step through). Keys beyond them change neither read: building
        // them through the command path costs an undo step per frame that holds every curve (quadratic: 20 GB at 9,999 frames,
        // and duplicating a keyed peg took minutes each). The scene stays 1,000 rows over 9,999 frames.
        eprintln!("stage: drawings exposed at {:.0} s", built.elapsed().as_secs_f64());
        let mut pegs = Vec::new();
        for batch in 0..3 {
            let n = if batch == 2 { 2 } else { 99 };
            let made = ok(&h, "element.create", json!({"type": "peg", "count": n, "name": format!("P{batch}")}));
            pegs.extend(made["elementIds"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()));
        }
        for (k, f) in (1..=400).step_by(4).enumerate() {
            ok(&h, "anim.key.add", json!({"nodeIds": pegs, "frame": f}));
            if k % 25 == 24 {
                eprintln!("stage: {} key frames at {:.0} s", k + 1, built.elapsed().as_secs_f64());
                reopen();
            }
        }
        reopen();
    }
    // The scene's drawings (D0, the 9,999-frame anchor, first) and pegs, by name.
    let listed = ok(&h, "element.list", json!({}))["elements"].as_array().unwrap().clone();
    let named = |prefix: char| -> Vec<(String, String)> {
        listed
            .iter()
            .filter_map(|e| {
                let name = e["name"].as_str()?;
                let mut chars = name.chars();
                (chars.next() == Some(prefix) && chars.next().is_some_and(|c| c.is_ascii_digit()))
                    .then(|| (name.to_owned(), e["id"].as_str().unwrap_or_default().to_owned()))
            })
            .collect()
    };
    let mut drawing_names = named('D');
    drawing_names.sort_by_key(|(n, _)| n != "D0");
    let drawings: Vec<String> = drawing_names.into_iter().map(|(_, id)| id).collect();
    let pegs: Vec<String> = named('P').into_iter().map(|(_, id)| id).collect();
    assert_eq!((drawings.len(), pegs.len()), (800, 200), "the scene's drawings and pegs");
    let rows = ok(&h, "timeline.rows.list", json!({"from": 1, "to": 120}))["rows"].as_array().unwrap().len();
    let length = ok(&h, "timeline.rows.list", json!({"from": 1, "to": 1}))["sceneLength"].clone();
    let build_s = built.elapsed().as_secs_f64();

    eprintln!("stage: scene built at {:.0} s", built.elapsed().as_secs_f64());
    // timeline.rows.list for 120-frame windows across the scene.
    let rows_list: Vec<f64> = (0..SAMPLES)
        .map(|i| {
            let from = 1 + (i * 331) % 9800;
            time(|| {
                ok(&h, "timeline.rows.list", json!({"from": from, "to": from + 119}));
            })
        })
        .collect();
    // xsheet.exposures.get for 60 rows x 200 frames.
    let exposures: Vec<f64> = (0..SAMPLES)
        .map(|i| {
            let ids: Vec<&String> = drawings.iter().skip((i * 7) % 700).take(60).collect();
            time(|| {
                ok(&h, "xsheet.exposures.get", json!({"elementIds": ids, "from": 1, "to": 200}));
            })
        })
        .collect();

    eprintln!("stage: reads measured at {:.0} s", built.elapsed().as_secs_f64());
    // Release to commit: press then release through the interpreter (the window's path), timed from the release.
    let shown: Vec<Value> =
        ok(&h, "timeline.rows.list", json!({"from": 1, "to": 120}))["rows"].as_array().unwrap().clone();
    let index_of = |id: &str| shown.iter().position(|r| r["elementId"] == id).unwrap() as f64;
    let rh = 22.0;
    let cw = 16.0;
    let release = |surface: &str, first_frame: i64, scroll: f64, samples: Value| -> f64 {
        let layout = json!({"surface": surface, "zoomLevel": 6, "firstFrame": first_frame, "scrollTop": scroll,
                            "platform": "windows"});
        let press =
            h.timing_gesture(Origin::User, json!({"phase": "press", "layout": layout, "sample": samples[0]})).unwrap();
        let ms = time(|| {
            let out = h
                .timing_gesture(
                    Origin::User,
                    json!({"phase": "release", "grabId": press["grabId"], "samples": samples}),
                )
                .unwrap();
            assert!(
                out["envelopes"].as_array().is_some_and(|e| !e.is_empty() && e.iter().all(|x| x["status"] == "ok")),
                "{out}"
            );
            eprintln!("plan {surface}: {}", out["plan"]);
        });
        // Every sample is a commit (ruling E-44: a sample that changed nothing measured nothing); it is undone before
        // the next.
        assert!(
            ok(&h, "edit.history.get", json!({}))["position"].as_u64().unwrap_or(0) > 0,
            "the {surface} sample committed nothing"
        );
        ok(&h, "edit.undo", json!({}));
        ms
    };
    let s = |x: f64, y: f64| json!({"x": x, "y": y, "modifiers": []});
    // A peg's bar body dragged 3 frames (its row scrolled to the top; frames 20 to 23).
    let peg_row = index_of(&pegs[10]);
    let bar: Vec<f64> = (0..SAMPLES)
        .map(|_| {
            release("tracks", 1, peg_row * rh, json!([s(19.5 * cw, 11.0), s(21.5 * cw, 11.0), s(22.5 * cw, 11.0)]))
        })
        .collect();
    // A row dragged up one row among the drawings (the root level: a reorder).
    let row_at = index_of(&drawings[50]);
    let reorder: Vec<f64> = (0..SAMPLES)
        .map(|_| {
            release(
                "layers",
                1,
                (row_at - 2.0) * rh,
                json!([s(120.0, 2.0 * rh + 11.0), s(120.0, 2.0 * rh), s(120.0, rh + 2.0)]),
            )
        })
        .collect();
    // The last two exposed cells of a drawing moved 10 frames later, into empty cells (ruling E-44: cells moved onto the
    // same drawing's occupied cells, as the first receipt did, change nothing and commit nothing).
    let cell_row = index_of(&drawings[5]);
    let end = shown[cell_row as usize]["bar"]["end"].as_i64().unwrap();
    let first = end - 60;
    let at = |frame: i64| ((frame - first) as f64 + 0.5) * cw;
    let cells: Vec<f64> = (0..SAMPLES)
        .map(|_| {
            ok(&h, "cell.select", json!({"elementIds": [drawings[5]], "from": end - 1, "to": end}));
            release(
                "tracks",
                first,
                cell_row * rh,
                json!([s(at(end - 1), 16.0), s(at(end + 3), 16.0), s(at(end + 9), 16.0)]),
            )
        })
        .collect();

    let out = json!({
        "kind": "toontown.c51b.kernel-budgets",
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "scene": { "rows": rows, "sceneLength": length, "pegs": pegs.len(), "keysPerPeg": 100, "keyedFrames": "1-400 every 4", "buildSeconds": build_s },
        "timeline.rows.list (120 frames)": summary(rows_list, 20.0),
        "xsheet.exposures.get (60 rows x 200 frames)": summary(exposures, 5.0),
        "release to commit: bar move": summary(bar, 50.0),
        "release to commit: row reorder": summary(reorder, 50.0),
        "release to commit: cell move": summary(cells, 50.0),
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
    ok(&h, "project.close", json!({"onUnsaved": "discard"}));
    if kept.is_none() {
        let _ = if package.is_dir() { std::fs::remove_dir_all(&package) } else { std::fs::remove_file(&package) };
    }
}
