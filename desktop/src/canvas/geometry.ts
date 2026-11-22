// View-space geometry the canvas builds itself (M4 C4.2b): the grid (02 §10.1) and the overlay (02 §4.2 layer 9;
// SI-138), as coloured triangles in view pixels. Drawing-space points go through the view's announced matrix
// (`drawingToView`, design note 6): the canvas never derives a mapping.

import type { Rgba, SceneGrid } from './scene';

/** An affine `[a, b, c, d, e, f]`: x' = a x + c y + e, y' = b x + d y + f. */
export type Affine = readonly [number, number, number, number, number, number];

export const IDENTITY: Affine = [1, 0, 0, 1, 0, 0];

export function apply(m: Affine, x: number, y: number): [number, number] {
  return [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
}

export function invert(m: Affine): Affine | null {
  const det = m[0] * m[3] - m[1] * m[2];
  if (!Number.isFinite(det) || Math.abs(det) < 1e-300) return null;
  const a = m[3] / det;
  const b = -m[1] / det;
  const c = -m[2] / det;
  const d = m[0] / det;
  return [a, b, c, d, -(a * m[4] + c * m[5]), -(b * m[4] + d * m[5])];
}

/** `a` after `b`: the affine mapping a point through `b`, then through `a`. */
export function compose(a: Affine, b: Affine): Affine {
  return [
    a[0] * b[0] + a[2] * b[1],
    a[1] * b[0] + a[3] * b[1],
    a[0] * b[2] + a[2] * b[3],
    a[1] * b[2] + a[3] * b[3],
    a[0] * b[4] + a[2] * b[5] + a[4],
    a[1] * b[4] + a[3] * b[5] + a[5],
  ];
}

/** `t`'s triangles mapped by `m` (view pixels to view pixels): the live preview's selection overlay. */
export function mapTris(t: ViewTris, m: Affine): ViewTris {
  const out = new ViewTris();
  const d = t.data;
  for (let i = 0; i + 5 < d.length; i += 6) {
    const [x, y] = apply(m, d[i] ?? 0, d[i + 1] ?? 0);
    out.data.push(x, y, d[i + 2] ?? 0, d[i + 3] ?? 0, d[i + 4] ?? 0, d[i + 5] ?? 0);
  }
  return out;
}

/** The view's linear scale (view pixels per drawing unit). */
export function scaleOf(m: Affine): number {
  return Math.sqrt(Math.abs(m[0] * m[3] - m[1] * m[2]));
}

/** Coloured triangles in view pixels: per vertex x, y, r, g, b, a (colour 0-1, straight alpha). */
export class ViewTris {
  readonly data: number[] = [];

  get vertexCount(): number {
    return this.data.length / 6;
  }

  private v(x: number, y: number, c: Rgba) {
    this.data.push(x, y, c[0] / 255, c[1] / 255, c[2] / 255, c[3] / 255);
  }

  tri(ax: number, ay: number, bx: number, by: number, cx: number, cy: number, c: Rgba) {
    this.v(ax, ay, c);
    this.v(bx, by, c);
    this.v(cx, cy, c);
  }

  /** A segment `width` view pixels wide. */
  line(x0: number, y0: number, x1: number, y1: number, width: number, c: Rgba) {
    const dx = x1 - x0;
    const dy = y1 - y0;
    const len = Math.hypot(dx, dy);
    if (!(len > 0)) return;
    const nx = (-dy / len) * (width / 2);
    const ny = (dx / len) * (width / 2);
    this.tri(x0 + nx, y0 + ny, x1 + nx, y1 + ny, x1 - nx, y1 - ny, c);
    this.tri(x0 + nx, y0 + ny, x1 - nx, y1 - ny, x0 - nx, y0 - ny, c);
  }

  /** A filled disc. */
  disc(cx: number, cy: number, r: number, c: Rgba) {
    if (!(r > 0)) return;
    const n = Math.max(8, Math.min(64, Math.ceil(r * 1.5)));
    for (let i = 0; i < n; i++) {
      const a0 = (i / n) * Math.PI * 2;
      const a1 = ((i + 1) / n) * Math.PI * 2;
      this.tri(
        cx,
        cy,
        cx + Math.cos(a0) * r,
        cy + Math.sin(a0) * r,
        cx + Math.cos(a1) * r,
        cy + Math.sin(a1) * r,
        c,
      );
    }
  }

  /** A polyline with round joins and caps. */
  polyline(points: readonly [number, number][], width: number, c: Rgba, closed = false) {
    const n = points.length;
    if (n === 0) return;
    for (let i = 0; i + 1 < n; i++) {
      const p = points[i];
      const q = points[i + 1];
      if (p && q) this.line(p[0], p[1], q[0], q[1], width, c);
    }
    if (closed && n > 2) {
      const p = points[n - 1];
      const q = points[0];
      if (p && q) this.line(p[0], p[1], q[0], q[1], width, c);
    }
    if (width > 2) for (const p of points) this.disc(p[0], p[1], width / 2, c);
  }

  /** A dashed polyline (the lasso, SI-138). */
  dashed(points: readonly [number, number][], width: number, c: Rgba, dash = 6, closed = false) {
    const pts = closed && points.length > 2 && points[0] ? [...points, points[0]] : [...points];
    let on = true;
    let left = dash;
    for (let i = 0; i + 1 < pts.length; i++) {
      const p = pts[i];
      const q = pts[i + 1];
      if (!p || !q) continue;
      let [x0, y0] = p;
      const len = Math.hypot(q[0] - x0, q[1] - y0);
      let rest = len;
      while (rest > 0) {
        const step = Math.min(rest, left);
        const t = step / rest;
        const x1 = x0 + (q[0] - x0) * t;
        const y1 = y0 + (q[1] - y0) * t;
        if (on) this.line(x0, y0, x1, y1, width, c);
        x0 = x1;
        y0 = y1;
        rest -= step;
        left -= step;
        if (left <= 1e-9) {
          on = !on;
          left = dash;
        }
      }
    }
  }

  /** An axis-aligned square of side `size`, centred (a handle, SI-138). */
  square(cx: number, cy: number, size: number, fill: Rgba, edge: Rgba) {
    const h = size / 2;
    this.tri(cx - h, cy - h, cx + h, cy - h, cx + h, cy + h, edge);
    this.tri(cx - h, cy - h, cx + h, cy + h, cx - h, cy + h, edge);
    const g = h - 1;
    this.tri(cx - g, cy - g, cx + g, cy - g, cx + g, cy + g, fill);
    this.tri(cx - g, cy - g, cx + g, cy + g, cx - g, cy + g, fill);
  }
}

/** Grid colours (SI-138): major and minor lines, field guides. */
export const GRID_MAJOR: Rgba = [0, 0, 0, 64];
export const GRID_MINOR: Rgba = [0, 0, 0, 26];
export const GRID_FIELD: Rgba = [40, 80, 160, 110];

/** The grid of 02 §10.1 in view pixels, for a view of `width` × `height` view pixels. */
export function gridGeometry(
  grid: SceneGrid,
  d2v: Affine,
  width: number,
  height: number,
  out: ViewTris,
): void {
  const v2d = invert(d2v);
  if (!v2d) return;
  const corners = [apply(v2d, 0, 0), apply(v2d, width, 0), apply(v2d, width, height), apply(v2d, 0, height)];
  const xs = corners.map((p) => p[0]);
  const ys = corners.map((p) => p[1]);
  const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  const seg = (ax: number, ay: number, bx: number, by: number, w: number, c: Rgba) => {
    const [px, py] = apply(d2v, ax, ay);
    const [qx, qy] = apply(d2v, bx, by);
    out.line(px, py, qx, qy, w, c);
  };
  const scale = scaleOf(d2v);
  if (grid.type === 'normal') {
    // Major lines every 100 du (one field), minor every 10 du, minor hidden closer than 4 view pixels.
    const minor = 10 * scale >= 4;
    const step = minor ? 10 : 100;
    if (!((x1 - x0) / step < 4000 && (y1 - y0) / step < 4000)) return;
    for (let x = Math.ceil(x0 / step) * step; x <= x1; x += step) {
      const major = Math.round(x) % 100 === 0;
      seg(x, y0, x, y1, 1, major ? GRID_MAJOR : GRID_MINOR);
    }
    for (let y = Math.ceil(y0 / step) * step; y <= y1; y += step) {
      const major = Math.round(y) % 100 === 0;
      seg(x0, y, x1, y, 1, major ? GRID_MAJOR : GRID_MINOR);
    }
    return;
  }
  // Field guides (4:3): nested rectangles for each field size centred on the origin, a centre cross-hair, and
  // graduations along the axes (numbering is drawn by the chrome, not here).
  const fields = grid.type === 'field12' ? 12 : 16;
  // One field is 100 du (02 §10.1; the kernel's `field12` region is ±600 × ±450).
  const fw = 100;
  for (let n = 1; n <= fields; n++) {
    const hw = (n * fw) / 2;
    const hh = (hw * 3) / 4;
    seg(-hw, -hh, hw, -hh, 1, GRID_FIELD);
    seg(hw, -hh, hw, hh, 1, GRID_FIELD);
    seg(hw, hh, -hw, hh, 1, GRID_FIELD);
    seg(-hw, hh, -hw, -hh, 1, GRID_FIELD);
  }
  const hw = (fields * fw) / 2;
  const hh = (hw * 3) / 4;
  seg(-hw, 0, hw, 0, 1, GRID_FIELD);
  seg(0, -hh, 0, hh, 1, GRID_FIELD);
  const tick = 4 / Math.max(scale, 1e-9);
  for (let n = 1; n <= fields; n++) {
    const x = (n * fw) / 2;
    const y = (x * 3) / 4;
    for (const s of [-1, 1]) {
      seg(s * x, -tick, s * x, tick, 1, GRID_FIELD);
      seg(-tick, s * y, tick, s * y, 1, GRID_FIELD);
    }
  }
}
