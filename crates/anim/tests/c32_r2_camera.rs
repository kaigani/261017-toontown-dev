//! Non-quarter-turn camera oracle using independent pinhole formulas.
use tt_anim::{camera::CameraView, matrix::Mat4};
#[test]
fn arbitrary_roll_fov_and_near_plane() {
    for n in 1..=4096 {
        let roll = n as f64 * 0.0713;
        let fov = 20. + (n % 140) as f64;
        let view = CameraView::new(&Mat4::translate(2., -3., 16.).mul(&Mat4::rotate(roll)), fov, 1920., 1080.);
        let (s, c) = roll.to_radians().sin_cos();
        let p = [2. + c * 1.25 - s * 0.7, -3. + s * 1.25 + c * 0.7, 0.];
        let projected = view.project(p).pixel.unwrap();
        let focal = 960. / (fov.to_radians() / 2.).tan();
        assert!((projected[0] - (960. + focal * 1.25 / 16.)).abs() < 1e-8);
        assert!((projected[1] - (540. - focal * 0.7 / 16.)).abs() < 1e-8);
        assert!(view.project([2., -3., 15.901]).pixel.is_none());
        assert!(view.project([2., -3., 15.899]).pixel.is_some());
    }
}
