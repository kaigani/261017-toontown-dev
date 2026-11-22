// The Contour Editor in the window (M4 C4.4b; design note 3 §4 and §6; SI-184, SI-185, SI-187): the overlay of the
// scene answer's `contours` (docs/canvas-format.md §11.8) drawn in view pixels, the hit test that picks the cursor and
// the preview, and the live preview of a drag. The command is the kernel's: its gesture driver
// (`crates/executor/src/contour_gesture.rs`) decides the target with the same rule, and the hit corpus
// (`testdata/contour-hits.json`, written by the kernel's test) holds both ports to it. Nothing here sends a command
// (PV1).

import { apply, invert, ViewTris, type Affine } from './geometry';
import type { Rgba } from './format';
import { SELECTION_ACCENT } from './selection';

export type Anchor6 = [number, number, number, number, number, number];
export type Pt = [number, number];

/** One stroke of the overlay (Rust `tt_executor::canvas::SceneContourStroke`). */
export interface ContourStroke {
  strokeId: string;
  /** An opaque key naming the stroke's layer. */
  layer: string;
  closed: boolean;
  /** Not shown: moved with a shown anchor at its exact coordinate (`draw.point.move`). */
  partner: boolean;
  /** `[x, y, inX, inY, outX, outY]`, handles as offsets, drawing units. */
  anchors: Anchor6[];
  /** The selected anchors' indices, ascending. */
  selected: number[];
}

/** The scene answer's `contours` (§11.8). */
export interface SceneContours {
  drawingId: string;
  locked: boolean;
  strokes: ContourStroke[];
}

/** The kernel's click slop in view pixels (`gesture.rs` `CLICK_SLOP_PX`): a press that stays within it is a click. */
export const CLICK_SLOP_PX = 3;
/** Hit radii in view pixels (SI-184; the kernel's `ANCHOR_PX`, `HANDLE_PX`, `SEGMENT_PX`). */
export const ANCHOR_PX = 6;
export const HANDLE_PX = 5;
export const SEGMENT_PX = 4;
/** Drawn sizes in view pixels (design note 3 §4). */
const ANCHOR_SIZE = 5;
const HANDLE_DISC = 2;
/** The flattening tolerance of drawn curves, view pixels (design note 3 §6). */
const FLATNESS = 0.25;
const WHITE: Rgba = [255, 255, 255, 255];
const ZERO: Anchor6 = [0, 0, 0, 0, 0, 0];
const GREY: Rgba = [140, 140, 140, 255];

export type ContourTarget =
  | { kind: 'anchor'; stroke: string; anchor: number }
  | { kind: 'handle'; stroke: string; anchor: number; out: boolean }
  | { kind: 'segment'; stroke: string; segment: number; t: number };

interface Cubic {
  p0: Pt;
  p1: Pt;
  p2: Pt;
  p3: Pt;
}

/** The segments of a path as the kernel's `Path::segments` numbers them. */
export function segmentsOf(s: Pick<ContourStroke, 'anchors' | 'closed'>): Cubic[] {
  const a = s.anchors;
  const n = a.length;
  const first = a[0];
  const count =
    n === 0
      ? 0
      : n === 1
        ? s.closed && first && (first[2] !== 0 || first[3] !== 0 || first[4] !== 0 || first[5] !== 0)
          ? 1
          : 0
        : s.closed
          ? n
          : n - 1;
  const out: Cubic[] = [];
  for (let i = 0; i < count; i++) out.push(segment(a, i));
  return out;
}

function segment(a: readonly Anchor6[], i: number): Cubic {
  const n = a.length;
  const p = a[i] ?? ZERO;
  const q = a[(i + 1) % n] ?? ZERO;
  return {
    p0: [p[0], p[1]],
    p1: [p[0] + p[4], p[1] + p[5]],
    p2: [q[0] + q[2], q[1] + q[3]],
    p3: [q[0], q[1]],
  };
}

