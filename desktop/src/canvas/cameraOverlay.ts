// The Camera view's overlays (layer 9; M4 C4.3b, camera-view note §6; SI-153, SI-150): built in view pixels from the
// scene answer alone (the overlays in camera output pixels, mapped by `cameraToView`), so the window computes no
// projection (CV3). Only the window draws them: they never reach a final render (06 §6.10, CV4). The handle hit test
// that decides a drag is the kernel's (`scene_gesture.rs`); the furniture drawn here follows its sizes.

import { apply, compose, ViewTris, type Affine } from './geometry';
import type { Rgba } from './format';
import type { Scene, SceneGrid } from './scene';
import { HANDLE_SIZE, SELECTION_ACCENT, type Pt } from './selection';

/** SI-153: outside the camera frame, black at 35 %; the frame's outline black at 60 %. */
export const DIM: Rgba = [0, 0, 0, 89];
export const FRAME_LINE: Rgba = [0, 0, 0, 153];
/** SI-153: another camera's frame in the accent colour at 70 %. */
export const OTHER_CAMERA: Rgba = [32, 128, 255, 179];
const HANDLE_FILL: Rgba = [255, 255, 255, 255];
/** SI-153: a pivot's cross, in view pixels. */
export const PIVOT_CROSS = 12;
/** SI-150: the skew handles sit this far outside each side's midpoint (view pixels). */
export const SKEW_OUT = 12;

/** The scene tools (05 §5.0) whose furniture C4.3 draws (SI-150). */
const FURNITURE_TOOLS = new Set(['select', 'transform']);

export interface CameraOverlay {
  tris: ViewTris;
  /** The selection's part of `tris` (boxes, pivots, furniture) on its own, for the live preview (preview note §3). */
  selection: ViewTris;
  /** The rest: the dimmed outside, the frame, the other cameras. */
  frame: ViewTris;
  /** What it shows, for the canvas summary (`inspect_ui`). */
  shown: { frame: boolean; pivots: number; boxes: number; otherCameras: number; handles: number };
  /** The furniture handles' centres in view pixels (corners, side midpoints, skew handles), for the cursor. */
  handles: Pt[];
}

/**
 * The grid's matrix in the Camera view (SI-153): grid units (one field = 100, as the Drawing view's field guide, 02
 * §10.1) centred on the camera frame, y up, scaled so the guide's fields span the frame's width (12 for `field12` and
 * `normal`, 16 for `field16`), then through `cameraToView`. Null without a camera.
 */
export function cameraGridMatrix(scene: Scene, grid: SceneGrid): Affine | null {
  const camera = scene.camera;
  if (!camera) return null;
  const [w, h] = camera.size;
  const fields = grid.type === 'field16' ? 16 : 12;
  const k = w / (fields * 100);
  return compose(camera.cameraToView, [k, 0, 0, -k, w / 2, h / 2]);
}

/**
 * The Camera view's overlay for `scene` in a `width` × `height` view, with `tool` the acting tool: a scene tool
 * (`select`, `transform`) draws its furniture on the selection's union box; a drawing tool draws none (its own overlay
 * is the Drawing view's, through the drawing mapping).
 */
