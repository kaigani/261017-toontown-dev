//! Cross-platform floating-point determinism probe (ADR-0004).
//!
//! Evaluates a fixed workload that uses only IEEE-754 correctly-rounded operations (`+ - * /` and `sqrt`) in a fixed
//! order, and hashes the bit patterns of every result. Windows x86_64 and macOS aarch64 must produce the same hash;
//! the self-test compares it with a golden value. Transcendental functions (`sin`, `exp`, `powf`, …) are deliberately
//! absent: they come from the platform maths library and are not guaranteed identical across OSes, which is why
//! persisted geometry and the final renderer must not depend on them (or must use an in-house implementation).

use sha2::{Digest, Sha256};

/// SHA-256 of the workload's output. Recorded on Windows x86_64 (MSVC) and cross-checked with an independent
/// Python implementation of the same operation sequence; every platform must reproduce it.
pub(crate) const GOLDEN_SHA256: &str = "d22c8ec2eb9c269ed207f8421f83e96edcf58774894cfad3186b5e2169ad5a16";

/// Runs the workload and returns the lowercase hex SHA-256 of its output bits.
pub(crate) fn probe_sha256() -> String {
    let mut hasher = Sha256::new();
    for value in workload() {
        hasher.update(value.to_bits().to_le_bytes());
    }
    hex(&hasher.finalize())
}

/// Cubic Bézier evaluation by de Casteljau at 1024 parameters, the running polyline arc length, and a normalised
/// tangent per sample. Representative of the vector engine's inner loops.
fn workload() -> Vec<f64> {
    const P: [(f64, f64); 4] = [(-412.5, 87.25), (-120.125, 640.0), (333.3, -275.75), (598.0, 12.5)];
    const N: u32 = 1024;
    let mut out = Vec::with_capacity(N as usize * 5);
    let mut length = 0.0_f64;
    let mut prev: Option<(f64, f64)> = None;
    for i in 0..N {
        let t = f64::from(i) / f64::from(N - 1);
        let lerp = |a: (f64, f64), b: (f64, f64)| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        let (a, b, c) = (lerp(P[0], P[1]), lerp(P[1], P[2]), lerp(P[2], P[3]));
        let (d, e) = (lerp(a, b), lerp(b, c));
        let p = lerp(d, e);
        let (tx, ty) = (e.0 - d.0, e.1 - d.1);
        let norm = (tx * tx + ty * ty).sqrt();
        if let Some(q) = prev {
            let (dx, dy) = (p.0 - q.0, p.1 - q.1);
            length += (dx * dx + dy * dy).sqrt();
        }
        prev = Some(p);
        out.extend_from_slice(&[p.0, p.1, length, tx / norm, ty / norm]);
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(char::from(DIGITS[usize::from(b >> 4)]));
        s.push(char::from(DIGITS[usize::from(b & 0x0f)]));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_is_stable_within_a_run() {
        assert_eq!(probe_sha256(), probe_sha256());
    }

    #[test]
    fn probe_matches_golden() {
        assert_eq!(probe_sha256(), GOLDEN_SHA256);
    }

    #[test]
    fn hex_encodes_lowercase() {
        assert_eq!(hex(&[0x00, 0xab, 0xff]), "00abff");
    }

    #[test]
    fn workload_has_expected_shape() {
        let w = workload();
        assert_eq!(w.len(), 1024 * 5);
        assert!(w.iter().all(|v| v.is_finite()));
        // Endpoints of the curve are the first and last control points, exactly.
        assert_eq!((w[0], w[1]), (-412.5, 87.25));
        assert_eq!((w[w.len() - 5], w[w.len() - 4]), (598.0, 12.5));
    }
}
