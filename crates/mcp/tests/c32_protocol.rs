//! M3 C3.2 (builder): extreme parameters of the animation commands through the generic MCP tool are typed refusals that
//! leave the document worker usable (the C31-R1-10 rule, readiness items 4, 8, 12).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod support;

use serde_json::{Value, json};

const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";
const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const CAMERA: &str = "scene:\"Intro\"/element:\"Camera\"";

fn call(c: &mut support::Client, id: &str, p: Value) -> Value {
    c.call("run_command", json!({ "id": id, "params": p }))["structuredContent"].clone()
}

#[test]
fn extreme_animation_parameters_refuse_typed_and_the_worker_answers() {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut opts = support::opts(&dir);
    opts.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = support::Client::connect(&h, opts, json!({}));
    let rev = h.snapshot().unwrap().unwrap().document_rev();
    let big = 1e308;
    let cases = [
        ("anim.transform.set", json!({ "nodeIds": [BOB], "target": "static", "position": { "x": big } })),
        ("anim.transform.set", json!({ "nodeIds": [BOB], "target": "static", "rotation": big })),
        ("anim.transform.set", json!({ "nodeIds": [BOB], "target": "static", "scale": { "x": big } })),
        ("anim.transform.set", json!({ "nodeIds": [BOB], "target": "static", "scale": { "x": 0 } })),
        ("anim.transform.set", json!({ "nodeIds": [BOB], "target": "static", "skew": 89.5 })),
        ("anim.transform.set", json!({ "nodeIds": [BOB], "frame": i64::MAX, "rotation": 1 })),
        ("anim.transform.set", json!({ "nodeIds": [BOB], "frame": i64::MIN, "rotation": 1 })),
        ("anim.transform.set", json!({ "nodeIds": [BOB], "frame": 3, "position": { "x": big } })),
        (
            "anim.path.set",
            json!({ "nodeId": BOB, "points": [{ "kind": "key", "frame": 1, "x": big, "y": 0, "z": 0 }] }),
        ),
        (
            "anim.path.set",
            json!({ "nodeId": BOB, "points": [{ "kind": "key", "frame": 1e300, "x": 0, "y": 0, "z": 0 }] }),
        ),
        ("anim.path.point.add", json!({ "nodeId": BOB, "kind": "key", "frame": 1e300 })),
        (
            "anim.path.point.add",
            json!({ "nodeId": BOB, "kind": "control", "at": { "segment": 4_000_000_000_u32, "s": 0.5 } }),
        ),
        ("anim.function.key.add", json!({ "nodeId": BOB, "channel": "rotation", "frame": 3, "value": big })),
        ("anim.function.key.add", json!({ "nodeId": BOB, "channel": "rotation", "frame": -1e300 })),
        ("anim.function.key.set", json!({ "nodeId": BOB, "channel": "rotation", "frame": 12, "newFrame": 1e300 })),
        ("anim.function.sample", json!({ "nodeId": BOB, "channel": "rotation", "from": 1, "to": big, "step": 1 })),
        ("anim.function.sample", json!({ "nodeId": BOB, "channel": "rotation", "from": 1, "to": 5, "step": 1e-300 })),
        ("anim.path.sample", json!({ "nodeId": BOB, "from": i64::MIN, "to": i64::MAX })),
        ("anim.evaluate", json!({ "nodeIds": [BOB], "frames": [big] })),
        ("anim.evaluate", json!({ "nodeIds": [BOB], "frames": [-big] })),
        ("anim.projection.get", json!({ "frame": i64::MAX })),
        ("anim.loop.set", json!({ "nodeId": PEG, "loops": i64::MAX })),
        ("anim.loop.set", json!({ "nodeId": PEG, "loops": 10_000 })),
        ("anim.camera.set", json!({ "cameraId": CAMERA, "fov": big })),
        ("anim.camera.set", json!({ "cameraId": CAMERA, "roll": big })),
        ("anim.camera.set", json!({ "cameraId": CAMERA, "position": { "z": big } })),
        ("anim.key.move", json!({ "nodeId": BOB, "fromFrame": 12, "toFrame": i64::MAX })),
        ("anim.segment.set", json!({ "nodeIds": [BOB], "frame": i64::MIN, "constant": true })),
        ("element.timing.set", json!({ "elementIds": [PEG], "duration": 100_001, "keyframes": "keep" })),
    ];
    for (id, p) in cases {
        let r = call(&mut c, id, p.clone());
        assert_eq!(r["status"], "error", "{id} {p}: {r}");
        let code = r["error"]["code"].as_str().unwrap_or_default().to_owned();
        assert!(
            matches!(
                code.as_str(),
                "arg.invalid"
                    | "anim.valueOutOfRange"
                    | "anim.frameInvalid"
                    | "anim.frameNotInteger"
                    | "limit.exceeded"
                    | "cell.frameOutOfRange"
            ),
            "{id} {p}: {r}"
        );
        assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), rev, "{id}: nothing changed");
        let state = call(&mut c, "doc.state.get", json!({}));
        assert_eq!(state["status"], "ok", "the worker still answers after {id} {p}");
    }
    // The limits themselves work.
    let ok = call(
        &mut c,
        "anim.transform.set",
        json!({ "nodeIds": [BOB], "target": "static", "position": { "x": 1e6 }, "scale": { "x": 0.001, "y": 1e6 }, "skew": 89 }),
    );
    assert_eq!(ok["status"], "ok", "{ok}");
    // A deep chain of large scales overflows only as a typed refusal, never as a JSON null. Scale 1e6 with a 45° turn and
    // an 89° skew grows each level by about 4e7, so the 46 levels the tree-depth limit allows pass 1e308.
    let mut parent = PEG.to_owned();
    for i in 0..46 {
        let made =
            call(&mut c, "element.create", json!({ "type": "peg", "name": format!("Tall{i}"), "parentId": parent }));
        assert_eq!(made["status"], "ok", "{made}");
        parent = made["result"]["elementIds"][0].as_str().unwrap().to_owned();
        let s = call(
            &mut c,
            "anim.transform.set",
            json!({ "nodeIds": [parent], "target": "static", "scale": { "x": 1e6, "y": 1e6 }, "rotation": 45, "skew": 89 }),
        );
        assert_eq!(s["status"], "ok", "{s}");
    }
    let e = call(&mut c, "anim.evaluate", json!({ "nodeIds": [parent], "frames": [1] }));
    assert_eq!(e["error"]["code"], "anim.valueOutOfRange", "{e}");
    assert_eq!(e["error"]["details"]["reason"], "nonFinite", "{e}");
    assert_eq!(call(&mut c, "doc.state.get", json!({}))["status"], "ok");
    c.close().unwrap();
}
