//! Independent default-frame oracle for 05 sections 1.2 and 2.7.
use tt_anim::{camera::CameraView, matrix::Mat4};
#[test]
fn c32_r1_default_frame_is_twelve_fields_within_normative_tolerance() {
    let view = CameraView::new(&Mat4::translate(0., 0., 16.), tt_model::defaults::DEFAULT_FOV, 1920., 1080.);
    let corners = view.frame_corners_at_z0().unwrap();
    let width = corners[1][0] - corners[0][0];
    assert!((width - 12.).abs() <= 1e-6, "default table width {width:.15}, expected 12 within 1e-6 field");
}

#[test]
fn explicit_display_fov_is_not_aliased() {
    let view = CameraView::new(&Mat4::translate(0., 0., 16.), 41.1121, 1920., 1080.);
    let corners = view.frame_corners_at_z0().unwrap();
    let width = corners[1][0] - corners[0][0];
    assert!((width - 12.00000306).abs() < 1e-7, "SI-104 explicit FOV width {width:.15}");
}
