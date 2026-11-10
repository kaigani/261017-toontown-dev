//! SI-102 and SI-104 command-boundary oracle.
mod common;
use common::*;
use serde_json::json;
#[test]
fn default_camera_reports_exact_fov_and_twelve_fields() {
    let h = fixture();
    let id = ok(&h, "anim.camera.create", json!({"name":"R2 default"}))["cameraId"].clone();
    let cam = ok(&h, "anim.camera.get", json!({"cameraId":id,"frame":1}));
    assert_eq!(cam["fov"], json!(tt_model::defaults::DEFAULT_FOV));
    assert_eq!(cam["effectiveFov"], cam["fov"]);
    let c = cam["frameCornersAtZ0"].as_array().unwrap();
    assert!((c[1]["x"].as_f64().unwrap() - c[0]["x"].as_f64().unwrap() - 12.).abs() < 1e-6);
}

#[test]
fn keyed_fov_and_arbitrary_roll_use_reported_effective_fov() {
    let h = fixture();
    let id = ok(&h, "anim.camera.create", json!({"name":"R2 keyed","fov":60}))["cameraId"].clone();
    ok(&h, "anim.camera.set", json!({"cameraId":id,"fovMode":"dynamic","frame":1,"roll":17.3}));
    ok(&h, "anim.camera.set", json!({"cameraId":id,"fov":90,"frame":11}));
    for (frame, fov) in [(1, 60_f64), (6, 75.), (11, 90.)] {
        let cam = ok(&h, "anim.camera.get", json!({"cameraId":id,"frame":frame}));
        assert_eq!(cam["effectiveFov"], json!(fov));
        let c = cam["frameCornersAtZ0"].as_array().unwrap();
        let dx = c[1]["x"].as_f64().unwrap() - c[0]["x"].as_f64().unwrap();
        let dy = c[1]["y"].as_f64().unwrap() - c[0]["y"].as_f64().unwrap();
        assert!((dx.hypot(dy) - 32. * (fov.to_radians() / 2.).tan()).abs() < 1e-9);
    }
}
