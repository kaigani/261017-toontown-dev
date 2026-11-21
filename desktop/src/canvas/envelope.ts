// The Perspective and Reposition All tools in the window (M4 C4.4b; design note 4 §2, §6; SI-188, SI-189, SI-190): the
// handles of the scene answer's `toolBox`, and the preview map of a drag in drawing units, by the kernel's rules
// (`crates/executor/src/envelope_gesture.rs` for the handle rows, `draw_transform.rs` `envelope_of` and
// `crates/vector/src/ops/transform.rs` `Envelope` for the map), ported operation for operation in f64 so the window
// takes the kernel's branch for the same corners. The vertex stage of both back ends applies the packed map
// (`packMap`) to the kernel's tessellated vertices; nothing here sends a command (PV1).

import { apply, compose, invert, type Affine } from './geometry';
import type { Rgba } from './format';
import { SELECTION_ACCENT } from './selection';
import { ViewTris } from './geometry';

export type Pt = [number, number];
/** `[minX, minY, maxX, maxY]`, drawing units. */
export type Box = readonly [number, number, number, number];

/** The kernel's handle radius and rotation zone, view pixels (`gesture.rs` `HANDLE_PX`, `ROTATE_PX`). */
export const HANDLE_PX = 6;
export const ROTATE_PX = 18;
/** The kernel's minimum handle scale (`gesture.rs` `MIN_SCALE`). */
const MIN_SCALE = 0.001;
const HANDLE_SIZE = 8;
const WHITE: Rgba = [255, 255, 255, 255];
const GREY: Rgba = [140, 140, 140, 255];

/** Element `i` of a list of points, `[0, 0]` past its end. */
const pt = (c: readonly Pt[], i: number): Pt => c[i] ?? [0, 0];
/** Element `i` of a list of numbers, 0 past its end. */
const nu = (c: ArrayLike<number>, i: number): number => c[i] ?? 0;

export type ToolHandle = { kind: 'corner'; i: number } | { kind: 'side'; i: number } | { kind: 'rotate' };

/** A map in drawing units: what a drag will do to the mapped content. */
export type PreviewMap =
  | { kind: 'affine'; m: Affine }
  | { kind: 'projective'; h: [number, number, number, number, number, number, number, number] }
  | { kind: 'bilinear'; min: Pt; max: Pt; corners: [Pt, Pt, Pt, Pt] };

/** Corners bottom-left first, counter-clockwise (drawing space, y up), as `box_corners`. */
export function boxCorners(b: Box): [Pt, Pt, Pt, Pt] {
  return [
    [b[0], b[1]],
    [b[2], b[1]],
    [b[2], b[3]],
    [b[0], b[3]],
  ];
}

/** The handle under view point `p` (the kernel's `handle_at`): corners, then side midpoints, within `HANDLE_PX`;
 *  with `rotate`, the rotation zone (outside the box, within `ROTATE_PX` of a corner). `pd` is `p` in drawing units. */
export function toolHandleAt(b: Box, d2v: Affine, p: Pt, rotate: boolean): ToolHandle | null {
  const c = boxCorners(b);
  const dist = (q: Pt) => {
    const v = apply(d2v, q[0], q[1]);
    return Math.hypot(v[0] - p[0], v[1] - p[1]);
  };
  const ci = c.findIndex((q) => dist(q) <= HANDLE_PX);
  if (ci >= 0) return { kind: 'corner', i: ci };
  const mid = (i: number): Pt => {
    const a = pt(c, i);
    const z = pt(c, (i + 1) % 4);
    return [(a[0] + z[0]) / 2, (a[1] + z[1]) / 2];
  };
  const si = [0, 1, 2, 3].findIndex((i) => dist(mid(i)) <= HANDLE_PX);
  if (si >= 0) return { kind: 'side', i: si };
  if (!rotate) return null;
  const v2d = invert(d2v);
  if (!v2d) return null;
  const pd = apply(v2d, p[0], p[1]);
  const inside = pd[0] >= b[0] && pd[0] <= b[2] && pd[1] >= b[1] && pd[1] <= b[3];
  return !inside && c.some((q) => dist(q) <= ROTATE_PX) ? { kind: 'rotate' } : null;
}

