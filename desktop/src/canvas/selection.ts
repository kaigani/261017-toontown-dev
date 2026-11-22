// The selection overlay of the Drawing view (layer 9; M4 C4.2b, frame-loop note §7, C42B-R1-22): the kernel's
// selection geometry (drawing units, from `canvas_scene`) drawn in view pixels through the current mapping, so it
// stays one view pixel wide at every zoom and follows rotation. Only the window draws it: it never reaches a final
// render (06 §6.10). The handle hit test that decides a drag is the kernel's (`gesture.rs`); the window's geometry
// here only draws the handles and picks the cursor.

import { apply, ViewTris, type Affine } from './geometry';
import type { Rgba } from './format';

/** `[minX, minY, maxX, maxY]` in drawing units. */
export type Box = [number, number, number, number];
export type Pt = [number, number];

/** The selection `canvas_scene` answers (Rust `tt_executor::canvas::SceneSelection`). */
export interface SceneSelection {
  objects: Box[];
  bounds: Box | null;
  pivot: Pt | null;
  points: Pt[];
}

/** SI-138: the accent colour, the handles' light centre. */
export const SELECTION_ACCENT: Rgba = [32, 128, 255, 255];
const HANDLE_FILL: Rgba = [255, 255, 255, 255];
/** Handle squares (SI-138) and anchor squares, in view pixels. */
export const HANDLE_SIZE = 8;
const ANCHOR_SIZE = 5;
/** The pivot's hit radius (SI-131 item 2), drawn as its ring; also the handles' hit radius (frame-loop note §7.3). */
export const HANDLE_RADIUS = 6;

/** Selection tools (02 §1.3): they show the selection; the other tools keep it hidden. */
const OUTLINE_TOOLS = new Set(['select', 'contourEditor', 'perspective', 'repositionAll']);

/** Corners counter-clockwise from the bottom-left in drawing space (y up), as the kernel's `box_corners`. */
export function boxCorners(b: Box): [Pt, Pt, Pt, Pt] {
  return [
    [b[0], b[1]],
    [b[2], b[1]],
    [b[2], b[3]],
    [b[0], b[3]],
  ];
}

/** The eight handles' centres in view pixels: four corners, then four side midpoints (the kernel's order). */
export function handlePoints(b: Box, d2v: Affine): Pt[] {
  const c = boxCorners(b).map(([x, y]) => apply(d2v, x, y));
  const mid = (i: number): Pt => {
    const a = c[i] as Pt;
    const z = c[(i + 1) % 4] as Pt;
    return [(a[0] + z[0]) / 2, (a[1] + z[1]) / 2];
  };
  return [...c, mid(0), mid(1), mid(2), mid(3)];
}

export interface SelectionOverlay {
  tris: ViewTris | null;
  /** What it shows, for the canvas summary (`inspect_ui`): outlines, handles, anchors, pivot. */
  shown: { objects: number; handles: number; points: number; pivot: boolean };
  /** The handles' centres in view pixels (empty when none are shown). */
  handles: Pt[];
}

/**
 * The overlay for `selection` (and the drawing pivot) with `tool` active, mapped by `d2v` (drawing → view pixels).
 * Select: object outlines, the bounding box, eight handles, the drawing pivot. Contour Editor, Perspective, Reposition
 * All: object outlines and selected anchors only. Any other tool: nothing (02 §1.3).
 */
export function selectionOverlay(
  selection: SceneSelection | null,
  drawingPivot: Pt | null,
  tool: string,
  d2v: Affine,
): SelectionOverlay {
  const none: SelectionOverlay = {
    tris: null,
    shown: { objects: 0, handles: 0, points: 0, pivot: false },
    handles: [],
  };
  if (!OUTLINE_TOOLS.has(tool)) return none;
  const t = new ViewTris();
  const ring = (b: Box) => boxCorners(b).map(([x, y]) => apply(d2v, x, y));
  const objects = selection?.objects ?? [];
  for (const o of objects) t.polyline(ring(o), 1, SELECTION_ACCENT, true);
  const points = selection?.points ?? [];
  for (const [x, y] of points) {
    const [vx, vy] = apply(d2v, x, y);
    t.square(vx, vy, ANCHOR_SIZE, SELECTION_ACCENT, SELECTION_ACCENT);
  }
  let handles: Pt[] = [];
  let pivot = false;
  if (tool === 'select') {
    if (selection?.bounds) {
      t.polyline(ring(selection.bounds), 1, SELECTION_ACCENT, true);
      handles = handlePoints(selection.bounds, d2v);
      for (const [x, y] of handles) t.square(x, y, HANDLE_SIZE, HANDLE_FILL, SELECTION_ACCENT);
    }
    if (drawingPivot) {
      const [px, py] = apply(d2v, drawingPivot[0], drawingPivot[1]);
      const n = 24;
      const circle: Pt[] = [];
      for (let i = 0; i < n; i++) {
        const a = (i / n) * 2 * Math.PI;
        circle.push([px + HANDLE_RADIUS * Math.cos(a), py + HANDLE_RADIUS * Math.sin(a)]);
      }
      t.polyline(circle, 1, SELECTION_ACCENT, true);
      t.line(px - HANDLE_RADIUS, py, px + HANDLE_RADIUS, py, 1, SELECTION_ACCENT);
      t.line(px, py - HANDLE_RADIUS, px, py + HANDLE_RADIUS, 1, SELECTION_ACCENT);
      pivot = true;
    }
  }
  const shown = { objects: objects.length, handles: handles.length, points: points.length, pivot };
  return { tris: t.vertexCount > 0 ? t : null, shown, handles };
}

/** The resize cursors of the eight directions (frame-loop note §7.3; SI-141: platforms offer no 15° set). */
const CURSORS = ['ew-resize', 'nwse-resize', 'ns-resize', 'nesw-resize'];

/**
 * The cursor over view point `p` with the Select tool: the nearest of the eight resize cursors over a handle (by the
 * handle's direction from the box's centre on screen), `move` inside the box, else the default.
 */
export function selectionCursor(handles: Pt[], bounds: Box | null, d2v: Affine, p: Pt): string {
  if (!bounds || handles.length !== 8) return '';
  const ring = boxCorners(bounds).map(([x, y]) => apply(d2v, x, y));
  const cx = ring.reduce((s, q) => s + q[0], 0) / 4;
  const cy = ring.reduce((s, q) => s + q[1], 0) / 4;
  for (const [hx, hy] of handles) {
    if (Math.hypot(hx - p[0], hy - p[1]) <= HANDLE_RADIUS) {
      // The direction from the centre, folded to a half turn and rounded to 45°.
      const deg = ((Math.atan2(cy - hy, hx - cx) * 180) / Math.PI + 360) % 180;
      return CURSORS[Math.round(deg / 45) % 4] ?? '';
    }
  }
  // Inside the (possibly rotated) box on screen: the point is on the same side of all four edges.
  let sign = 0;
  for (let i = 0; i < 4; i++) {
    const a = ring[i] as Pt;
    const b = ring[(i + 1) % 4] as Pt;
    const cross = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
    if (cross === 0) continue;
    if (sign === 0) sign = Math.sign(cross);
    else if (Math.sign(cross) !== sign) return '';
  }
  return 'move';
}