function evalCubic(c: Cubic, t: number): Pt {
  const u = 1 - t;
  const a = u * u * u;
  const b = 3 * u * u * t;
  const d = 3 * u * t * t;
  const e = t * t * t;
  return [
    a * c.p0[0] + b * c.p1[0] + d * c.p2[0] + e * c.p3[0],
    a * c.p0[1] + b * c.p1[1] + d * c.p2[1] + e * c.p3[1],
  ];
}

/** The point of `c` nearest view point `q` in view pixels through `d2v`: `[distance, t]`; 64 samples, then 40
 *  bisections, as the kernel's `nearest_in_view` (an affine map keeps the parameter). */
export function nearestInView(c: Cubic, q: Pt, d2v: Affine): [number, number] {
  const at = (t: number) => {
    const p = evalCubic(c, t);
    const v = apply(d2v, p[0], p[1]);
    return Math.hypot(v[0] - q[0], v[1] - q[1]);
  };
  let t = 0;
  let best = Infinity;
  for (let k = 0; k <= 64; k++) {
    const d = at(k / 64);
    if (d < best) {
      best = d;
      t = k / 64;
    }
  }
  let step = 1 / 64;
  for (let i = 0; i < 40; i++) {
    step /= 2;
    // The first of equal minima, as Rust's `min_by`.
    let pick = t;
    let pd = Infinity;
    for (const c2 of [Math.max(0, t - step), t, Math.min(1, t + step)]) {
      const d = at(c2);
      if (d < pd) {
        pd = d;
        pick = c2;
      }
    }
    t = pick;
  }
  return [at(t), t];
}

const TIE = 1e-12;

/**
 * The target of a press at view point `q` (SI-184), the kernel's `target_at`: the nearest anchor of a shown stroke or
 * handle end of a selected anchor within its radius (an anchor on a tie, the front-most stroke between strokes); else
 * the nearest segment of a shown stroke within `SEGMENT_PX` with `t` strictly inside (0, 1); else null.
 */
export function contourHitAt(c: SceneContours | null, d2v: Affine, q: Pt): ContourTarget | null {
  if (!c) return null;
  const dist = (x: number, y: number) => {
    const v = apply(d2v, x, y);
    return Math.hypot(v[0] - q[0], v[1] - q[1]);
  };
  let best: { d: number; prio: number; order: number; t: ContourTarget } | null = null;
  const keep = (d: number, prio: number, order: number, t: ContourTarget) => {
    const better =
      best === null ||
      d < best.d - TIE ||
      (Math.abs(d - best.d) <= TIE && (prio < best.prio || (prio === best.prio && order > best.order)));
    if (better) best = { d, prio, order, t };
  };
  const shown = c.strokes.filter((s) => !s.partner);
  shown.forEach((s, order) => {
    s.anchors.forEach((a, k) => {
      const d = dist(a[0], a[1]);
      if (d <= ANCHOR_PX) keep(d, 0, order, { kind: 'anchor', stroke: s.strokeId, anchor: k });
    });
    for (const k of s.selected) {
      const a = s.anchors[k];
      if (!a) continue;
      for (const [out, hx, hy] of [
        [false, a[2], a[3]],
        [true, a[4], a[5]],
      ] as const) {
        if (hx === 0 && hy === 0) continue;
        const d = dist(a[0] + hx, a[1] + hy);
        if (d <= HANDLE_PX) keep(d, 1, order, { kind: 'handle', stroke: s.strokeId, anchor: k, out });
      }
    }
  });
  const found = best as { t: ContourTarget } | null;
  if (found) return found.t;
  let seg: { d: number; order: number; t: ContourTarget } | null = null;
  shown.forEach((s, order) => {
    segmentsOf(s).forEach((cubic, i) => {
      const [d, t] = nearestInView(cubic, q, d2v);
      if (d <= SEGMENT_PX && t > 0 && t < 1) {
        const better = seg === null || d < seg.d - TIE || (Math.abs(d - seg.d) <= TIE && order > seg.order);
        if (better) seg = { d, order, t: { kind: 'segment', stroke: s.strokeId, segment: i, t } };
      }
    });
  });
  const s2 = seg as { t: ContourTarget } | null;
  return s2 ? s2.t : null;
}

