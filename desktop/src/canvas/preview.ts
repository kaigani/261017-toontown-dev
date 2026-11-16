// The live transform preview (M4 C4.3b; preview note 8): a view-pixel affine `P` the window applies to what it draws
// for the selection while a drag is in progress, by the same furniture rule as the command the kernel's gesture driver
// will produce (Drawing view: `gesture.rs` `select`/`handle_at`/`handle_drag`, SI-141; Camera view:
// `scene_gesture.rs` `handle_at`/`handle_drag`, SI-150, SI-151). It never sends a command (PV1): the commit is the
// kernel's, and the committed state replaces the preview when it is drawn.

import { apply, compose, invert, type Affine } from './geometry';

export type Pt = readonly [number, number];
/** `[minX, minY, maxX, maxY]`. */
export type Box = readonly [number, number, number, number];

/** The kernel's handle radius, rotation zone and skew-handle offset in view pixels (`gesture.rs`, `scene_gesture.rs`). */
export const HANDLE_PX = 6;
export const ROTATE_PX = 18;
export const SKEW_OUT_PX = 12;
/** tan 89°: the skew channel's range (−89…89°, A row 613) as a shear factor. */
const TAN_89 = 57.28996163075943;

export type Handle =
  | { kind: 'corner'; i: number }
  | { kind: 'side'; i: number }
  | { kind: 'skew'; i: number }
  | { kind: 'rotate' }
  | { kind: 'pivot' }
  | { kind: 'move' };

const dist = (a: Pt, b: Pt) => Math.hypot(a[0] - b[0], a[1] - b[1]);

/** The corner `i` (modulo the count) of a box's corners. */
function at(c: readonly Pt[], i: number): Pt {
  return c[((i % c.length) + c.length) % c.length] ?? [0, 0];
}

/** The factor along one axis about `pivot`: 1 when the press is on the pivot's line (as the kernel's `factor`). */
function factor(press: number, now: number, pivot: number): number {
  const den = press - pivot;
  return Math.abs(den) < 1e-9 ? 1 : (now - pivot) / den;
}

/** Scaling by `sx`, `sy` about `o`. */
function scaleAbout(sx: number, sy: number, o: Pt): Affine {
  return [sx, 0, 0, sy, o[0] - sx * o[0], o[1] - sy * o[1]];
}

/** Rotation by `rad` (in the coordinates' own orientation) about `o`. */
function rotateAbout(rad: number, o: Pt): Affine {
  const c = Math.cos(rad);
  const s = Math.sin(rad);
  return [c, s, -s, c, o[0] - (c * o[0] - s * o[1]), o[1] - (s * o[0] + c * o[1])];
}

/** The wrapped difference of two angles in degrees, in (-180, 180]. */
function wrapDeg(d: number): number {
  let x = d;
  if (x > 180) x -= 360;
  else if (x <= -180) x += 360;
  return x;
}

// ----- Camera view (scene tools; SI-150, SI-151) -----------------------------------------------------------------------

/** The box's corners in view pixels (y down): top-left, top-right, bottom-right, bottom-left (the kernel's order). */
export function viewCorners(b: Box): [Pt, Pt, Pt, Pt] {
  return [
    [b[0], b[1]],
    [b[2], b[1]],
    [b[2], b[3]],
    [b[0], b[3]],
  ];
}

/** The scene-tool handle under `p` (SI-150), in the kernel's order: pivot marker, corners, sides, skew handles, the
 *  rotation zone; inside the box a move; elsewhere none (a rubber band). */
export function sceneHandleAt(b: Box, pivot: Pt | null, p: Pt): Handle | null {
  if (pivot && dist(pivot, p) <= HANDLE_PX) return { kind: 'pivot' };
  const c = viewCorners(b);
  const ci = c.findIndex((q) => dist(q, p) <= HANDLE_PX);
  if (ci >= 0) return { kind: 'corner', i: ci };
  const mx = (b[0] + b[2]) / 2;
  const my = (b[1] + b[3]) / 2;
  const mids: Pt[] = [
    [mx, b[1]],
    [b[2], my],
    [mx, b[3]],
    [b[0], my],
  ];
  const si = mids.findIndex((q) => dist(q, p) <= HANDLE_PX);
  if (si >= 0) return { kind: 'side', i: si };
  const skews: Pt[] = [
    [mx, b[1] - SKEW_OUT_PX],
    [b[2] + SKEW_OUT_PX, my],
    [mx, b[3] + SKEW_OUT_PX],
    [b[0] - SKEW_OUT_PX, my],
  ];
  const ki = skews.findIndex((q) => dist(q, p) <= HANDLE_PX);
  if (ki >= 0) return { kind: 'skew', i: ki };
  const inside = p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3];
  if (!inside && c.some((q) => dist(q, p) <= ROTATE_PX)) return { kind: 'rotate' };
  return inside ? { kind: 'move' } : null;
}

