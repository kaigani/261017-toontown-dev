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

mod part_b_aids {
    use super::part_b_persistence::{job, over};
    use super::*;
    use serde_json::Value;
    fn root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("tt-review-aids-{}", tt_executor::random_token(8)))
    }
    fn file(root: &std::path::Path, version: &str, data: Value) {
        let dirs = tt_executor::UserDirs::under(root);
        std::fs::create_dir_all(&dirs.settings).unwrap();
        std::fs::write(
            dirs.settings.join("drawing.json"),
            json!({"format":"toontown-settings","kind":"drawing","formatVersion":version,"data":data}).to_string(),
        )
        .unwrap();
    }
    #[test]
    fn stale_aid_values_fall_back_individually() {
        let root = root();
        file(
            &root,
            "1.0",
            json!({"activeTool":"ellipse","aids":{"strokes":true,"grid":{"visible":true,"type":"obsolete","overlay":true},"onionSkin":{"previous":99,"next":2},"background":{"colour":"bad","useSceneColour":true}}}),
        );
        let h = over(&root);
        assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
        let state = ok(&h, "drawAid.state.get", json!({}));
        drop(h);
        std::fs::remove_dir_all(root).unwrap();
        assert_eq!(
            state["grid"],
            json!({"visible":true,"type":"normal","overlay":true}),
            "stale grid type alone falls back"
        );
        assert_eq!(state["onionSkin"]["previous"], 1);
        assert_eq!(state["onionSkin"]["next"], 2);
        assert_eq!(state["background"]["colour"], "#F0F0F0FF");
        assert_eq!(state["background"]["useSceneColour"], true);
        assert_eq!(state["strokes"], true);
    }
    #[test]
    fn unsupported_version_and_whole_shape_are_told_once_and_kept() {
        let root = root();
        let mut copies = Vec::new();
        for (version, data, reason) in [
            ("2.0", json!({}), "unsupportedVersion"),
            ("1.0", json!([]), "wrongShape"),
            ("1.0", json!({"aids":false}), "wrongShape"),
        ] {
            file(&root, version, data);
            let h = over(&root);
            let rx = h.subscribe();
            ok(&h, "app.state.get", json!({}));
            ok(&h, "app.state.get", json!({}));
            let messages: Vec<_> = rx
                .try_iter()
                .filter(|e| e.name == "ui.message.shown" && e.payload["messageId"] == "prefs.corrupt")
                .collect();
            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0].payload["details"]["reason"], reason);
            let path = messages[0].payload["details"]["keptAt"].as_str().unwrap().to_owned();
            assert!(!copies.contains(&path));
            copies.push(path);
            drop(h);
        }
        assert!(copies.iter().all(|p| std::path::Path::new(p).exists()));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn user_aids_and_options_survive_restart_without_document_content() {
        let root = root();
        let h = over(&root);
        ok(&h, "project.create", json!({"name":"Aids"}));
        let before = h.snapshot().unwrap().unwrap();
        let revision = rev(&h);
        for (id, p) in [
            ("drawAid.grid.set", json!({"visible":true,"type":"field16","overlay":true})),
            ("drawAid.onionSkin.set", json!({"enabled":true,"previous":3,"next":2})),
            (
                "drawAid.onionSkinStyle.set",
                json!({"outline":true,"transparency":false,"colourShading":false,"previousColour":"#123456","nextColour":"#654321"}),
            ),
            ("drawAid.autoLightTable.set", json!({"enabled":true})),
            ("drawAid.lightTableShade.set", json!({"auto":false,"static":false})),
            ("drawAid.staticLightTablePanel.set", json!({"visible":true})),
            ("drawAid.strokes.set", json!({"visible":true})),
            ("drawAid.currentOnTop.set", json!({"enabled":true})),
            (
                "drawAid.selectionDisplay.set",
                json!({"outlineOnSelection":true,"boxHighlighting":false,"keepSelectionVisible":true}),
            ),
            ("drawAid.background.set", json!({"colour":"#123456","useSceneColour":true})),
            ("tool.active.set", json!({"toolId":"polyline"})),
            ("tool.polyline.options.set", json!({"snap":false,"snapRadius":22})),
            ("tool.symmetry.set", json!({"mode":"mirror","angle":30})),
            ("tool.placeBehind.set", json!({"enabled":true})),
            ("tool.drawTopLayer.set", json!({"enabled":true})),
            ("tool.snapToContour.set", json!({"enabled":true})),
            ("tool.tablet.set", json!({"ignoreTablet":true,"portraitCorrection":true,"compatibilityMode":true})),
        ] {
            ok(&h, id, p.clone());
            let rx = h.subscribe();
            ok(&h, id, p);
            assert!(
                !rx.try_iter()
                    .any(|e| ["drawAid.changed", "tool.active.changed", "tool.options.changed"]
                        .contains(&e.name.as_str())),
                "{id}"
            );
        }
        assert_eq!(rev(&h), revision);
        let after = h.snapshot().unwrap().unwrap();
        for key in before.entry_keys() {
            assert_eq!(before.entry_canonical(&key).unwrap(), after.entry_canonical(&key).unwrap());
        }
        let quality = json!({"display.antiAliasing":false,"display.smoothBitmaps":false,"display.smoothToolOverlays":false,"display.fastPencilRendering":true,"display.bitmapPreviewMaxSize":512,"display.cacheMemoryPercent":40});
        ok(&h, "prefs.set", json!({"values":quality}));
        let aids = ok(&h, "drawAid.state.get", json!({}));
        let tools = ok(&h, "tool.list", json!({}));
        let settings = ok(&h, "tool.settings.get", json!({}));
        let path = root.join("Aids.animpkg");
        job(&h, "project.saveAs", json!({"path":path}));
        ok(&h, "project.close", json!({}));
        drop(h);
        let h = over(&root);
        job(&h, "project.open", json!({"path":path}));
        assert_eq!(ok(&h, "drawAid.state.get", json!({})), aids);
        let restored = ok(&h, "tool.list", json!({}));
        assert_eq!(restored["activeToolId"], tools["activeToolId"]);
        for (a, b) in restored["tools"].as_array().unwrap().iter().zip(tools["tools"].as_array().unwrap()) {
            let mut x = a["options"].clone();
            let mut y = b["options"].clone();
            if a["toolId"] == "polyline" {
                assert_eq!(x["snapRadius"].as_f64(), y["snapRadius"].as_f64());
                x.as_object_mut().unwrap().remove("snapRadius");
                y.as_object_mut().unwrap().remove("snapRadius");
            }
            assert_eq!(x, y, "{}", a["toolId"]);
        }
        assert_eq!(ok(&h, "tool.settings.get", json!({})), settings);
        let keys: Vec<_> = quality.as_object().unwrap().keys().collect();
        assert_eq!(ok(&h, "prefs.get", json!({"keys":keys}))["values"], quality);
        ok(&h, "project.close", json!({}));
        drop(h);
        std::fs::remove_dir_all(root).unwrap();
    }
}