/** The cursor over view point `q` (design note 3 §4). */
export function contourCursor(c: SceneContours | null, d2v: Affine, q: Pt, tempSelect: boolean): string {
  if (!c) return '';
  if (c.locked) return 'not-allowed';
  const t = contourHitAt(c, d2v, q);
  if (!t) return '';
  if (t.kind === 'handle') return 'crosshair';
  if (t.kind === 'segment' && tempSelect) return 'copy';
  return 'move';
}

/** The number of chords that flattens `c` (drawing units) to `FLATNESS` view pixels through `d2v`. */
function chords(c: Cubic, d2v: Affine): number {
  const v = [c.p0, c.p1, c.p2, c.p3].map((p) => apply(d2v, p[0], p[1]));
  const [a, b, cc, d] = v as [Pt, Pt, Pt, Pt];
  const m = Math.max(
    Math.hypot(a[0] - 2 * b[0] + cc[0], a[1] - 2 * b[1] + cc[1]),
    Math.hypot(b[0] - 2 * cc[0] + d[0], b[1] - 2 * cc[1] + d[1]),
  );
  // The chord error of a cubic is at most 6 m / (8 n^2) (m: the control polygon's largest second difference).
  return Math.max(1, Math.min(4096, Math.ceil(Math.sqrt((6 * m) / (8 * FLATNESS)))));
}

function drawCurve(t: ViewTris, c: Cubic, d2v: Affine, colour: Rgba) {
  const n = chords(c, d2v);
  const pts: Pt[] = [];
  for (let i = 0; i <= n; i++) {
    const p = evalCubic(c, i / n);
    pts.push(apply(d2v, p[0], p[1]));
  }
  t.polyline(pts, 1, colour);
}

function drawAnchor(t: ViewTris, a: Anchor6, selected: boolean, d2v: Affine, colour: Rgba) {
  const [x, y] = apply(d2v, a[0], a[1]);
  if (selected) {
    t.square(x, y, ANCHOR_SIZE, colour, colour);
    for (const [hx, hy] of [
      [a[2], a[3]],
      [a[4], a[5]],
    ] as const) {
      if (hx === 0 && hy === 0) continue;
      const [ex, ey] = apply(d2v, a[0] + hx, a[1] + hy);
      t.line(x, y, ex, ey, 1, colour);
      t.disc(ex, ey, HANDLE_DISC, colour);
    }
  } else {
    t.square(x, y, ANCHOR_SIZE, WHITE, colour);
  }
}

export interface ContourOverlay {
  tris: ViewTris | null;
  /** For the canvas summary (`inspect_ui`): strokes, anchors, selected anchors, handles drawn. */
  shown: { strokes: number; anchors: number; selected: number; handles: number; locked: boolean };
}

/** The overlay of `c` through `d2v` (design note 3 §4): each shown stroke's contour as a 1 px curve, its anchors, the
 *  handles of selected anchors; grey on a locked drawing. Partners are not drawn. */
export function contourOverlay(c: SceneContours | null, d2v: Affine): ContourOverlay {
  const shown = { strokes: 0, anchors: 0, selected: 0, handles: 0, locked: c?.locked ?? false };
  if (!c) return { tris: null, shown };
  const colour = c.locked ? GREY : SELECTION_ACCENT;
  const t = new ViewTris();
  for (const s of c.strokes) {
    if (s.partner) continue;
    shown.strokes++;
    for (const cubic of segmentsOf(s)) drawCurve(t, cubic, d2v, colour);
    const sel = new Set(s.selected);
    s.anchors.forEach((a, k) => {
      shown.anchors++;
      if (sel.has(k)) {
        shown.selected++;
        if (a[2] !== 0 || a[3] !== 0) shown.handles++;
        if (a[4] !== 0 || a[5] !== 0) shown.handles++;
      }
      drawAnchor(t, a, sel.has(k), d2v, colour);
    });
  }
  return { tris: t.vertexCount > 0 ? t : null, shown };
}