/**
 * The preview of a scene-tool drag in the Camera view (view pixels), from the selection's union box `b` and the
 * Transform tool's pivot marker (`pivot`, null for Select), the press `p0` and the current sample `pn`. `skewTool` is
 * the Transform tool's temporary Skew (SI-151). Null when the drag changes no occurrence (a rubber band, a pivot drag,
 * the Skew tool off a side).
 */
/** The same linear map, with its fixed point moved to `a`: the kernel takes a handle's factor, angle or shear from the
 *  box (SI-150) and applies it to a node about the node's own pivot. */
export function reanchor(m: Affine, a: Pt): Affine {
  const [p, q, r, s] = m;
  return [p, q, r, s, a[0] - (p * a[0] + r * a[1]), a[1] - (q * a[0] + s * a[1])];
}

export function scenePreview(
  b: Box,
  pivot: Pt | null,
  p0: Pt,
  pn: Pt,
  constrain: boolean,
  skewTool = false,
  anchor: Pt | null = null,
): Affine | null {
  const m = sceneAffine(b, pivot, p0, pn, constrain, skewTool);
  // One node: its scale, rotation or skew is applied about its pivot, so the preview is exact (preview note §4).
  // Several nodes turn about their own pivots: the preview about the box approximates them.
  return m && anchor && !(m[0] === 1 && m[1] === 0 && m[2] === 0 && m[3] === 1) ? reanchor(m, anchor) : m;
}

function sceneAffine(
  b: Box,
  pivot: Pt | null,
  p0: Pt,
  pn: Pt,
  constrain: boolean,
  skewTool: boolean,
): Affine | null {
  const h = sceneHandleAt(b, skewTool ? null : pivot, p0);
  if (!h) return null;
  const centre: Pt = [(b[0] + b[2]) / 2, (b[1] + b[3]) / 2];
  const halfW = (b[2] - b[0]) / 2;
  if (skewTool) {
    if (h.kind !== 'side' && h.kind !== 'skew') return null;
    if (h.i % 2 === 0) return horizontalShear(h.i, b, p0, pn);
    // The vertical shear y' = y + k x (scene, y up); in view pixels (y down) y' = y - k (x - cx).
    const d = h.i === 1 ? p0[1] - pn[1] : pn[1] - p0[1];
    const k = halfW > 1e-9 ? d / halfW : 0;
    return [1, -k, 0, 1, 0, k * centre[0]];
  }
  switch (h.kind) {
    case 'move':
      return [1, 0, 0, 1, pn[0] - p0[0], pn[1] - p0[1]];
    case 'pivot':
      return null;
    case 'corner': {
      const o = at(viewCorners(b), h.i + 2);
      let sx = factor(p0[0], pn[0], o[0]);
      let sy = factor(p0[1], pn[1], o[1]);
      if (constrain) {
        const s = Math.abs(sx) >= Math.abs(sy) ? sx : sy;
        sx = s;
        sy = s;
      }
      return scaleAbout(sx, sy, o);
    }
    case 'side': {
      switch (h.i) {
        case 0:
          return scaleAbout(1, factor(p0[1], pn[1], b[3]), [centre[0], b[3]]);
        case 2:
          return scaleAbout(1, factor(p0[1], pn[1], b[1]), [centre[0], b[1]]);
        case 1:
          return scaleAbout(factor(p0[0], pn[0], b[0]), 1, [b[0], centre[1]]);
        default:
          return scaleAbout(factor(p0[0], pn[0], b[2]), 1, [b[2], centre[1]]);
      }
    }
    case 'rotate': {
      // The kernel's angle is -(a1 - a0) in degrees (anticlockwise on screen positive); snapped to 15° when
      // constrained. On screen (y down) that is a rotation by -angle.
      const a0 = Math.atan2(p0[1] - centre[1], p0[0] - centre[0]);
      const a1 = Math.atan2(pn[1] - centre[1], pn[0] - centre[0]);
      let deg = wrapDeg(-((a1 - a0) * 180) / Math.PI);
      if (constrain) deg = Math.round(deg / 15) * 15;
      return rotateAbout((-deg * Math.PI) / 180, centre);
    }
    case 'skew':
      return h.i % 2 === 0 ? horizontalShear(h.i, b, p0, pn) : horizontalShearOfSide(h.i, b, p0, pn);
  }
}

/** A horizontal shear about the box's centre by the kernel's skew angle of a top (0) or bottom (2) side. */
function horizontalShear(i: number, b: Box, p0: Pt, pn: Pt): Affine {
  const halfH = (b[3] - b[1]) / 2;
  const d = i === 0 ? pn[0] - p0[0] : p0[0] - pn[0];
  // The kernel's angle is atan(d / half) clamped to ±89° (the skew channel's range); its tangent is d / half clamped
  // to ±tan 89°.
  const t = halfH > 1e-9 ? Math.min(TAN_89, Math.max(-TAN_89, d / halfH)) : 0;
  const cy = (b[1] + b[3]) / 2;
  // Scene x' = x + tan(κ) y (y up); in view pixels x' = x + tan(κ) (cy - y).
  return [1, 0, -t, 1, t * cy, 0];
}