mod part_b_settings_write {
    use super::*;
    #[test]
    fn drawing_write_failure_is_reported_once() {
        let root = std::env::temp_dir().join(format!("tt-review-write-{}", tt_executor::random_token(8)));
        let dirs = tt_executor::UserDirs::under(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&dirs.settings, b"file blocks directory").unwrap();
        let h = super::part_b_persistence::over(&root);
        assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
        let rx = h.subscribe();
        ok(&h, "drawAid.strokes.set", json!({"visible":true}));
        ok(&h, "drawAid.strokes.set", json!({"visible":false}));
        let messages: Vec<_> = rx
            .try_iter()
            .filter(|e| {
                e.name == "ui.message.shown"
                    && e.payload["messageId"] == "prefs.saveFailed"
                    && e.payload["details"]["kind"] == "drawing"
            })
            .collect();
        assert_eq!(messages.len(), 1);
        drop(h);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn static_table_cap_and_unknown_ids_are_typed_and_atomic() {
        let h = ready();
        for _ in 0..256 {
            ok(&h, "drawAid.staticLightTable.add", json!({}));
        }
        let before = ok(&h, "drawAid.staticLightTable.list", json!({}));
        assert_eq!(err(&h, "drawAid.staticLightTable.add", json!({})).code, "limit.exceeded");
        assert_eq!(
            err(&h, "drawAid.staticLightTable.set", json!({"entryIds":["lt1","absent"],"visible":false})).code,
            "ref.notFound"
        );
        assert_eq!(ok(&h, "drawAid.staticLightTable.list", json!({})), before);
    }
}

mod part_b_busy {
    use super::*;
    #[test]
    fn agent_tool_changes_respect_user_gesture_and_dialog() {
        let mut accepted = Vec::new();
        for kind in ["gesture", "dialog"] {
            let h = ready();
            h.set_wait_for_user(std::time::Duration::ZERO);
            h.report_shell(serde_json::from_value(json!({"modal":{"kind":kind,"title":"User activity"}})).unwrap());
            let before = ok(&h, "tool.active.get", json!({}));
            let result = h.invoke(tt_executor::Invocation {
                command: "tool.active.set".into(),
                params: json!({"toolId":"brush"}),
                options: Default::default(),
                origin: agent(&[tt_registry::Scope::Read, tt_registry::Scope::Ui]),
            });
            if result.is_ok() {
                accepted.push(kind);
            } else {
                assert!(["ui.busy", "ui.modalOpen"].contains(&result.error.unwrap().code.as_str()));
                assert_eq!(ok(&h, "tool.active.get", json!({})), before);
            }
        }
        assert!(accepted.is_empty(), "agent changed tool during user activity: {accepted:?}");
    }
}

mod part_c_equivalence {
    use super::*;
    use serde_json::Value;
    fn canonical(h: &tt_executor::ExecutorHandle) -> Vec<String> {
        let d = h.snapshot().unwrap().unwrap();
        d.entry_keys()
            .iter()
            .map(|k| tt_model::canonical::to_canonical_string(&d.entry_raw_json(k).unwrap()).unwrap())
            .collect()
    }
    #[test]
    fn freehand_samples_and_direct_content() {
        // 02 section 13.3: pressure is per sample, including duplicate points and release.
        for tool in ["brush", "pencil", "stroke"] {
            for n in [2, 7, 100] {
                let (a, b) = (ready(), ready());
                let events: Vec<Value> = (0..n).map(|i| json!({"type":if i==0 {"down"} else if i==n-1 {"up"} else {"move"},"x":1000.0+(if n == 2 { i } else { i/2 }) as f64,"y":500.0,"pressure":(i%5) as f64/4.0,"dt":i%3})).collect();
                let points: Vec<Value> = events
                    .iter()
                    .map(|e| json!({"x":e["x"].as_f64().unwrap()-960.0,"y":40.0,"pressure":e["pressure"]}))
                    .collect();
                let env = a.pointer(tt_executor::Origin::User, json!({"view":"drawing","tool":tool,"events":events}));
                assert!(env.is_ok(), "{env:?}");
                let c = &env.result.as_ref().unwrap()["commands"][1];
                assert_eq!(c["commandId"], "draw.stroke.add");
                assert_eq!(c["params"]["points"], json!(points));
                assert_eq!(c["params"]["kind"], if tool == "stroke" { "invisible" } else { tool });
                assert_eq!(c["params"]["fit"], true);
                // Explicit resolved styling is replayed; this comparison does not independently validate styling.
                ok(&b, "draw.stroke.add", c["params"].clone());
                assert_eq!(canonical(&a), canonical(&b));
                assert_eq!(
                    ok(&a, "edit.history.get", json!({}))["entries"][0]["label"],
                    ok(&b, "edit.history.get", json!({}))["entries"][0]["label"]
                );
            }
        }
    }
}

mod part_c_pointer {
    use super::*;
    use serde_json::Value;
    use std::time::Instant;
    fn request(n: usize) -> Value {
        let events: Vec<Value>=(0..n).map(|i| json!({"type":if i==0 {"down"} else if i==n-1 {"up"} else {"move"},"x":1000.0+i as f64/10.0,"y":500.0+(i%7) as f64,"pressure":0.5,"dt":2})).collect();
        json!({"view":"drawing","tool":"brush","events":events})
    }
    #[test]
    fn limits_sequence_and_incomplete_gesture_are_atomic() {
        let h = ready();
        let before = rev(&h);
        for events in [
            json!([{"type":"up","x":1,"y":1}]),
            json!([{"type":"down","x":1,"y":1},{"type":"down","x":2,"y":2}]),
            json!([{"type":"down","x":1,"y":1,"pressure":1.01}]),
            json!([{"type":"down","x":10000001,"y":1}]),
            json!([{"type":"down","x":null,"y":1}]),
        ] {
            let e = h.pointer(tt_executor::Origin::User, json!({"view":"drawing","events":events}));
            assert_eq!(e.error.unwrap().code, "arg.invalid");
            assert_eq!(rev(&h), before);
        }
        assert_eq!(h.pointer(tt_executor::Origin::User, request(10001)).error.unwrap().code, "limit.exceeded");
        let history = ok(&h, "edit.history.get", json!({}));
        let sel = ok(&h, "draw.selection.get", json!({}));
        let e=h.pointer(tt_executor::Origin::User,json!({"view":"drawing","tool":"pencil","events":[{"type":"move","x":2,"y":2},{"type":"down","x":3,"y":3},{"type":"move","x":4,"y":4}]}));
        assert!(e.is_ok());
        assert_eq!(rev(&h), before);
        assert_eq!(ok(&h, "edit.history.get", json!({})), history);
        assert_eq!(ok(&h, "draw.selection.get", json!({})), sel);
        assert_eq!(ok(&h, "tool.active.get", json!({}))["toolId"], "pencil");
    }
    #[test]
    fn full_cap_query_latency_receipt() {
        let h = ready();
        let r = request(10000);
        let worker_h = h.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            tx.send(()).unwrap();
            let t = Instant::now();
            let e = worker_h.pointer(tt_executor::Origin::User, r);
            (e, t.elapsed())
        });
        rx.recv().unwrap();
        let admission = Instant::now();
        while admission.elapsed() < std::time::Duration::from_millis(10) {
            std::thread::yield_now();
        }
        let t = Instant::now();
        // Admission is not instrumented: this is an observed competing query, not a precise hold measurement.
        let q = run(&h, "view.transform.get", json!({"viewId":"drawingView"}));
        let latency = t.elapsed();
        let (e, total) = worker.join().unwrap();
        eprintln!("10000-event pointer total {total:?}, competing query {latency:?}");
        assert!(e.is_ok(), "{e:?}");
        assert!(q.is_ok());
        assert!(
            latency < std::time::Duration::from_millis(60),
            "plan section 6: 20 ms hold, generous 60 ms query bound; observed {latency:?}"
        );
    }
}

