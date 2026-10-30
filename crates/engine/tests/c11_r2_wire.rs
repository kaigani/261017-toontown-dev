//! Round-2 wire boundaries and optional Rust -> TypeScript -> Rust exchange.
use serde_json::{Value, json};
use tt_engine::wire::{from_wire_json, to_wire_json};

#[test]
fn integer_edges_are_lossless_or_typed_errors() {
    let accepted = [
        "18446744073709551614",
        "18446744073709551615",
        "18446744073709551616",
        "-9223372036854775808",
        "9007199254740991",
        "9007199254740992",
        "9007199254740993",
        "-9007199254740993",
    ];
    for token in accepted {
        let text = format!("{{\"meta\":{{\"n\":{token}}}}}");
        let v: Value = from_wire_json(&text).unwrap();
        assert_eq!(to_wire_json(&v).unwrap(), text);
    }
    for token in ["18446744073709551617", "-9223372036854775809", "-18446744073709551615"] {
        assert_eq!(from_wire_json::<Value>(token).unwrap_err().code, "arg.invalid");
    }
}

#[test]
fn writer_never_emits_text_its_reader_rejects_at_depth_boundary() {
    let v = (0..128).fold(json!(1), |v, _| json!([v]));
    if let Ok(text) = to_wire_json(&v) {
        assert!(from_wire_json::<Value>(&text).is_ok(), "wire writer emits scalar at depth 129, reader caps at 128");
    }
}

#[test]
fn wire_depth_one_under_and_one_over() {
    for arrays in [126, 127, 128, 129] {
        let text = format!("{}1{}", "[".repeat(arrays), "]".repeat(arrays));
        assert_eq!(from_wire_json::<Value>(&text).is_ok(), arrays < 128, "arrays={arrays}");
    }
}

#[test]
fn nested_nonfinite_f32_f64_maps_and_sequences_are_errors() {
    #[derive(serde::Serialize)]
    struct Values {
        nested: Vec<Option<f64>>,
        small: f32,
    }
    for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(to_wire_json(&Values { nested: vec![Some(n)], small: 0.0 }).unwrap_err().code, "arg.invalid");
    }
    assert!(to_wire_json(&Values { nested: vec![], small: f32::NAN }).is_err());
}

#[test]
fn fresh_hostile_text_cases_reject_and_escaped_unicode_roundtrips() {
    for text in [r#"{"meta":{"x":1,"\u0078":2}}"#, r#"{"\ud83d":1}"#, r#"["\udfff"]"#, "[1e9999]", "[1e+]"] {
        assert_eq!(from_wire_json::<Value>(text).unwrap_err().code, "arg.invalid", "{text}");
    }
    let v: Value = from_wire_json(r#"{"\ud83d\ude00":"\u0065\u0301","__proto__":{"x":1}}"#).unwrap();
    assert_eq!(v["😀"], "e\u{301}");
    assert_eq!(from_wire_json::<Value>(&to_wire_json(&v).unwrap()).unwrap(), v);
}

#[test]
fn cross_language_exchange() {
    let input = json!({"values":[u64::MAX-1,u64::MAX,9_007_199_254_740_991_u64,9_007_199_254_740_992_u64,9_007_199_254_740_993_u64],"negative":i64::MIN,"exactDouble":18446744073709551616.0_f64});
    let text = to_wire_json(&input).unwrap();
    if let Ok(path) = std::env::var("TT_R2_WIRE_EXPORT") {
        std::fs::write(path, &text).unwrap();
    }
    if let Ok(path) = std::env::var("TT_R2_WIRE_IMPORT") {
        let output = std::fs::read_to_string(path).unwrap();
        assert_eq!(from_wire_json::<Value>(&output).unwrap(), input);
        assert_eq!(output, text);
    }
    assert_eq!(from_wire_json::<Value>(&text).unwrap(), input);
}
