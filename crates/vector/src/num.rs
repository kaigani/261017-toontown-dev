//! Deterministic transcendental functions (ADR-0004 rule 3): the pure-software `libm` port (`force-soft-floats`), so
//! persisted geometry never depends on the platform maths library.

use crate::geom::{Affine, P};

/// π.
pub const PI: f64 = std::f64::consts::PI;

/// Sine.
#[must_use]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// Cosine.
#[must_use]
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// Arc cosine.
#[must_use]
pub fn acos(x: f64) -> f64 {
    libm::acos(x)
}

/// Degrees to radians.
#[must_use]
pub fn rad(deg: f64) -> f64 {
    deg * (PI / 180.0)
}

/// The unit vector at `deg` degrees counter-clockwise from +x. Multiples of 90° are exact.
#[must_use]
pub fn dir(deg: f64) -> P {
    let r = deg.rem_euclid(360.0);
    if r == 0.0 {
        P::new(1.0, 0.0)
    } else if r == 90.0 {
        P::new(0.0, 1.0)
    } else if r == 180.0 {
        P::new(-1.0, 0.0)
    } else if r == 270.0 {
        P::new(0.0, -1.0)
    } else {
        let a = rad(r);
        P::new(cos(a), sin(a))
    }
}

/// Rotation by `deg` degrees counter-clockwise about the origin.
#[must_use]
pub fn rotation(deg: f64) -> Affine {
    let u = dir(deg);
    Affine([u.x, u.y, -u.y, u.x, 0.0, 0.0])
}

/// Reflection about the line through the origin with direction `deg` degrees from +x.
#[must_use]
pub fn reflection(deg: f64) -> Affine {
    // R = 2·d·dᵀ − I with d = (cos θ, sin θ) = (cos 2θ, sin 2θ; sin 2θ, −cos 2θ).
    let d = dir(deg);
    let c2 = d.x * d.x - d.y * d.y;
    let s2 = 2.0 * d.x * d.y;
    Affine([c2, s2, s2, -c2, 0.0, 0.0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_values_are_platform_independent() {
        // Bit patterns of the software implementation (pinned; any platform must reproduce them).
        assert_eq!(sin(1.0).to_bits(), 0x3FEA_ED54_8F09_0CEE);
        assert_eq!(cos(1.0).to_bits(), 0x3FE1_4A28_0FB5_068C);
        assert_eq!(dir(90.0), P::new(0.0, 1.0));
    }

    #[test]
    fn reflection_about_the_y_axis() {
        let r = reflection(90.0);
        assert_eq!(r.apply(P::new(2.0, 3.0)), P::new(-2.0, 3.0));
    }
}
