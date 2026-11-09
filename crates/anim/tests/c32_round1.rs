//! M3 C3.2 round 1 (builder): the default field of view (C32-R1-03, SI-102, SI-104) and the decomposition of finite
//! world matrices whose squared coefficients overflow or underflow (C32-R1-06). The passage-frame regressions
//! (C32-R1-01) are in `c32_passage.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use tt_anim::camera::{CameraView, projection_fov};
use tt_anim::matrix::{Mat4, norm};
use tt_anim::num;
use tt_model::defaults::{DEFAULT_FOV, DEFAULT_FOV_DISPLAY};

fn table_width(fov: f64) -> f64 {
    let view = CameraView::new(&Mat4::translate(0.0, 0.0, 16.0), fov, 1920.0, 1080.0);
    let c = view.frame_corners_at_z0().unwrap();
    c[1][0] - c[0][0]
}

#[test]
fn the_default_fov_is_the_exact_expression_through_the_pinned_path() {
    // 05 §2.7: 2·atan(0.375), computed once through the soft-float `libm` port; the constant carries its bits.
    let exact = 2.0 * num::atan_deg(0.375);
    assert_eq!(DEFAULT_FOV.to_bits(), exact.to_bits(), "{DEFAULT_FOV} vs {exact}");
    // Its 4-decimal display is the tables' value.
    assert_eq!((DEFAULT_FOV * 1e4).round() / 1e4, DEFAULT_FOV_DISPLAY);
    // An independent check of the value: tan(fov/2) = 3/8 to a few ulps.
    assert!((num::tan_deg(DEFAULT_FOV / 2.0) - 0.375).abs() < 1e-15);
}

#[test]
fn the_default_frame_is_twelve_fields_wide_at_distance_sixteen() {
    // "The 12-field width exactly fills the frame" (05 §2.7): to rounding, far inside the 1e-6 numeric tolerance.
    let w = table_width(DEFAULT_FOV);
    assert!((w - 12.0).abs() < 1e-12, "{w:.17}");
    // The tables' 41.1121 is the display of that value and projects as it (SI-104), not 12.000003.
    assert_eq!(projection_fov(DEFAULT_FOV_DISPLAY).to_bits(), DEFAULT_FOV.to_bits());
    assert_eq!(table_width(DEFAULT_FOV_DISPLAY).to_bits(), w.to_bits());
    // Any other value projects as itself: 41.1122 is wider by the expected amount, 60 is 2·16·tan 30°.
    assert_eq!(projection_fov(41.1122), 41.1122);
    let near = table_width(41.1122);
    let want = 32.0 * num::tan_deg(41.1122 / 2.0);
    assert!((near - want).abs() < 1e-12 && near > 12.0 + 1e-6, "{near}");
    let wide = table_width(60.0);
    assert!((wide - 32.0 / 3f64.sqrt()).abs() < 1e-12, "{wide}");
    // The view keeps the FOV as set.
    let view = CameraView::new(&Mat4::translate(0.0, 0.0, 16.0), DEFAULT_FOV_DISPLAY, 1920.0, 1080.0);
    assert_eq!(view.fov, DEFAULT_FOV_DISPLAY);
}

#[test]
fn the_norm_keeps_ordinary_bits_and_scales_only_out_of_range_sums() {
    // In the normal range the result is the plain formula's, bit for bit (E4 bytes and every earlier result unchanged).
    for (a, b) in [(3.0, 4.0), (0.1, 0.2), (1e150, 1e150), (-2.5, 1e-100), (1e-150, 1e-150), (0.0, 7.0)] {
        assert_eq!(norm(a, b).to_bits(), (a * a + b * b).sqrt().to_bits(), "{a} {b}");
    }
    assert_eq!(norm(0.0, 0.0), 0.0);
    // Overflow: 1e200² is infinite, the norm is not.
    let big = norm(3e200, 4e200);
    assert!((big / 5e200 - 1.0).abs() < 1e-15, "{big}");
    assert!((norm(f64::MAX, 0.0) - f64::MAX).abs() == 0.0);
    // Underflow: 1e-200² is 0, the norm is not.
    let small = norm(3e-200, -4e-200);
    assert!((small / 5e-200 - 1.0).abs() < 1e-15, "{small}");
}

#[test]
fn a_finite_world_matrix_decomposes_to_finite_values() {
    // 26 nested scales of 1e6 (the per-node bound): coefficients 1e156, squares out of range (the reviewer's case).
    let mut m = Mat4::IDENTITY;
    let step = Mat4::scale(1e6, 1e6);
    for _ in 0..26 {
        m = m.mul(&step);
    }
    assert!(m.is_finite());
    let d = m.decompose();
    assert!((d.sx / 1e156 - 1.0).abs() < 1e-12 && (d.sy / 1e156 - 1.0).abs() < 1e-12, "{d:?}");
    assert_eq!((d.rotation, d.skew), (0.0, 0.0));
    // Rotated by 30° and mirrored: the analytic values survive the scaling too.
    let r = Mat4::rotate(30.0).mul(&m).mul(&Mat4::scale(1.0, -1.0));
    let d = r.decompose();
    assert!((d.sx / 1e156 - 1.0).abs() < 1e-12 && (d.sy / -1e156 - 1.0).abs() < 1e-12, "{d:?}");
    assert!((d.rotation - 30.0).abs() < 1e-9 && d.skew.abs() < 1e-9, "{d:?}");
    // The other way: 1e-156.
    let mut t = Mat4::IDENTITY;
    for _ in 0..26 {
        t = t.mul(&Mat4::scale(1e-6, 1e-6));
    }
    let d = t.decompose();
    assert!((d.sx / 1e-156 - 1.0).abs() < 1e-12 && (d.sy / 1e-156 - 1.0).abs() < 1e-12, "{d:?}");
}