mod part_c_availability {
    use super::*;
    #[test]
    fn constructed_view_states_agree_with_availability() {
        // O-24: all parameters present; refusal is caused by the view type.
        let mut mismatches = Vec::new();
        for view_type in ["camera", "top", "side"] {
            let h = ready();
            ok(&h, "view.type.set", json!({"viewId":"drawingView","type":view_type}));
            let p = json!({"viewId":"drawingView","from":"view","to":"drawing","point":{"x":960,"y":540}});
            let a = h.availability(vec![("view.coordinates.convert".into(), p.as_object().unwrap().clone())]).unwrap();
            let result = run(&h, "view.coordinates.convert", p);
            if a[0].enabled != result.is_ok() {
                mismatches.push(format!("{view_type}: availability {:?}, handler {result:?}", a[0]));
            }
        }
        assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    }
}

mod part_d_equivalence {
    use super::*;
    use serde_json::Value;
    use tt_executor::ExecutorHandle;
    fn point(x: f64, y: f64) -> Value {
        json!({"x":x-960.0,"y":540.0-y})
    }
    fn events(ps: &[(f64, f64)], mods: &[&str]) -> Value {
        json!(ps.iter().enumerate().map(|(i,(x,y))|json!({"type":if i==0 {"down"} else if i+1==ps.len(){"up"}else{"move"},"x":x,"y":y,"modifiers":mods})).collect::<Vec<_>>())
    }
    fn canonical(h: &ExecutorHandle) -> Vec<String> {
        let d = h.snapshot().unwrap().unwrap();
        d.entry_keys()
            .iter()
            .map(|k| tt_model::canonical::to_canonical_string(&d.entry_raw_json(k).unwrap()).unwrap())
            .collect()
    }
    fn hist(h: &ExecutorHandle) -> Vec<Value> {
        ok(h, "edit.history.get", json!({}))["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| json!({"label":e["label"],"rev":e["rev"]}))
            .collect()
    }
    fn colour(h: &ExecutorHandle) -> Value {
        json!(
            h.snapshot()
                .unwrap()
                .unwrap()
                .palettes()
                .find_map(|p| p.swatches().first().map(|s| s.colour_id().as_str().to_owned()))
                .unwrap()
        )
    }
    fn style(h: &ExecutorHandle, mut p: Value) -> Value {
        let pens = ok(h, "tool.pen.list", json!({}));
        let pen = &pens["pens"].as_array().unwrap().iter().find(|p| p["type"] == "brush").unwrap();
        p["penId"] = pen["penId"].clone();
        p["thickness"] = pen["maxSize"].clone();
        p["colourId"] = colour(h);
        p["layer"] = json!("auto");
        p["symmetry"] = json!(true);
        p
    }
    fn check(a: &ExecutorHandle, b: &ExecutorHandle, tool: &str, ev: Value, id: &str, p: Value) {
        let ha = hist(a);
        let hb = hist(b);
        assert_eq!(ha, hb);
        let e = a.pointer(tt_executor::Origin::User, json!({"view":"drawing","tool":tool,"events":ev}));
        let direct = run(b, id, p.clone());
        assert_eq!(e.status, direct.status, "{tool}: pointer {e:?}, direct {direct:?}");
        if e.is_ok() {
            let c = &e.result.as_ref().unwrap()["commands"][1];
            assert_eq!(c["commandId"], id);
            assert_eq!(c["params"], p, "{tool}");
        } else {
            assert_eq!(e.error.as_ref().unwrap().code, direct.error.as_ref().unwrap().code);
        }
        assert_eq!(canonical(a), canonical(b), "bytes and ids {tool}");
        assert_eq!(hist(a), hist(b), "history {tool}");
        assert_eq!(ok(a, "draw.selection.get", json!({})), ok(b, "draw.selection.get", json!({})), "selection {tool}");
    }
    fn shape(tool: &str) {
        for mods in [vec![], vec!["shift"], vec!["alt"], vec!["shift", "alt"]] {
            for end in [(1080.0, 460.0), (1000.0, 500.0)] {
                let (a, b) = (ready(), ready());
                let mut p = json!({"from":point(1000.0,500.0),"to":point(end.0,end.1)});
                if tool == "line" {
                    p["constrainAngle"] = json!(mods.contains(&"shift"));
                    p["continuePrevious"] = json!(mods.contains(&"alt"));
                    if mods.contains(&"alt") {
                        p.as_object_mut().unwrap().remove("from");
                    }
                } else {
                    p[if tool == "rectangle" { "square" } else { "circle" }] = json!(mods.contains(&"shift"));
                    p["fromCentre"] = json!(mods.contains(&"alt"));
                }
                let p = style(&b, p);
                check(&a, &b, tool, events(&[(1000.0, 500.0), end], &mods), &format!("draw.{tool}.add"), p);
            }
        }
    }

    #[test]
    fn brush_and_rectangle_edges() {
        for tool in ["brush", "rectangle"] {
            for ps in [
                vec![(-100.0, -100.0), (-100.0, -100.0), (-80.0, -60.0)],
                vec![(10_000_000.0, 500.0), (10_000_000.0, 500.0)],
            ] {
                let (a, b) = (ready(), ready());
                let p = if tool == "rectangle" {
                    style(
                        &b,
                        json!({"from":point(ps[0].0,ps[0].1),"to":point(ps.last().unwrap().0,ps.last().unwrap().1),"square":false,"fromCentre":false}),
                    )
                } else {
                    let points: Vec<Value> = ps
                        .iter()
                        .map(|(x, y)| {
                            let mut p = point(*x, *y);
                            p["pressure"] = json!(1.0);
                            p
                        })
                        .collect();
                    let mut p = style(&b, json!({"kind":"brush","points":points,"fit":true}));
                    p.as_object_mut().unwrap().remove("thickness");
                    p
                };
                check(
                    &a,
                    &b,
                    tool,
                    events(&ps, &[]),
                    if tool == "rectangle" { "draw.rectangle.add" } else { "draw.stroke.add" },
                    p,
                );
            }
        }
    }
    #[test]
    fn temporary_overrides_restore_tool() {
        for (mods, kind, id, p) in [
            (
                vec!["ctrl"],
                "mouse",
                "draw.selection.set",
                json!({"by":"point","point":{"x":40.0,"y":40.0},"mode":"replace"}),
            ),
            (
                vec!["alt"],
                "mouse",
                "colour.pick",
                json!({"point":{"x":40.0,"y":40.0},"space":"drawing","view":"drawingView"}),
            ),
            (vec![], "penEraser", "draw.erase.apply", json!({})),
        ] {
            let (a, b) = (ready(), ready());
            ok(&a, "tool.active.set", json!({"toolId":"brush"}));
            let mut p = p;
            if kind == "penEraser" {
                let pens = ok(&b, "tool.pen.list", json!({"type":"eraser"}));
                p = json!({"path":[{"x":40.0,"y":40.0,"pressure":1.0},{"x":40.0,"y":40.0,"pressure":1.0}],"penId":pens["pens"][0]["penId"]});
            }
            let e = a.pointer(
                tt_executor::Origin::User,
                json!({"view":"drawing","pointerType":kind,"events":events(&[(1000.0,500.0),(1000.0,500.0)],&mods)}),
            );
            let direct = run(&b, id, p.clone());
            assert_eq!(e.status, direct.status);
            if e.is_ok() {
                let c = &e.result.as_ref().unwrap()["commands"][0];
                assert_eq!(c["commandId"], id);
                assert_eq!(c["params"], p);
            }
            assert_eq!(canonical(&a), canonical(&b));
            assert_eq!(hist(&a), hist(&b));
            assert_eq!(ok(&a, "tool.active.get", json!({}))["toolId"], "brush");
        }
    }
    fn painted(h: &ExecutorHandle) {
        ok(h, "draw.rectangle.add", json!({"from":{"x":300,"y":300},"to":{"x":400,"y":360}}));
        let c = colour(h);
        ok(h, "colour.paint.apply", json!({"points":[{"x":350,"y":330}],"colourId":c}));
    }
    #[test]
    fn eraser() {
        let (a, b) = (ready(), ready());
        painted(&a);
        painted(&b);
        let pens = ok(&b, "tool.pen.list", json!({"type":"eraser"}));
        let pen = pens["pens"][0]["penId"].clone();
        check(
            &a,
            &b,
            "eraser",
            events(&[(1310.0, 250.0), (1310.0, 170.0)], &[]),
            "draw.erase.apply",
            json!({"path":[{"x":350.0,"y":290.0,"pressure":1.0},{"x":350.0,"y":370.0,"pressure":1.0}],"penId":pen}),
        );
    }
    fn cut(tool: &str, alternate: bool) {
        let (a, b) = (ready(), ready());
        painted(&a);
        painted(&b);
        let ps = if tool == "cutter" || alternate {
            vec![(1280.0, 230.0), (1340.0, 230.0), (1340.0, 190.0), (1280.0, 190.0), (1280.0, 230.0)]
        } else {
            vec![(1280.0, 230.0), (1340.0, 190.0)]
        };
        let p = if ps.len() > 2 {
            json!({"polygon":ps.iter().map(|(x,y)|point(*x,*y)).collect::<Vec<_>>()})
        } else {
            json!({"rect":{"x":320.0,"y":310.0,"width":60.0,"height":40.0}})
        };
        check(&a, &b, tool, events(&ps, if alternate { &["alt"] } else { &[] }), "draw.cut.apply", p);
    }
    #[test]
    fn cutter() {
        cut("cutter", false);
    }
    #[test]
    fn scissor() {
        cut("scissor", false);
        cut("scissor", true);
    }
    fn painting(tool: &str, id: &str) {
        for drag in [false, true] {
            let (a, b) = (ready(), ready());
            painted(&a);
            painted(&b);
            let ps = if drag {
                vec![(1300.0, 220.0), (1320.0, 220.0), (1320.0, 200.0), (1300.0, 200.0), (1300.0, 220.0)]
            } else {
                vec![(1310.0, 210.0), (1310.0, 210.0)]
            };
            let mut p = json!({"space":"drawing","view":"drawingView","gapClosing":"off"});
            let key = if !drag || tool == "unpaint" {
                "points"
            } else if tool == "paint" {
                "path"
            } else {
                "lasso"
            };
            p[key] = if key == "points" {
                json!([point(ps[0].0, ps[0].1)])
            } else {
                json!(ps.iter().map(|(x, y)| point(*x, *y)).collect::<Vec<_>>())
            };
            if tool != "unpaint" {
                p["colourId"] = colour(&b);
            }
            check(&a, &b, tool, events(&ps, &[]), id, p);
        }
    }
    #[test]
    fn paint() {
        painting("paint", "colour.paint.apply");
    }
    #[test]
    fn unpaint() {
        painting("unpaint", "colour.paint.remove");
    }
    #[test]
    fn paint_unpainted() {
        painting("paintUnpainted", "colour.paintUnpainted.apply");
    }
    #[test]
    fn close_gap() {
        let (a, b) = (ready(), ready());
        check(
            &a,
            &b,
            "closeGap",
            events(&[(1500.0, 400.0), (1520.0, 400.0)], &[]),
            "colour.gap.close",
            json!({"from":point(1500.0,400.0),"to":point(1520.0,400.0),"space":"drawing","view":"drawingView"}),
        );
    }
    #[test]
    fn grabber() {
        let (a, b) = (ready(), ready());
        check(
            &a,
            &b,
            "grabber",
            events(&[(1000.0, 500.0), (1080.0, 460.0)], &[]),
            "view.pan.set",
            json!({"viewId":"drawingView","by":{"x":80.0,"y":-40.0}}),
        );
        assert_eq!(
            ok(&a, "view.transform.get", json!({"viewId":"drawingView"})),
            ok(&b, "view.transform.get", json!({"viewId":"drawingView"}))
        );
    }
    #[test]
    fn zoom_click() {
        for alt in [false, true] {
            let (a, b) = (ready(), ready());
            check(
                &a,
                &b,
                "zoom",
                events(&[(1000.0, 500.0), (1000.0, 500.0)], if alt { &["alt"] } else { &[] }),
                "view.zoom.set",
                json!({"viewId":"drawingView","steps":if alt{-1}else{1},"anchor":{"x":1000.0,"y":500.0}}),
            );
            assert_eq!(
                ok(&a, "view.transform.get", json!({"viewId":"drawingView"})),
                ok(&b, "view.transform.get", json!({"viewId":"drawingView"}))
            );
        }
    }
    #[test]
    fn zoom_marquee_fits_region() {
        let h = ready();
        let e = h.pointer(
            tt_executor::Origin::User,
            json!({"view":"drawing","tool":"zoom","events":events(&[(1000.0,500.0),(1200.0,380.0)],&[])}),
        );
        assert!(e.is_ok(), "{e:?}");
        let t = ok(&h, "view.transform.get", json!({"viewId":"drawingView"}));
        assert_eq!(t["zoom"], 9.0, "02 section 7.5 line 658: rectangle zooms to fit; min(1920/200,1080/120)=9");
    }
    fn selected(h: &ExecutorHandle) {
        ok(h, "draw.rectangle.add", json!({"from":{"x":300,"y":300},"to":{"x":400,"y":360}}));
        ok(h, "draw.selection.set", json!({"by":"point","point":{"x":300,"y":330},"mode":"replace"}));
    }
    #[test]
    fn select() {
        for (ps, mods, p) in [
            (
                vec![(1260.0, 210.0), (1260.0, 210.0)],
                vec![],
                json!({"by":"point","point":{"x":300.0,"y":330.0},"mode":"replace"}),
            ),
            (
                vec![(1500.0, 500.0), (1500.0, 500.0)],
                vec![],
                json!({"by":"point","point":{"x":540.0,"y":40.0},"mode":"replace"}),
            ),
            (
                vec![(1260.0, 210.0), (1260.0, 210.0)],
                vec!["shift"],
                json!({"by":"point","point":{"x":300.0,"y":330.0},"mode":"add"}),
            ),
            (
                vec![(1200.0, 270.0), (1400.0, 150.0)],
                vec![],
                json!({"by":"rect","rect":{"x":240.0,"y":270.0,"width":200.0,"height":120.0},"rectRule":"inside","mode":"replace"}),
            ),
            (
                vec![(1200.0, 270.0), (1400.0, 270.0), (1400.0, 150.0), (1200.0, 150.0), (1200.0, 270.0)],
                vec!["alt"],
                json!({"by":"lasso","polygon":[{"x":240.0,"y":270.0},{"x":440.0,"y":270.0},{"x":440.0,"y":390.0},{"x":240.0,"y":390.0},{"x":240.0,"y":270.0}],"rectRule":"inside","mode":"replace"}),
            ),
        ] {
            let (a, b) = (ready(), ready());
            selected(&a);
            selected(&b);
            check(&a, &b, "select", events(&ps, &mods), "draw.selection.set", p);
        }
        for copy in [false, true] {
            let (a, b) = (ready(), ready());
            selected(&a);
            selected(&b);
            check(
                &a,
                &b,
                "select",
                events(&[(1310.0, 210.0), (1330.0, 200.0)], if copy { &["alt"] } else { &[] }),
                "draw.transform.apply",
                json!({"translate":{"x":20.0,"y":10.0},"copy":copy}),
            );
        }
        for delta in [2.99, 3.0, 3.01] {
            let (a, b) = (ready(), ready());
            let click = delta <= 3.0;
            check(
                &a,
                &b,
                "select",
                events(&[(1500.0, 500.0), (1500.0 + delta, 500.0)], &[]),
                "draw.selection.set",
                if click {
                    json!({"by":"point","point":point(1500.0,500.0),"mode":"replace"})
                } else {
                    json!({"by":"rect","rect":{"x":540.0,"y":40.0,"width":(1500.0+delta-1500.0),"height":0.0},"rectRule":"inside","mode":"replace"})
                },
            );
        }
        let (a, b) = (ready(), ready());
        let d = ok(&b, "draw.drawing.get", json!({}));
        let pivot = &d["pivot"];
        let (x, y) = (pivot["x"].as_f64().unwrap() + 960.0, 540.0 - pivot["y"].as_f64().unwrap());
        check(
            &a,
            &b,
            "select",
            events(&[(x, y), (x + 20.0, y - 10.0)], &[]),
            "draw.pivot.set",
            json!({"point":point(x+20.0,y-10.0)}),
        );
    }
    #[test]
    fn rectangle() {
        shape("rectangle");
    }
    #[test]
    fn ellipse() {
        shape("ellipse");
    }
    #[test]
    fn line() {
        shape("line");
    }
    #[test]
    fn stroke() {
        let (a, b) = (ready(), ready());
        let mut p = style(
            &b,
            json!({"kind":"invisible","points":[{"x":40.0,"y":40.0,"pressure":1.0},{"x":120.0,"y":80.0,"pressure":1.0}],"fit":true}),
        );
        p.as_object_mut().unwrap().remove("thickness");
        check(&a, &b, "stroke", events(&[(1000.0, 500.0), (1080.0, 460.0)], &[]), "draw.stroke.add", p);
    }
    #[test]
    fn polyline() {
        for closed in [false, true] {
            let (a, b) = (ready(), ready());
            let mut ev = Vec::new();
            for (x, y) in [(1000.0, 500.0), (1080.0, 500.0), (1080.0, 460.0)] {
                ev.push(json!({"type":"down","x":x,"y":y}));
                ev.push(json!({"type":"up","x":x,"y":y}));
            }
            if closed {
                ev.push(json!({"type":"down","x":1000.0,"y":500.0}));
                ev.push(json!({"type":"up","x":1000.0,"y":500.0}));
            } else {
                ev.push(json!({"type":"doubleClick","x":1080.0,"y":460.0}));
            }
            let p = style(
                &b,
                json!({"points":[point(1000.0,500.0),point(1080.0,500.0),point(1080.0,460.0)],"closed":closed,"snap":true,"snapRadius":8.0}),
            );
            check(&a, &b, "polyline", json!(ev), "draw.polyline.add", p);
        }
    }
}
