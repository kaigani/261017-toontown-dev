//! Independent Bernstein oracle with non-horizontal handles and non-dyadic crossings.
use tt_anim::{
    function::KeyView,
    tcb::{Path, Point, Velocity},
};
use tt_model::entities::channel::CurveType;
fn bez(p: [f64; 4], u: f64) -> f64 {
    let v = 1. - u;
    v * v * v * p[0] + 3. * v * v * u * p[1] + 3. * v * u * u * p[2] + u * u * u * p[3]
}
#[test]
fn seeded_nonhorizontal_rounding_and_charged_refusal() {
    let point =
        |x, f| Point { pos: [x, 0., 0.], t: 0., c: 0., b: 0., frame: Some(f), constant: false, lock_depth: false };
    let path = Path::new(vec![point(0., 1.), point(1., 4.)], false);
    let mut seed = 0xC322CAFEu64;
    for case in 0..4096 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let u = 0.01 + 0.98 * ((seed >> 11) as f64 / ((1u64 << 53) as f64));
        let target = bez([0., 0.8, 0.9, 1.], u);
        let expected = ((bez([1., 1.15, 2.6, 4.], u) * 1000.) + 0.5).floor() / 1000.;
        let mut a = KeyView::plain(1., 0.);
        a.right = (0.15, 0.8);
        let mut b = KeyView::plain(4., 1.);
        b.left = (-1.4, -0.1);
        let velocity = Velocity { curve: CurveType::Spline, keys: vec![a, b] };
        let mut charged = 0u64;
        let got = velocity
            .passage(&path, 0, target, &mut |n| {
                charged += n;
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(got, expected, "seed C322CAFE case {case}");
        assert!(charged > 0);
        assert!(velocity.passage(&path, 0, target, &mut |_| Err::<(), _>("budget")).is_err());
    }
}

#[test]
fn preset_crossings_next_to_rounding_boundaries() {
    use tt_model::entities::channel::Easing;
    let point =
        |x, f| Point { pos: [x, 0., 0.], t: 0., c: 0., b: 0., frame: Some(f), constant: false, lock_depth: false };
    let path = Path::new(vec![point(0., 1.), point(1., 4.)], false);
    for easing in [Easing::Linear, Easing::EaseInOut] {
        let mut a = KeyView::plain(1., 0.);
        a.easing = Some(easing);
        let velocity = Velocity { curve: CurveType::Interpolation, keys: vec![a, KeyView::plain(4., 1.)] };
        for n in 1001..3999 {
            for delta in [0.0004999, 0.0005001] {
                let frame = n as f64 / 1000. + delta;
                let u = (frame - 1.) / 3.;
                let target = if easing == Easing::Linear { u } else { u * u * (3. - 2. * u) };
                let got = velocity.passage(&path, 0, target, &mut |_| Ok::<_, ()>(())).unwrap();
                assert_eq!(got, (frame * 1000. + 0.5).floor() / 1000.);
            }
        }
    }
}
