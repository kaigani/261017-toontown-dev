//! Deterministic transcendental functions (ADR-0004 rule 3, ADR-0019): the pure-software `libm` port, so evaluator
//! results never depend on the platform maths library. Multiples of 90° are exact.

/// π.
pub const PI: f64 = std::f64::consts::PI;

/// Degrees to radians.
#[must_use]
pub fn rad(deg: f64) -> f64 {
    deg * (PI / 180.0)
}

/// Radians to degrees.
#[must_use]
pub fn deg(rad: f64) -> f64 {
    rad * (180.0 / PI)
}

/// `deg` reduced to `[0, 360)`.
fn turn(deg: f64) -> f64 {
    let r = libm::fmod(deg, 360.0);
    if r < 0.0 { r + 360.0 } else { r }
}

/// Sine of an angle in degrees.
#[must_use]
pub fn sin_deg(deg: f64) -> f64 {
    let r = turn(deg);
    if r == 0.0 || r == 180.0 {
        0.0
    } else if r == 90.0 {
        1.0
    } else if r == 270.0 {
        -1.0
    } else {
        libm::sin(rad(r))
    }
}

/// Cosine of an angle in degrees.
#[must_use]
pub fn cos_deg(deg: f64) -> f64 {
    let r = turn(deg);
    if r == 90.0 || r == 270.0 {
        0.0
    } else if r == 0.0 {
        1.0
    } else if r == 180.0 {
        -1.0
    } else {
        libm::cos(rad(r))
    }
}

/// Tangent of an angle in degrees (skew, half field of view; both stay away from ±90°).
#[must_use]
pub fn tan_deg(deg: f64) -> f64 {
    if deg == 0.0 { 0.0 } else { libm::tan(rad(deg)) }
}

/// Arc tangent, in degrees.
#[must_use]
pub fn atan_deg(x: f64) -> f64 {
    deg(libm::atan(x))
}

/// Two-argument arc tangent, in degrees.
#[must_use]
pub fn atan2_deg(y: f64, x: f64) -> f64 {
    deg(libm::atan2(y, x))
}

/// `-0.0` as `0.0` (results are serialised through one path; the sign of a zero carries no meaning here).
#[must_use]
pub fn tidy(v: f64) -> f64 {
    if v == 0.0 { 0.0 } else { v }
}
