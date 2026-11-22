// @vitest-environment node
/// <reference types="node" />
// The Camera view canvas, window half (M4 C4.3b; camera-view note §6): CV1 (the kernel's order and each occurrence
// through `cameraToView · matrix`; `currentOnTop`), CV2 (a drawing shown twice is loaded once, at the finer bucket),
// CV3 (a source check: no projection is computed in the window), the overlays (dimming outside the frame only, boxes
// with box highlighting, the scene tools' furniture) and the grid on the camera frame (SI-153).
import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

import { parseWireJson } from '../ipc/wireJson';
import { bucketOf } from './bucket';
import { cameraGridMatrix, cameraOverlay, DIM, SKEW_OUT } from './cameraOverlay';
import { apply, compose, scaleOf, type Affine } from './geometry';
import { cameraOrdered, drawFrame, entryToView, ordered, type LayerOps } from './layers';
import { CanvasModel, type CanvasHost, type ResidentSink } from './model';
import { parseSceneAnswer, type Scene, type SceneEntry } from './scene';

const here = fileURLToPath(new URL('./', import.meta.url));

/** A value the fixture must have (a missing one fails the test where it is read). */
function must<T>(v: T | null | undefined, what: string): T {
  if (v === null || v === undefined) throw new Error(`the fixture has no ${what}`);
  return v;
}
const answer = () =>
  parseSceneAnswer(parseWireJson(readFileSync(`${here}testdata/scene/valid/camera.json`, 'utf8')));

/** A recording back end: the entries drawn, in order. */
function recorder(): { ops: LayerOps; drawn: SceneEntry[] } {
  const drawn: SceneEntry[] = [];
  const ops: LayerOps = {
    background: () => undefined,
    grid: () => undefined,
    drawing: (e) => drawn.push(e),
    endStatic: () => undefined,
    overlay: () => undefined,
  };
  return { ops, drawn };
}

