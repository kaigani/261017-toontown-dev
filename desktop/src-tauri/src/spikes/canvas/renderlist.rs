//! C0.3 render-list prototype (**spike code**): scene generators and the binary encoding sent to the webview.
//!
//! Standard library only, so the rasteriser spike (`crates/render/examples/c03_raster`) can include this file to
//! render the same scenes on the CPU for the host-raster fallback estimate.
//!
//! # Binary format `TTRL` version 1 (little-endian)
//!
//! | offset | size | field |
//! |---|---|---|
//! | 0 | 4 | magic `TTRL` |
//! | 4 | 2 | version (1) |
//! | 6 | 2 | header length in bytes (72) |
//! | 8 | 4 | scene kind (0 = paths100k, 1 = groups1k, 2 = brush10k) |
//! | 12 | 4 | seed |
//! | 16 | 4 | frame (group transforms of `groups1k` depend on it) |
//! | 20 | 4 | group count G |
//! | 24 | 4 | path count P |
//! | 28 | 4 | verb count V |
//! | 32 | 4 | point count N (x, y pairs) |
//! | 36 | 16 | bounds `[min x, min y, max x, max y]` (f32) in world units |
//! | 52 | 4 | flags (0) |
//! | 56 | 8 | encode time, ns (u64; time to serialise this buffer) |
//! | 64 | 8 | generate time, ns (u64; time to build the scene model, 0 when cached) |
//!
//! Then, in order: G group transforms (6 × f32 `[a, b, c, d, e, f]`, 24 bytes each); P path records (28 bytes:
//! verb start u32, verb count u32, point start u32, group u32, paint u32 = RGBA8 bytes, style u32 (0 fill non-zero,
//! 1 fill even-odd, 2 stroke), stroke width f32); N points (2 × f32); V verbs (u8: 0 move, 1 line, 2 cubic,
//! 3 close), zero-padded to a multiple of 4. Total length = 72 + 24G + 28P + 8N + pad4(V).
//!
//! The groups are one flat level (a path belongs to one group). M3's render list is a tree with paints, clips and
//! effects; this prototype only has to be replaceable, not complete.

#![allow(missing_docs, reason = "spike module: the format table above documents every constant and field")]

use std::time::Instant;

pub const MAGIC: &[u8; 4] = b"TTRL";
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 72;
pub const GROUP_BYTES: usize = 24;
pub const PATH_BYTES: usize = 28;

pub const VERB_MOVE: u8 = 0;
pub const VERB_LINE: u8 = 1;
pub const VERB_CUBIC: u8 = 2;
pub const VERB_CLOSE: u8 = 3;

pub const STYLE_FILL_NONZERO: u32 = 0;
pub const STYLE_FILL_EVENODD: u32 = 1;
pub const STYLE_STROKE: u32 = 2;

/// World extent of every scene (square).
pub const WORLD: f32 = 4096.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// 100 000 random stroked paths of two cubic Bézier segments each.
    Paths100k,
    /// 1 000 transformed groups of 20 paths (10 filled cubic shapes, 10 strokes) = 20 000 paths.
    Groups1k,
    /// 10 000 brush strokes as filled polygons with 20–200 vertices.
    Brush10k,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "paths100k" => Some(Kind::Paths100k),
            "groups1k" => Some(Kind::Groups1k),
            "brush10k" => Some(Kind::Brush10k),
            _ => None,
        }
    }
    pub fn code(self) -> u32 {
        match self {
            Kind::Paths100k => 0,
            Kind::Groups1k => 1,
            Kind::Brush10k => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathRec {
    pub verb_start: u32,
    pub verb_count: u32,
    pub point_start: u32,
    pub group: u32,
    /// RGBA8, straight alpha.
    pub paint: [u8; 4],
    pub style: u32,
    pub width: f32,
}

/// The scene model the encoder serialises.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub seed: u32,
    /// Base transforms (frame 0).
    pub groups: Vec<[f32; 6]>,
    pub paths: Vec<PathRec>,
    pub verbs: Vec<u8>,
    pub points: Vec<f32>,
    pub bounds: [f32; 4],
}

/// SplitMix64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn colour(&mut self, alpha_lo: f32) -> [u8; 4] {
        let b = self.next().to_le_bytes();
        [b[0], b[1], b[2], (255.0 * self.range(alpha_lo, 1.0)) as u8]
    }
}

