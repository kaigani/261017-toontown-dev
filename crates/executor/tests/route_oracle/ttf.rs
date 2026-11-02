//! A minimal, independent TrueType reader (C24-R2-01: Break Apart's glyph outlines predicted from the fixture font
//! without the product's font code): the table directory, `head` (units per em, `loca` format), `maxp`, `cmap` format
//! 4, `loca` and simple `glyf` outlines (quadratic B-splines with implied on-curve points), flattened finely.

use std::collections::BTreeMap;

/// A parsed font.
pub struct Font {
    data: Vec<u8>,
    tables: BTreeMap<[u8; 4], (usize, usize)>,
    pub units_per_em: f64,
    long_loca: bool,
    num_glyphs: usize,
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn i16_at(d: &[u8], o: usize) -> Option<i16> {
    Some(i16::from_be_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*d.get(o)?, *d.get(o + 1)?, *d.get(o + 2)?, *d.get(o + 3)?]))
}

impl Font {
    /// Parses the font's table directory and the header tables.
    pub fn parse(data: Vec<u8>) -> Option<Font> {
        let n = usize::from(u16_at(&data, 4)?);
        let mut tables = BTreeMap::new();
        for i in 0..n {
            let r = 12 + 16 * i;
            let tag = [*data.get(r)?, *data.get(r + 1)?, *data.get(r + 2)?, *data.get(r + 3)?];
            tables.insert(tag, (u32_at(&data, r + 8)? as usize, u32_at(&data, r + 12)? as usize));
        }
        let head = tables.get(b"head")?.0;
        let maxp = tables.get(b"maxp")?.0;
        let units_per_em = f64::from(u16_at(&data, head + 18)?);
        let long_loca = i16_at(&data, head + 50)? == 1;
        let num_glyphs = usize::from(u16_at(&data, maxp + 4)?);
        Some(Font { data, tables, units_per_em, long_loca, num_glyphs })
    }

    /// The glyph index of a character (`cmap` format 4, Windows Unicode BMP).
    pub fn glyph_index(&self, c: char) -> Option<u16> {
        let d = &self.data;
        let cmap = self.tables.get(b"cmap")?.0;
        let count = usize::from(u16_at(d, cmap + 2)?);
        let code = u32::from(c);
        for i in 0..count {
            let r = cmap + 4 + 8 * i;
            let (platform, encoding) = (u16_at(d, r)?, u16_at(d, r + 2)?);
            if !((platform == 3 && (encoding == 1 || encoding == 0)) || platform == 0) {
                continue;
            }
            let sub = cmap + u32_at(d, r + 4)? as usize;
            if u16_at(d, sub)? != 4 || code > 0xFFFF {
                continue;
            }
            let segs = usize::from(u16_at(d, sub + 6)? / 2);
            let ends = sub + 14;
            let starts = ends + 2 * segs + 2;
            let deltas = starts + 2 * segs;
            let offsets = deltas + 2 * segs;
            for s in 0..segs {
                let end = u32::from(u16_at(d, ends + 2 * s)?);
                let start = u32::from(u16_at(d, starts + 2 * s)?);
                if code > end {
                    continue;
                }
                if code < start {
                    return None;
                }
                let delta = u16_at(d, deltas + 2 * s)?;
                let ro = u16_at(d, offsets + 2 * s)?;
                if ro == 0 {
                    return Some((code as u16).wrapping_add(delta));
                }
                let at = offsets + 2 * s + usize::from(ro) + 2 * (code - start) as usize;
                let g = u16_at(d, at)?;
                return (g != 0).then(|| g.wrapping_add(delta));
            }
        }
        None
    }