// ----- the live preview (design note 3 §6) ----------------------------------------------------------------------------

export interface ContourPress {
  target: ContourTarget | null;
  alternate: boolean;
}

/** What a drag changes: per stroke, the new anchors and the indices changed. */
export interface ContourChange {
  strokeId: string;
  anchors: Anchor6[];
  closed: boolean;
  changed: number[];
}

const key = (x: number, y: number, layer: string) => `${layer}\u0000${String(x)}\u0000${String(y)}`;

/** The anchors `draw.point.move` moves for `moved` (stroke id -> anchor indices): the anchors at a moved anchor's exact
 *  coordinate in the same layer (the kernel's junction rule). */
function pointMove(c: SceneContours, moved: Map<string, Set<number>>): Map<string, Set<number>> {
  const coords = new Set<string>();
  for (const s of c.strokes) {
    const ks = moved.get(s.strokeId);
    if (!ks) continue;
    for (const k of ks) {
      const a = s.anchors[k];
      if (a) coords.add(key(a[0], a[1], s.layer));
    }
  }
  const all = new Map<string, Set<number>>();
  for (const s of c.strokes) {
    s.anchors.forEach((a, k) => {
      if (moved.get(s.strokeId)?.has(k) || coords.has(key(a[0], a[1], s.layer))) {
        let set = all.get(s.strokeId);
        if (!set) {
          set = new Set();
          all.set(s.strokeId, set);
        }
        set.add(k);
      }
    });
  }
  return all;
}

function selectionOf(c: SceneContours): Map<string, Set<number>> {
  const m = new Map<string, Set<number>>();
  for (const s of c.strokes) if (!s.partner && s.selected.length > 0) m.set(s.strokeId, new Set(s.selected));
  return m;
}

/**
 * The preview of a drag from the press to a delta `d` in drawing units (the press mapping's inverse applied to the view
 * delta, as the kernel's `pn.d - p0.d`): the strokes the command will change, with their new anchors, by the row's rule
 * (design note 3 §2). Empty for a drag that changes nothing before the commit (on nothing: a marquee; a locked
 * drawing; an alternate segment drag without selected anchors). Snap to Contour is not previewed (SI-187).
 */
