//! Permanent wire regressions for C1.1; no product edits.
use serde_json::{Value, json};
use tt_engine::wire::{from_wire_json, to_wire_json};

#[test]
fn opaque_json_integer_outside_u64_is_not_silently_rounded() {
    let text = r#"{"com.example.tool":{"n":-18446744073709551615}}"#;
    let v: Value = from_wire_json(text).unwrap();
    assert_eq!(to_wire_json(&v).unwrap(), text, "opaque extension integer loses precision in Rust");
}

#[test]
fn nested_duplicate_keys_are_rejected_in_requests() {
    assert!(
        from_wire_json::<Value>(r#"{"meta":{"x":1,"x":2}}"#).is_err(),
        "duplicate keys must not silently erase a value"
    );
}

#[test]
fn typed_nonfinite_wire_numbers_are_rejected_not_changed_to_null() {
    assert!(to_wire_json(&f64::NAN).is_err(), "10 §1.1 NaN is invalid; serde_json silently writes null");
    assert!(to_wire_json(&f64::INFINITY).is_err());
}

#[test]
fn supported_integer_boundaries_and_utf8_controls() {
    for v in [json!(u64::MAX), json!(i64::MIN), json!(9007199254740993u64)] {
        assert_eq!(from_wire_json::<Value>(&to_wire_json(&v).unwrap()).unwrap(), v);
    }
    assert!(from_wire_json::<Value>("\"\\ud800\"").is_err());
    assert!(from_wire_json::<Value>("{} trailing").is_err());
    assert!(serde_json::from_slice::<Value>(b"\"\xff\"").is_err());
    let huge = "é\u{2028}😀".repeat(300_000);
    assert_eq!(from_wire_json::<String>(&to_wire_json(&huge).unwrap()).unwrap(), huge);
}