/** The envelope corners of a handle drag by `d` (the kernel's `envelope_corners`, SI-188). */
export function envelopeCorners(h: ToolHandle, b: Box, d: Pt, symmetric: boolean): [Pt, Pt, Pt, Pt] | null {
  const c = boxCorners(b);
  const add = (p: Pt, x: number, y: number): Pt => [p[0] + x, p[1] + y];
  if (h.kind === 'rotate') return null;
  if (h.kind === 'corner') {
    c[h.i] = add(pt(c, h.i), d[0], d[1]);
    return c;
  }
  const i = h.i;
  const j = (i + 1) % 4;
  if (!symmetric) {
    c[i] = add(pt(c, i), d[0], d[1]);
    c[j] = add(pt(c, j), d[0], d[1]);
    return c;
  }
  const alongX = i % 2 === 0;
  const [s, t] = i === 0 ? [d[0], -d[1]] : i === 1 ? [d[1], d[0]] : i === 2 ? [d[0], d[1]] : [d[1], -d[0]];
  const [ux, uy] = alongX ? [1, 0] : [0, 1];
  const [nx, ny] = i === 0 ? [0, -1] : i === 1 ? [1, 0] : i === 2 ? [0, 1] : [-1, 0];
  const coord = (p: Pt) => (alongX ? p[0] : p[1]);
  const [hi, lo] = coord(pt(c, i)) >= coord(pt(c, j)) ? [i, j] : [j, i];
  c[hi] = add(pt(c, hi), s * ux + t * nx, s * uy + t * ny);
  c[lo] = add(pt(c, lo), -s * ux + t * nx, -s * uy + t * ny);
  return c;
}

/** Gaussian elimination with partial pivoting, the kernel's `solve` (first of equal pivots, the 1e-12 guard). */
function solve(a: number[][], b: number[]): number[] | null {
  const n = b.length;
  const row = (i: number): number[] => a[i] ?? [];
  for (let col = 0; col < n; col++) {
    let piv = col;
    for (let i = col + 1; i < n; i++) {
      // Rust's `max_by` keeps the last of equal maxima.
      if (Math.abs(nu(row(i), col)) >= Math.abs(nu(row(piv), col))) piv = i;
    }
    if (Math.abs(nu(row(piv), col)) < 1e-12) return null;
    const [ra, rb] = [row(col), row(piv)];
    a[col] = rb;
    a[piv] = ra;
    const [ba, bb] = [nu(b, col), nu(b, piv)];
    b[col] = bb;
    b[piv] = ba;
    const pr = row(col);
    for (let r0 = col + 1; r0 < n; r0++) {
      const r = row(r0);
      const f = nu(r, col) / nu(pr, col);
      for (let k = col; k < n; k++) r[k] = nu(r, k) - f * nu(pr, k);
      b[r0] = nu(b, r0) - f * nu(b, col);
    }
  }
  const x = new Array<number>(n).fill(0);
  for (let i = n - 1; i >= 0; i--) {
    const r = row(i);
    let s = 0;
    for (let k = i + 1; k < n; k++) s += nu(r, k) * nu(x, k);
    x[i] = (nu(b, i) - s) / nu(r, i);
  }
  return x;
}

const cross = (o: Pt, a: Pt, b: Pt) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);

/** The kernel's `envelope_of`: the exact affine map of a parallelogram target under `auto`, else `Envelope::new`
 *  (projective for a strictly convex target, else bilinear); null where the kernel refuses (`drawing.invalidGeometry`). */
