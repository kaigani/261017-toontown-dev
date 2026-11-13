//! Motion paths (05 §9, §10.5): the Kochanek–Bartels curve with reflected end neighbours, the 64-step arc-length
//! table, and travel along the path by the velocity channel. Passage-frame computation (timing) is separate from the
//! geometry; every evaluation runs in the fixed order local frame → velocity → arc length → curve (ADR-0019).

use tt_model::entities::channel::{CurveType, MotionPoint};

use crate::function::{self, KeyView};

/// Parameter steps per segment (05 §9.2): 65 samples.
pub const SAMPLES_PER_SEGMENT: usize = 64;

/// The accuracy 05 §10.5 requires of a passage frame before it is reported to 3 decimals. The bisection goes past it,
/// to the bracket's floating-point resolution, so the 3-decimal rounding is that of the crossing itself and a crossing
/// within 1e-4 below a rounding boundary is not carried across it (C32-R1-01).
pub const PASSAGE_TOLERANCE: f64 = 1e-4;

/// Upper bound of the passage bisection: a bracket is one monotone piece of a keyframe segment, at most 100 000 frames,
/// and halving that 64 times leaves under 1e-14 frame, below the spacing of doubles there; the bisection stops earlier
/// when the midpoint is no longer strictly inside the bracket (ADR-0019).
pub const PASSAGE_MAX_ITERATIONS: u32 = 64;

/// Relative tolerance of a touch: a candidate frame (a node or an extremum of the velocity curve) whose value is within
/// `PASSAGE_TOUCH · max(1, |arc|)` below the arc reaches it. An extremum that equals the arc exactly (a tangent touch)
/// evaluates one rounding error below it in doubles; without this the touch is missed and the later crossing reported
/// (found by the 4096-case passage property in M3 C3.4 round 3: a maximum of exactly 0.9 at frame 3 reported 3.6).
pub const PASSAGE_TOUCH: f64 = 1e-12;

/// A tangent below this length has no direction (05 §8.3).
pub const MIN_TANGENT: f64 = 1e-9;

/// A motion point as the curve reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// Position (animated, parent space).
    pub pos: [f64; 3],
    /// Tension.
    pub t: f64,
    /// Continuity.
    pub c: f64,
    /// Bias.
    pub b: f64,
    /// Keyframe frame (local); `None` = control point.
    pub frame: Option<f64>,
    /// The segment starting at this keyframe holds the position.
    pub constant: bool,
    /// Constant Z for the segment starting at this keyframe.
    pub lock_depth: bool,
}

impl From<&MotionPoint> for Point {
    fn from(p: &MotionPoint) -> Self {
        let q = p.position();
        Self {
            pos: [q.x, q.y, q.z],
            t: *p.tension(),
            c: *p.continuity(),
            b: *p.bias(),
            frame: p.frame().map(f64::from),
            constant: *p.constant_segment(),
            lock_depth: *p.lock_depth(),
        }
    }
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// `Dout(i)` and `Din(i)` of 05 §9.1 for the neighbour differences `before = P[i] − P[i−1]`, `after = P[i+1] − P[i]`.
#[must_use]
pub fn tangents(p: &Point, before: [f64; 3], after: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let (t, c, b) = (p.t, p.c, p.b);
    let dout = add(
        scale(before, (1.0 - t) * (1.0 + b) * (1.0 + c) / 2.0),
        scale(after, (1.0 - t) * (1.0 - b) * (1.0 - c) / 2.0),
    );
    let din = add(
        scale(before, (1.0 - t) * (1.0 + b) * (1.0 - c) / 2.0),
        scale(after, (1.0 - t) * (1.0 - b) * (1.0 + c) / 2.0),
    );
    (dout, din)
}

/// A motion path with its tangents and arc-length table.
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    /// The points in path order.
    pub points: Vec<Point>,
    dout: Vec<[f64; 3]>,
    din: Vec<[f64; 3]>,
    /// Cumulative chord length at each sample: `segments × 64 + 1` values.
    cum: Vec<f64>,
    /// Indices of the keyframes among `points`.
    keys: Vec<usize>,
}

impl Path {
    /// Work units a path of `n` points costs to build.
    #[must_use]
    pub fn cost(n: usize) -> u64 {
        u64::try_from(n.saturating_mul(SAMPLES_PER_SEGMENT + 1)).unwrap_or(u64::MAX)
    }

