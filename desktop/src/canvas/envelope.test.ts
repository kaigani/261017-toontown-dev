// @vitest-environment node
// Perspective and Reposition All in the window (M4 C4.4b; design note 4 §2, §6, §9): the port of the kernel's map
// against the golden the kernel writes (`testdata/envelopes.json`, `cargo test -p tt-executor --test c44b_envelope`),
// the handle rows against values derived here (PV4), the packing the vertex stages read, handles and cursors.
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

import {
  envelopeCorners,
  envelopeOf,
  mapPoint,
  packMap,
  toolBoxCursor,
  toolBoxOverlay,
  toolHandleAt,
  toolPreview,
  unpackApply,
  type Box,
  type PreviewMap,
  type Pt,
  type ToolPress,
} from './envelope';
import type { Affine } from './geometry';

interface Golden {
  cases: {
    name: string;
    box: [number, number, number, number];
    corners: [Pt, Pt, Pt, Pt];
    mapping: 'auto' | 'projective' | 'bilinear';
    points: Pt[];
    mapped: Pt[];
  }[];
}

const golden = JSON.parse(
  readFileSync(new URL('./testdata/envelopes.json', import.meta.url), 'utf8'),
) as Golden;

function must<T>(v: T | null | undefined): T {
  if (v === null || v === undefined) throw new Error('missing value');
  return v;
}

describe("the window's map is the kernel's (envelopes.json)", () => {
  it('covers the affine, projective and bilinear branches', () => {
    const kinds = golden.cases.map((c) => must(envelopeOf(c.box, c.corners, c.mapping)).kind);
    expect(kinds).toEqual(['projective', 'bilinear', 'affine', 'bilinear', 'projective']);
  });
  for (const c of golden.cases) {
    it(`${c.name}: every anchor where the command put it, within 1e-9 du`, () => {
      const m = must(envelopeOf(c.box, c.corners, c.mapping));
      c.points.forEach((p, i) => {
        const q = mapPoint(m, p);
        const want = must(c.mapped[i]);
        expect(Math.abs(q[0] - want[0])).toBeLessThanOrEqual(1e-9);
        expect(Math.abs(q[1] - want[1])).toBeLessThanOrEqual(1e-9);
      });
    });
  }
});

const BOX: Box = [300, 300, 400, 360];
/** Drawing -> view with y down, the Drawing view's default mapping. */
const D2V: Affine = [1, 0, 0, -1, 960, 540];
const view = (x: number, y: number): Pt => [x + 960, 540 - y];

describe('handles (design note 4 §2)', () => {
  it('corners before sides, 6 view px, the rotation zone only for Reposition All', () => {
    expect(toolHandleAt(BOX, D2V, view(400, 360), false)).toEqual({ kind: 'corner', i: 2 });
    expect(toolHandleAt(BOX, D2V, view(406, 360), false)).toEqual({ kind: 'corner', i: 2 });
    expect(toolHandleAt(BOX, D2V, view(406.01, 360), false)).toBeNull();
    expect(toolHandleAt(BOX, D2V, view(350, 300), false)).toEqual({ kind: 'side', i: 0 });
    expect(toolHandleAt(BOX, D2V, view(410, 370), false)).toBeNull();
    expect(toolHandleAt(BOX, D2V, view(410, 370), true)).toEqual({ kind: 'rotate' });
    // At 200 % the 6 px radius is 3 du.
    const zoom2: Affine = [2, 0, 0, -2, 960, 540];
    const v2 = (x: number, y: number): Pt => [2 * x + 960, 540 - 2 * y];
    expect(toolHandleAt(BOX, zoom2, v2(403, 360), false)).toEqual({ kind: 'corner', i: 2 });
    expect(toolHandleAt(BOX, zoom2, v2(403.01, 360), false)).toBeNull();
  });

  it('eight handles, grey when locked', () => {
    const t = toolBoxOverlay(BOX, D2V, true);
    const colours = new Set<string>();
    for (let i = 0; i < t.data.length; i += 6)
      colours.add(
        t.data
          .slice(i + 2, i + 6)
          .map((x) => Math.round(x * 255))
          .join(','),
      );
    expect([...colours].sort()).toEqual(['140,140,140,255', '255,255,255,255']);
  });

  it('cursors with and without alternate', () => {
    expect(toolBoxCursor('perspective', BOX, D2V, view(400, 360), false, false)).toBe('move');
    expect(toolBoxCursor('perspective', BOX, D2V, view(350, 360), true, false)).toBe('ns-resize');
    expect(toolBoxCursor('perspective', BOX, D2V, view(400, 330), true, false)).toBe('ew-resize');
    expect(toolBoxCursor('perspective', BOX, D2V, view(0, 0), false, false)).toBe('');
    expect(toolBoxCursor('perspective', BOX, D2V, view(400, 360), false, true)).toBe('not-allowed');
    expect(toolBoxCursor('repositionAll', BOX, D2V, view(0, 0), false, false)).toBe('move');
    expect(toolBoxCursor('repositionAll', BOX, D2V, view(400, 360), true, false)).toBe('move');
    expect(toolBoxCursor('repositionAll', BOX, D2V, view(410, 370), false, false)).toBe('grab');
    expect(toolBoxCursor('repositionAll', null, D2V, view(0, 0), false, false)).toBe('move');
  });
});

const press = (over: Partial<ToolPress>): ToolPress => ({
  tool: 'perspective',
  box: BOX,
  handle: null,
  alternate: false,
  constrain: false,
  mapping: 'auto',
  ...over,
});