export function contourPreview(c: SceneContours | null, press: ContourPress, d: Pt): ContourChange[] {
  if (!c || c.locked || !press.target) return [];
  const t = press.target;
  const byId = new Map(c.strokes.map((s) => [s.strokeId, s]));
  const out = new Map<string, ContourChange>();
  const edit = (id: string): ContourChange | null => {
    const s = byId.get(id);
    if (!s) return null;
    let ch = out.get(id);
    if (!ch) {
      ch = { strokeId: id, anchors: s.anchors.map((a) => [...a] as Anchor6), closed: s.closed, changed: [] };
      out.set(id, ch);
    }
    return ch;
  };
  const mark = (ch: ContourChange, k: number) => {
    if (!ch.changed.includes(k)) ch.changed.push(k);
  };
  const translate = (moved: Map<string, Set<number>>) => {
    for (const [id, ks] of pointMove(c, moved)) {
      const ch = edit(id);
      if (!ch) continue;
      for (const k of ks) {
        const a = ch.anchors[k];
        if (!a) continue;
        a[0] += d[0];
        a[1] += d[1];
        mark(ch, k);
      }
    }
  };
  if (t.kind === 'handle') {
    const ch = edit(t.stroke);
    const a = ch?.anchors[t.anchor];
    if (!ch || !a) return [];
    // `symmetric` mirrors the other handle (the kernel's `draw.point.set`); alternate (`broken`) leaves it.
    const hx = (t.out ? a[4] : a[2]) + d[0];
    const hy = (t.out ? a[5] : a[3]) + d[1];
    if (t.out) {
      a[4] = hx;
      a[5] = hy;
    } else {
      a[2] = hx;
      a[3] = hy;
    }
    if (!press.alternate) {
      if (t.out) {
        a[2] = -hx;
        a[3] = -hy;
      } else {
        a[4] = -hx;
        a[5] = -hy;
      }
    }
    mark(ch, t.anchor);
  } else if (t.kind === 'anchor' && press.alternate) {
    // SI-185: both handles follow the pointer, the anchor stays.
    const ch = edit(t.stroke);
    const a = ch?.anchors[t.anchor];
    if (!ch || !a) return [];
    a[2] += d[0];
    a[3] += d[1];
    a[4] += d[0];
    a[5] += d[1];
    mark(ch, t.anchor);
  } else if (t.kind === 'anchor') {
    const stroke = byId.get(t.stroke);
    const inSelection = stroke?.selected.includes(t.anchor) ?? false;
    translate(inSelection ? selectionOf(c) : new Map([[t.stroke, new Set([t.anchor])]]));
  } else if (press.alternate) {
    const sel = selectionOf(c);
    if (sel.size === 0) return [];
    translate(sel);
  } else {
    // `draw.segment.move`: both control points move by w = d / (3t(1-t)).
    const ch = edit(t.stroke);
    if (!ch) return [];
    const n = ch.anchors.length;
    const f = 1 / (3 * t.t * (1 - t.t));
    const a = ch.anchors[t.segment];
    const b = ch.anchors[(t.segment + 1) % n];
    if (!a || !b) return [];
    a[4] += d[0] * f;
    a[5] += d[1] * f;
    b[2] += d[0] * f;
    b[3] += d[1] * f;
    mark(ch, t.segment);
    mark(ch, (t.segment + 1) % n);
  }
  return [...out.values()];
}

/** The segments of a changed stroke that touch a changed anchor (the only ones a frame recomputes). */
export function affectedSegments(ch: ContourChange): number[] {
  const count = segmentsOf(ch).length;
  const n = ch.anchors.length;
  const out = new Set<number>();
  for (const k of ch.changed) {
    if (k < count) out.add(k);
    const before = (k - 1 + n) % n;
    if (before < count && (ch.closed || k > 0)) out.add(before);
  }
  return [...out].sort((a, b) => a - b);
}

export interface ContourPreviewDrawing {
  tris: ViewTris | null;
  /** Segments recomputed this frame (Vitest counts them: a one-anchor drag recomputes at most 2). */
  segments: number;
  /** The changed anchors and handle ends in view pixels, for the e2e's comparison with the committed state (PV2). */
  anchors: Pt[];
}

/** Draws a preview: the affected segments as 1 px accent curves, the changed anchors with their handles. */
export function drawContourPreview(changes: ContourChange[], d2v: Affine): ContourPreviewDrawing {
  const t = new ViewTris();
  let segments = 0;
  const anchors: Pt[] = [];
  for (const ch of changes) {
    for (const i of affectedSegments(ch)) {
      segments++;
      drawCurve(t, segment(ch.anchors, i), d2v, SELECTION_ACCENT);
    }
    for (const k of ch.changed) {
      const a = ch.anchors[k];
      if (!a) continue;
      drawAnchor(t, a, true, d2v, SELECTION_ACCENT);
      anchors.push(apply(d2v, a[0], a[1]));
    }
  }
  return { tris: t.vertexCount > 0 ? t : null, segments, anchors };
}

/** The drawing-unit delta of a drag from `p0` to `pn` (view pixels) through the press mapping `d2v`. */
export function drawingDelta(d2v: Affine, p0: Pt, pn: Pt): Pt | null {
  const v2d = invert(d2v);
  if (!v2d) return null;
  const a = apply(v2d, p0[0], p0[1]);
  const b = apply(v2d, pn[0], pn[1]);
  return [b[0] - a[0], b[1] - a[1]];
}
