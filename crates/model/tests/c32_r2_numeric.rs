//! RFC numeric equality at exact-integer storage boundaries.
use serde_json::{Value, json};
use tt_model::change::json_equal;
#[test]
fn large_numbers_nested_types_and_nonfinite() {
    for n in [9007199254740991u64, 9007199254740992, 9007199254740993, u64::MAX] {
        assert!(json_equal(&json!(n), &json!(n)));
        assert!(!json_equal(&json!(n), &json!(n - 1)));
        let f = n as f64;
        assert_eq!(json_equal(&json!(n), &json!(f)), f as u128 == u128::from(n));
    }
    assert!(json_equal(&json!({"b":[100,-0.0],"a":1}), &json!({"a":1.0,"b":[1e2,0]})));
    for v in [Value::Null, json!(false), json!("0"), json!([]), json!({})] {
        assert!(!json_equal(&v, &json!(0)));
    }
    for f in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(serde_json::Number::from_f64(f).is_none());
    }
}
