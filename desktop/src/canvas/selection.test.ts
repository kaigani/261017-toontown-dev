// @vitest-environment node
// The selection overlay (layer 9; frame-loop note §7, C42B-R1-22): which tools show what, and that the overlay follows
// the selection and the mapping (zoom, pan, rotation) in view pixels.
import { describe, expect, it } from 'vitest';

import { apply, IDENTITY, type Affine } from './geometry';
import {
  boxCorners,
  HANDLE_SIZE,
  handlePoints,
  selectionCursor,
  selectionOverlay,
  type SceneSelection,
} from './selection';

const sel: SceneSelection = {
  objects: [
    [0, 0, 100, 50],
    [20, 10, 40, 30],
  ],
  bounds: [0, 0, 100, 50],
  pivot: [50, 25],
  points: [],
};

/** The headless Drawing view's mapping at a zoom, pan and rotation: drawing (y up) to view pixels (y down). */
function mapping(zoom: number, degrees: number, tx: number, ty: number): Affine {
  const r = (degrees * Math.PI) / 180;
  const c = Math.cos(r) * zoom;
  const s = Math.sin(r) * zoom;
  return [c, -s, -s, -c, tx, ty];
}

describe('the selection overlay', () => {
  it('shows outlines, the box, eight handles and the pivot for Select only; outlines for the contour tools', () => {
    const select = selectionOverlay(sel, [10, 10], 'select', IDENTITY);
    expect(select.shown).toEqual({ objects: 2, handles: 8, points: 0, pivot: true });
    expect(select.tris?.vertexCount).toBeGreaterThan(0);
    const contour = selectionOverlay({ ...sel, points: [[5, 5]] }, [10, 10], 'contourEditor', IDENTITY);
    expect(contour.shown).toEqual({ objects: 2, handles: 0, points: 1, pivot: false });
    // 02 §1.3: a creation tool keeps the selection and hides it.
    for (const tool of ['brush', 'pencil', 'rectangle', 'paint', 'zoom']) {
      const o = selectionOverlay(sel, [10, 10], tool, IDENTITY);
      expect(o.tris).toBeNull();
      expect(o.shown).toEqual({ objects: 0, handles: 0, points: 0, pivot: false });
    }
    // Nothing selected: the Select tool still shows the drawing pivot (02 §7.1.1).
    expect(selectionOverlay(null, [0, 0], 'select', IDENTITY).shown).toEqual({
      objects: 0,
      handles: 0,
      points: 0,
      pivot: true,
    });
    expect(selectionOverlay(null, null, 'select', IDENTITY).tris).toBeNull();
  });

  it('follows the selection: another selection gives other handles', () => {
    const a = selectionOverlay(sel, null, 'select', IDENTITY).handles;
    const b = selectionOverlay({ ...sel, bounds: [10, 10, 30, 70] }, null, 'select', IDENTITY).handles;
    expect(b).not.toEqual(a);
    expect(b.slice(0, 4)).toEqual(boxCorners([10, 10, 30, 70]));
  });

  it('follows the mapping: zoomed, panned and rotated, the handles sit at the mapped corners and midpoints', () => {
    for (const m of [mapping(1, 0, 960, 540), mapping(2, 0, 900, 500), mapping(0.5, 30, 700, 400)]) {
      const h = selectionOverlay(sel, null, 'select', m).handles;
      const corners = boxCorners(sel.bounds ?? [0, 0, 0, 0]).map(([x, y]) => apply(m, x, y));
      for (let i = 0; i < 4; i++) {
        expect(h[i]?.[0]).toBeCloseTo(corners[i]?.[0] ?? NaN, 9);
        expect(h[i]?.[1]).toBeCloseTo(corners[i]?.[1] ?? NaN, 9);
      }
      // The bottom side's midpoint (handle 4) between corners 0 and 1.
      expect(h[4]?.[0]).toBeCloseTo(((corners[0]?.[0] ?? 0) + (corners[1]?.[0] ?? 0)) / 2, 9);
      expect(handlePoints(sel.bounds ?? [0, 0, 0, 0], m)).toEqual(h);
    }
    // The outlines stay one view pixel wide at every zoom: the same number of triangles at 1x and 4x.
    const one = selectionOverlay(sel, null, 'select', mapping(1, 0, 0, 0)).tris?.vertexCount;
    const four = selectionOverlay(sel, null, 'select', mapping(4, 0, 0, 0)).tris?.vertexCount;
    expect(four).toBe(one);
  });

  it('keeps the handles a constant size in view pixels at zoom 1/4, 1 and 4 and under rotation', () => {
    for (const zoom of [0.25, 1, 4]) {
      for (const degrees of [0, 30, 90]) {
        const o = selectionOverlay(sel, null, 'select', mapping(zoom, degrees, 640, 360));
        const data = o.tris?.data ?? [];
        expect(o.handles.length).toBe(8);
        // Each handle's light centre (only handle squares are white) is two consecutive triangles: six vertices.
        const white: [number, number][] = [];
        for (let i = 0; i < data.length; i += 6) {
          if (data[i + 2] === 1 && data[i + 3] === 1 && data[i + 4] === 1)
            white.push([data[i] ?? 0, data[i + 1] ?? 0]);
        }
        expect(white.length).toBe(8 * 6);
        o.handles.forEach(([hx, hy], k) => {
          const square = white.slice(k * 6, k * 6 + 6);
          const xs = square.map((v) => v[0]);
          const ys = square.map((v) => v[1]);
          // An 8-pixel square with a 1-pixel accent edge: a 6-pixel light centre, axis-aligned on the screen.
          expect(Math.max(...xs) - Math.min(...xs)).toBeCloseTo(HANDLE_SIZE - 2, 9);
          expect(Math.max(...ys) - Math.min(...ys)).toBeCloseTo(HANDLE_SIZE - 2, 9);
          expect((Math.max(...xs) + Math.min(...xs)) / 2).toBeCloseTo(hx, 9);
          expect((Math.max(...ys) + Math.min(...ys)) / 2).toBeCloseTo(hy, 9);
        });
      }
    }
  });

  it('picks the resize cursor of the nearest direction over a handle, move inside, none outside', () => {
    const m = mapping(1, 0, 500, 500);
    const h = handlePoints(sel.bounds ?? [0, 0, 0, 0], m);
    const at = (i: number): [number, number] => [h[i]?.[0] ?? 0, h[i]?.[1] ?? 0];
    // Corners 0 and 2 (bottom-left, top-right) lie on one diagonal, 1 and 3 on the other; sides 0/2 vertical, 1/3
    // horizontal.
    expect(selectionCursor(h, sel.bounds, m, at(4))).toBe('ns-resize');
    expect(selectionCursor(h, sel.bounds, m, at(5))).toBe('ew-resize');
    expect(selectionCursor(h, sel.bounds, m, at(0))).not.toBe(selectionCursor(h, sel.bounds, m, at(1)));
    expect(selectionCursor(h, sel.bounds, m, apply(m, 50, 25))).toBe('move');
    expect(selectionCursor(h, sel.bounds, m, apply(m, 300, 300))).toBe('');
    expect(selectionCursor([], null, m, apply(m, 50, 25))).toBe('');
  });
});