/** The kernel's skew handle on a left (3) or right (1) side: the same `skew` channel (a horizontal shear in the node's
 *  frame), its angle from the vertical drag over half the width (SI-150). */
function horizontalShearOfSide(i: number, b: Box, p0: Pt, pn: Pt): Affine {
  const halfW = (b[2] - b[0]) / 2;
  const d = i === 1 ? p0[1] - pn[1] : pn[1] - p0[1];
  const t = halfW > 1e-9 ? d / halfW : 0;
  const cy = (b[1] + b[3]) / 2;
  return [1, 0, -t, 1, t * cy, 0];
}

// ----- Drawing view (Select; SI-141) -----------------------------------------------------------------------------------

/**
 * The preview of a Select drag in the Drawing view (view pixels): the kernel's `select` in drawing units (the
 * selection's `bounds` and the drawing pivot in drawing units; samples mapped by `d2v`'s inverse), conjugated into view
 * pixels. Null for a drag that changes no geometry (a pivot drag, a marquee or lasso).
 */
export function drawingPreview(
  bounds: Box | null,
  drawingPivot: Pt | null,
  d2v: Affine,
  p0v: Pt,
  pnv: Pt,
  constrain: boolean,
): Affine | null {
  const v2d = invert(d2v);
  if (!v2d || !bounds) return null;
  if (drawingPivot && dist(apply(d2v, drawingPivot[0], drawingPivot[1]), p0v) <= HANDLE_PX) return null;
  const p0 = apply(v2d, p0v[0], p0v[1]);
  const pn = apply(v2d, pnv[0], pnv[1]);
  // Corners counter-clockwise from the bottom-left in drawing space (y up), as the kernel's `box_corners`.
  const c: Pt[] = [
    [bounds[0], bounds[1]],
    [bounds[2], bounds[1]],
    [bounds[2], bounds[3]],
    [bounds[0], bounds[3]],
  ];
  const vdist = (q: Pt) => dist(apply(d2v, q[0], q[1]), p0v);
  let t: Affine | null = null;
  const ci = c.findIndex((q) => vdist(q) <= HANDLE_PX);
  const mid = (i: number): Pt => {
    const a = at(c, i);
    const z = at(c, i + 1);
    return [(a[0] + z[0]) / 2, (a[1] + z[1]) / 2];
  };
  const si = [0, 1, 2, 3].findIndex((i) => vdist(mid(i)) <= HANDLE_PX);
  const inside = p0[0] >= bounds[0] && p0[0] <= bounds[2] && p0[1] >= bounds[1] && p0[1] <= bounds[3];
  if (ci >= 0) {
    const o = at(c, ci + 2);
    let sx = factor(p0[0], pn[0], o[0]);
    let sy = factor(p0[1], pn[1], o[1]);
    if (constrain) {
      const s = Math.abs(sx) >= Math.abs(sy) ? sx : sy;
      sx = s;
      sy = s;
    }
    t = scaleAbout(sx, sy, o);
  } else if (si >= 0) {
    const mx = (bounds[0] + bounds[2]) / 2;
    const my = (bounds[1] + bounds[3]) / 2;
    t =
      si === 0
        ? scaleAbout(1, factor(p0[1], pn[1], bounds[3]), [mx, bounds[3]])
        : si === 2
          ? scaleAbout(1, factor(p0[1], pn[1], bounds[1]), [mx, bounds[1]])
          : si === 1
            ? scaleAbout(factor(p0[0], pn[0], bounds[0]), 1, [bounds[0], my])
            : scaleAbout(factor(p0[0], pn[0], bounds[2]), 1, [bounds[2], my]);
  } else if (!inside && c.some((q) => vdist(q) <= ROTATE_PX)) {
    const o: Pt = [(bounds[0] + bounds[2]) / 2, (bounds[1] + bounds[3]) / 2];
    const a0 = Math.atan2(p0[1] - o[1], p0[0] - o[0]);
    const a1 = Math.atan2(pn[1] - o[1], pn[0] - o[0]);
    t = rotateAbout((wrapDeg(((a1 - a0) * 180) / Math.PI) * Math.PI) / 180, o);
  } else if (inside) {
    t = [1, 0, 0, 1, pn[0] - p0[0], pn[1] - p0[1]];
  }
  return t ? compose(d2v, compose(t, v2d)) : null;
}

/** The Grabber's preview (preview note §3): the view panned by the drag, in view pixels. */
export function grabberPreview(p0: Pt, pn: Pt): Affine {
  return [1, 0, 0, 1, pn[0] - p0[0], pn[1] - p0[1]];
}
