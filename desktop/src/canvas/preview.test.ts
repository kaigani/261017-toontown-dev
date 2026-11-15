// @vitest-environment node
// The live transform preview (M4 C4.3b; preview note 8): PV4, the preview's affine follows the furniture rule of the
// command the kernel will produce, for every handle (SI-141 for the Drawing view's Select, SI-150 and SI-151 for the
// scene tools), against values derived here from the rule, not read from the preview code.
import { describe, expect, it } from 'vitest';

import { apply, compose, type Affine } from './geometry';
import {
  drawingPreview,
  grabberPreview,
  sceneHandleAt,
  scenePreview,
  SKEW_OUT_PX,
  type Box,
} from './preview';

const close = (got: readonly number[], want: readonly number[], what: string) => {
  for (let i = 0; i < want.length; i++) expect(got[i], `${what}[${String(i)}]`).toBeCloseTo(want[i] ?? 0, 9);
};

describe('the Camera view preview (SI-150, SI-151)', () => {
  // The selection's union box in view pixels (y down): 200..400 x 100..250.
  const b: Box = [200, 100, 400, 250];
  const centre: [number, number] = [300, 175];

  it('move: the translation by the drag (05 §5.1 line 405)', () => {
    const p = scenePreview(b, null, [300, 175], [340, 160], false);
    close(p as Affine, [1, 0, 0, 1, 40, -15], 'move');
  });

  it('a corner scales about the opposite corner; the dragged corner follows the pointer (constrain: equal factors)', () => {
    // Top-right (400, 100) dragged to (450, 80); the opposite corner is bottom-left (200, 250).
    const p = scenePreview(b, null, [400, 100], [450, 80], false) as Affine;
    close(apply(p, 200, 250), [200, 250], 'the opposite corner stays');
    close(apply(p, 400, 100), [450, 80], 'the dragged corner is under the pointer');
    const c = scenePreview(b, null, [400, 100], [450, 80], true) as Affine;
    // Factors (450-200)/(400-200) = 1.25 and (80-250)/(100-250) = 1.1333; constrained: the larger in magnitude.
    close([c[0], c[3]], [1.25, 1.25], 'constrained factors');
  });

  it('a side scales one axis about the opposite side (C43A-R1-22: one axis)', () => {
    const right = scenePreview(b, null, [400, 175], [430, 190], false) as Affine;
    close([right[0], right[3]], [(430 - 200) / (400 - 200), 1], 'right side: x only');
    close(apply(right, 200, 175), [200, 175], 'the left side stays');
    const top = scenePreview(b, null, [300, 100], [310, 70], false) as Affine;
    close([top[0], top[3]], [1, (70 - 250) / (100 - 250)], 'top side: y only');
    close(apply(top, 300, 250), [300, 250], 'the bottom side stays');
  });

  it('rotation about the box centre, by the pointer angle; 15° steps when constrained', () => {
    // Outside the top-right corner, within 18 px: (409, 91) to a point 30° further round the centre.
    const a0 = Math.atan2(91 - centre[1], 409 - centre[0]);
    const r = Math.hypot(409 - centre[0], 91 - centre[1]);
    const a1 = a0 + (37 * Math.PI) / 180;
    const pn: [number, number] = [centre[0] + r * Math.cos(a1), centre[1] + r * Math.sin(a1)];
    const p = scenePreview(b, null, [409, 91], pn, false) as Affine;
    close(apply(p, centre[0], centre[1]), centre, 'the centre stays');
    close(apply(p, 409, 91), pn, 'the press point follows the pointer');
    const s = scenePreview(b, null, [409, 91], pn, true) as Affine;
    // 37° snapped to 30°: the press point turns by 30° about the centre.
    const want: [number, number] = [
      centre[0] + r * Math.cos(a0 + Math.PI / 6),
      centre[1] + r * Math.sin(a0 + Math.PI / 6),
    ];
    close(apply(s, 409, 91), want, 'snapped to 15° steps');
  });

  it('a top skew handle shears horizontally about the centre: the top moves by the drag, the bottom against it', () => {
    const p = scenePreview(b, null, [300, 100 - SKEW_OUT_PX], [320, 100 - SKEW_OUT_PX], false) as Affine;
    close(apply(p, 300, 100), [320, 100], 'the top midpoint moves by 20');
    close(apply(p, 300, 250), [280, 250], 'the bottom midpoint by -20');
    close(apply(p, centre[0], centre[1]), centre, 'the centre stays');
  });

  it('the temporary Skew (SI-151): a top side shears horizontally, a right side vertically; elsewhere nothing', () => {
    const top = scenePreview(b, null, [300, 100], [320, 100], false, true) as Affine;
    close(apply(top, 300, 100), [320, 100], 'top side: horizontal shear');
    const right = scenePreview(b, null, [400, 175], [400, 155], false, true) as Affine;
    close(apply(right, 400, 175), [400, 155], 'the right midpoint moves up by 20');
    close(apply(right, 200, 175), [200, 195], 'the left midpoint down by 20');
    expect(scenePreview(b, null, [300, 175], [340, 175], false, true)).toBeNull();
    expect(scenePreview(b, null, [400, 100], [450, 80], false, true)).toBeNull();
  });

  it('a pivot drag and a rubber band move no occurrence; the pivot marker wins over a corner (SI-150 order)', () => {
    expect(scenePreview(b, [300, 175], [300, 175], [320, 180], false)).toBeNull();
    expect(scenePreview(b, null, [50, 50], [120, 90], false)).toBeNull();
    expect(sceneHandleAt(b, [400, 100], [400, 100])).toEqual({ kind: 'pivot' });
    expect(sceneHandleAt(b, null, [400, 100])).toEqual({ kind: 'corner', i: 1 });
  });
});