describe('the preview of each handle row (design note 4 §3, §4) against values derived here', () => {
  it('Perspective corner: that corner follows the drag; a keystone is projective', () => {
    const m = must(toolPreview(press({ handle: { kind: 'corner', i: 2 } }), [400, 360], [376, 360]));
    expect(m.kind).toBe('projective');
    expect(mapPoint(m, [400, 360])[0]).toBeCloseTo(376, 9);
    expect(mapPoint(m, [300, 300])[0]).toBeCloseTo(300, 9);
  });

  it('Perspective plain side: both corners follow (a translated side keeps a parallelogram: the exact affine map)', () => {
    const m = must(toolPreview(press({ handle: { kind: 'side', i: 2 } }), [350, 360], [350, 370]));
    expect(mapPoint(m, [400, 360])).toEqual([400, 370]);
    expect(mapPoint(m, [300, 360])).toEqual([300, 370]);
    expect(m.kind).toBe('affine');
    // Any translation of one side keeps a parallelogram.
    const k = must(toolPreview(press({ handle: { kind: 'side', i: 1 } }), [400, 330], [410, 340]));
    expect(k.kind).toBe('affine');
  });

  it('Perspective alternate side: the corners spread symmetrically (SI-188)', () => {
    expect(envelopeCorners({ kind: 'side', i: 2 }, BOX, [15, 4], true)).toEqual([
      [300, 300],
      [400, 300],
      [415, 364],
      [285, 364],
    ]);
  });

  it('Perspective off the handles: no preview (a selection)', () => {
    expect(toolPreview(press({ handle: null }), [0, 0], [10, 10])).toBeNull();
  });

  it('Reposition: offset anywhere, Select scale on a corner, the rotation about the centre', () => {
    const r = (h: ToolPress['handle'], extra: Partial<ToolPress> = {}) =>
      press({ tool: 'repositionAll', handle: h, ...extra });
    expect(toolPreview(r(null), [10, 10], [40, 0])).toEqual({ kind: 'affine', m: [1, 0, 0, 1, 30, -10] });
    const s = must(toolPreview(r({ kind: 'corner', i: 2 }), [400, 360], [420, 370]));
    // About the opposite corner (300, 300): factors 120/100 and 70/60.
    expect(s).toEqual({ kind: 'affine', m: [1.2, 0, 0, 70 / 60, 300 - 1.2 * 300, 300 - (70 / 60) * 300] });
    const rot = must(toolPreview(r({ kind: 'rotate' }), [410, 370], [290, 370]));
    if (rot.kind !== 'affine') throw new Error(rot.kind);
    // The centre stays.
    const c = mapPoint(rot, [350, 330]);
    expect(c[0]).toBeCloseTo(350, 9);
    expect(c[1]).toBeCloseTo(330, 9);
  });

  it('Reposition alternate: envelope corners; with constrain on a side, the symmetric drag', () => {
    const r = (h: ToolPress['handle'], extra: Partial<ToolPress>) =>
      press({ tool: 'repositionAll', handle: h, ...extra });
    const m = must(toolPreview(r({ kind: 'corner', i: 1 }, { alternate: true }), [400, 300], [406, 296]));
    expect(mapPoint(m, [400, 300])[0]).toBeCloseTo(406, 9);
    const sym = must(
      toolPreview(r({ kind: 'side', i: 1 }, { alternate: true, constrain: true }), [400, 330], [403, 325]),
    );
    expect(mapPoint(sym, [400, 360])[0]).toBeCloseTo(403, 9);
    // The drag's part along the side (-5) moves the upper corner down and the lower one up: they close.
    expect(mapPoint(sym, [400, 360])[1]).toBeCloseTo(355, 9);
    expect(mapPoint(sym, [400, 300])[1]).toBeCloseTo(305, 9);
  });

  it('a drag scaling below the minimum previews nothing (the kernel refuses it)', () => {
    expect(
      toolPreview(press({ tool: 'repositionAll', handle: { kind: 'side', i: 1 } }), [400, 330], [300, 330]),
    ).toBeNull();
  });
});

describe('the packing the vertex stages read', () => {
  const maps: PreviewMap[] = [
    must(
      envelopeOf(
        BOX,
        [
          [300, 300],
          [400, 300],
          [376, 360],
          [324, 360],
        ],
        'auto',
      ),
    ),
    must(
      envelopeOf(
        BOX,
        [
          [300, 300],
          [400, 300],
          [340, 320],
          [300, 360],
        ],
        'auto',
      ),
    ),
  ];
  for (const m of maps) {
    it(`${m.kind}: the shader's arithmetic on the packed floats is the map within float32 precision`, () => {
      const packed = packMap(m);
      expect(packed.affine).toBeNull();
      expect(packed.data.length).toBe(12);
      for (const p of [
        [300, 300],
        [350, 330],
        [399, 359],
        [320, 345],
      ] as Pt[]) {
        const a = mapPoint(m, p);
        const b = unpackApply(packed.kind, packed.data, p);
        expect(Math.abs(a[0] - b[0])).toBeLessThan(1e-3);
        expect(Math.abs(a[1] - b[1])).toBeLessThan(1e-3);
      }
    });
  }

  it('an affine map is composed into the matrix (no vertex-stage work)', () => {
    const packed = packMap({ kind: 'affine', m: [1, 0, 0, 1, 5, 5] });
    expect(packed.kind).toBe(0);
    expect(packed.affine).toEqual([1, 0, 0, 1, 5, 5]);
  });
});