    /// Builds the tangents and the table. `node_lock_depth` is the node's Constant Z flag (§9.3): within a locked
    /// keyframe segment chords are measured in the X/Y plane, so control-point Z does not affect timing (SI-099).
    #[must_use]
    pub fn new(points: Vec<Point>, node_lock_depth: bool) -> Self {
        let n = points.len();
        let mut dout = Vec::with_capacity(n);
        let mut din = Vec::with_capacity(n);
        for (i, p) in points.iter().enumerate() {
            if n < 2 {
                dout.push([0.0; 3]);
                din.push([0.0; 3]);
                continue;
            }
            // Reflected phantom neighbours: P[−1] = 2·P[0] − P[1], P[n] = 2·P[n−1] − P[n−2].
            let before = if i == 0 { sub(points[1].pos, points[0].pos) } else { sub(p.pos, points[i - 1].pos) };
            let after = if i + 1 == n { sub(p.pos, points[i - 1].pos) } else { sub(points[i + 1].pos, p.pos) };
            let (o, a) = tangents(p, before, after);
            dout.push(o);
            din.push(a);
        }
        let keys: Vec<usize> = points.iter().enumerate().filter(|(_, p)| p.frame.is_some()).map(|(i, _)| i).collect();
        let mut path = Self { points, dout, din, cum: vec![0.0], keys };
        let mut total = 0.0;
        let mut locked = false;
        for seg in 0..n.saturating_sub(1) {
            if let Some(f) = path.points.get(seg).filter(|p| p.frame.is_some()) {
                locked = node_lock_depth || f.lock_depth;
            }
            let mut prev = path.curve(seg, 0.0);
            for i in 1..=SAMPLES_PER_SEGMENT {
                let q = path.curve(seg, i as f64 / SAMPLES_PER_SEGMENT as f64);
                let d = sub(q, prev);
                let z = if locked { 0.0 } else { d[2] * d[2] };
                total += (d[0] * d[0] + d[1] * d[1] + z).sqrt();
                path.cum.push(total);
                prev = q;
            }
        }
        path
    }

    /// C(s) of segment `seg`, s ∈ [0, 1] (05 §9.1).
    #[must_use]
    pub fn curve(&self, seg: usize, s: f64) -> [f64; 3] {
        let (Some(p0), Some(p1)) = (self.points.get(seg), self.points.get(seg + 1)) else {
            return self.points.get(seg).map_or([0.0; 3], |p| p.pos);
        };
        let (d0, d1) = (self.dout[seg], self.din[seg + 1]);
        let s2 = s * s;
        let s3 = s2 * s;
        let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
        let h10 = s3 - 2.0 * s2 + s;
        let h01 = -2.0 * s3 + 3.0 * s2;
        let h11 = s3 - s2;
        let mut out = [0.0; 3];
        for (a, o) in out.iter_mut().enumerate() {
            *o = h00 * p0.pos[a] + h10 * d0[a] + h01 * p1.pos[a] + h11 * d1[a];
        }
        out
    }

    /// C′(s) of segment `seg`.
    #[must_use]
    pub fn derivative(&self, seg: usize, s: f64) -> [f64; 3] {
        let (Some(p0), Some(p1)) = (self.points.get(seg), self.points.get(seg + 1)) else { return [0.0; 3] };
        let (d0, d1) = (self.dout[seg], self.din[seg + 1]);
        let s2 = s * s;
        let h00 = 6.0 * s2 - 6.0 * s;
        let h10 = 3.0 * s2 - 4.0 * s + 1.0;
        let h01 = -6.0 * s2 + 6.0 * s;
        let h11 = 3.0 * s2 - 2.0 * s;
        let mut out = [0.0; 3];
        for (a, o) in out.iter_mut().enumerate() {
            *o = h00 * p0.pos[a] + h10 * d0[a] + h01 * p1.pos[a] + h11 * d1[a];
        }
        out
    }

    /// Number of curve segments.
    #[must_use]
    pub fn segments(&self) -> usize {
        self.points.len().saturating_sub(1)
    }

    /// Arc length at point `i`.
    #[must_use]
    pub fn arc_of_point(&self, i: usize) -> f64 {
        self.cum.get(i.saturating_mul(SAMPLES_PER_SEGMENT)).copied().unwrap_or(0.0)
    }

