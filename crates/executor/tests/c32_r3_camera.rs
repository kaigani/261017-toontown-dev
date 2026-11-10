//! SI-104 explicit display value must project as reported, also after replay.
mod common;
use common::*;
use serde_json::json;
#[test]
fn display_fov_is_one_value_through_dynamic_history() {
    let h = fixture();
    let id = ok(&h, "anim.camera.create", json!({"name":"R3","fov":41.1121}))["cameraId"].clone();
    for replay in [false, true] {
        if replay {
            ok(&h, "anim.camera.set", json!({"cameraId":id,"fovMode":"dynamic","frame":1,"fov":41.1121}));
            ok(&h, "anim.camera.set", json!({"cameraId":id,"frame":11,"fov":60}));
            ok(&h, "edit.undo", json!({}));
            ok(&h, "edit.redo", json!({}));
        }
        let cam = ok(&h, "anim.camera.get", json!({"cameraId":id,"frame":1}));
        assert_eq!(cam["effectiveFov"], json!(41.1121));
        let Some(a) = cam["frameCornersAtZ0"][0]["x"].as_f64() else { unreachable!() };
        let Some(b) = cam["frameCornersAtZ0"][1]["x"].as_f64() else { unreachable!() };
        let expected = 32.0 * (41.1121_f64.to_radians() / 2.0).tan();
        assert!((b - a - expected).abs() < 1e-9);
        assert!((b - a - 12.0).abs() > 1e-6);
    }
}
