//! Affine 4 × 4 matrices (05 §2.1): column vectors, row-major serialisation, translation in m03, m13, m23. Rotation,
//! skew and scale act only in the X/Y plane; only translation changes Z. Products evaluate left to right in a fixed
//! order (ADR-0004, ADR-0019).

use crate::num;

/// A row-major affine matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4(pub [f64; 16]);

/// The §2.5 decomposition of a matrix's X/Y part, with the translation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decomposition {
    /// Translation.
    pub position: [f64; 3],
    /// θ, degrees.
    pub rotation: f64,
    /// sx.
    pub sx: f64,
    /// sy (negative for a mirrored matrix).
    pub sy: f64,
    /// κ, degrees.
    pub skew: f64,
}

impl Mat4 {
    /// The identity.
    pub const IDENTITY: Self = Self([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]);

    /// T(x, y, z).
    #[must_use]
    pub fn translate(x: f64, y: f64, z: f64) -> Self {
        Self([1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z, 0.0, 0.0, 0.0, 1.0])
    }

    /// R(θ): counter-clockwise rotation in X/Y.
    #[must_use]
    pub fn rotate(deg: f64) -> Self {
        let (c, s) = (num::cos_deg(deg), num::sin_deg(deg));
        Self([c, -s, 0.0, 0.0, s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0])
    }

    /// K(κ): horizontal shear, x′ = x + y·tan κ.
    #[must_use]
    pub fn skew(deg: f64) -> Self {
        Self([1.0, num::tan_deg(deg), 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0])
    }

    /// S(sx, sy).
    #[must_use]
    pub fn scale(sx: f64, sy: f64) -> Self {
        Self([sx, 0.0, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0])
    }

    /// The product `self · b`.
    #[must_use]
    pub fn mul(&self, b: &Self) -> Self {
        let a = &self.0;
        let b = &b.0;
        let mut out = [0.0; 16];
        for r in 0..4 {
            for c in 0..4 {
                out[r * 4 + c] =
                    a[r * 4] * b[c] + a[r * 4 + 1] * b[4 + c] + a[r * 4 + 2] * b[8 + c] + a[r * 4 + 3] * b[12 + c];
            }
        }
        Self(out)
    }

    /// `M · (x, y, z, 1)ᵀ`.
    #[must_use]
    pub fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        let m = &self.0;
        [
            m[0] * p[0] + m[1] * p[1] + m[2] * p[2] + m[3],
            m[4] * p[0] + m[5] * p[1] + m[6] * p[2] + m[7],
            m[8] * p[0] + m[9] * p[1] + m[10] * p[2] + m[11],
        ]
    }

    /// The translation column.
    #[must_use]
    pub fn translation(&self) -> [f64; 3] {
        [self.0[3], self.0[7], self.0[11]]
    }

    /// The determinant of the X/Y part (the only part that can differ from a translation).
    #[must_use]
    pub fn determinant(&self) -> f64 {
        self.0[0] * self.0[5] - self.0[1] * self.0[4]
    }

    /// The inverse of a matrix of this model's form (X/Y block plus translation), `None` when singular.
    #[must_use]
    pub fn inverse(&self) -> Option<Self> {
        let m = &self.0;
        let det = self.determinant();
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        let (a, c, b, d) = (m[5] / det, -m[1] / det, -m[4] / det, m[0] / det);
        let (tx, ty, tz) = (m[3], m[7], m[11]);
        Some(Self([
            a,
            c,
            0.0,
            -(a * tx + c * ty),
            b,
            d,
            0.0,
            -(b * tx + d * ty),
            0.0,
            0.0,
            1.0,
            -tz,
            0.0,
            0.0,
            0.0,
            1.0,
        ]))
    }

    /// A = R(θ) · K(κ) · S(sx, sy) (05 §2.5).
    #[must_use]
    pub fn decompose(&self) -> Decomposition {
        let m = &self.0;
        let (a, c, b, d) = (m[0], m[1], m[4], m[5]);
        let sx = (a * a + b * b).sqrt();
        let rotation = num::atan2_deg(b, a);
        let (cos, sin) = if sx == 0.0 { (1.0, 0.0) } else { (a / sx, b / sx) };
        let c2 = cos * c + sin * d;
        let d2 = -sin * c + cos * d;
        let skew = if d2 == 0.0 { 0.0 } else { num::atan_deg(c2 / d2) };
        Decomposition { position: self.translation(), rotation, sx, sy: d2, skew }
    }

    /// Whether every entry is finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.0.iter().all(|v| v.is_finite())
    }
}