describe('the Drawing view preview (SI-141)', () => {
  // The Drawing view at zoom 2 about (40, 30) in a 1920 x 1080 view: view = (2 (x - 40) + 960, 540 - 2 (y - 30)).
  const d2v: Affine = [2, 0, 0, -2, 960 - 80, 540 + 60];
  // The selection's bounds in drawing units.
  const bounds: Box = [0, 0, 100, 50];
  const v = (x: number, y: number) => apply(d2v, x, y);

  it('move: the drag in view pixels', () => {
    const p0 = v(50, 25);
    const p = drawingPreview(bounds, null, d2v, p0, [p0[0] + 30, p0[1] - 10], false) as Affine;
    close(apply(p, 0, 0), [30, -10], 'translation');
  });

  it('a corner scales about the opposite corner in drawing space; the dragged corner follows the pointer', () => {
    // The top-right corner (100, 50) is at view (1080, 560); the opposite, bottom-left (0, 0), at (880, 660).
    const pn: [number, number] = [1100, 540];
    const p = drawingPreview(bounds, null, d2v, v(100, 50), pn, false) as Affine;
    close(apply(p, ...v(0, 0)), v(0, 0), 'the opposite corner stays');
    close(apply(p, ...v(100, 50)), pn, 'the dragged corner is under the pointer');
  });

  it('a rotation turns about the box centre by the pointer angle; the drawing pivot drag moves nothing', () => {
    const o = v(50, 25);
    const p0: [number, number] = [v(100, 50)[0] + 8, v(100, 50)[1] - 8];
    const r = Math.hypot(p0[0] - o[0], p0[1] - o[1]);
    const a0 = Math.atan2(p0[1] - o[1], p0[0] - o[0]);
    const pn: [number, number] = [o[0] + r * Math.cos(a0 + 0.4), o[1] + r * Math.sin(a0 + 0.4)];
    const p = drawingPreview(bounds, null, d2v, p0, pn, false) as Affine;
    close(apply(p, o[0], o[1]), o, 'the centre stays');
    close(apply(p, p0[0], p0[1]), pn, 'the press point follows the pointer');
    expect(drawingPreview(bounds, [10, 10], d2v, v(10, 10), [900, 600], false)).toBeNull();
  });

  it('a marquee from outside the selection previews nothing', () => {
    expect(drawingPreview(bounds, null, d2v, v(-200, -200), v(-100, -100), false)).toBeNull();
    expect(drawingPreview(null, null, d2v, v(50, 25), v(60, 25), false)).toBeNull();
  });
});

describe('the Grabber preview (preview note §3)', () => {
  it('pans everything drawn by the drag', () => {
    const p = grabberPreview([100, 100], [130, 80]);
    close(compose(p, [1, 0, 0, 1, 0, 0]), [1, 0, 0, 1, 30, -20], 'pan');
  });
});
