//! Independent integer affine oracle, seed 0xC3200101, 4096 cases.
use tt_anim::matrix::Mat4;
#[test]
fn c32_r1_integer_affine_oracle_4096() {
    let mut seed = 0xc3200101_u64;
    for _ in 0..4096 {
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 32) % 31) as i64 - 15
        };
        let (x, y, px, py) = (next(), next(), next(), next());
        let sx = if next() < 0 { -1 } else { 1 };
        let sy = if next() < 0 { -1 } else { 1 };
        let quarter = next().rem_euclid(4);
        let (c, s) = match quarter {
            0 => (1, 0),
            1 => (0, 1),
            2 => (-1, 0),
            _ => (0, -1),
        };
        let m = Mat4::translate(x as f64, y as f64, 0.0)
            .mul(&Mat4::rotate((quarter * 90) as f64))
            .mul(&Mat4::scale(sx as f64, sy as f64));
        let expected = [(x + c * sx * px - s * sy * py) as f64, (y + s * sx * px + c * sy * py) as f64, 0.0];
        assert_eq!(m.apply([px as f64, py as f64, 0.0]), expected);
        assert_eq!(m.determinant(), (sx * sy) as f64);
        assert_eq!(m.inverse().unwrap().apply(expected), [px as f64, py as f64, 0.0]);
    }
}