    /// Total length.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.cum.last().copied().unwrap_or(0.0)
    }

    /// Point indices of the keyframes.
    #[must_use]
    pub fn key_indices(&self) -> &[usize] {
        &self.keys
    }

    /// The local frame of keyframe `j` (index among the keyframes).
    #[must_use]
    pub fn key_frame(&self, j: usize) -> f64 {
        self.keys.get(j).and_then(|i| self.points[*i].frame).unwrap_or(1.0)
    }

    /// The arc length `S_j` of keyframe `j`.
    #[must_use]
    pub fn key_arc(&self, j: usize) -> f64 {
        self.keys.get(j).map_or(0.0, |i| self.arc_of_point(*i))
    }

    /// The location `(segment, s)` at arc length `arc` between points `lo` and `hi` (05 §9.2): the bracketing samples
    /// are located, the parameter interpolated linearly between them.
    #[must_use]
    pub fn locate(&self, arc: f64, lo: usize, hi: usize) -> (usize, f64) {
        let (a, b) = (lo * SAMPLES_PER_SEGMENT, hi * SAMPLES_PER_SEGMENT);
        if hi <= lo || arc <= self.cum.get(a).copied().unwrap_or(0.0) {
            return (lo, 0.0);
        }
        if arc >= self.cum.get(b).copied().unwrap_or(0.0) {
            return (hi - 1, 1.0);
        }
        let idx = a + self.cum[a..=b].partition_point(|v| *v < arc);
        let i = idx - 1;
        let (c0, c1) = (self.cum[i], self.cum[idx]);
        let t = if c1 > c0 { (arc - c0) / (c1 - c0) } else { 0.0 };
        let seg = i / SAMPLES_PER_SEGMENT;
        let within = (i - seg * SAMPLES_PER_SEGMENT) as f64;
        (seg, (within + t) / SAMPLES_PER_SEGMENT as f64)
    }
}

/// The `velocity` channel as stored (05 §10.5): shape data and intermediate keys normalised to their keyframe segment.
#[derive(Debug, Clone, PartialEq)]
pub struct Velocity {
    /// `spline` or `interpolation`.
    pub curve: CurveType,
    /// Keys sorted by local frame; `value` is `u`. A key at a pin frame carries shape only.
    pub keys: Vec<KeyView>,
}

impl Default for Velocity {
    fn default() -> Self {
        Self { curve: CurveType::Interpolation, keys: Vec::new() }
    }
}

impl Velocity {
    /// `u(f)` in the keyframe segment `[f0, f1]`, strictly inside it.
    /// The curve's nodes in the keyframe segment `[f0, f1]`: the start pin (u = 0), the keys strictly inside, the end
    /// pin (u = 1), each pin with the shape of a key record at its frame.
    fn nodes(&self, f0: f64, f1: f64) -> Vec<KeyView> {
        let shape = |frame: f64, u: f64| -> KeyView {
            let i = self.keys.partition_point(|k| k.frame < frame);
            match self.keys.get(i).filter(|k| k.frame == frame) {
                Some(k) => KeyView { frame, value: u, constant: false, ..*k },
                None => KeyView::plain(frame, u),
            }
        };
        let mut out = vec![shape(f0, 0.0)];
        out.extend(
            self.keys.iter().filter(|k| k.frame > f0 && k.frame < f1).map(|k| KeyView { constant: false, ..*k }),
        );
        out.push(shape(f1, 1.0));
        out
    }

    fn u(&self, f0: f64, f1: f64, f: f64) -> f64 {
        let shape = |frame: f64, u: f64| -> KeyView {
            let i = self.keys.partition_point(|k| k.frame < frame);
            match self.keys.get(i).filter(|k| k.frame == frame) {
                Some(k) => KeyView { frame, value: u, constant: false, ..*k },
                None => KeyView::plain(frame, u),
            }
        };
        let i = self.keys.partition_point(|k| k.frame <= f);
        let a = i
            .checked_sub(1)
            .and_then(|j| self.keys.get(j))
            .filter(|k| k.frame > f0)
            .map_or_else(|| shape(f0, 0.0), |k| KeyView { constant: false, ..*k });
        if a.frame == f {
            return a.value;
        }
        let b = self
            .keys
            .get(i)
            .filter(|k| k.frame < f1)
            .map_or_else(|| shape(f1, 1.0), |k| KeyView { constant: false, ..*k });
        function::segment_value(self.curve, &a, &b, f, None)
    }