export function envelopeOf(
  b: Box,
  quad: [Pt, Pt, Pt, Pt],
  mapping: 'auto' | 'projective' | 'bilinear',
): PreviewMap | null {
  const [min, max]: [Pt, Pt] = [
    [b[0], b[1]],
    [b[2], b[3]],
  ];
  const w = Math.max(max[0] - min[0], 1e-12);
  const h = Math.max(max[1] - min[1], 1e-12);
  const ex: Pt = [(quad[1][0] - quad[0][0]) * (1 / w), (quad[1][1] - quad[0][1]) * (1 / w)];
  const ey: Pt = [(quad[3][0] - quad[0][0]) * (1 / h), (quad[3][1] - quad[0][1]) * (1 / h)];
  const lin = (p: Pt): Pt => [ex[0] * p[0] + ey[0] * p[1], ex[1] * p[0] + ey[1] * p[1]];
  const l = lin(min);
  const fit: Affine = [ex[0], ex[1], ey[0], ey[1], quad[0][0] - l[0], quad[0][1] - l[1]];
  const para =
    quad[2][0] === quad[1][0] + quad[3][0] - quad[0][0] &&
    quad[2][1] === quad[1][1] + quad[3][1] - quad[0][1];
  if (para && mapping === 'auto') return { kind: 'affine', m: fit };
  const s = [0, 1, 2, 3].map((i) => cross(pt(quad, i), pt(quad, (i + 1) % 4), pt(quad, (i + 2) % 4)));
  const convex = s.every((v) => v > 0) || s.every((v) => v < 0);
  const projective = mapping === 'auto' ? convex : mapping === 'projective';
  if (!projective) {
    return max[0] > min[0] && max[1] > min[1] ? { kind: 'bilinear', min, max, corners: quad } : null;
  }
  const src: Pt[] = [
    [min[0], min[1]],
    [max[0], min[1]],
    [max[0], max[1]],
    [min[0], max[1]],
  ];
  const a: number[][] = [];
  const rhs: number[] = [];
  for (let i = 0; i < 4; i++) {
    const p = src[i] ?? [0, 0];
    const t = quad[i] ?? [0, 0];
    a.push([p[0], p[1], 1, 0, 0, 0, -p[0] * t[0], -p[1] * t[0]]);
    rhs.push(t[0]);
    a.push([0, 0, 0, p[0], p[1], 1, -p[0] * t[1], -p[1] * t[1]]);
    rhs.push(t[1]);
  }
  const x = solve(a, rhs);
  return x
    ? { kind: 'projective', h: x as [number, number, number, number, number, number, number, number] }
    : null;
}

/** The kernel's `Envelope::apply` (and the affine map's), in f64. */
export function mapPoint(m: PreviewMap, p: Pt): Pt {
  if (m.kind === 'affine') return apply(m.m, p[0], p[1]);
  if (m.kind === 'projective') {
    const h = m.h;
    let w = h[6] * p[0] + h[7] * p[1] + 1;
    if (Math.abs(w) < 1e-12) w = w < 0 || Object.is(w, -0) ? -1e-12 : 1e-12;
    return [(h[0] * p[0] + h[1] * p[1] + h[2]) / w, (h[3] * p[0] + h[4] * p[1] + h[5]) / w];
  }
  const u = (p[0] - m.min[0]) / (m.max[0] - m.min[0]);
  const v = (p[1] - m.min[1]) / (m.max[1] - m.min[1]);
  const [c0, c1, c2, c3] = m.corners;
  const lerp = (a: Pt, b: Pt, t: number): Pt => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
  return lerp(lerp(c0, c1, u), lerp(c3, c2, u), v);
}

/**
 * The map's uniforms, one layout for both back ends' vertex stages: `kind` 0 none or affine (the affine part is composed
 * into the drawing-to-clip matrix), 1 projective, 2 bilinear; `data` twelve floats: projective `h0..h7`; bilinear
 * `minX, minY, 1/width, 1/height, c0, c1 - c0, c3 - c0, c2 - c3 - c1 + c0` (the expanded bilinear form).
 */
