//! Seeded independent cubic-extrema earliest crossing oracle. Intentionally fails on R1-01.
use tt_anim::{
    function::KeyView,
    tcb::{Path, Point, Velocity},
};
use tt_model::entities::channel::CurveType;
fn bez(p: [f64; 4], t: f64) -> f64 {
    let s = 1.0 - t;
    s * s * s * p[0] + 3.0 * s * s * t * p[1] + 3.0 * s * t * t * p[2] + t * t * t * p[3]
}
fn crossing(p: [f64; 4], target: f64) -> Option<f64> {
    let a = 3.0 * (-p[0] + 3.0 * p[1] - 3.0 * p[2] + p[3]);
    let b = 6.0 * (p[0] - 2.0 * p[1] + p[2]);
    let c = 3.0 * (p[1] - p[0]);
    let mut cuts = vec![0.0, 1.0];
    if a.abs() < 1e-14 {
        if b.abs() > 1e-14 {
            cuts.push(-c / b);
        }
    } else {
        let d = b * b - 4.0 * a * c;
        if d >= 0.0 {
            cuts.push((-b - d.sqrt()) / (2.0 * a));
            cuts.push((-b + d.sqrt()) / (2.0 * a));
        }
    }
    cuts.retain(|x| *x >= 0.0 && *x <= 1.0);
    cuts.sort_by(f64::total_cmp);
    for w in cuts.windows(2) {
        if bez(p, w[0]) >= target {
            return Some(w[0]);
        }
        if bez(p, w[1]) < target {
            continue;
        }
        let (mut l, mut h) = (w[0], w[1]);
        while h - l > 1e-10 {
            let m = (l + h) / 2.0;
            if bez(p, m) >= target {
                h = m;
            } else {
                l = m;
            }
        }
        return Some(h);
    }
    None
}
fn point(x: f64, f: f64) -> Point {
    Point { pos: [x, 0.0, 0.0], t: 0.0, c: 0.0, b: 0.0, frame: Some(f), constant: false, lock_depth: false }
}
#[test]
fn seeded_4096_earliest_crossings() {
    let path = Path::new(vec![point(0.0, 1.0), point(1.0, 3.0), point(2.0, 5.0)], false);
    let mut seed = 0xC320_19AB_6701u64;
    let mut random = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / ((1u64 << 53) as f64)
    };
    let mut missed = 0;
    let mut first = None;
    for case in 0..4096 {
        let h0 = 4.0 * random() - 1.0;
        let h1 = 4.0 * random() - 1.0;
        let v = 0.2 * random();
        let target = 0.25 + 0.5 * random();
        let mut a = KeyView::plain(1.0, 0.0);
        a.right = (1.0 / 3.0, h0);
        let mut b = KeyView::plain(2.0, v);
        b.left = (-1.0 / 3.0, h1 - v);
        b.right = (1.0 / 3.0, (1.0 - v) / 3.0);
        let mut c = KeyView::plain(3.0, 1.0);
        c.left = (-1.0 / 3.0, -(1.0 - v) / 3.0);
        let velocity = Velocity { curve: CurveType::Spline, keys: vec![a, b, c] };
        let expected =
            if let Some(u) = crossing([0.0, h0, h1, v], target) { 1.0 + u } else { 2.0 + (target - v) / (1.0 - v) };
        let expected = (expected * 1000.0 + 0.5).floor() / 1000.0;
        let actual = velocity.passage(&path, 0, target, &mut |_| Ok::<_, ()>(())).unwrap();
        if (actual - expected).abs() > 0.001 {
            missed += 1;
            if first.is_none() {
                first = Some((case, h0, h1, v, target, actual, expected));
            }
        }
    }
    eprintln!("seed=C32019AB6701 cases=4096 missed={missed} first={first:?}");
    assert_eq!(missed, 0, "R1-01: earliest crossings missed");
}
#[test]
fn exact_whole_frame_crossing() {
    let path = Path::new(vec![point(0.0, 1.0), point(1.0, 3.0)], false);
    assert_eq!(Velocity::default().passage(&path, 0, 0.5, &mut |_| Ok::<_, ()>(())).unwrap(), 2.0);
}
