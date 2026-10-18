//! Fixture scenes for the C0.3 rasteriser determinism spike (**spike code**).
//!
//! Every coordinate, colour and transform is built from literals and exact arithmetic (`+ - * /` on `f64`). Angles
//! use the rational parametrisation of the unit circle, `((1 - t²) / (1 + t²), 2t / (1 + t²))`, so no fixture
//! depends on `sin`/`cos`/`pow` from the platform's libm.

/// RGBA8, straight (not premultiplied) alpha, sRGB-encoded.
pub type Rgba = [u8; 4];

/// A 2-D affine transform `[a, b, c, d, e, f]`: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
pub type Affine = [f64; 6];

pub const IDENTITY: Affine = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    CubicTo(f64, f64, f64, f64, f64, f64),
    Close,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path(pub Vec<Seg>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Solid(Rgba),
    /// Linear gradient from `p0` to `p1`, pad extend, stops `(offset, colour)` with increasing offsets.
    Linear {
        p0: (f64, f64),
        p1: (f64, f64),
        stops: Vec<(f32, Rgba)>,
    },
    /// Radial gradient centred at `c` with radius `r`, pad extend.
    Radial {
        c: (f64, f64),
        r: f64,
        stops: Vec<(f32, Rgba)>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Fill {
        path: Path,
        paint: Paint,
        rule: FillRule,
        transform: Affine,
    },
    /// Butt caps, bevel joins.
    Stroke {
        path: Path,
        paint: Paint,
        width: f64,
        transform: Affine,
    },
    /// Children are clipped to `path` (non-zero rule) under `transform`.
    Clip {
        path: Path,
        transform: Affine,
        children: Vec<Item>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub name: &'static str,
    pub width: u16,
    pub height: u16,
    /// Opaque background colour.
    pub background: Rgba,
    pub items: Vec<Item>,
    /// What the scene exercises.
    pub covers: &'static str,
}

// ---- building blocks ------------------------------------------------------------------------------------------

impl Path {
    pub fn new() -> Self {
        Self(Vec::new())
    }
    pub fn m(mut self, x: f64, y: f64) -> Self {
        self.0.push(Seg::MoveTo(x, y));
        self
    }
    pub fn l(mut self, x: f64, y: f64) -> Self {
        self.0.push(Seg::LineTo(x, y));
        self
    }
    pub fn c(mut self, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) -> Self {
        self.0.push(Seg::CubicTo(x1, y1, x2, y2, x, y));
        self
    }
    pub fn z(mut self) -> Self {
        self.0.push(Seg::Close);
        self
    }
    pub fn append(mut self, other: Path) -> Self {
        self.0.extend(other.0);
        self
    }
}

/// Point on the unit circle for parameter `t` (rational parametrisation; `t` in [-1, 1] covers a half circle).
pub fn circle_point(t: f64) -> (f64, f64) {
    let d = 1.0 + t * t;
    ((1.0 - t * t) / d, (2.0 * t) / d)
}

/// `n` points on the unit circle, distinct directions, counter-clockwise, all from exact arithmetic.
pub fn circle_points(n: usize) -> Vec<(f64, f64)> {
    // Quarter circle from t in [0, 1]; the other quarters by exact sign swaps.
    let q = n.div_ceil(4).max(1);
    let mut quarter = Vec::with_capacity(q);
    for i in 0..q {
        quarter.push(circle_point(i as f64 / q as f64));
    }
    let mut out = Vec::with_capacity(4 * q);
    for &(x, y) in &quarter {
        out.push((x, y));
    }
    for &(x, y) in &quarter {
        out.push((-y, x));
    }
    for &(x, y) in &quarter {
        out.push((-x, -y));
    }
    for &(x, y) in &quarter {
        out.push((y, -x));
    }
    out
}

/// Circle as four cubics (standard 0.5522847498 control-point factor, a literal).
pub fn circle(cx: f64, cy: f64, r: f64) -> Path {
    let k = 0.552_284_749_830_793_4 * r;
    Path::new()
        .m(cx + r, cy)
        .c(cx + r, cy + k, cx + k, cy + r, cx, cy + r)
        .c(cx - k, cy + r, cx - r, cy + k, cx - r, cy)
        .c(cx - r, cy - k, cx - k, cy - r, cx, cy - r)
        .c(cx + k, cy - r, cx + r, cy - k, cx + r, cy)
        .z()
}

pub fn rect(x: f64, y: f64, w: f64, h: f64) -> Path {
    Path::new().m(x, y).l(x + w, y).l(x + w, y + h).l(x, y + h).z()
}

/// Rotation about `(cx, cy)` by the angle of circle parameter `t` (exact: built from `circle_point`).
pub fn rotation_about(t: f64, cx: f64, cy: f64) -> Affine {
    let (c, s) = circle_point(t);
    [c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy]
}

pub fn translate(x: f64, y: f64) -> Affine {
    [1.0, 0.0, 0.0, 1.0, x, y]
}

pub fn scale_about(s: f64, cx: f64, cy: f64) -> Affine {
    [s, 0.0, 0.0, s, cx - s * cx, cy - s * cy]
}

/// Deterministic integer PRNG (SplitMix64); values are mapped to multiples of 1/64 so fixtures are exact.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// A value in [0, max) on a 1/64 grid.
    pub fn grid(&mut self, max: u32) -> f64 {
        (self.next_u64() % (u64::from(max) * 64)) as f64 / 64.0
    }
    pub fn byte(&mut self) -> u8 {
        (self.next_u64() & 0xFF) as u8
    }
}

fn fill(path: Path, paint: Paint) -> Item {
    Item::Fill { path, paint, rule: FillRule::NonZero, transform: IDENTITY }
}

fn fill_t(path: Path, paint: Paint, transform: Affine) -> Item {
    Item::Fill { path, paint, rule: FillRule::NonZero, transform }
}

fn fill_eo(path: Path, paint: Paint) -> Item {
    Item::Fill { path, paint, rule: FillRule::EvenOdd, transform: IDENTITY }
}

fn stroke(path: Path, paint: Paint, width: f64) -> Item {
    Item::Stroke { path, paint, width, transform: IDENTITY }
}

fn solid(r: u8, g: u8, b: u8, a: u8) -> Paint {
    Paint::Solid([r, g, b, a])
}

const WHITE: Rgba = [255, 255, 255, 255];

fn scene(name: &'static str, covers: &'static str, items: Vec<Item>) -> Scene {
    Scene { name, width: 128, height: 128, background: WHITE, items, covers }
}

/// A glyph-like outline resembling a lower-case "e": outer bowl, inner counter (reversed), cross bar.
fn glyph_e() -> Path {
    Path::new()
        .m(96.0, 70.0)
        .l(36.0, 70.0)
        .c(38.0, 88.0, 50.0, 98.0, 66.0, 98.0)
        .c(78.0, 98.0, 86.0, 93.0, 92.0, 86.0)
        .l(100.0, 94.0)
        .c(92.0, 104.0, 80.0, 110.0, 65.0, 110.0)
        .c(40.0, 110.0, 24.0, 92.0, 24.0, 68.0)
        .c(24.0, 44.0, 41.0, 26.0, 64.0, 26.0)
        .c(86.0, 26.0, 98.0, 42.0, 98.0, 62.0)
        .l(98.0, 66.0)
        .c(98.0, 68.0, 97.0, 70.0, 96.0, 70.0)
        .z()
        // counter (reverse direction)
        .m(37.0, 59.0)
        .l(85.0, 59.0)
        .c(84.0, 46.0, 76.0, 38.0, 63.0, 38.0)
        .c(50.0, 38.0, 40.0, 46.0, 37.0, 59.0)
        .z()
}

fn star(cx: f64, cy: f64, r: f64, points: usize, step: usize) -> Path {
    let pts = circle_points(points * 4);
    let n = pts.len();
    let stride = n / points;
    let mut p = Path::new();
    for i in 0..=points {
        let (x, y) = pts[(i * step * stride) % n];
        let (x, y) = (cx + r * x, cy + r * y);
        p = if i == 0 { p.m(x, y) } else { p.l(x, y) };
    }
    p.z()
}

/// All fixture scenes, in a fixed order.
pub fn scenes() -> Vec<Scene> {
    let mut out = Vec::new();

    out.push(scene(
        "cubic_blobs",
        "filled cubic Bézier shapes with inflections",
        vec![
            fill(
                Path::new()
                    .m(10.0, 64.0)
                    .c(10.0, 10.0, 118.0, 10.0, 118.0, 64.0)
                    .c(118.0, 100.0, 64.0, 70.0, 40.0, 118.0)
                    .c(30.0, 100.0, 10.0, 90.0, 10.0, 64.0)
                    .z(),
                solid(30, 90, 200, 255),
            ),
            fill(
                Path::new()
                    .m(20.0, 20.0)
                    .c(120.0, 0.0, 0.0, 128.0, 108.0, 108.0)
                    .c(64.0, 90.0, 40.0, 60.0, 20.0, 20.0)
                    .z(),
                solid(220, 60, 40, 180),
            ),
        ],
    ));

    let mut thin = Vec::new();
    for (i, w) in [0.1, 0.25, 0.5, 0.75, 1.0, 1.5].iter().enumerate() {
        let y = 10.0 + 20.0 * i as f64;
        thin.push(stroke(Path::new().m(6.0, y).l(122.0, y + 6.0), solid(0, 0, 0, 255), *w));
        thin.push(stroke(
            Path::new().m(6.0 + 20.0 * i as f64, 122.0).l(10.0 + 20.0 * i as f64, 4.0),
            solid(0, 0, 160, 255),
            *w,
        ));
    }
    out.push(scene("thin_strokes", "hairline and sub-pixel stroke widths 0.1–1.5 px, near-horizontal/vertical", thin));

    let mut wedges = Vec::new();
    for (x, y) in circle_points(64) {
        wedges.push(fill(
            Path::new()
                .m(64.0, 64.0)
                .l(64.0 + 60.0 * x, 64.0 + 60.0 * y)
                .l(64.0 + 60.0 * x - 2.0 * y, 64.0 + 60.0 * y + 2.0 * x)
                .z(),
            solid(0, 0, 0, 255),
        ));
    }
    out.push(scene("aa_edge_angles", "anti-aliased edges at 64 directions (thin wedges)", wedges));

    out.push(scene(
        "translucent_overlap",
        "overlapping translucent fills (alpha 128) in RGB",
        vec![
            fill(circle(48.0, 48.0, 34.0), solid(255, 0, 0, 128)),
            fill(circle(80.0, 48.0, 34.0), solid(0, 255, 0, 128)),
            fill(circle(64.0, 80.0, 34.0), solid(0, 0, 255, 128)),
        ],
    ));

    let mut stack = Vec::new();
    for i in 0..20 {
        let o = 4.0 * i as f64;
        stack.push(fill(rect(4.0 + o, 4.0 + o, 60.0, 60.0), solid(40, 40, 120, 26)));
    }
    out.push(scene("alpha_stack", "20 layers of alpha 26/255 (accumulated rounding)", stack));

    let stops = vec![(0.0, [255, 0, 0, 255]), (0.5, [255, 255, 0, 255]), (1.0, [0, 0, 255, 255])];
    out.push(scene(
        "linear_gradient",
        "linear gradient, three stops, pad",
        vec![fill(
            rect(8.0, 8.0, 112.0, 112.0),
            Paint::Linear { p0: (8.0, 8.0), p1: (120.0, 100.0), stops: stops.clone() },
        )],
    ));
    out.push(scene(
        "radial_gradient",
        "radial gradient, translucent outer stop, pad",
        vec![fill(
            circle(64.0, 64.0, 58.0),
            Paint::Radial {
                c: (54.0, 54.0),
                r: 60.0,
                stops: vec![(0.0, [255, 255, 255, 255]), (0.6, [0, 128, 255, 255]), (1.0, [0, 0, 64, 96])],
            },
        )],
    ));
    out.push(scene(
        "gradient_transformed",
        "linear gradient under a rotation (rational sine/cosine)",
        vec![fill_t(
            rect(24.0, 24.0, 80.0, 80.0),
            Paint::Linear { p0: (24.0, 64.0), p1: (104.0, 64.0), stops },
            rotation_about(0.25, 64.0, 64.0),
        )],
    ));

    let mut clipped = Vec::new();
    for i in 0..16 {
        let x = 8.0 * i as f64;
        clipped.push(fill(rect(x, 0.0, 4.0, 128.0), solid(200, 30, 30, 255)));
    }
    out.push(scene(
        "clip_circle",
        "stripes clipped to a circle",
        vec![Item::Clip { path: circle(64.0, 64.0, 50.0), transform: IDENTITY, children: clipped }],
    ));
    out.push(scene(
        "clip_nested",
        "nested clips (rotated square inside circle), translucent content",
        vec![Item::Clip {
            path: circle(64.0, 64.0, 56.0),
            transform: IDENTITY,
            children: vec![
                fill(rect(0.0, 0.0, 128.0, 128.0), solid(255, 200, 0, 255)),
                Item::Clip {
                    path: rect(34.0, 34.0, 60.0, 60.0),
                    transform: rotation_about(0.4, 64.0, 64.0),
                    children: vec![fill(circle(64.0, 64.0, 40.0), solid(0, 80, 200, 160))],
                },
            ],
        }],
    ));

    out.push(scene(
        "glyph_nonzero",
        "glyph-like outline with a reversed counter, non-zero",
        vec![fill(glyph_e(), solid(0, 0, 0, 255))],
    ));
    out.push(scene("glyph_evenodd", "the same outline, even-odd", vec![fill_eo(glyph_e(), solid(0, 0, 0, 255))]));

    out.push(scene(
        "degenerate",
        "zero-area, collinear, zero-length and empty paths (should draw nothing visible)",
        vec![
            fill(Path::new().m(10.0, 10.0).l(100.0, 100.0).l(55.0, 55.0).z(), solid(0, 0, 0, 255)),
            fill(Path::new().m(20.0, 20.0).l(20.0, 20.0).l(20.0, 20.0).z(), solid(0, 0, 0, 255)),
            fill(Path::new(), solid(0, 0, 0, 255)),
            fill(Path::new().m(30.0, 90.0).c(30.0, 90.0, 30.0, 90.0, 30.0, 90.0).z(), solid(0, 0, 0, 255)),
            stroke(Path::new().m(64.0, 64.0).l(64.0, 64.0), solid(255, 0, 0, 255), 4.0),
            fill(rect(40.0, 40.0, 48.0, 0.0), solid(0, 0, 255, 255)),
        ],
    ));

    out.push(scene(
        "star_nonzero",
        "self-intersecting pentagram, non-zero (centre filled)",
        vec![fill(star(64.0, 64.0, 58.0, 5, 2), solid(120, 0, 160, 255))],
    ));
    out.push(scene(
        "star_evenodd",
        "self-intersecting pentagram, even-odd (centre empty)",
        vec![fill_eo(star(64.0, 64.0, 58.0, 5, 2), solid(120, 0, 160, 255))],
    ));

    out.push(scene(
        "coincident_edges",
        "abutting squares (shared edges, seams) and a path cancelled by its reverse",
        vec![
            fill(rect(16.0, 16.0, 32.0, 32.0), solid(0, 120, 0, 255)),
            fill(rect(48.0, 16.0, 32.0, 32.0), solid(0, 120, 0, 255)),
            fill(rect(16.5, 60.25, 31.75, 32.0), solid(0, 0, 120, 255)),
            fill(rect(48.25, 60.25, 31.75, 32.0), solid(0, 0, 120, 255)),
            fill(
                rect(84.0, 16.0, 32.0, 96.0)
                    .append(Path::new().m(84.0, 16.0).l(84.0, 112.0).l(116.0, 112.0).l(116.0, 16.0).z()),
                solid(255, 0, 0, 255),
            ),
        ],
    ));

    out.push(scene(
        "extreme_coordinates",
        "huge (±1e6..1e7) and tiny (1e-3 px) geometry crossing the canvas",
        vec![
            fill(Path::new().m(-1.0e6, -1.0e6).l(1.0e7, 64.0).l(-1.0e6, 1.0e6).z(), solid(255, 128, 0, 200)),
            fill(Path::new().m(64.0, -4.0e6).l(70.0, 4.0e6).l(66.0, 4.0e6).z(), solid(0, 0, 0, 255)),
            fill(rect(20.0, 20.0, 0.001, 0.001), solid(0, 0, 0, 255)),
            fill(circle(100.0, 100.0, 0.25), solid(0, 0, 0, 255)),
        ],
    ));

    let mut sub = Vec::new();
    for i in 0..8 {
        let dx = i as f64 / 8.0;
        sub.push(fill_t(rect(4.0, 4.0 + 15.0 * i as f64, 40.0, 10.0), solid(0, 0, 0, 255), translate(dx, dx / 2.0)));
        sub.push(fill_t(circle(80.0, 9.0 + 15.0 * i as f64, 6.0), solid(0, 0, 0, 255), translate(dx, 0.0)));
    }
    out.push(scene("subpixel_translation", "identical shapes translated by k/8 px", sub));

    let mut scales = Vec::new();
    for (i, s) in [0.37, 0.5, 1.5, 2.71].iter().enumerate() {
        let x = 16.0 + 30.0 * i as f64;
        scales.push(fill_t(star(x, 32.0, 10.0, 5, 2), solid(0, 0, 0, 255), scale_about(*s, x, 32.0)));
        scales.push(Item::Stroke {
            path: circle(x, 92.0, 9.0),
            paint: solid(200, 0, 0, 255),
            width: 1.0,
            transform: scale_about(*s, x, 92.0),
        });
    }
    out.push(scene("non_integer_scales", "fills and strokes under scales 0.37, 0.5, 1.5, 2.71", scales));

    out.push(scene(
        "stroked_curves",
        "stroked cubics, widths 1–12, bevel joins, butt caps",
        vec![
            stroke(Path::new().m(10.0, 100.0).c(30.0, 10.0, 90.0, 10.0, 118.0, 100.0), solid(0, 90, 160, 255), 12.0),
            stroke(Path::new().m(10.0, 20.0).c(60.0, 120.0, 70.0, -20.0, 118.0, 60.0), solid(200, 40, 0, 200), 4.0),
            stroke(Path::new().m(20.0, 64.0).l(60.0, 40.0).l(100.0, 64.0).l(60.0, 88.0).z(), solid(0, 0, 0, 255), 1.0),
        ],
    ));

    let mut rng = Rng::new(0x0C03_0001);
    let mut many = Vec::new();
    for _ in 0..500 {
        let (x, y) = (rng.grid(124), rng.grid(124));
        let p =
            Path::new().m(x, y).l(x + rng.grid(8), y + rng.grid(8) - 4.0).l(x + rng.grid(8) - 4.0, y + rng.grid(8)).z();
        let c = solid(rng.byte(), rng.byte(), rng.byte(), 64 + rng.byte() / 2);
        many.push(fill(p, c));
    }
    out.push(scene("many_small_paths", "500 random small translucent triangles (seeded, 1/64 grid)", many));

    let mut grid = Vec::new();
    for i in 0..16 {
        let o = 4.0 + 8.0 * i as f64 + if i % 2 == 0 { 0.5 } else { 0.0 };
        grid.push(stroke(Path::new().m(o, 2.0).l(o, 126.0), solid(0, 0, 0, 255), 1.0));
        grid.push(stroke(Path::new().m(2.0, o).l(126.0, o), solid(0, 0, 0, 160), 1.0));
    }
    out.push(scene("hairline_grid", "1 px lines on and between pixel centres, crossing", grid));

    let mut big = Vec::new();
    let mut rng = Rng::new(0x0C03_0002);
    for _ in 0..2000 {
        let (x, y) = (rng.grid(512), rng.grid(512));
        let r = 2.0 + rng.grid(24);
        let p = Path::new()
            .m(x, y)
            .c(x + r, y - r, x + 2.0 * r, y + r, x + r, y + 2.0 * r)
            .c(x, y + r, x - r, y + r, x, y)
            .z();
        big.push(fill(p, solid(rng.byte(), rng.byte(), rng.byte(), 96 + rng.byte() / 4)));
    }
    out.push(Scene {
        name: "perf_2000_cubics_512",
        width: 512,
        height: 512,
        background: WHITE,
        items: big,
        covers: "speed: 2000 translucent cubic shapes at 512×512",
    });
    out
}
