// @vitest-environment node
// The Contour Editor in the window (M4 C4.4b; design note 3 §4, §6 and §8): the hit test against the kernel's driver
// (the corpus `testdata/contour-hits.json`, written by `cargo test -p tt-executor --test c44b_contour`; `t` within
// 1e-7: the distance's minimum is flat, so float noise moves the minimiser by about the square root of epsilon), the overlay,
// the preview of every drag row against values derived here, the cost of a frame, and rule R8 of the scene answer.
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

import {
  affectedSegments,
  contourCursor,
  contourHitAt,
  contourOverlay,
  contourPreview,
  drawContourPreview,
  drawingDelta,
  type Anchor6,
  type ContourTarget,
  type SceneContours,
} from './contour';
import type { Affine } from './geometry';
import { parseSceneAnswer } from './scene';

interface Corpus {
  configs: {
    name: string;
    view: string;
    drawingToView: Affine;
    contours: SceneContours;
    presses: { p: [number, number]; target: ContourTarget | null }[];
  }[];
}

/** The value, or the test fails here. */
function must<T>(v: T | null | undefined): T {
  if (v === null || v === undefined) throw new Error('missing value');
  return v;
}

const corpus = JSON.parse(
  readFileSync(new URL('./testdata/contour-hits.json', import.meta.url), 'utf8'),
) as Corpus;

describe('contourHitAt is the kernel driver (one rule, two ports)', () => {
  it('covers both view types, zooms 0.5 to 2, rotation, and every kind of target', () => {
    const names = corpus.configs.map((c) => c.name);
    expect(names).toEqual([
      'drawing-zoom-0.5',
      'drawing-zoom-1',
      'drawing-zoom-2',
      'drawing-rotated',
      'camera',
      'camera-zoom-rotated',
    ]);
    const kinds = new Set(corpus.configs.flatMap((c) => c.presses.map((p) => p.target?.kind ?? 'none')));
    expect([...kinds].sort()).toEqual(['anchor', 'handle', 'none', 'segment']);
  });

  for (const config of corpus.configs) {
    it(`${config.name}: the same target for every press, t within 1e-7`, () => {
      const misses: string[] = [];
      for (const { p, target } of config.presses) {
        const got = contourHitAt(config.contours, config.drawingToView, p);
        const same =
          target === null
            ? got === null
            : got !== null &&
              got.kind === target.kind &&
              got.stroke === target.stroke &&
              (got.kind !== 'anchor' || target.kind !== 'anchor' || got.anchor === target.anchor) &&
              (got.kind !== 'handle' ||
                target.kind !== 'handle' ||
                (got.anchor === target.anchor && got.out === target.out)) &&
              (got.kind !== 'segment' ||
                target.kind !== 'segment' ||
                (got.segment === target.segment && Math.abs(got.t - target.t) <= 1e-7));
        if (!same)
          misses.push(
            `${JSON.stringify(p)}: kernel ${JSON.stringify(target)}, window ${JSON.stringify(got)}`,
          );
      }
      expect(misses).toEqual([]);
    });
  }
});

const I: Affine = [1, 0, 0, 1, 0, 0];
/** Drawing units -> view pixels with y down, as the Drawing view's default mapping. */
const D2V: Affine = [1, 0, 0, -1, 960, 540];

function square(selected: number[] = [], extra: Partial<SceneContours> = {}): SceneContours {
  return {
    drawingId: 'dr_1',
    locked: false,
    strokes: [
      {
        strokeId: 'st_a',
        layer: '/layers/0',
        closed: true,
        partner: false,
        anchors: [
          [0, 0, 0, 0, 0, 0],
          [100, 0, -10, 5, 20, 10],
          [100, 100, 0, 0, 0, 0],
          [0, 100, 0, 0, 0, 0],
        ],
        selected,
      },
    ],
    ...extra,
  };
}

describe('overlay (design note 3 §4)', () => {
  it('draws shown strokes, their anchors and the handles of selected anchors; partners are not drawn', () => {
    const c = square([1]);
    c.strokes.push({ ...must(c.strokes[0]), strokeId: 'st_p', partner: true, selected: [] });
    const o = contourOverlay(c, D2V);
    expect(o.shown).toEqual({ strokes: 1, anchors: 4, selected: 1, handles: 2, locked: false });
    expect(o.tris).not.toBeNull();
  });

  it('a locked drawing draws grey, its cursor is not-allowed and it has no preview', () => {
    const c = square([], { locked: true });
    const o = contourOverlay(c, I);
    expect(o.shown.locked).toBe(true);
    // Every vertex is grey or the anchors' white fill: no accent.
    const data = o.tris?.data ?? [];
    const colours = new Set<string>();
    for (let i = 0; i < data.length; i += 6)
      colours.add(
        data
          .slice(i + 2, i + 6)
          .map((x) => Math.round(x * 255))
          .join(','),
      );
    expect([...colours].sort()).toEqual(['140,140,140,255', '255,255,255,255']);
    expect(contourCursor(c, I, [0, 0], false)).toBe('not-allowed');
    expect(
      contourPreview(c, { target: { kind: 'anchor', stroke: 'st_a', anchor: 0 }, alternate: false }, [5, 5]),
    ).toEqual([]);
  });

  it('cursors: move over an anchor or a segment, crosshair over a handle, copy with temporary select over a segment', () => {
    const c = square([1]);
    expect(contourCursor(c, I, [1, 1], false)).toBe('move');
    expect(contourCursor(c, I, [50, 2], false)).toBe('move');
    expect(contourCursor(c, I, [50, 2], true)).toBe('copy');
    expect(contourCursor(c, I, [120, 10], false)).toBe('crosshair');
    expect(contourCursor(c, I, [50, 50], false)).toBe('');
  });
});

