//! Final-round cross-language boundary controls.
use serde_json::{Value, json};
use tt_engine::wire::{from_wire_json, to_wire_json};
use tt_model::value::FrameRange;

#[test]
fn mixed_depth_boundary_writer_output_is_readable() {
    for containers in [126, 127, 128] {
        let mut value = json!(9_007_199_254_740_997u64);
        for i in 0..containers {
            value = if i % 2 == 0 { json!([value]) } else { json!({"level": value}) };
        }
        if containers <= 127 {
            let text = to_wire_json(&value).unwrap();
            assert_eq!(from_wire_json::<Value>(&text).unwrap(), value);
        } else {
            assert!(to_wire_json(&value).is_err());
        }
    }
}

#[test]
fn negative_and_noninteger_range_frames_are_typed_wire_errors() {
    for text in [r#"{"from":-2,"to":1}"#, r#"{"from":1.25,"to":2}"#, r#"{"from":1,"to":-1}"#, r#"{"from":1,"to":2.75}"#]
    {
        assert_eq!(from_wire_json::<FrameRange>(text).unwrap_err().code, "arg.invalid");
    }
}
