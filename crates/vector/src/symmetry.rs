//! Symmetry copies of new strokes (02 §6.2): Mirror reflects each stroke about an axis through the drawing origin at
//! `angle` degrees counter-clockwise from +y; Kaleidoscope replicates it with dihedral symmetry — `mirrors` lines
//! through the origin at `angle + k × 180°/mirrors`, giving `2 × mirrors` images (rotations by `360°/mirrors` and
//! their reflections). The first map is always the identity (the original stroke).

use crate::geom::Affine;
use crate::num;

/// Symmetry drawing mode (02 §6.2; a global user setting, §14 decision 5).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Symmetry {
    /// Strokes are drawn normally.
    #[default]
    None,
    /// One mirror axis.
    Mirror {
        /// Axis angle from +y, counter-clockwise (degrees).
        angle: f64,
    },
    /// Dihedral symmetry.
    Kaleidoscope {
        /// Angle of the first mirror line from +y (degrees).
        angle: f64,
        /// Number of mirror lines (2–16).
        mirrors: u32,
    },
}

/// The maps producing every copy, identity first.
#[must_use]
pub fn maps(s: Symmetry) -> Vec<Affine> {
    match s {
        Symmetry::None => vec![Affine::IDENTITY],
        // An axis at `angle` from +y has direction `90° + angle` from +x.
        Symmetry::Mirror { angle } => vec![Affine::IDENTITY, num::reflection(90.0 + angle)],
        Symmetry::Kaleidoscope { angle, mirrors } => {
            let n = mirrors.clamp(2, 16);
            let mirror = num::reflection(90.0 + angle);
            let mut out = Vec::with_capacity(2 * n as usize);
            for k in 0..n {
                out.push(num::rotation(360.0 * f64::from(k) / f64::from(n)));
            }
            for k in 0..n {
                out.push(num::rotation(360.0 * f64::from(k) / f64::from(n)).then_after(&mirror));
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::P;

    #[test]
    fn mirror_at_zero_is_left_right() {
        let m = maps(Symmetry::Mirror { angle: 0.0 });
        assert_eq!(m.len(), 2);
        assert_eq!(m[1].apply(P::new(3.0, 5.0)), P::new(-3.0, 5.0));
    }

    #[test]
    fn kaleidoscope_gives_two_n_images() {
        let m = maps(Symmetry::Kaleidoscope { angle: 0.0, mirrors: 6 });
        assert_eq!(m.len(), 12);
        assert_eq!(m[0], Affine::IDENTITY);
        assert!(m[6].det() < 0.0 && m[1].det() > 0.0);
    }
}