export function cameraOverlay(
  scene: Scene,
  width: number,
  height: number,
  tool: string | null,
): CameraOverlay {
  const t = new ViewTris();
  const sel = new ViewTris();
  const shown = { frame: false, pivots: 0, boxes: 0, otherCameras: 0, handles: 0 };
  const camera = scene.camera;
  const o = scene.overlays;
  if (!camera || !o) return { tris: t, selection: sel, frame: t, shown, handles: [] };
  const v = camera.cameraToView;
  // A box's four corners in view pixels, top-left first, clockwise on screen (output pixels are y down).
  const ring = (b: readonly [number, number, number, number]): [Pt, Pt, Pt, Pt] => [
    apply(v, b[0], b[1]),
    apply(v, b[2], b[1]),
    apply(v, b[2], b[3]),
    apply(v, b[0], b[3]),
  ];
  // The area outside the camera frame dimmed: the frame is a convex quadrilateral (corners clockwise on screen, the
  // view transform keeps orientation); around it a square centred on it and larger than the view and the frame
  // together, its corners in the same clockwise order starting from the direction of the frame's first corner; the
  // four strips between them cover exactly the outside of the frame (and beyond the view, which clips).
  const f = ring(o.cameraFrame);
  const [f0, f1, f2, f3] = f;
  const cx = (f0[0] + f1[0] + f2[0] + f3[0]) / 4;
  const cy = (f0[1] + f1[1] + f2[1] + f3[1]) / 4;
  const reach = Math.max(...f.map((q) => Math.hypot(q[0] - cx, q[1] - cy)));
  const big = 4 * (reach + Math.hypot(width, height) + Math.hypot(cx, cy));
  const out = (q: Pt): Pt => {
    const a = Math.atan2(q[1] - cy, q[0] - cx);
    return [cx + big * Math.cos(a), cy + big * Math.sin(a)];
  };
  const strip = (inA: Pt, inB: Pt) => {
    const a = out(inA);
    const b = out(inB);
    t.tri(a[0], a[1], b[0], b[1], inB[0], inB[1], DIM);
    t.tri(a[0], a[1], inB[0], inB[1], inA[0], inA[1], DIM);
  };
  strip(f0, f1);
  strip(f1, f2);
  strip(f2, f3);
  strip(f3, f0);
  t.polyline([f0, f1, f2, f3], 1, FRAME_LINE, true);
  shown.frame = true;
  for (const corners of o.otherCameras) {
    t.polyline(
      corners.map(([x, y]) => apply(v, x, y)),
      1,
      OTHER_CAMERA,
      true,
    );
    shown.otherCameras++;
  }
  if (o.selectionDisplay.boxHighlighting) {
    for (const b of o.selectionBoxes) {
      sel.polyline(ring(b), 1, SELECTION_ACCENT, true);
      shown.boxes++;
    }
  }
  for (const [x, y] of o.pivots) {
    const [px, py] = apply(v, x, y);
    const r = PIVOT_CROSS / 2;
    sel.line(px - r, py, px + r, py, 1, SELECTION_ACCENT);
    sel.line(px, py - r, px, py + r, 1, SELECTION_ACCENT);
    shown.pivots++;
  }
  const handles: Pt[] = [];
  if (tool && FURNITURE_TOOLS.has(tool) && o.selectionUnion) {
    const c = ring(o.selectionUnion);
    const xs = c.map((p) => p[0]);
    const ys = c.map((p) => p[1]);
    const box = [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)] as const;
    const corners: Pt[] = [
      [box[0], box[1]],
      [box[2], box[1]],
      [box[2], box[3]],
      [box[0], box[3]],
    ];
    sel.polyline(corners, 1, SELECTION_ACCENT, true);
    const mx = (box[0] + box[2]) / 2;
    const my = (box[1] + box[3]) / 2;
    const mids: Pt[] = [
      [mx, box[1]],
      [box[2], my],
      [mx, box[3]],
      [box[0], my],
    ];
    const skews: Pt[] = [
      [mx, box[1] - SKEW_OUT],
      [box[2] + SKEW_OUT, my],
      [mx, box[3] + SKEW_OUT],
      [box[0] - SKEW_OUT, my],
    ];
    for (const [x, y] of [...corners, ...mids]) sel.square(x, y, HANDLE_SIZE, HANDLE_FILL, SELECTION_ACCENT);
    for (const [x, y] of skews) sel.disc(x, y, HANDLE_SIZE / 2, SELECTION_ACCENT);
    handles.push(...corners, ...mids, ...skews);
    shown.handles = handles.length;
  }
  const all = new ViewTris();
  all.data.push(...t.data);
  for (const x of sel.data) all.data.push(x);
  return { tris: all, selection: sel, frame: t, shown, handles };
}
