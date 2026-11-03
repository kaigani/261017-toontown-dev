//! M3 C3.1 round 1 (builder): extreme timing and frame parameters through the generic MCP tool are typed refusals
//! that leave the document worker usable (C31-R1-09, C31-R1-10).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod support;

use serde_json::{Value, json};

const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";
const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";

fn call(c: &mut support::Client, id: &str, p: Value) -> Value {
    c.call("run_command", json!({ "id": id, "params": p }))["structuredContent"].clone()
}

#[test]
fn extreme_parameters_refuse_typed_and_the_worker_answers() {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut opts = support::opts(&dir);
    opts.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = support::Client::connect(&h, opts, json!({}));
    let rev = h.snapshot().unwrap().unwrap().document_rev();
    let cases = [
        ("element.timing.set", json!({ "elementIds": [PEG], "start": i64::MAX })),
        ("element.timing.set", json!({ "elementIds": [PEG], "startDelta": i64::MIN })),
        ("element.timing.set", json!({ "elementIds": [PEG], "duration": i64::MAX })),
        ("element.timing.set", json!({ "elementIds": [PEG], "loops": i64::MAX })),
        ("element.timing.set", json!({ "elementIds": [PEG], "start": 100_000, "duration": 2 })),
        ("element.timing.set", json!({ "elementIds": [PEG], "startDelta": -100_000 })),
        (
            "element.mediaLength.set",
            json!({ "elementId": "scene:\"Intro\"/element:\"Reference\"", "length": i64::MAX }),
        ),
        ("cell.label.set", json!({ "elementIds": [BOB], "from": 1, "to": 1, "step": i64::MAX })),
        ("cell.fill.apply", json!({ "elementIds": [BOB], "frame": 1, "toFrame": 10_000 })),
        ("cell.fill.apply", json!({ "elementIds": [BOB], "frame": 1, "toFrame": i64::MAX })),
        ("element.childExposure.apply", json!({ "pegId": PEG, "toFrame": 10_000 })),
        ("cell.exposure.set", json!({ "elementIds": [BOB], "from": i64::MIN, "to": 1, "length": 2 })),
    ];
    for (id, p) in cases {
        let r = call(&mut c, id, p.clone());
        assert_eq!(r["status"], "error", "{id} {p}: {r}");
        let code = r["error"]["code"].as_str().unwrap_or_default().to_owned();
        assert!(
            matches!(
                code.as_str(),
                "arg.invalid" | "cell.frameOutOfRange" | "doc.limitExceeded" | "cell.drawingNotFound"
            ),
            "{id} {p}: {r}"
        );
        assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), rev, "{id}: nothing changed");
        let state = call(&mut c, "doc.state.get", json!({}));
        assert_eq!(state["status"], "ok", "the worker still answers after {id}");
    }
    // The bounds themselves are accepted.
    let ok = call(&mut c, "cell.fill.apply", json!({ "elementIds": [BOB], "frame": 1, "toFrame": 9999 }));
    assert_eq!(ok["status"], "ok", "{ok}");
    c.close().unwrap();
}
