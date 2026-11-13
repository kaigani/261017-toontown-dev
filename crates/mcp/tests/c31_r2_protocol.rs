//! Round 2 MCP extrema: every numeric member of the C3.1 timing families, not only the original crash.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod support;
use serde_json::{Value, json};
const B: &str = "scene:\"Intro\"/element:\"Bob\"";
const P: &str = "scene:\"Intro\"/element:\"Bob-P\"";
fn call(c: &mut support::Client, id: &str, p: Value) -> Value {
    c.call("run_command", json!({"id":id,"params":p}))["structuredContent"].clone()
}
#[test]
fn r10_all_timing_numeric_extrema_are_typed_and_worker_survives() {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(0x0031_2009).unwrap(), None, false).is_ok());
    let mut opts = support::opts(&dir);
    opts.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = support::Client::connect(&h, opts, json!({}));
    let cases = [
        (
            "element.timing.set",
            json!({"elementIds":[P],"start":1,"duration":24,"loops":1}),
            vec!["start", "startDelta", "duration", "loops"],
        ),
        (
            "element.mediaLength.set",
            json!({"elementId":"scene:\"Intro\"/element:\"Reference\"","length":3}),
            vec!["length"],
        ),
        ("element.childExposure.apply", json!({"pegId":P,"toFrame":20}), vec!["toFrame"]),
        ("element.drawing.create", json!({"elementId":B,"exposeFrom":1,"exposeTo":2}), vec!["exposeFrom", "exposeTo"]),
        ("cell.exposure.add", json!({"elementIds":[B],"from":1,"to":2,"frames":1}), vec!["from", "to", "frames"]),
        ("cell.exposure.remove", json!({"elementIds":[B],"from":1,"to":2,"frames":1}), vec!["from", "to", "frames"]),
        ("cell.exposure.set", json!({"elementIds":[B],"from":1,"to":2,"length":2}), vec!["from", "to", "length"]),
        ("cell.fill.apply", json!({"elementIds":[B],"frame":1,"toFrame":20}), vec!["frame", "toFrame"]),
        ("cell.blank.add", json!({"elementIds":[B],"frame":1,"count":2}), vec!["frame", "count"]),
        ("cell.blank.remove", json!({"elementIds":[B],"frame":7,"count":1}), vec!["frame", "count"]),
        ("cell.range.add", json!({"elementIds":[B],"frame":1,"count":2}), vec!["frame", "count"]),
        ("cell.range.delete", json!({"elementIds":[B],"from":1,"to":2}), vec!["from", "to"]),
        (
            "cell.range.move",
            json!({"sourceElementIds":[B],"from":1,"to":2,"targetFrame":20}),
            vec!["from", "to", "targetFrame"],
        ),
        ("cell.label.set", json!({"elementIds":[B],"from":1,"to":2,"step":1}), vec!["from", "to", "step"]),
        ("cell.drawing.duplicate", json!({"elementId":B,"from":1,"to":2}), vec!["from", "to"]),
        ("cell.cycle.create", json!({"elementId":B,"from":1,"to":2,"cycles":2}), vec!["from", "to", "cycles"]),
        (
            "cell.columnate.apply",
            json!({"elementId":B,"frame":1,"firstDrawing":"1","lastDrawing":"10","exposeEach":2,"exposeLast":1,"loops":2,"totalFrames":20,"loopMode":"continuous"}),
            vec!["frame", "exposeEach", "exposeLast", "loops", "totalFrames"],
        ),
        ("cell.note.set", json!({"elementId":B,"frame":1,"text":"extrema731"}), vec!["frame"]),
        ("cell.clipboard.set", json!({"elementIds":[B],"from":1,"to":2}), vec!["from", "to"]),
        ("cell.select", json!({"elementIds":[B],"from":1,"to":2}), vec!["from", "to"]),
        ("xsheet.exposures.get", json!({"elementIds":[B],"from":1,"to":2}), vec!["from", "to"]),
    ];
    for (id, base, fields) in cases {
        for field in fields {
            for extreme in [i64::MIN, i64::MAX] {
                let mut p = base.clone();
                p[field] = json!(extreme);
                if id == "element.timing.set" && field == "startDelta" {
                    p.as_object_mut().unwrap().remove("start");
                }
                let rev = h.snapshot().unwrap().unwrap().document_rev();
                let r = call(&mut c, id, p.clone());
                assert_eq!(r["status"], "error", "{id} {p}: {r}");
                let code = r["error"]["code"].as_str().unwrap();
                assert!(!matches!(code, "internal" | "command.timeout"), "{id} {p}: {r}");
                assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), rev);
                assert_eq!(call(&mut c, "doc.state.get", json!({}))["status"], "ok");
            }
        }
    }
    // Large products whose individual members are otherwise legal also refuse without stopping the worker.
    for p in [json!({"elementIds":[P],"loops":100000}), json!({"elementIds":[P],"startDelta":99999,"duration":2})] {
        assert_eq!(call(&mut c, "element.timing.set", p)["status"], "error");
        assert_eq!(call(&mut c, "doc.state.get", json!({}))["status"], "ok");
    }
}
