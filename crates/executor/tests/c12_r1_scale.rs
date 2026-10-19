//! Independent pathological-document and large-input sweep; automatically discovered by cargo test.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs,
    reason = "review scale evidence"
)]
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};
use tt_executor::{ExecutorConfig, ExecutorHandle};
use tt_model::document::Document;
use tt_model::entities::drawing::{Drawing, DrawingBank};
use tt_model::object::from_json;

fn large(strokes: bool) -> Document {
    let mut parts = tt_model::fixture::sample_parts(991).unwrap();
    if strokes {
        let index = parts.drawings.iter().position(|d| d.name() == "1").unwrap();
        let mut d = serde_json::to_value(&parts.drawings[index]).unwrap();
        let template = d["nodes"][0]["strokes"][0].clone();
        d["nodes"][0]["strokes"] = json!(
            (0..100_000)
                .map(|n| {
                    let mut s = template.clone();
                    s["id"] = json!(format!("review{n}"));
                    s
                })
                .collect::<Vec<_>>()
        );
        parts.drawings[index] = from_json::<Drawing>(&d).unwrap();
    } else {
        let mut generator = tt_model::id::IdGenerator::seeded(0xc12_10000, 1_760_000_000_000);
        let mut bank_ids = parts.banks[0].drawings().ids().to_vec();
        let mut template = serde_json::to_value(&parts.drawings[0]).unwrap();
        template["nodes"] = json!([]);
        template["colourRecovery"] = json!([]);
        while parts.drawings.len() < 10_000 {
            let id = generator.next(tt_model::ObjectType::Drawing).unwrap();
            template["id"] = json!(id);
            template["name"] = json!(format!("review-{}", parts.drawings.len()));
            parts.drawings.push(from_json::<Drawing>(&template).unwrap());
            bank_ids.push(id);
        }
        parts.banks[0] = DrawingBank::new(parts.banks[0].id().clone(), bank_ids);
    }
    Document::from_parts(parts).unwrap()
}

#[test]
fn every_command_rejects_huge_malformed_input_on_large_documents() {
    let mut evidence = Vec::new();
    for strokes in [true, false] {
        let name = if strokes { "100000 strokes" } else { "10000 drawings" };
        let t = Instant::now();
        let doc = large(strokes);
        let assemble = t.elapsed().as_secs_f64();
        let count = doc.object_count();
        let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(60) }).unwrap();
        assert!(h.open_document(doc, None, false).is_ok());
        let input = json!({"__review_unknown":"x".repeat(1024*1024)});
        let mut maximum = 0.0_f64;
        let sweep = Instant::now();
        for e in tt_executor::registry().unwrap().entries() {
            let t = Instant::now();
            let env = run(&h, e.id(), input.clone());
            maximum = maximum.max(t.elapsed().as_secs_f64());
            assert!(env.is_error(), "{} accepted malformed input on {name}", e.id());
            assert_ne!(env.error.unwrap().code, "internal", "{} panicked/stopped", e.id());
        }
        let sweep_seconds = sweep.elapsed().as_secs_f64();
        assert_eq!(rev(&h), 1);
        let t = Instant::now();
        for n in 0..20 {
            ok(&h, "checkpoint.create", json!({"name":format!("scale-{n}")}));
        }
        let checkpoint_seconds = t.elapsed().as_secs_f64();
        assert_eq!(ok(&h, "checkpoint.list", json!({})).as_array().unwrap().len(), 20);
        let t = Instant::now();
        let result = ok(&h, "doc.object.find", json!({"type":if strokes {"stroke"}else{"drawing"},"limit":100}));
        let query_seconds = t.elapsed().as_secs_f64();
        assert_eq!(result["items"].as_array().unwrap().len(), 100);
        evidence.push(json!({"fixture":name,"objects":count,"assembleSeconds":assemble,"malformedInvocations":796,"inputBytes":input.to_string().len(),"sweepSeconds":sweep_seconds,"maximumCallSeconds":maximum,"create20CheckpointsSeconds":checkpoint_seconds,"find100Seconds":query_seconds,"documentRev":rev(&h)}));
    }
    let receipt = json!({"profile":if cfg!(debug_assertions){"debug"}else{"release"},"cases":evidence});
    if let Ok(dir) = std::env::var("TT_C12_REVIEW_ARTIFACT_DIR") {
        std::fs::write(
            std::path::Path::new(&dir).join("large-documents.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
    }
    println!("{receipt}");
}
