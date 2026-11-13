//! Finite hierarchy and command-aware encoding backstop oracle.
mod common;
use common::*;
use serde_json::json;
#[test]
fn hierarchy_remains_finite_or_returns_typed_refusal() {
    for (count, rotation, skew, finite) in [(26, 0., 0., true), (46, 45., 89., false)] {
        let h = fixture();
        let mut parent = serde_json::Value::Null;
        for n in 0..count {
            let id = ok(&h, "element.create", json!({"type":"peg","name":format!("R2 {n}")}))["elementIds"][0].clone();
            if !parent.is_null() {
                ok(&h, "element.parent.set", json!({"elementIds":[id],"parentId":parent}));
            }
            ok(
                &h,
                "anim.transform.set",
                json!({"nodeIds":[id],"target":"static","scale":{"x":1e6,"y":1e6},"rotation":rotation,"skew":skew}),
            );
            parent = id;
        }
        let result = run(&h, "anim.evaluate", json!({"nodeIds":[parent],"frames":[1],"include":["world"]}));
        if finite {
            assert_eq!(result.status, tt_executor::Status::Ok);
            let x = result.result.unwrap()["frames"][0]["nodes"][0]["world"]["scale"]["x"].as_f64().unwrap();
            assert!((x / 1e156 - 1.).abs() < 1e-12);
        } else {
            let e = result.error.unwrap();
            assert_eq!(e.code, "anim.valueOutOfRange");
            assert_eq!(e.details["reason"], "nonFinite");
        }
    }
    for id in [
        "anim.camera.get",
        "anim.evaluate",
        "anim.path.get",
        "anim.function.get",
        "anim.projection.get",
        "anim.transform.get",
    ] {
        for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let e = tt_registry::encode_command_result(id, &[x]).unwrap_err();
            assert_eq!(e.code, "anim.valueOutOfRange");
        }
    }
}
