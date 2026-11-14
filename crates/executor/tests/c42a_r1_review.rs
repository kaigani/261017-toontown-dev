//! Independent C4.2a round 1 review probes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
fn ready() -> tt_executor::ExecutorHandle {
    let h = fixture();
    ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"", "frame":1}));
    h
}
mod views {
    use super::*;
    #[test]
    fn navigation_is_session_only_and_isolated() {
        let h = ready();
        let r = rev(&h);
        let history = ok(&h, "edit.history.get", json!({}));
        for id in ["drawingView", "cameraView", "topView", "sideView"] {
            ok(&h, "view.zoom.set", json!({"viewId":id,"zoom":2}));
            ok(&h, "view.pan.set", json!({"viewId":id,"centre":{"x":42,"y":-9}}));
            ok(&h, "view.rotation.set", json!({"viewId":id,"degrees":37.5}));
        }
        assert_eq!(rev(&h), r);
        assert_eq!(ok(&h, "edit.history.get", json!({})), history);
        ok(&h, "view.type.set", json!({"viewId":"drawingView","type":"camera"}));
        assert_eq!(ok(&h, "view.transform.get", json!({"viewId":"drawingView"}))["zoom"], 1.0);
        ok(&h, "view.type.set", json!({"viewId":"drawingView","type":"previous"}));
        assert_eq!(ok(&h, "view.transform.get", json!({"viewId":"drawingView"}))["zoom"], 2.0);
        let fresh = ready();
        assert_eq!(ok(&fresh, "view.transform.get", json!({"viewId":"drawingView"}))["zoom"], 1.0);
    }
    #[test]
    fn navigation_rejects_invalid_external_values() {
        let h = ready();
        for z in [0.0, -1.0, 0.0624, 64.01] {
            assert_eq!(err(&h, "view.zoom.set", json!({"viewId":"drawingView","zoom":z})).code, "arg.invalid");
        }
        for p in [json!({"x":1e308,"y":0}), json!({"x":null,"y":0})] {
            assert_eq!(err(&h, "view.pan.set", json!({"viewId":"drawingView","centre":p})).code, "arg.invalid");
        }
        assert_eq!(err(&h, "view.transform.get", json!({"viewId":"missing"})).code, "view.notFound");
        for z in [0.0625, 64.0] {
            ok(&h, "view.zoom.set", json!({"viewId":"drawingView","zoom":z}));
        }
    }
}
mod mapping {
    use super::*;
    fn near(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-5, "{a} != {b}");
    }
    fn apply(m: &serde_json::Value, x: f64, y: f64) -> (f64, f64) {
        let f = |i: usize| m[i].as_f64().unwrap();
        (f(0) * x + f(2) * y + f(4), f(1) * x + f(3) * y + f(5))
    }
    #[test]
    fn independently_derived_mapping_grid() {
        let h = ready();
        for (width, height) in [(1920.0, 1080.0), (811.0, 423.0)] {
            h.report_shell(tt_executor::host::ShellReport {
                view_sizes: Some(std::collections::BTreeMap::from([(
                    "drawingView".into(),
                    tt_executor::shell::Size { width, height },
                )])),
                ..Default::default()
            });
            for zoom in [0.0625, 1.0, 64.0] {
                for degrees in [0.0_f64, 90.0, 37.5] {
                    for (cx, cy) in [(0.0, 0.0), (1e6, -1e6)] {
                        ok(&h, "view.zoom.set", json!({"viewId":"drawingView","zoom":zoom}));
                        ok(&h, "view.rotation.set", json!({"viewId":"drawingView","degrees":degrees}));
                        ok(&h, "view.pan.set", json!({"viewId":"drawingView","centre":{"x":cx,"y":cy}}));
                        let m = ok(&h, "draw.viewMapping.get", json!({"view":"drawingView"}));
                        let (sin, cos) = degrees.to_radians().sin_cos();
                        for (dx, dy) in [(0.0, 0.0), (17.5, -23.25), (-100.0, 200.0)] {
                            let (x, y) = (cx + dx, cy + dy);
                            // y inversion, then clockwise screen rotation, scale, viewport translation.
                            let expected = (
                                width / 2.0 + zoom * (cos * dx + sin * dy),
                                height / 2.0 + zoom * (sin * dx - cos * dy),
                            );
                            let p = ok(
                                &h,
                                "view.coordinates.convert",
                                json!({"viewId":"drawingView","from":"drawing","to":"view","point":{"x":x,"y":y}}),
                            );
                            near(p["point"]["x"].as_f64().unwrap(), expected.0);
                            near(p["point"]["y"].as_f64().unwrap(), expected.1);
                            let a = apply(&m["drawingToView"], x, y);
                            near(a.0, expected.0);
                            near(a.1, expected.1);
                            let b = apply(&m["viewToDrawing"], a.0, a.1);
                            near(b.0, x);
                            near(b.1, y);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn rotated_colour_pick_and_hit_test_agree_with_independent_point() {
        let h = ready();
        ok(&h, "draw.rectangle.add", json!({"from":{"x":300,"y":300},"to":{"x":400,"y":360},"layer":"new"}));
        ok(&h, "colour.paint.apply", json!({"points":[{"x":350,"y":330}]}));
        ok(&h, "view.zoom.set", json!({"viewId":"drawingView","zoom":2}));
        ok(&h, "view.rotation.set", json!({"viewId":"drawingView","degrees":37.5}));
        ok(&h, "view.pan.set", json!({"viewId":"drawingView","centre":{"x":350,"y":330}}));
        let drawing = ok(&h, "colour.pick", json!({"point":{"x":350,"y":330},"space":"drawing"}));
        let view = ok(&h, "colour.pick", json!({"point":{"x":960,"y":540},"space":"view","view":"drawingView"}));
        assert_eq!(drawing, view);
        let hits = ok(&h, "inspect.hitTest", json!({"space":"view","point":{"x":960,"y":540},"view":"drawingView"}));
        assert!(!hits["hits"].as_array().unwrap().is_empty());
    }
}
mod screenshot {
    use super::*;
    use std::time::{Duration, Instant};
    fn decode(v: &serde_json::Value) -> (u16, u16, Vec<u8>) {
        let bytes = tt_persist::b64::decode(v["png"].as_str().unwrap()).unwrap();
        assert_eq!(v["sha256"], tt_persist::fsx::sha256(&bytes));
        let (w, h, p) = tt_render::decode_png(&bytes, 4096).unwrap();
        assert_eq!(v["width"], w);
        assert_eq!(v["height"], h);
        (w, h, p)
    }
    #[test]
    fn png_determinism_crop_limits_and_known_pixel() {
        let h = ready();
        ok(&h, "draw.rectangle.add", json!({"from":{"x":-40,"y":-30},"to":{"x":40,"y":30},"layer":"new"}));
        ok(&h, "colour.paint.apply", json!({"points":[{"x":0,"y":0}]}));
        let a = ok(&h, "inspect.screenshot", json!({}));
        let b = ok(&h, "inspect.screenshot", json!({}));
        assert_eq!(a["png"], b["png"]);
        let (w, _, pixels) = decode(&a);
        let crop = ok(&h, "inspect.screenshot", json!({"region":{"x":940,"y":520,"w":40,"h":40}}));
        let (_, _, cropped) = decode(&crop);
        for row in 0..40usize {
            assert_eq!(
                &cropped[row * 160..(row + 1) * 160],
                &pixels[((520 + row) * usize::from(w) + 940) * 4..((520 + row) * usize::from(w) + 980) * 4]
            );
        }
        assert_ne!(&pixels[(540 * 1920 + 960) * 4..(540 * 1920 + 960) * 4 + 4], &[240, 240, 240, 255]);
        ok(&h, "view.active.set", json!({"viewId":"drawingView"}));
        let rendered = ok(&h, "render.frame.run", json!({"composition":"activeView","background":"#F0F0F0FF"}));
        let data = rendered["data"].as_str().or_else(|| rendered["png"].as_str()).unwrap();
        let (_, _, rp) = tt_render::decode_png(&tt_persist::b64::decode(data).unwrap(), 4096).unwrap();
        assert_eq!(pixels, rp);
        ok(&h, "view.pan.set", json!({"viewId":"drawingView","by":{"x":100,"y":0}}));
        let after_pan = ok(&h, "inspect.screenshot", json!({}));
        assert_ne!(a["png"], after_pan["png"]);
        ok(&h, "draw.rectangle.add", json!({"from":{"x":100,"y":100},"to":{"x":200,"y":200},"layer":"new"}));
        assert_ne!(after_pan["png"], ok(&h, "inspect.screenshot", json!({}))["png"]);
        let start = Instant::now();
        assert_eq!(
            err(&h, "inspect.screenshot", json!({"region":{"x":0,"y":0,"w":2049,"h":1}})).code,
            "render.inlineTooLarge"
        );
        assert!(start.elapsed() < Duration::from_millis(500));
        for target in ["dialogs", "properties", "missing"] {
            assert_eq!(err(&h, "inspect.screenshot", json!({"target":target})).code, "view.notFound");
        }
        for region in [
            json!({"x":0,"y":0,"w":0,"h":10}),
            json!({"x":0,"y":0,"w":-1,"h":10}),
            json!({"x":null,"y":0,"w":10,"h":10}),
        ] {
            assert_eq!(err(&h, "inspect.screenshot", json!({"region":region})).code, "arg.invalid");
        }
    }
    #[test]
    fn screenshot_does_not_block_user_command() {
        let h = ready();
        let mut latencies = Vec::new();
        for _ in 0..3 {
            let shot = h.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                tx.send(()).unwrap();
                ok(&shot, "inspect.screenshot", json!({}));
            });
            rx.recv().unwrap();
            // Allow the capture request to enter the queue, without sleeping in a timing bound.
            let entered = Instant::now();
            while entered.elapsed() < Duration::from_millis(10) {
                std::thread::yield_now();
            }
            let start = Instant::now();
            ok(&h, "view.transform.get", json!({"viewId":"drawingView"}));
            latencies.push(start.elapsed());
            worker.join().unwrap();
        }
        eprintln!("user query during screenshot: {latencies:?}");
        // Three times the 20 ms hold budget allows scheduling/IPC overhead on slow CI.
        assert!(
            *latencies.iter().min().unwrap() < Duration::from_millis(60),
            "screenshot blocks the document thread: {latencies:?}"
        );
    }
}

mod part_b_tools {
    use super::*;
    #[test]
    fn text_ranges_match_the_text_panel() {
        let h = ready();
        let mut accepted = Vec::new();
        for (key, value) in [("leftMargin", -1), ("rightMargin", 1001), ("indent", -100), ("lineSpacing", 1001)] {
            let before = ok(&h, "tool.options.get", json!({"toolId":"text"}));
            let result = run(&h, "tool.options.set", json!({"toolId":"text", "options":{key:value}}));
            if result.is_ok() {
                accepted.push(key);
            } else {
                assert_eq!(ok(&h, "tool.options.get", json!({"toolId":"text"})), before);
            }
        }
        assert!(accepted.is_empty(), "out-of-spec text values accepted: {accepted:?}");
    }
    #[test]
    fn all_tools_and_atomic_options_do_not_edit_document() {
        let h = ready();
        let r = rev(&h);
        let history = ok(&h, "edit.history.get", json!({}));
        let tools = ok(&h, "tool.list", json!({}));
        for tool in tools["tools"].as_array().unwrap() {
            let id = &tool["toolId"];
            ok(&h, "tool.active.set", json!({"toolId":id}));
            let rx = h.subscribe();
            ok(&h, "tool.active.set", json!({"toolId":id}));
            assert!(!rx.try_iter().any(|e| e.name == "tool.active.changed"));
            let before = ok(&h, "tool.options.get", json!({"toolId":id}));
            assert_eq!(
                err(&h, "tool.options.set", json!({"toolId":id,"options":{"nonexistent":true}})).code,
                "tool.notApplicable"
            );
            assert_eq!(ok(&h, "tool.options.get", json!({"toolId":id})), before);
        }
        assert_eq!(rev(&h), r);
        assert_eq!(ok(&h, "edit.history.get", json!({})), history);
    }
}

mod part_b_pens {
    use super::*;
    #[test]
    fn defaults_mutations_history_and_project_round_trip() {
        let root = std::env::temp_dir().join(format!("tt-review-pens-{}", tt_executor::random_token(8)));
        let h = super::part_b_persistence::over(&root);
        ok(&h, "project.create", json!({"name":"Review"}));
        let pens = ok(&h, "tool.pen.list", json!({}));
        let list = pens["pens"].as_array().unwrap();
        assert_eq!(list.len(), 8);
        for (name, max, min) in [
            ("Pen 1", 2.0, 0.5),
            ("Pen 2", 4.0, 1.0),
            ("Pen 3", 8.0, 2.0),
            ("Pen 4", 16.0, 4.0),
            ("Pen 5", 32.0, 8.0),
            ("Eraser 1", 10.0, 10.0),
            ("Eraser 2", 20.0, 20.0),
            ("Eraser 3", 40.0, 40.0),
        ] {
            let p = list.iter().find(|p| p["name"] == name).unwrap();
            assert_eq!(p["maxSize"].as_f64(), Some(max));
            assert_eq!(p["minSize"].as_f64(), Some(min));
            assert_eq!(p["smoothness"], 3);
        }
        let r = rev(&h);
        let id = ok(&h, "tool.pen.create", json!({"name":"Review pen"}))["penId"].clone();
        assert!(rev(&h) > r);
        for (command, params, label) in [
            ("tool.pen.rename", json!({"penId":id,"name":"Renamed"}), "Rename Pen"),
            ("tool.pen.set", json!({"penId":id,"minSize":90}), "Set Pen Style"),
            ("tool.pen.delete", json!({"penId":id}), "Delete Pen"),
        ] {
            let r = rev(&h);
            ok(&h, command, params);
            assert!(rev(&h) > r);
            assert!(ok(&h, "edit.history.get", json!({})).to_string().contains(label));
            ok(&h, "edit.undo", json!({}));
            ok(&h, "edit.redo", json!({}));
        }
        ok(&h, "edit.undo", json!({}));
        for value in [json!(-1), json!(101), json!(1e100)] {
            let before = ok(&h, "tool.pen.list", json!({}));
            assert!(
                ["arg.invalid", "tool.valueOutOfRange"]
                    .contains(&err(&h, "tool.pen.set", json!({"penId":id,"maxSize":value})).code.as_str())
            );
            assert_eq!(ok(&h, "tool.pen.list", json!({})), before);
        }
        assert_eq!(err(&h, "tool.pen.create", json!({"name":"Pen 1"})).code, "pen.nameExists");
        assert!(run(&h, "tool.pen.create", json!({"name":""})).error.is_some());
        assert!(run(&h, "tool.pen.create", json!({"name":"x".repeat(10000)})).error.is_some());
        let r = rev(&h);
        ok(&h, "tool.pen.select", json!({"penId":id}));
        assert_eq!(rev(&h), r);
        let rx = h.subscribe();
        ok(&h, "tool.pen.select", json!({"penId":id}));
        assert!(!rx.try_iter().any(|e| e.name == "tool.pen.changed"));
        let before = ok(&h, "tool.pen.list", json!({}));
        let path = root.join("Review.animpkg");
        super::part_b_persistence::job(&h, "project.saveAs", json!({"path":path}));
        ok(&h, "project.close", json!({"onUnsaved":"discard"}));
        drop(h);
        let h = super::part_b_persistence::over(&root);
        super::part_b_persistence::job(&h, "project.open", json!({"path":path}));
        assert_eq!(ok(&h, "tool.pen.list", json!({})), before);
        ok(&h, "project.close", json!({"onUnsaved":"discard"}));
        drop(h);
        std::fs::remove_dir_all(root).unwrap();
    }
}
mod part_b_persistence {
    use super::*;
    use serde_json::Value;
    pub fn over(root: &std::path::Path) -> tt_executor::ExecutorHandle {
        tt_executor::ExecutorHandle::spawn_with_options(tt_executor::ExecutorOptions {
            persist: tt_executor::PersistOptions {
                user_dirs: Some(tt_executor::UserDirs::under(root)),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap()
    }
    pub fn job(h: &tt_executor::ExecutorHandle, id: &str, params: Value) {
        let started = ok(h, id, params);
        let result = ok(h, "job.wait", json!({"jobId":started["jobId"],"timeoutMs":60000}));
        let record = result.get("record").unwrap_or(&result);
        assert_eq!(record["state"], "succeeded", "{result}");
    }
}
