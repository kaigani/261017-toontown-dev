// C0.3 canvas spike (spike code): the scripted pan/zoom camera and the per-frame group animation.

export const WORLD = 4096;
export const PAN_ZOOM_MS = 10_000;

/** World → device pixels: `device = world * scale + offset`. */
export interface Camera {
  scale: number;
  offsetX: number;
  offsetY: number;
  /** Target size in device pixels. */
  width: number;
  height: number;
}

/**
 * Camera at `ms` into the 10 s pan/zoom run: zoom oscillates between 0.35× and 2.8× of "world fits the view" with a
 * 5 s period, while the view centre circles the world centre (radius 900 world units, 10 s period). The motion
 * never stops, so every frame needs a full redraw.
 */
export function cameraAt(ms: number, width: number, height: number): Camera {
  const fit = Math.min(width, height) / WORLD;
  const zoom = fit * Math.pow(2, 1.5 * Math.sin((2 * Math.PI * ms) / 5000));
  const angle = (2 * Math.PI * ms) / 10_000;
  const cx = WORLD / 2 + 900 * Math.cos(angle);
  const cy = WORLD / 2 + 900 * Math.sin(angle);
  return { scale: zoom, offsetX: width / 2 - cx * zoom, offsetY: height / 2 - cy * zoom, width, height };
}

/**
 * Rotates every group's linear part about its own origin (`groups1k` animation; other scenes have one identity
 * group and are left unchanged). `out` receives 8 floats per group: `[a, b, c, d, e, f, 0, 0]` (GPU-friendly).
 */
export function animateGroups(base: Float32Array, out: Float32Array, ms: number, animate: boolean): void {
  const n = base.length / 6;
  for (let i = 0; i < n; i++) {
    const a = base[6 * i] ?? 1;
    const b = base[6 * i + 1] ?? 0;
    const c = base[6 * i + 2] ?? 0;
    const d = base[6 * i + 3] ?? 1;
    let ra = a;
    let rb = b;
    let rc = c;
    let rd = d;
    if (animate) {
      const angle = (ms / 1000) * 0.8 + i * 0.1;
      const cs = Math.cos(angle);
      const sn = Math.sin(angle);
      ra = a * cs - b * sn;
      rb = a * sn + b * cs;
      rc = c * cs - d * sn;
      rd = c * sn + d * cs;
    }
    out.set([ra, rb, rc, rd, base[6 * i + 4] ?? 0, base[6 * i + 5] ?? 0, 0, 0], 8 * i);
  }
}