describe('the Camera view canvas (camera-view note §6)', () => {
  it('CV1: draws the occurrences in the kernel order, each through cameraToView · matrix', () => {
    const full = answer().scene;
    const s: Scene = { ...full, overlays: full.overlays && { ...full.overlays, currentOnTop: false } };
    const { ops, drawn } = recorder();
    drawFrame(s, ops, () => true, true);
    expect(drawn.map((e) => e.elementId)).toEqual(s.entries.map((e) => e.elementId));
    const v = s.camera?.cameraToView as Affine;
    for (const e of drawn) {
      const m = entryToView(e, v, [1, 0, 0, 1, 0, 0]);
      // Spot check against an independent product at two points of the drawing.
      for (const [x, y] of [
        [0, 0],
        [37, -12],
      ] as [number, number][]) {
        const [ox, oy] = apply(e.matrix as Affine, x, y);
        const want = apply(v, ox, oy);
        const got = apply(m, x, y);
        expect(got[0]).toBeCloseTo(want[0], 9);
        expect(got[1]).toBeCloseTo(want[1], 9);
      }
    }
    // The Drawing view's role order does not apply: `current` keeps its place in the kernel's order.
    expect(ordered(s.entries, s).map((e) => e.role)).toEqual(s.entries.map((e) => e.role));
  });

  it('CV1: currentOnTop moves only the current drawing after the others; outline entries stay last', () => {
    const s = answer().scene;
    const outline: SceneEntry = { ...must(s.entries[0], 'entry'), outline: true, tint: [32, 128, 255, 255] };
    const entries = [...s.entries, outline];
    const plain = cameraOrdered(entries, false);
    expect(plain).toEqual([...s.entries, outline]);
    const top = cameraOrdered(entries, true);
    const current = s.entries.filter((e) => e.role === 'current');
    const others = s.entries.filter((e) => e.role !== 'current');
    expect(current.length).toBeGreaterThan(0);
    expect(top).toEqual([...others, ...current, outline]);
  });

  it('CV2: a drawing shown by two occurrences is loaded once, at the bucket of the larger', async () => {
    const loads: { drawingId: string; bucket: number }[] = [];
    let release: ((v: unknown) => void) | null = null;
    const host: CanvasHost = {
      canvasScene: () => new Promise((r) => (release = r)),
      canvasLoad: (drawingId, bucket) => {
        loads.push({ drawingId, bucket });
        return new Promise(() => undefined);
      },
    };
    const sink: ResidentSink = { upload: () => undefined, drop: () => undefined };
    const model = new CanvasModel('cameraView', host, sink, { now: () => 0 });
    const base = answer();
    const e0 = must(base.scene.entries[0], 'entry');
    const small: SceneEntry = { ...e0, elementId: 'el_a', matrix: [0.5, 0, 0, -0.5, 100, 100] };
    const large: SceneEntry = { ...e0, elementId: 'el_b', matrix: [3, 0, 0, -3, 900, 500] };
    const s: Scene = { ...base.scene, entries: [small, large] };
    model.setGeneration(Number(base.generation));
    model.frame();
    (release as unknown as (v: unknown) => void)({ generation: base.generation, scene: s });
    await new Promise<void>((r) => setTimeout(r, 0));
    model.frame();
    model.frame();
    const v = s.camera?.cameraToView as Affine;
    const want = Math.max(
      bucketOf(scaleOf(compose(v, small.matrix as Affine))),
      bucketOf(scaleOf(compose(v, large.matrix as Affine))),
    );
    expect(loads).toEqual([{ drawingId: e0.drawingId, bucket: want }]);
    expect(model.bucketIn(s, e0.drawingId)).toBe(want);
  });

  it('CV3: no projection is computed in the window (a source check over desktop/src/canvas)', () => {
    const files = readdirSync(here).filter((n) => /\.(ts|tsx)$/.test(n) && !/\.test\.tsx?$/.test(n));
    expect(files.length).toBeGreaterThan(10);
    const forbidden: [RegExp, string][] = [
      [/\bfov\b/i, 'a field of view'],
      [/Math\.tan\(/, 'a tangent'],
      [/Math\.atan\(/, 'an arc tangent of a ratio'],
      // (The Perspective tool's name is not a projection.)
      [/perspective\s*(divi|matri|proj)/i, 'a perspective division'],
      [/nearPlane|NEAR_PLANE/, 'the near plane'],
    ];
    const found: string[] = [];
    for (const f of files) {
      const text = readFileSync(`${here}${f}`, 'utf8');
      for (const [re, what] of forbidden) if (re.test(text)) found.push(`${f}: ${what}`);
    }
    expect(found).toEqual([]);
  });

  it('the overlay dims outside the camera frame only, draws boxes with box highlighting, and the furniture', () => {
    const s = answer().scene;
    const o = s.overlays;
    if (!o || !s.camera) throw new Error('fixture without overlays');
    const W = 960;
    const H = 540;
    const inside = (tris: number[], x: number, y: number, colour: readonly number[]) => {
      // Whether (x, y) is inside a triangle of the given colour (the vertex data: x, y, r, g, b, a per vertex).
      const c = colour.map((v) => v / 255);
      for (let i = 0; i + 17 < tris.length; i += 18) {
        const col = tris.slice(i + 2, i + 6);
        if (col.some((v, k) => Math.abs(v - (c[k] ?? 0)) > 1e-6)) continue;
        const at = (k: number) => tris[i + k] ?? Number.NaN;
        const [ax, ay, bx, by, cx, cy] = [at(0), at(1), at(6), at(7), at(12), at(13)];
        const d = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
        const u = ((bx - x) * (cy - y) - (by - y) * (cx - x)) / d;
        const w = ((cx - x) * (ay - y) - (cy - y) * (ax - x)) / d;
        if (u >= 0 && w >= 0 && u + w <= 1) return true;
      }
      return false;
    };
    const v = s.camera.cameraToView;
    const f = o.cameraFrame;
    const centre = apply(v, (f[0] + f[2]) / 2, (f[1] + f[3]) / 2);
    const corner = apply(v, f[0] - 40, f[1] - 40);
    const c = cameraOverlay(s, W, H, 'transform');
    const data = c.tris.data;
    expect(inside(data, centre[0], centre[1], DIM)).toBe(false);
    expect(inside(data, corner[0], corner[1], DIM)).toBe(true);
    expect(c.shown.frame).toBe(true);
    expect(c.shown.boxes).toBe(o.selectionDisplay.boxHighlighting ? o.selectionBoxes.length : 0);
    expect(c.shown.pivots).toBe(o.pivots.length);
    expect(c.shown.otherCameras).toBe(o.otherCameras.length);
    // SI-150: four corners, four side midpoints, four skew handles SKEW_OUT outside the midpoints.
    expect(c.handles.length).toBe(o.selectionUnion ? 12 : 0);
    if (o.selectionUnion) {
      const [top, skewTop] = [c.handles[4], c.handles[8]] as [[number, number], [number, number]];
      expect(top[1] - skewTop[1]).toBeCloseTo(SKEW_OUT, 9);
    }
    // A drawing tool draws no scene furniture.
    expect(cameraOverlay(s, W, H, null).handles).toEqual([]);
    // Box highlighting off: no boxes.
    const off = cameraOverlay(
      { ...s, overlays: { ...o, selectionDisplay: { ...o.selectionDisplay, boxHighlighting: false } } },
      W,
      H,
      null,
    );
    expect(off.shown.boxes).toBe(0);
  });

  it('SI-153: the field guide spans the camera frame (12 fields for field12, 16 for field16), centred on it', () => {
    const s = answer().scene;
    const cam = s.camera;
    if (!cam) throw new Error('fixture without a camera');
    for (const [type, fields] of [
      ['field12', 12],
      ['normal', 12],
      ['field16', 16],
    ] as const) {
      const g = must(cameraGridMatrix(s, { visible: true, type, overlay: false }), 'camera');
      // Grid units: one field is 100; the frame's half width is fields / 2 fields.
      const right = apply(g, (fields * 100) / 2, 0);
      const want = apply(cam.cameraToView, cam.size[0], cam.size[1] / 2);
      expect(right[0]).toBeCloseTo(want[0], 6);
      expect(right[1]).toBeCloseTo(want[1], 6);
      const middle = apply(g, 0, 0);
      const centre = apply(cam.cameraToView, cam.size[0] / 2, cam.size[1] / 2);
      expect(middle[0]).toBeCloseTo(centre[0], 6);
      expect(middle[1]).toBeCloseTo(centre[1], 6);
    }
  });
});