    /// The contours of a simple glyph in font units (y up): each a closed polygon flattened from its quadratic
    /// segments (`steps` per curve). `None` for a composite or unreadable glyph; empty for a blank one.
    pub fn contours(&self, glyph: u16, steps: u32) -> Option<Vec<Vec<(f64, f64)>>> {
        let d = &self.data;
        let g = usize::from(glyph);
        if g >= self.num_glyphs {
            return None;
        }
        let loca = self.tables.get(b"loca")?.0;
        let (a, b) = if self.long_loca {
            (u32_at(d, loca + 4 * g)? as usize, u32_at(d, loca + 4 * g + 4)? as usize)
        } else {
            (2 * usize::from(u16_at(d, loca + 2 * g)?), 2 * usize::from(u16_at(d, loca + 2 * g + 2)?))
        };
        if a == b {
            return Some(Vec::new());
        }
        let o = self.tables.get(b"glyf")?.0 + a;
        let n = i16_at(d, o)?;
        if n < 0 {
            return None;
        }
        let n = n as usize;
        let mut ends = Vec::with_capacity(n);
        for i in 0..n {
            ends.push(usize::from(u16_at(d, o + 10 + 2 * i)?));
        }
        let points = ends.last().map_or(0, |e| e + 1);
        let ins = usize::from(u16_at(d, o + 10 + 2 * n)?);
        let mut p = o + 12 + 2 * n + ins;
        let mut flags = Vec::with_capacity(points);
        while flags.len() < points {
            let f = *d.get(p)?;
            p += 1;
            flags.push(f);
            if f & 8 != 0 {
                let r = *d.get(p)?;
                p += 1;
                for _ in 0..r {
                    flags.push(f);
                }
            }
        }
        flags.truncate(points);
        let coord = |short: u8, same: u8, p: &mut usize| -> Option<Vec<i32>> {
            let mut v = 0i32;
            let mut out = Vec::with_capacity(points);
            for &f in &flags {
                if f & short != 0 {
                    let x = i32::from(*d.get(*p)?);
                    *p += 1;
                    v += if f & same != 0 { x } else { -x };
                } else if f & same == 0 {
                    v += i32::from(i16_at(d, *p)?);
                    *p += 2;
                }
                out.push(v);
            }
            Some(out)
        };
        let xs = coord(2, 16, &mut p)?;
        let ys = coord(4, 32, &mut p)?;
        let mut out = Vec::with_capacity(n);
        let mut start = 0;
        for &e in &ends {
            let pts: Vec<((f64, f64), bool)> =
                (start..=e).map(|i| ((f64::from(xs[i]), f64::from(ys[i])), flags[i] & 1 != 0)).collect();
            start = e + 1;
            out.push(flatten_quadratic(&pts, steps));
        }
        Some(out)
    }
}

/// A closed quadratic B-spline contour (points with on-curve flags) flattened into a polygon.
fn flatten_quadratic(pts: &[((f64, f64), bool)], steps: u32) -> Vec<(f64, f64)> {
    let n = pts.len();
    if n == 0 {
        return Vec::new();
    }
    let mid = |a: (f64, f64), b: (f64, f64)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    // Start at an on-curve point (or the implied one between the first two off-curve points).
    let first = (0..n).find(|&i| pts[i].1);
    let (start, origin) = match first {
        Some(i) => (i, pts[i].0),
        None => (0, mid(pts[0].0, pts[1 % n].0)),
    };
    let mut out = vec![origin];
    let mut cur = origin;
    let mut ctrl: Option<(f64, f64)> = None;
    let begin = if first.is_some() { start + 1 } else { 0 };
    for k in 0..n {
        let (p, on) = pts[(begin + k) % n];
        if on {
            match ctrl.take() {
                Some(c) => {
                    quad(&mut out, cur, c, p, steps);
                }
                None => out.push(p),
            }
            cur = p;
        } else {
            if let Some(c) = ctrl {
                let m = mid(c, p);
                quad(&mut out, cur, c, m, steps);
                cur = m;
            }
            ctrl = Some(p);
        }
    }
    if let Some(c) = ctrl {
        quad(&mut out, cur, c, origin, steps);
    }
    if out.len() > 1 && out.last() == out.first() {
        out.pop();
    }
    out.dedup();
    out
}

fn quad(out: &mut Vec<(f64, f64)>, a: (f64, f64), c: (f64, f64), b: (f64, f64), steps: u32) {
    for i in 1..=steps {
        let t = f64::from(i) / f64::from(steps);
        let u = 1.0 - t;
        out.push((u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0, u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1));
    }
}