    /// V(f) on `path`: pinned at keyframes, held outside them, `S_j + u(f)·(S_{j+1} − S_j)` between (05 §10.5). Not
    /// clamped: [`Path`] travel clamps it per segment.
    #[must_use]
    pub fn value(&self, path: &Path, f: f64) -> f64 {
        let n = path.key_indices().len();
        if n == 0 {
            return 0.0;
        }
        if f <= path.key_frame(0) {
            return path.key_arc(0);
        }
        if f >= path.key_frame(n - 1) {
            return path.key_arc(n - 1);
        }
        let j = key_segment(path, f);
        let (f0, f1) = (path.key_frame(j), path.key_frame(j + 1));
        let (s0, s1) = (path.key_arc(j), path.key_arc(j + 1));
        if f == f0 {
            return s0;
        }
        s0 + self.u(f0, f1, f) * (s1 - s0)
    }

    /// The smallest frame in the keyframe segment `[f0, f1]` with `V(f) ≥ arc`, within [`PASSAGE_TOLERANCE`], rounded to 3
    /// decimals (05 §10.5). Each velocity sub-segment (between consecutive nodes) is a cubic or a line in its
    /// parameter; its value extrema ([`function::extremum_frames`]) split it into monotone pieces. The candidate frames
    /// — every node and every extremum, in order — are evaluated until the first one with `V ≥ arc`; the crossing lies
    /// in the monotone piece before it, which is bisected (C32-R1-01: a sub-frame excursion is not skipped). A crossing
    /// exactly at a candidate frame reports that frame. `spend` is charged per curve evaluation.
    pub fn passage<E>(
        &self,
        path: &Path,
        j: usize,
        arc: f64,
        spend: &mut dyn FnMut(u64) -> Result<(), E>,
    ) -> Result<f64, E> {
        let (f0, f1) = (path.key_frame(j), path.key_frame(j + 1));
        if path.key_arc(j + 1) <= path.key_arc(j) || arc <= path.key_arc(j) {
            return Ok(round3(f0));
        }
        let nodes = self.nodes(f0, f1);
        let mut candidates: Vec<f64> = Vec::with_capacity(nodes.len() * 3);
        for w in nodes.windows(2) {
            candidates.push(w[0].frame);
            if let Some((x, y)) = function::segment_bezier(self.curve, &w[0], &w[1]) {
                candidates.extend(function::extremum_frames(x, y));
            }
        }
        candidates.push(f1);
        let (mut lo, mut hi) = (f0, f1);
        let touch = PASSAGE_TOUCH * arc.abs().max(1.0);
        for f in candidates.into_iter().skip(1) {
            spend(1)?;
            if self.value(path, f) >= arc - touch {
                hi = f;
                break;
            }
            lo = f;
        }
        for _ in 0..PASSAGE_MAX_ITERATIONS {
            let mid = (lo + hi) / 2.0;
            if mid <= lo || mid >= hi {
                break;
            }
            spend(1)?;
            if self.value(path, mid) >= arc {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        Ok(round3(hi))
    }
}

/// Rounded to 3 decimals, half up.
#[must_use]
pub fn round3(v: f64) -> f64 {
    libm::floor(v * 1000.0 + 0.5) / 1000.0
}

/// The keyframe segment index `j` with `f_j ≤ f < f_{j+1}`, for `f` inside the keyframes' span.
#[must_use]
pub fn key_segment(path: &Path, f: f64) -> usize {
    let n = path.key_indices().len();
    let mut lo = 0;
    let mut hi = n.saturating_sub(1);
    while lo + 1 < hi {
        let mid = (lo + hi) / 2;
        if path.key_frame(mid) <= f {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Where a node is on its path at a frame, with what orient-to-path needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Travel {
    /// Animated position.
    pub position: [f64; 3],
    /// Curve segment and parameter; `None` on a constant segment.
    pub at: Option<(usize, f64)>,
}

/// The node's animated position at local frame `f` (05 §2.9, §9.3, §10.5).
#[must_use]
pub fn travel(path: &Path, velocity: &Velocity, node_lock_depth: bool, f: f64) -> Travel {
    let n = path.key_indices().len();
    if n == 0 {
        return Travel { position: path.points.first().map_or([0.0; 3], |p| p.pos), at: None };
    }
    let last = n - 1;
    if f <= path.key_frame(0) {
        let i = path.key_indices()[0];
        let constant = path.points[i].constant;
        return Travel { position: path.points[i].pos, at: (!constant && n > 1).then_some((i, 0.0)) };
    }
    if f >= path.key_frame(last) {
        let i = path.key_indices()[last];
        let tweened = last > 0 && !path.points[path.key_indices()[last - 1]].constant;
        return Travel { position: path.points[i].pos, at: (tweened && i > 0).then(|| (i - 1, 1.0)) };
    }
    let j = key_segment(path, f);
    let (a, b) = (path.key_indices()[j], path.key_indices()[j + 1]);
    let start = path.points[a];
    if f == path.key_frame(j) {
        return Travel { position: start.pos, at: (!start.constant).then_some((a, 0.0)) };
    }
    if start.constant {
        return Travel { position: start.pos, at: None };
    }
    let (s0, s1) = (path.key_arc(j), path.key_arc(j + 1));
    let arc = velocity.value(path, f).clamp(s0, s1.max(s0));
    let (seg, s) = path.locate(arc, a, b);
    let mut position = path.curve(seg, s);
    if node_lock_depth || start.lock_depth {
        position[2] = start.pos[2];
    }
    Travel { position, at: Some((seg, s)) }
}

/// The orient-to-path angle at a location (05 §8.3): atan2 of the tangent in X/Y; on a constant segment or where the
/// tangent vanishes, the last valid angle searching backward in time along the path, else the next valid one forward,
/// else 0. `spend` is charged per sample looked at.
pub fn orient_angle<E>(
    path: &Path,
    at: Option<(usize, f64)>,
    f: f64,
    spend: &mut dyn FnMut(u64) -> Result<(), E>,
) -> Result<f64, E> {
    let direction = |seg: usize, s: f64| -> Option<f64> {
        let d = path.derivative(seg, s);
        ((d[0] * d[0] + d[1] * d[1]).sqrt() >= MIN_TANGENT).then(|| crate::num::atan2_deg(d[1], d[0]))
    };
    if let Some((seg, s)) = at
        && let Some(a) = direction(seg, s)
    {
        return Ok(a);
    }
    let segments = path.segments();
    if segments == 0 {
        return Ok(0.0);
    }
    // Which curve segments are travelled (not inside a constant keyframe segment).
    let mut tweened = vec![false; segments];
    let mut on = false;
    for (seg, slot) in tweened.iter_mut().enumerate() {
        if let Some(p) = path.points.get(seg).filter(|p| p.frame.is_some()) {
            on = !p.constant;
        }
        *slot = on;
    }
    let (start_seg, start_step) = match at {
        Some((seg, s)) => (seg, libm::floor(s * SAMPLES_PER_SEGMENT as f64) as usize),
        None if path.key_indices().is_empty() => (0, 0),
        None => {
            let n = path.key_indices().len();
            let j = if f <= path.key_frame(0) {
                0
            } else if f >= path.key_frame(n - 1) {
                n - 1
            } else {
                key_segment(path, f)
            };
            (path.key_indices()[j].min(segments - 1), 0)
        }
    };
    // Backward from the location, then forward.
    let mut seg = start_seg.min(segments - 1);
    let mut step = start_step.min(SAMPLES_PER_SEGMENT);
    loop {
        spend(1)?;
        if tweened[seg]
            && let Some(a) = direction(seg, step as f64 / SAMPLES_PER_SEGMENT as f64)
        {
            return Ok(a);
        }
        if step > 0 {
            step -= 1;
        } else if seg > 0 {
            seg -= 1;
            step = SAMPLES_PER_SEGMENT;
        } else {
            break;
        }
    }
    let mut seg = start_seg.min(segments - 1);
    let mut step = start_step.min(SAMPLES_PER_SEGMENT);
    loop {
        spend(1)?;
        if tweened[seg]
            && let Some(a) = direction(seg, step as f64 / SAMPLES_PER_SEGMENT as f64)
        {
            return Ok(a);
        }
        if step < SAMPLES_PER_SEGMENT {
            step += 1;
        } else if seg + 1 < segments {
            seg += 1;
            step = 0;
        } else {
            break;
        }
    }
    Ok(0.0)
}
