//! Independent review regression; the finding test intentionally remains failing.
use tt_anim::{
    function::KeyView,
    tcb::{Path, Point, Velocity},
};
use tt_model::entities::channel::CurveType;

#[test]
fn c32_r1_passage_must_find_subframe_first_crossing() {
    let point = |x, frame| Point {
        pos: [x, 0.0, 0.0],
        t: 0.0,
        c: 0.0,
        b: 0.0,
        frame: Some(frame),
        constant: false,
        lock_depth: false,
    };
    let path = Path::new(vec![point(0.0, 1.0), point(1.0, 3.0)], false);
    let mut a = KeyView::plain(1.0, 0.0);
    a.right = (1.0 / 3.0, 1.0);
    let mut b = KeyView::plain(2.0, 0.0);
    b.left = (-1.0 / 3.0, 1.0);
    let velocity = Velocity { curve: CurveType::Spline, keys: vec![a, b] };
    // On [1,2], x(u)=1+u and V(u)=3u(1-u). First crossing of 1/2:
    // u=(1-sqrt(1/3))/2. Both integer endpoints are below the target.
    let expected = ((1.0 + (1.0 - (1.0_f64 / 3.0).sqrt()) / 2.0) * 1000.0).round() / 1000.0;
    let actual = velocity.passage(&path, 0, 0.5, &mut |_| Ok::<_, ()>(())).unwrap();
    assert!((actual - expected).abs() <= 0.001, "actual={actual}, independent={expected}");
}