struct Builder {
    scene: Scene,
}

impl Builder {
    fn begin(&mut self) -> (u32, u32) {
        (self.scene.verbs.len() as u32, (self.scene.points.len() / 2) as u32)
    }
    fn mv(&mut self, x: f32, y: f32) {
        self.scene.verbs.push(VERB_MOVE);
        self.scene.points.extend([x, y]);
    }
    fn ln(&mut self, x: f32, y: f32) {
        self.scene.verbs.push(VERB_LINE);
        self.scene.points.extend([x, y]);
    }
    fn cu(&mut self, p: [f32; 6]) {
        self.scene.verbs.push(VERB_CUBIC);
        self.scene.points.extend(p);
    }
    fn close(&mut self) {
        self.scene.verbs.push(VERB_CLOSE);
    }
    fn end(&mut self, start: (u32, u32), group: u32, paint: [u8; 4], style: u32, width: f32) {
        self.scene.paths.push(PathRec {
            verb_start: start.0,
            verb_count: self.scene.verbs.len() as u32 - start.0,
            point_start: start.1,
            group,
            paint,
            style,
            width,
        });
    }
}

fn blob(b: &mut Builder, rng: &mut Rng, cx: f32, cy: f32, r: f32) {
    // A closed shape of four cubics around (cx, cy) with jittered radius.
    let k = 0.5523 * r;
    let j = |rng: &mut Rng| rng.range(0.7, 1.2);
    let (r0, r1, r2, r3) = (r * j(rng), r * j(rng), r * j(rng), r * j(rng));
    b.mv(cx + r0, cy);
    b.cu([cx + r0, cy + k, cx + k, cy + r1, cx, cy + r1]);
    b.cu([cx - k, cy + r1, cx - r2, cy + k, cx - r2, cy]);
    b.cu([cx - r2, cy - k, cx - k, cy - r3, cx, cy - r3]);
    b.cu([cx + k, cy - r3, cx + r0, cy - k, cx + r0, cy]);
    b.close();
}

/// Group transform of group `i` at `frame` (rotation about the group's centre; `frame` 0 = base layout).
pub fn group_transform(i: usize, frame: u32, base: &[f32; 6]) -> [f32; 6] {
    if frame == 0 {
        return *base;
    }
    let angle = frame as f32 * 0.02 + i as f32 * 0.1;
    let (s, c) = angle.sin_cos();
    let (e, f) = (base[4], base[5]);
    // Rotate the group's local frame about its origin, keep its position.
    [base[0] * c - base[1] * s, base[0] * s + base[1] * c, base[2] * c - base[3] * s, base[2] * s + base[3] * c, e, f]
}