export function packMap(m: PreviewMap | null): { kind: number; data: Float32Array; affine: Affine | null } {
  const data = new Float32Array(12);
  if (!m) return { kind: 0, data, affine: null };
  if (m.kind === 'affine') return { kind: 0, data, affine: m.m };
  if (m.kind === 'projective') {
    data.set(m.h, 0);
    return { kind: 1, data, affine: null };
  }
  const [c0, c1, c2, c3] = m.corners;
  data.set(
    [
      m.min[0],
      m.min[1],
      1 / (m.max[0] - m.min[0]),
      1 / (m.max[1] - m.min[1]),
      c0[0],
      c0[1],
      c1[0] - c0[0],
      c1[1] - c0[1],
      c3[0] - c0[0],
      c3[1] - c0[1],
      c2[0] - c3[0] - c1[0] + c0[0],
      c2[1] - c3[1] - c1[1] + c0[1],
    ],
    0,
  );
  return { kind: 2, data, affine: null };
}

/** What the vertex stage computes from a packed map (the shader's arithmetic, for the test of the packing). */
export function unpackApply(kind: number, d: ArrayLike<number>, p: Pt): Pt {
  const g = (i: number) => nu(d, i);
  if (kind === 1) {
    let w = g(6) * p[0] + g(7) * p[1] + 1;
    // The kernel's signed denominator guard (`Envelope::apply`; finding C44B-R1S-06), as both shaders apply it.
    if (Math.abs(w) < 1e-12) w = w < 0 ? -1e-12 : 1e-12;
    return [(g(0) * p[0] + g(1) * p[1] + g(2)) / w, (g(3) * p[0] + g(4) * p[1] + g(5)) / w];
  }
  if (kind === 2) {
    const u = (p[0] - g(0)) * g(2);
    const v = (p[1] - g(1)) * g(3);
    return [g(4) + u * g(6) + v * g(8) + u * v * g(10), g(5) + u * g(7) + v * g(9) + u * v * g(11)];
  }
  return p;
}

export interface ToolPress {
  tool: 'perspective' | 'repositionAll';
  box: Box | null;
  handle: ToolHandle | null;
  alternate: boolean;
  constrain: boolean;
  mapping: 'auto' | 'projective' | 'bilinear';
}

const factor = (press: number, now: number, pivot: number) => {
  const den = press - pivot;
  return Math.abs(den) < 1e-9 ? 1 : (now - pivot) / den;
};

/** The affine of Select's handle drag (the kernel's `handle_drag`) in drawing units, null below the minimum scale. */
function handleAffine(h: ToolHandle, b: Box, p0: Pt, pn: Pt, constrain: boolean): Affine | null {
  const c = boxCorners(b);
  if (h.kind === 'rotate') {
    const o: Pt = [(b[0] + b[2]) / 2, (b[1] + b[3]) / 2];
    let deg =
      ((Math.atan2(pn[1] - o[1], pn[0] - o[0]) - Math.atan2(p0[1] - o[1], p0[0] - o[0])) * 180) / Math.PI;
    if (deg > 180) deg -= 360;
    else if (deg <= -180) deg += 360;
    const r = (deg * Math.PI) / 180;
    const cs = Math.cos(r);
    const sn = Math.sin(r);
    return [cs, sn, -sn, cs, o[0] - (cs * o[0] - sn * o[1]), o[1] - (sn * o[0] + cs * o[1])];
  }
  let sx: number;
  let sy: number;
  let o: Pt;
  if (h.kind === 'corner') {
    o = pt(c, (h.i + 2) % 4);
    sx = factor(p0[0], pn[0], o[0]);
    sy = factor(p0[1], pn[1], o[1]);
    if (constrain) {
      const s = Math.abs(sx) >= Math.abs(sy) ? sx : sy;
      sx = s;
      sy = s;
    }
  } else {
    const mx = (b[0] + b[2]) / 2;
    const my = (b[1] + b[3]) / 2;
    [sx, sy, o] =
      h.i === 0
        ? [1, factor(p0[1], pn[1], b[3]), [mx, b[3]]]
        : h.i === 2
          ? [1, factor(p0[1], pn[1], b[1]), [mx, b[1]]]
          : h.i === 1
            ? [factor(p0[0], pn[0], b[0]), 1, [b[0], my]]
            : [factor(p0[0], pn[0], b[2]), 1, [b[2], my]];
  }
  if (!(Math.abs(sx) >= MIN_SCALE && Math.abs(sy) >= MIN_SCALE && Number.isFinite(sx) && Number.isFinite(sy)))
    return null;
  return [sx, 0, 0, sy, o[0] - sx * o[0], o[1] - sy * o[1]];
}

