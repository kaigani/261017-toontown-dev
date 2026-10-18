// C0.3 canvas spike (spike code): decodes a certified `TTRL` render list into GPU-ready buffers.
//
// * Strokes become one instance per flattened line segment (stride 32 bytes: x0, y0, x1, y1, half width (f32),
//   colour RGBA8 (u32), group (u32), padding). The vertex shader expands each instance into an anti-aliased quad.
// * Fills are flattened and triangulated with earcut (one contour per subpath); vertices have stride 16 bytes
//   (x, y (f32), colour (u32), group (u32)) with a u32 index buffer. Anti-aliasing comes from MSAA.
// * Cubics are flattened once at decode time to a world-space tolerance; zooming in far shows facets. A product
//   renderer would re-flatten per zoom level or evaluate curves on the GPU.
// * Draw order within the spike is "all fills, then all strokes" (two draw calls); the scenes are chosen so that
//   this does not change what is drawn materially. M3's renderer must preserve painter's order.
//
// Every index read from the buffer is range-checked against the certified header; malformed content throws.
import earcut from 'earcut';

import type { RenderListHeader } from '../../ipc';

const HEADER = 72;
const GROUP_BYTES = 24;
const PATH_BYTES = 28;
const VERB_MOVE = 0;
const VERB_LINE = 1;
const VERB_CUBIC = 2;
const VERB_CLOSE = 3;
const STYLE_STROKE = 2;
export const STROKE_STRIDE = 32;
export const FILL_STRIDE = 16;
const MAX_SEGMENTS_PER_CUBIC = 32;

export interface Geometry {
  strokes: ArrayBuffer;
  strokeCount: number;
  fillVertices: ArrayBuffer;
  fillVertexCount: number;
  fillIndices: Uint32Array;
  /** Base group transforms, 6 floats each `[a, b, c, d, e, f]`. */
  groups: Float32Array;
  stats: DecodeStats;
}

export interface DecodeStats {
  paths: number;
  strokePaths: number;
  fillPaths: number;
  cubics: number;
  segments: number;
  fillVertices: number;
  triangles: number;
  /** World-space flattening tolerance used. */
  tolerance: number;
  strokeBytes: number;
  fillBytes: number;
}

class GrowBuffer {
  bytes: ArrayBuffer;
  f32: Float32Array;
  u32: Uint32Array;
  length = 0; // in 4-byte words

  constructor(words: number) {
    this.bytes = new ArrayBuffer(words * 4);
    this.f32 = new Float32Array(this.bytes);
    this.u32 = new Uint32Array(this.bytes);
  }

  reserve(words: number): void {
    if (this.length + words <= this.f32.length) return;
    let n = this.f32.length * 2;
    while (n < this.length + words) n *= 2;
    const next = new ArrayBuffer(n * 4);
    new Uint32Array(next).set(this.u32.subarray(0, this.length));
    this.bytes = next;
    this.f32 = new Float32Array(next);
    this.u32 = new Uint32Array(next);
  }

  slice(): ArrayBuffer {
    return this.bytes.slice(0, this.length * 4);
  }
}

function bad(message: string): Error {
  return new Error(`malformed render list: ${message}`);
}

/** Flattens one cubic from (x0,y0) into `out` (excluding the start point), returns the point count added. */
function flattenCubic(
  out: number[],
  p: Float32Array,
  i: number,
  x0: number,
  y0: number,
  tol: number,
): number {
  const x1 = p[i] ?? 0;
  const y1 = p[i + 1] ?? 0;
  const x2 = p[i + 2] ?? 0;
  const y2 = p[i + 3] ?? 0;
  const x3 = p[i + 4] ?? 0;
  const y3 = p[i + 5] ?? 0;
  const ddx = Math.max(Math.abs(x0 - 2 * x1 + x2), Math.abs(x1 - 2 * x2 + x3));
  const ddy = Math.max(Math.abs(y0 - 2 * y1 + y2), Math.abs(y1 - 2 * y2 + y3));
  const dd = Math.sqrt(ddx * ddx + ddy * ddy);
  const n = Math.min(MAX_SEGMENTS_PER_CUBIC, Math.max(1, Math.ceil(Math.sqrt((0.75 * dd) / tol))));
  for (let k = 1; k <= n; k++) {
    const t = k / n;
    const u = 1 - t;
    const b0 = u * u * u;
    const b1 = 3 * u * u * t;
    const b2 = 3 * u * t * t;
    const b3 = t * t * t;
    out.push(b0 * x0 + b1 * x1 + b2 * x2 + b3 * x3, b0 * y0 + b1 * y1 + b2 * y2 + b3 * y3);
  }
  return n;
}