pub fn generate(kind: Kind, seed: u32) -> Scene {
    let mut rng = Rng(u64::from(seed) ^ (u64::from(kind.code()) << 32) ^ 0x0C03_CA9A);
    let mut b = Builder { scene: Scene { seed, bounds: [0.0, 0.0, WORLD, WORLD], ..Scene::default() } };
    match kind {
        Kind::Paths100k => {
            b.scene.groups.push([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
            for _ in 0..100_000 {
                let (x, y) = (rng.range(0.0, WORLD), rng.range(0.0, WORLD));
                let mut p = || rng.range(-60.0, 60.0);
                let ctrl = [x + p(), y + p(), x + p(), y + p(), x + p(), y + p()];
                let ctrl2 = [ctrl[4] + p(), ctrl[5] + p(), ctrl[4] + p(), ctrl[5] + p(), ctrl[4] + p(), ctrl[5] + p()];
                let start = b.begin();
                b.mv(x, y);
                b.cu(ctrl);
                b.cu(ctrl2);
                let colour = rng.colour(0.4);
                let width = rng.range(0.5, 3.0);
                b.end(start, 0, colour, STYLE_STROKE, width);
            }
        }
        Kind::Groups1k => {
            let cell = WORLD / 32.0;
            for g in 0..1000_u32 {
                let (gx, gy) = ((g % 32) as f32 * cell + cell / 2.0, (g / 32) as f32 * cell + cell / 2.0);
                let s = rng.range(0.6, 1.0);
                b.scene.groups.push([s, 0.0, 0.0, s, gx, gy]);
                for k in 0..20 {
                    let start = b.begin();
                    let (lx, ly) = (rng.range(-40.0, 40.0), rng.range(-40.0, 40.0));
                    if k < 10 {
                        let r = rng.range(6.0, 24.0);
                        blob(&mut b, &mut rng, lx, ly, r);
                        let colour = rng.colour(0.5);
                        b.end(start, g, colour, STYLE_FILL_NONZERO, 0.0);
                    } else {
                        let mut p = || rng.range(-30.0, 30.0);
                        b.mv(lx, ly);
                        b.cu([lx + p(), ly + p(), lx + p(), ly + p(), lx + p(), ly + p()]);
                        let colour = rng.colour(0.6);
                        let width = rng.range(1.0, 4.0);
                        b.end(start, g, colour, STYLE_STROKE, width);
                    }
                }
            }
        }
        Kind::Brush10k => {
            b.scene.groups.push([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
            for _ in 0..10_000 {
                // A brush stroke: a cubic spine sampled at n points; the outline goes out along one side and back
                // along the other, with a pressure-like width profile (tapered ends).
                let n = 10 + (rng.next() % 91) as usize; // 10..=100 samples -> 20..=200 vertices
                let (x0, y0) = (rng.range(0.0, WORLD), rng.range(0.0, WORLD));
                let len = rng.range(40.0, 260.0);
                let mut q = || rng.range(-1.0, 1.0) * len;
                let c = [x0 + q(), y0 + q(), x0 + q(), y0 + q(), x0 + q(), y0 + q()];
                let wmax = rng.range(2.0, 14.0);
                let mut left = Vec::with_capacity(n);
                let mut right = Vec::with_capacity(n);
                for i in 0..n {
                    let t = i as f32 / (n - 1) as f32;
                    let u = 1.0 - t;
                    let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    let x = b0 * x0 + b1 * c[0] + b2 * c[2] + b3 * c[4];
                    let y = b0 * y0 + b1 * c[1] + b2 * c[3] + b3 * c[5];
                    // Tangent (derivative of the cubic).
                    let dx = 3.0 * (u * u * (c[0] - x0) + 2.0 * u * t * (c[2] - c[0]) + t * t * (c[4] - c[2]));
                    let dy = 3.0 * (u * u * (c[1] - y0) + 2.0 * u * t * (c[3] - c[1]) + t * t * (c[5] - c[3]));
                    let l = (dx * dx + dy * dy).sqrt().max(1e-3);
                    let w = wmax * (4.0 * t * u).max(0.08); // taper
                    left.push((x - dy / l * w, y + dx / l * w));
                    right.push((x + dy / l * w, y - dx / l * w));
                }
                let start = b.begin();
                let (fx, fy) = left[0];
                b.mv(fx, fy);
                for &(x, y) in left.iter().skip(1) {
                    b.ln(x, y);
                }
                for &(x, y) in right.iter().rev() {
                    b.ln(x, y);
                }
                b.close();
                let colour = rng.colour(0.6);
                b.end(start, 0, colour, STYLE_FILL_NONZERO, 0.0);
            }
        }
    }
    b.scene
}

/// Total encoded length for a scene.
pub fn encoded_len(scene: &Scene) -> usize {
    HEADER_LEN
        + GROUP_BYTES * scene.groups.len()
        + PATH_BYTES * scene.paths.len()
        + 4 * scene.points.len()
        + scene.verbs.len().div_ceil(4) * 4
}

/// Serialises `scene` at `frame`; writes the encode time (and the given generate time) into the header.
pub fn encode(kind: Kind, scene: &Scene, frame: u32, generate_ns: u64) -> Vec<u8> {
    let t0 = Instant::now();
    let mut out = Vec::with_capacity(encoded_len(scene));
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(HEADER_LEN as u16).to_le_bytes());
    for v in [
        kind.code(),
        scene.seed,
        frame,
        scene.groups.len() as u32,
        scene.paths.len() as u32,
        scene.verbs.len() as u32,
        (scene.points.len() / 2) as u32,
    ] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in scene.bounds {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&0_u32.to_le_bytes()); // flags
    out.extend_from_slice(&0_u64.to_le_bytes()); // encode ns, patched below
    out.extend_from_slice(&generate_ns.to_le_bytes());
    for (i, g) in scene.groups.iter().enumerate() {
        for v in group_transform(i, frame, g) {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    for p in &scene.paths {
        for v in [p.verb_start, p.verb_count, p.point_start, p.group, u32::from_le_bytes(p.paint), p.style] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&p.width.to_le_bytes());
    }
    for v in &scene.points {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&scene.verbs);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    let ns = u64::try_from(t0.elapsed().as_nanos()).unwrap_or(u64::MAX);
    out[56..64].copy_from_slice(&ns.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenes_have_the_specified_sizes() {
        let p = generate(Kind::Paths100k, 1);
        assert_eq!(p.paths.len(), 100_000);
        assert!(p.paths.iter().all(|r| r.verb_count == 3 && r.style == STYLE_STROKE));
        let g = generate(Kind::Groups1k, 1);
        assert_eq!((g.groups.len(), g.paths.len()), (1000, 20_000));
        let b = generate(Kind::Brush10k, 1);
        assert_eq!(b.paths.len(), 10_000);
        // Polygons with 20..=200 vertices: move + (2n - 1) lines + close.
        assert!(b.paths.iter().all(|r| (21..=201).contains(&r.verb_count)));
    }

    #[test]
    fn encoding_has_the_documented_layout() {
        let s = generate(Kind::Groups1k, 7);
        let bytes = encode(Kind::Groups1k, &s, 3, 42);
        assert_eq!(bytes.len(), encoded_len(&s));
        assert_eq!(&bytes[0..4], MAGIC);
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), VERSION);
        assert_eq!(u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]), 3);
        assert_eq!(u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]), 20_000);
        assert_eq!(u64::from_le_bytes(bytes[64..72].try_into().unwrap()), 42);
        assert_eq!(bytes.len() % 4, 0);
    }

    /// Two groups and three paths: a stroked cubic (group 0), a filled triangle and an even-odd square (group 1).
    pub(super) fn tiny() -> Scene {
        let mut b = Builder { scene: Scene { seed: 9, bounds: [0.0, 0.0, 100.0, 100.0], ..Scene::default() } };
        b.scene.groups.push([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        b.scene.groups.push([2.0, 0.0, 0.0, 2.0, 10.0, 20.0]);
        let s = b.begin();
        b.mv(0.0, 0.0);
        b.cu([10.0, 0.0, 20.0, 10.0, 30.0, 30.0]);
        b.end(s, 0, [255, 0, 0, 255], STYLE_STROKE, 2.0);
        let s = b.begin();
        b.mv(0.0, 0.0);
        b.ln(10.0, 0.0);
        b.ln(0.0, 10.0);
        b.close();
        b.end(s, 1, [0, 128, 255, 200], STYLE_FILL_NONZERO, 0.0);
        let s = b.begin();
        b.mv(20.0, 20.0);
        b.ln(30.0, 20.0);
        b.ln(30.0, 30.0);
        b.ln(20.0, 30.0);
        b.close();
        b.end(s, 1, [0, 0, 0, 128], STYLE_FILL_EVENODD, 0.0);
        b.scene
    }

    /// Cross-language contract: the TypeScript decoder's test (desktop/src/spikes/canvas/decode.test.ts) reads the
    /// committed `testdata/tiny.ttrl`, which must equal this encoding (encode time zeroed). Regenerate with
    /// `TT_WRITE_FIXTURES=1 cargo test -p tt-desktop tiny_fixture`.
    #[test]
    fn tiny_fixture_matches_committed_bytes() {
        let mut bytes = encode(Kind::Groups1k, &tiny(), 0, 0);
        bytes[56..64].fill(0);
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/spikes/canvas/testdata/tiny.ttrl");
        if std::env::var_os("TT_WRITE_FIXTURES").is_some() {
            std::fs::write(path, &bytes).unwrap();
        }
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }

    #[test]
    fn generation_is_reproducible() {
        assert_eq!(generate(Kind::Brush10k, 5), generate(Kind::Brush10k, 5));
        assert_ne!(generate(Kind::Brush10k, 5).points, generate(Kind::Brush10k, 6).points);
    }
}