describe('preview (design note 3 §6): each drag row against the kernel command it previews', () => {
  const anchorsOf = (changes: ReturnType<typeof contourPreview>, id = 'st_a') =>
    changes.find((c) => c.strokeId === id)?.anchors ?? null;

  it('an unselected anchor drag moves that anchor only (draw.point.move)', () => {
    const ch = contourPreview(
      square([3]),
      { target: { kind: 'anchor', stroke: 'st_a', anchor: 0 }, alternate: false },
      [7, -3],
    );
    const a = must(anchorsOf(ch));
    expect(a[0]).toEqual([7, -3, 0, 0, 0, 0]);
    expect(a[3]).toEqual([0, 100, 0, 0, 0, 0]);
    expect(must(ch[0]).changed).toEqual([0]);
  });

  it('a selected anchor drag moves the whole point selection, and junction partners in the same layer', () => {
    const c = square([0, 2]);
    c.strokes.push({
      strokeId: 'st_j',
      layer: '/layers/0',
      closed: false,
      partner: true,
      anchors: [
        [0, 0, 0, 0, 0, 0],
        [-50, -50, 0, 0, 0, 0],
      ],
      selected: [],
    });
    c.strokes.push({
      strokeId: 'st_other_layer',
      layer: '/layers/1',
      closed: false,
      partner: true,
      anchors: [
        [0, 0, 0, 0, 0, 0],
        [-50, 50, 0, 0, 0, 0],
      ],
      selected: [],
    });
    const ch = contourPreview(
      c,
      { target: { kind: 'anchor', stroke: 'st_a', anchor: 0 }, alternate: false },
      [2, 3],
    );
    expect(must(must(anchorsOf(ch))[0]).slice(0, 2)).toEqual([2, 3]);
    expect(must(must(anchorsOf(ch))[2]).slice(0, 2)).toEqual([102, 103]);
    expect(must(must(anchorsOf(ch, 'st_j'))[0]).slice(0, 2)).toEqual([2, 3]);
    expect(anchorsOf(ch, 'st_other_layer')).toBeNull();
  });

  it('a handle drag sets the handle and mirrors it (symmetric); alternate breaks it', () => {
    const c = square([1]);
    const sym = contourPreview(
      c,
      { target: { kind: 'handle', stroke: 'st_a', anchor: 1, out: true }, alternate: false },
      [5, -2],
    );
    expect(must(anchorsOf(sym))[1]).toEqual([100, 0, -25, -8, 25, 8]);
    const broken = contourPreview(
      c,
      { target: { kind: 'handle', stroke: 'st_a', anchor: 1, out: false }, alternate: true },
      [5, -2],
    );
    expect(must(anchorsOf(broken))[1]).toEqual([100, 0, -5, 3, 20, 10]);
  });

  it('an alternate anchor drag adds the delta to both handles, the anchor unmoved (SI-185)', () => {
    const ch = contourPreview(
      square(),
      { target: { kind: 'anchor', stroke: 'st_a', anchor: 1 }, alternate: true },
      [4, 6],
    );
    expect(must(anchorsOf(ch))[1]).toEqual([100, 0, -6, 11, 24, 16]);
  });

  it('a segment drag adds w = d / (3t(1-t)) to the segment handles (draw.segment.move)', () => {
    const t = 0.25;
    const w = 1 / (3 * t * (1 - t));
    const ch = contourPreview(
      square(),
      { target: { kind: 'segment', stroke: 'st_a', segment: 3, t }, alternate: false },
      [0, 9],
    );
    const a = must(anchorsOf(ch));
    // Segment 3 of the closed square runs from anchor 3 to anchor 0.
    expect(a[3]).toEqual([0, 100, 0, 0, 0, 9 * w]);
    expect(a[0]).toEqual([0, 0, 0, 9 * w, 0, 0]);
    // B(t) moved by exactly d.
    expect(3 * t * (1 - t) * (9 * w)).toBeCloseTo(9, 12);
  });

  it('an alternate segment drag moves the point selection; without one it previews nothing', () => {
    const seg: ContourTarget = { kind: 'segment', stroke: 'st_a', segment: 0, t: 0.5 };
    const ch = contourPreview(square([2]), { target: seg, alternate: true }, [3, 0]);
    expect(must(must(anchorsOf(ch))[2]).slice(0, 2)).toEqual([103, 100]);
    expect(contourPreview(square([]), { target: seg, alternate: true }, [3, 0])).toEqual([]);
  });

  it('a press on nothing previews nothing (its marquee is the gesture overlay)', () => {
    expect(contourPreview(square(), { target: null, alternate: false }, [3, 3])).toEqual([]);
  });

  it('the delta is the press mapping inverse applied to the view delta', () => {
    expect(drawingDelta(D2V, [960, 540], [970, 530])).toEqual([10, 10]);
    const zoom2: Affine = [2, 0, 0, -2, 960, 540];
    expect(drawingDelta(zoom2, [960, 540], [970, 530])).toEqual([5, 5]);
  });

  it('the drawn preview reports the changed anchors in view pixels (PV2)', () => {
    const ch = contourPreview(
      square(),
      { target: { kind: 'anchor', stroke: 'st_a', anchor: 2 }, alternate: false },
      [1, 1],
    );
    const d = drawContourPreview(ch, D2V);
    expect(d.anchors).toEqual([[1061, 439]]);
    expect(d.segments).toBe(2);
  });
});

