//! 100k-stroke drawing invalidation, independently assembled with a new seed.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review scale regression")]
use serde_json::json;
use std::time::Duration;
use tt_executor::{ExecutorConfig, ExecutorHandle, Invocation};
use tt_model::{document::Document, entities::drawing::Drawing, fixture::sample_parts, object::from_json};

#[test]
fn hundred_thousand_stroke_change_reports_drawing_in_envelope_event_and_diff() {
    let mut parts = sample_parts(0xc122_9461).unwrap();
    let at = parts.drawings.iter().position(|d| d.name() == "1").unwrap();
    let mut d = serde_json::to_value(&parts.drawings[at]).unwrap();
    let template = d["nodes"][0]["strokes"][0].clone();
    d["nodes"][0]["strokes"] = json!(
        (0..100_000)
            .map(|i| {
                let mut s = template.clone();
                s["id"] = json!(format!("r2s{i}"));
                s
            })
            .collect::<Vec<_>>()
    );
    parts.drawings[at] = from_json::<Drawing>(&d).unwrap();
    let drawing = parts.drawings[at].id().as_str().to_owned();
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(120) }).unwrap();
    assert!(h.open_document(Document::from_parts(parts).unwrap(), None, false).is_ok());
    let rx = h.subscribe();
    let e = h.invoke(Invocation::user(
        "doc.object.patch",
        json!({"targets":[format!("{drawing}/r2s73481")],"props":{"meta":{"r2":"large-content"}}}),
    ));
    assert!(e.is_ok(), "{e:?}");
    assert!(e.changes.iter().any(|c| c.id == drawing && c.rev == 2));
    h.snapshot().unwrap();
    let events: Vec<_> = rx.try_iter().collect();
    assert!(events.iter().any(|e| e.name == "doc.changed"
        && e.payload["changes"].as_array().unwrap().iter().any(|c| c["id"] == drawing && c["rev"] == 2)));
    let e = h.invoke(Invocation::user("doc.diff", json!({"sinceRev":1})));
    assert!(e.is_ok(), "{e:?}");
    assert!(
        e.result.unwrap()["changes"].as_array().unwrap().iter().any(|c| c["id"] == drawing),
        "doc.diff must carry the drawing invalidation already present in envelope and event"
    );
}
