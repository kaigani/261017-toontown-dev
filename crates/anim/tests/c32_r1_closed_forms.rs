//! Independent closed-form TCB/affine/camera oracle; seed C3203201.
use tt_anim::{
    camera::CameraView,
    matrix::Mat4,
    tcb::{Path, Point},
};
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
#[test]
fn c32_r1_closed_forms_4096() {
    let mut seed = 0xc3203201_u64;
    for _case in 0..4096 {
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed >> 32) % 17) as i32 - 8
        };
        let mut ps = Vec::new();
        for i in 0..4 {
            ps.push(Point {
                pos: [next() as f64, next() as f64, next() as f64],
                t: next() as f64 / 8.,
                c: next() as f64 / 8.,
                b: next() as f64 / 8.,
                frame: Some((i * 10 + 1) as f64),
                constant: false,
                lock_depth: false,
            });
        }
        let path = Path::new(ps.clone(), false);
        // Convert Hermite endpoint tangents into Bezier controls; evaluate Bernstein, independently of product basis.
        let mut chord = 0.;
        for seg in 0..3 {
            let p = ps[seg];
            let q = ps[seg + 1];
            let mut prev = p.pos;
            for j in 0..=64 {
                let u = j as f64 / 64.;
                let v = 1. - u;
                let mut want = [0.; 3];
                for (a, wanted) in want.iter_mut().enumerate() {
                    let before = if seg == 0 { q.pos[a] - p.pos[a] } else { p.pos[a] - ps[seg - 1].pos[a] };
                    let after = q.pos[a] - p.pos[a];
                    let following = if seg == 2 { after } else { ps[seg + 2].pos[a] - q.pos[a] };
                    let out = (1. - p.t) * ((1. + p.b) * (1. + p.c) * before + (1. - p.b) * (1. - p.c) * after) / 2.;
                    let incoming =
                        (1. - q.t) * ((1. + q.b) * (1. - q.c) * after + (1. - q.b) * (1. + q.c) * following) / 2.;
                    *wanted = v * v * v * p.pos[a]
                        + 3. * v * v * u * (p.pos[a] + out / 3.)
                        + 3. * v * u * u * (q.pos[a] - incoming / 3.)
                        + u * u * u * q.pos[a];
                    close(path.curve(seg, u)[a], *wanted);
                }
                if j > 0 {
                    chord += ((want[0] - prev[0]).powi(2) + (want[1] - prev[1]).powi(2) + (want[2] - prev[2]).powi(2))
                        .sqrt();
                }
                prev = want;
            }
        }
        close(path.length(), chord);
        let sx = if next() < 0 { -1. } else { 1. };
        let sy = if next() < 0 { -1. } else { 1. };
        let r = (next() * 90) as f64;
        let skew = next() as f64 * 5.;
        let m = Mat4::translate(next() as f64, next() as f64, 2.)
            .mul(&Mat4::rotate(r))
            .mul(&Mat4::skew(skew))
            .mul(&Mat4::scale(sx, sy));
        close(m.determinant(), sx * sy);
        let d = m.decompose();
        let rebuilt = Mat4::translate(d.position[0], d.position[1], d.position[2])
            .mul(&Mat4::rotate(d.rotation))
            .mul(&Mat4::skew(d.skew))
            .mul(&Mat4::scale(d.sx, d.sy));
        for i in 0..16 {
            close(m.0[i], rebuilt.0[i]);
        }
        let p = [next() as f64, next() as f64, 0.];
        let back = m.inverse().unwrap().apply(m.apply(p));
        for a in 0..3 {
            close(back[a], p[a]);
        }
        let camera = CameraView::new(&Mat4::translate(2., -3., 16.).mul(&Mat4::rotate(r)), 90., 1920., 1080.);
        let projected = camera.project(p);
        let back = camera.unproject(projected.pixel.unwrap(), projected.depth).unwrap();
        for a in 0..3 {
            close(back[a], p[a]);
        }
        let near = camera.project([2., -3., 16.]);
        assert!(near.pixel.is_none());
    }
}

#[test]
fn c32_r1_integer_cycle_mapping_4096() {
    use tt_anim::timing::Bar;
    for n in 1..=4096_i64 {
        let start = n % 31 + 1;
        let duration = n % 97 + 1;
        let cycle = n % 13 + 1;
        let bar = Bar { start: start as f64, duration: duration as f64, cycle: Some(cycle as f64) };
        for frame in [1, start, start + duration - 1, start + duration, start + 10000] {
            let local = (frame - start + 1).max(1).min(duration);
            close(bar.local(frame as f64), local as f64);
            let (got, index) = bar.cycle_frame(local as f64);
            let want = (local - 1) % cycle + 1;
            close(got, want as f64);
            close(index, ((local - 1) / cycle) as f64);
        }
    }
}