export function decodeRenderList(header: RenderListHeader, bytes: ArrayBuffer, tolerance = 0.5): Geometry {
  const G = header.groups;
  const P = header.paths;
  const N = header.points;
  const V = header.verbs;
  const groupsOff = HEADER;
  const pathsOff = groupsOff + G * GROUP_BYTES;
  const pointsOff = pathsOff + P * PATH_BYTES;
  const verbsOff = pointsOff + N * 8;
  if (verbsOff + V > bytes.byteLength) throw bad('sections exceed the buffer');
  const groups = new Float32Array(bytes.slice(groupsOff, pathsOff));
  const paths = new DataView(bytes, pathsOff, P * PATH_BYTES);
  const points = new Float32Array(bytes, pointsOff, N * 2);
  const verbs = new Uint8Array(bytes, verbsOff, V);
  for (const g of groups) if (!Number.isFinite(g)) throw bad('non-finite group transform');

  const strokes = new GrowBuffer(1 << 20);
  const fillV = new GrowBuffer(1 << 18);
  const fillI: number[] = [];
  const stats: DecodeStats = {
    paths: P,
    strokePaths: 0,
    fillPaths: 0,
    cubics: 0,
    segments: 0,
    fillVertices: 0,
    triangles: 0,
    tolerance,
    strokeBytes: 0,
    fillBytes: 0,
  };
  const flat: number[] = [];
  const contours: number[][] = [];

  for (let p = 0; p < P; p++) {
    const o = p * PATH_BYTES;
    const verbStart = paths.getUint32(o, true);
    const verbCount = paths.getUint32(o + 4, true);
    let pt = paths.getUint32(o + 8, true);
    const group = paths.getUint32(o + 12, true);
    const colour = paths.getUint32(o + 16, true);
    const style = paths.getUint32(o + 20, true);
    const width = paths.getFloat32(o + 24, true);
    if (verbStart + verbCount > V) throw bad(`path ${String(p)} verbs out of range`);
    if (group >= G) throw bad(`path ${String(p)} group ${String(group)} out of range`);
    if (style > STYLE_STROKE) throw bad(`path ${String(p)} unknown style ${String(style)}`);
    if (!Number.isFinite(width) || width < 0) throw bad(`path ${String(p)} invalid width`);

    // Flatten into contours (world coordinates).
    contours.length = 0;
    let current: number[] | null = null;
    let cx = 0;
    let cy = 0;
    const take = (count: number): number => {
      if (pt + count > N) throw bad(`path ${String(p)} points out of range`);
      const at = pt * 2;
      pt += count;
      return at;
    };
    for (let v = verbStart; v < verbStart + verbCount; v++) {
      const verb = verbs[v];
      if (verb === VERB_MOVE) {
        const at = take(1);
        cx = points[at] ?? 0;
        cy = points[at + 1] ?? 0;
        current = [cx, cy];
        contours.push(current);
      } else if (verb === VERB_LINE) {
        const at = take(1);
        cx = points[at] ?? 0;
        cy = points[at + 1] ?? 0;
        if (!current) throw bad(`path ${String(p)} line before move`);
        current.push(cx, cy);
      } else if (verb === VERB_CUBIC) {
        const at = take(3);
        if (!current) throw bad(`path ${String(p)} cubic before move`);
        stats.cubics++;
        flattenCubic(current, points, at, cx, cy, tolerance);
        cx = points[at + 4] ?? 0;
        cy = points[at + 5] ?? 0;
      } else if (verb === VERB_CLOSE) {
        current = null;
      } else {
        throw bad(`path ${String(p)} unknown verb ${String(verb)}`);
      }
    }

    if (style === STYLE_STROKE) {
      stats.strokePaths++;
      const hw = width / 2;
      for (const c of contours) {
        const segs = c.length / 2 - 1;
        if (segs < 1) continue;
        strokes.reserve(segs * 8);
        for (let k = 0; k < segs; k++) {
          const w = strokes.length;
          strokes.f32[w] = c[2 * k] ?? 0;
          strokes.f32[w + 1] = c[2 * k + 1] ?? 0;
          strokes.f32[w + 2] = c[2 * k + 2] ?? 0;
          strokes.f32[w + 3] = c[2 * k + 3] ?? 0;
          strokes.f32[w + 4] = hw;
          strokes.u32[w + 5] = colour;
          strokes.u32[w + 6] = group;
          strokes.u32[w + 7] = 0;
          strokes.length += 8;
        }
        stats.segments += segs;
      }
    } else {
      stats.fillPaths++;
      for (const c of contours) {
        if (c.length < 6) continue;
        flat.length = 0;
        for (const v of c) flat.push(v);
        const base = fillV.length / 4;
        const tris = earcut(flat);
        const count = flat.length / 2;
        fillV.reserve(count * 4);
        for (let k = 0; k < count; k++) {
          const w = fillV.length;
          fillV.f32[w] = flat[2 * k] ?? 0;
          fillV.f32[w + 1] = flat[2 * k + 1] ?? 0;
          fillV.u32[w + 2] = colour;
          fillV.u32[w + 3] = group;
          fillV.length += 4;
        }
        for (const t of tris) fillI.push(base + t);
        stats.fillVertices += count;
        stats.triangles += tris.length / 3;
      }
    }
  }
  const strokeBytes = strokes.slice();
  const fillBytes = fillV.slice();
  stats.strokeBytes = strokeBytes.byteLength;
  stats.fillBytes = fillBytes.byteLength + fillI.length * 4;
  return {
    strokes: strokeBytes,
    strokeCount: strokes.length / 8,
    fillVertices: fillBytes,
    fillVertexCount: fillV.length / 4,
    fillIndices: Uint32Array.from(fillI),
    groups,
    stats,
  };
}
