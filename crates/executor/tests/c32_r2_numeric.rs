//! RFC test rollback at the exact-integer boundary.
mod common;
use common::*;
use serde_json::json;
#[test]
fn exact_large_integer_test_and_failed_test_rollback() {
    let h = fixture();
    let b = "scene:\"Intro\"/element:\"Bob\"";
    ok(
        &h,
        "doc.object.patch",
        json!({"targets":[b],"ops":[{"op":"add","path":"/meta","value":{"n":18446744073709551615u64,"nested":[100,-0.0]}}]}),
    );
    let before = get(&h, b);
    let revision = rev(&h);
    ok(
        &h,
        "doc.object.patch",
        json!({"targets":[b],"ops":[{"op":"test","path":"/meta","value":{"nested":[1e2,0],"n":18446744073709551615u64}}]}),
    );
    assert_eq!(rev(&h), revision);
    let e = err(
        &h,
        "doc.object.patch",
        json!({"targets":[b],"ops":[{"op":"replace","path":"/note","value":"must roll back"},{"op":"test","path":"/meta/n","value":18446744073709551616_f64}]}),
    );
    assert_eq!(e.code, "doc.patchTestFailed");
    assert_eq!(get(&h, b), before);
    assert_eq!(rev(&h), revision);
}