describe('cost of a preview frame (design note 3 §6, §9)', () => {
  it('a one-anchor drag on a 50,000-anchor stroke recomputes two segments', () => {
    const anchors: Anchor6[] = [];
    for (let i = 0; i < 50_000; i++) anchors.push([i, Math.sin(i / 10) * 20, 0, 0, 0, 0]);
    const c: SceneContours = {
      drawingId: 'dr_1',
      locked: false,
      strokes: [
        { strokeId: 'st_big', layer: '/layers/0', closed: false, partner: false, anchors, selected: [] },
      ],
    };
    const ch = contourPreview(
      c,
      { target: { kind: 'anchor', stroke: 'st_big', anchor: 25_000 }, alternate: false },
      [0, 5],
    );
    expect(affectedSegments(must(ch[0]))).toEqual([24_999, 25_000]);
    expect(drawContourPreview(ch, D2V).segments).toBe(2);
    // The ends of an open stroke have one segment each.
    const end = contourPreview(
      c,
      { target: { kind: 'anchor', stroke: 'st_big', anchor: 0 }, alternate: false },
      [0, 5],
    );
    expect(affectedSegments(must(end[0]))).toEqual([0]);
  });
});

describe('scene answer rule R8 (docs/canvas-format.md §11.5, §11.8)', () => {
  const base = (contours: unknown) => ({
    generation: 1,
    scene: {
      sceneForm: '1.2',
      viewId: 'drawingView',
      viewType: 'drawing',
      documentRev: 3,
      background: [255, 255, 255, 255],
      grid: { visible: false, type: 'normal', overlay: false },
      entries: [],
      colours: {},
      paletteStamp: 0,
      drawingPivot: null,
      selection: null,
      ...(contours === undefined ? {} : { contours }),
    },
  });
  const good = {
    drawingId: 'dr_1',
    locked: false,
    strokes: [
      {
        strokeId: 'st_a',
        layer: '/l/0',
        closed: true,
        partner: false,
        anchors: [[0, 0, 0, 0, 0, 0]],
        selected: [0],
      },
    ],
  };

  it('reads a valid overlay, and absent or null as none', () => {
    expect(parseSceneAnswer(base(good)).scene.contours).toEqual(good);
    expect(parseSceneAnswer(base(undefined)).scene.contours).toBeNull();
    expect(parseSceneAnswer(base(null)).scene.contours).toBeNull();
  });

  const stroke = must(good.strokes[0]);
  const corrupt: [string, unknown, string][] = [
    ['drawingId empty', { ...good, drawingId: '' }, 'contours.drawingId'],
    ['locked not a boolean', { ...good, locked: 1 }, 'contours.locked'],
    ['strokeId empty', { ...good, strokes: [{ ...stroke, strokeId: '' }] }, 'strokeId'],
    ['layer missing', { ...good, strokes: [{ ...stroke, layer: undefined }] }, 'layer'],
    ['anchors empty', { ...good, strokes: [{ ...stroke, anchors: [], selected: [] }] }, 'anchors'],
    [
      'an anchor of five numbers',
      { ...good, strokes: [{ ...stroke, anchors: [[0, 0, 0, 0, 0]] }] },
      'anchors/0',
    ],
    [
      'a non-finite anchor',
      { ...good, strokes: [{ ...stroke, anchors: [[0, 0, 0, 0, 0, null]] }] },
      'anchors/0',
    ],
    ['selected out of range', { ...good, strokes: [{ ...stroke, selected: [1] }] }, 'selected/0'],
    [
      'selected not ascending',
      {
        ...good,
        strokes: [{ ...stroke, anchors: [stroke.anchors[0], stroke.anchors[0]], selected: [1, 0] }],
      },
      'selected',
    ],
    [
      'a partner with a selection',
      { ...good, strokes: [{ ...stroke, partner: true }] },
      'partner selects nothing',
    ],
  ];
  for (const [name, value, field] of corrupt) {
    it(`refuses ${name}`, () => {
      expect(() => parseSceneAnswer(base(value))).toThrow(field);
    });
  }
});