/**
 * The preview map of a drag (design note 4 §3, §4), from the press `p0` to `pn` in drawing units (the press mapping's
 * inverse applied to view samples), or null when the drag changes no geometry before its commit (Perspective off its
 * handles: a selection; a refused map).
 */
export function toolPreview(press: ToolPress, p0: Pt, pn: Pt): PreviewMap | null {
  const d: Pt = [pn[0] - p0[0], pn[1] - p0[1]];
  const offset: PreviewMap = { kind: 'affine', m: [1, 0, 0, 1, d[0], d[1]] };
  if (press.tool === 'perspective') {
    if (!press.box || !press.handle) return null;
    const c = envelopeCorners(press.handle, press.box, d, press.alternate);
    return c ? envelopeOf(press.box, c, press.mapping) : null;
  }
  if (!press.box || !press.handle) return offset;
  if (press.alternate) {
    const symmetric = press.constrain && press.handle.kind === 'side';
    const c = envelopeCorners(press.handle, press.box, d, symmetric);
    return c ? envelopeOf(press.box, c, 'auto') : offset;
  }
  const m = handleAffine(press.handle, press.box, p0, pn, press.constrain);
  return m ? { kind: 'affine', m } : null;
}

/** The handles of `b` through `d2v` (design note 4 §2): eight squares at the corners and side midpoints, grey when
 *  `locked`; during a drag, the target quadrilateral under `map`. */
export function toolBoxOverlay(
  b: Box,
  d2v: Affine,
  locked: boolean,
  map: PreviewMap | null = null,
): ViewTris {
  const t = new ViewTris();
  const colour = locked ? GREY : SELECTION_ACCENT;
  const c = boxCorners(b).map((q) => (map ? mapPoint(map, q) : q));
  const v = c.map((q) => apply(d2v, q[0], q[1]));
  t.polyline(v, 1, colour, true);
  const mids = [0, 1, 2, 3].map((i): Pt => {
    const a = pt(c, i);
    const z = pt(c, (i + 1) % 4);
    // A side's handle sits on the mapped side's midpoint (projective maps keep lines straight).
    const m = map ? mapPoint(map, mid(pt(boxCorners(b), i), pt(boxCorners(b), (i + 1) % 4))) : mid(a, z);
    return apply(d2v, m[0], m[1]);
  });
  for (const [x, y] of [...v, ...mids]) t.square(x, y, HANDLE_SIZE, WHITE, colour);
  return t;
}

function mid(a: Pt, z: Pt): Pt {
  return [(a[0] + z[0]) / 2, (a[1] + z[1]) / 2];
}

/** The cursor over view point `p` (design note 4 §2). */
export function toolBoxCursor(
  tool: 'perspective' | 'repositionAll',
  b: Box | null,
  d2v: Affine,
  p: Pt,
  alternate: boolean,
  locked: boolean,
): string {
  if (!b) return tool === 'repositionAll' ? 'move' : '';
  const h = toolHandleAt(b, d2v, p, tool === 'repositionAll');
  if (h && locked) return 'not-allowed';
  if (tool === 'perspective') {
    if (!h) return '';
    if (alternate && h.kind === 'side') return h.i % 2 === 0 ? 'ns-resize' : 'ew-resize';
    return 'move';
  }
  if (!h || alternate) return 'move';
  if (h.kind === 'rotate') return 'grab';
  if (h.kind === 'side') return h.i % 2 === 0 ? 'ns-resize' : 'ew-resize';
  return h.i % 2 === 0 ? 'nesw-resize' : 'nwse-resize';
}

/** The view-pixel affine of an affine preview map (for the overlay furniture): `d2v * m * d2v^-1`. */
export function viewAffine(m: PreviewMap, d2v: Affine): Affine | null {
  if (m.kind !== 'affine') return null;
  const v2d = invert(d2v);
  return v2d ? compose(d2v, compose(m.m, v2d)) : null;
}
