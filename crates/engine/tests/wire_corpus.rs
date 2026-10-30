//! The shared hostile-JSON corpus (C11-R1-17): the engine's text reader accepts and rejects exactly the texts the
//! front end's `parseWireJson` does (`desktop/src/ipc/wireCorpus.test.ts` runs the same file).
#![allow(clippy::unwrap_used, clippy::panic, reason = "test")]
use serde_json::Value;
use tt_engine::wire::{from_wire_json, to_wire_json};

fn corpus() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/wire/hostile-json.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn text(case: &Value) -> String {
    match case.get("nest") {
        Some(n) => {
            let count = usize::try_from(n["count"].as_u64().unwrap()).unwrap();
            format!(
                "{}{}{}",
                n["open"].as_str().unwrap().repeat(count),
                n["inner"].as_str().unwrap(),
                n["close"].as_str().unwrap().repeat(count)
            )
        }
        None => case["text"].as_str().unwrap().to_owned(),
    }
}

#[test]
fn the_corpus_depth_is_the_engine_limit() {
    assert_eq!(corpus()["maxDepth"], tt_model::json_text::MAX_TEXT_DEPTH);
}

#[test]
fn every_hostile_text_is_rejected_with_a_typed_error() {
    let c = corpus();
    let cases = c["reject"].as_array().unwrap();
    assert!(cases.len() > 50);
    for case in cases {
        match from_wire_json::<Value>(&text(case)) {
            Ok(v) => panic!("accepted {}: {v}", case["why"]),
            Err(e) => assert_eq!(e.code, "arg.invalid", "{}", case["why"]),
        }
    }
}

#[test]
fn every_valid_text_is_accepted_and_written_canonically() {
    let c = corpus();
    for case in c["accept"].as_array().unwrap() {
        let v: Value = from_wire_json(&text(case)).unwrap_or_else(|e| panic!("rejected {}: {e:?}", case["why"]));
        assert_eq!(to_wire_json(&v).unwrap(), case["canonical"].as_str().unwrap(), "{}", case["why"]);
    }
}

#[test]
fn invalid_utf8_bodies_are_rejected() {
    let c = corpus();
    for case in c["bytes"].as_array().unwrap() {
        let bytes: Vec<u8> =
            case["bytes"].as_array().unwrap().iter().map(|b| u8::try_from(b.as_u64().unwrap()).unwrap()).collect();
        assert!(std::str::from_utf8(&bytes).is_err(), "{}", case["why"]);
    }
}
