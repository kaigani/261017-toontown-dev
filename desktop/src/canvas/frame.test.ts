// @vitest-environment node
// F3 (02 §4.2's layer order, with overlay buffering), F4 (the canvas rule set's step 1 on the product selector), the
// zoom bucket's parity with Rust's `bucket_of`, the grid of 02 §10.1, and the scene validator.
import { describe, expect, it } from 'vitest';

import { cellOf, score, selectBackend, startingBackend, type Cell, type Measurement } from './backend';
import { bucketOf } from './bucket';
import { apply, gridGeometry, invert, ViewTris, type Affine } from './geometry';
import { drawFrame, type LayerOps } from './layers';
import { parseSceneAnswer, type Scene, type SceneEntry } from './scene';
import { drawingToClip } from './webgl2';

const entry = (drawingId: string, role: SceneEntry['role']): SceneEntry => ({
  drawingId,
  rev: 1n,
  role,
  opacity: 1,
  tint: null,
  shade: false,
  outline: false,
});

function scene(grid: Scene['grid'], entries: SceneEntry[]): Scene {
  return {
    viewId: 'drawingView',
    documentRev: null,
    background: [1, 2, 3, 255],
    grid,
    entries,
    colours: {},
    paletteStamp: 0,
    drawingPivot: null,
    selection: null,
    sceneForm: '1.1',
    viewType: 'drawing',
    frame: null,
    camera: null,
    overlays: null,
    contours: null,
    toolBox: null,
  };
}

function recorder() {
  const calls: string[] = [];
  const ops: LayerOps = {
    background: () => calls.push('1 background'),
    grid: (_g, p) => calls.push(p === 'under' ? '2 grid' : '8 grid'),
    drawing: (e) => calls.push(`${e.role} ${e.drawingId}`),
    endStatic: () => calls.push('static done'),
    overlay: () => calls.push('9 overlay'),
  };
  return { calls, ops };
}

describe('F3: the layers of 02 §4.2', () => {
  // The kernel's order is shuffled on purpose: the canvas orders by role, stable within a role.
  const shuffled = [
    entry('d-over', 'staticOver'),
    entry('d-current', 'current'),
    entry('d-onion-2', 'onion'),
    entry('d-light', 'autoLight'),
    entry('d-under', 'staticUnder'),
    entry('d-onion-1', 'onion'),
  ];

  it('draws background, grid under, layers 3 to 7 by role, then the overlay', () => {
    const { calls, ops } = recorder();
    drawFrame(scene({ visible: true, type: 'normal', overlay: false }, shuffled), ops, () => true, true);
    expect(calls).toEqual([
      '1 background',
      '2 grid',
      'staticUnder d-under',
      'autoLight d-light',
      'onion d-onion-2',
      'onion d-onion-1',
      'current d-current',
      'staticOver d-over',
      'static done',
      '9 overlay',
    ]);
  });

  it('puts an overlay grid after the drawings and skips an invisible one', () => {
    const over = recorder();
    drawFrame(
      scene({ visible: true, type: 'field12', overlay: true }, [entry('d', 'current')]),
      over.ops,
      () => true,
      true,
    );
    expect(over.calls).toEqual(['1 background', 'current d', '8 grid', 'static done', '9 overlay']);
    const hidden = recorder();
    drawFrame(
      scene({ visible: false, type: 'normal', overlay: true }, [entry('d', 'current')]),
      hidden.ops,
      () => true,
      true,
    );
    expect(hidden.calls).toEqual(['1 background', 'current d', 'static done', '9 overlay']);
  });

  it('skips drawings not yet resident and reuses the static layers when only the overlay changed', () => {
    const { calls, ops } = recorder();
    const s = scene({ visible: false, type: 'normal', overlay: false }, [
      entry('a', 'onion'),
      entry('b', 'current'),
    ]);
    drawFrame(s, ops, (id) => id === 'b', true);
    drawFrame(s, ops, () => true, false);
    expect(calls).toEqual(['1 background', 'current b', 'static done', '9 overlay', '9 overlay']);
  });
});

describe('F4: the canvas rule set, step 1, on the product selector', () => {
  const met = (fps: number): Cell => ({ status: 'met', fps });
  const missed = (fps: number): Cell => ({ status: 'missed', fps });
  const all = (c: Cell) => ({ paths100k: c, groups1k: c, brush10k: c });

  it('step 0: each run maps to exactly one cell status', () => {
    expect(cellOf(undefined)).toEqual({ status: 'notMeasured' });
    expect(cellOf({ fps: null })).toEqual({ status: 'insufficient' });
    expect(cellOf({ fps: 60 })).toEqual({ status: 'met', fps: 60 });
    expect(cellOf({ fps: 56.9 })).toEqual({ status: 'missed', fps: 56.9 });
    expect(cellOf({ fps: 57 })).toEqual({ status: 'met', fps: 57 });
    expect(cellOf({ fps: null, error: 'webgpu unavailable: no adapter' })).toEqual({ status: 'unavailable' });
    expect(cellOf({ fps: null, error: 'webgl2 unavailable: no context' })).toEqual({ status: 'unavailable' });
    expect(cellOf({ fps: 30, error: 'device lost' })).toEqual({ status: 'failed' });
  });

  it("a score is the worst scene's fps, only when all three cells are met or missed", () => {
    expect(score({ paths100k: met(60), groups1k: missed(40), brush10k: met(59) })).toBe(40);
    expect(score({ paths100k: met(60), groups1k: met(60) })).toBeNull();
    expect(score({ ...all(met(60)), brush10k: { status: 'insufficient' } })).toBeNull();
  });

  const cases: [string, Measurement, 'webgpu' | 'webgl2' | null][] = [
    ['the higher score wins (WebGPU)', { webgpu: all(met(60)), webgl2: all(missed(50)) }, 'webgpu'],
    [
      'WebGPU available but slower selects WebGL2',
      { webgpu: all(missed(40)), webgl2: all(met(58)) },
      'webgl2',
    ],
    ['a tie goes to WebGPU', { webgpu: all(met(60)), webgl2: all(met(60)) }, 'webgpu'],
    [
      'an unavailable WebGPU never beats a scored WebGL2',
      { webgpu: all({ status: 'unavailable' }), webgl2: all(missed(10)) },
      'webgl2',
    ],
    [
      'a failed cell makes a back end unscorable',
      { webgpu: { ...all(met(60)), groups1k: { status: 'failed' } }, webgl2: all(missed(20)) },
      'webgl2',
    ],
    [
      'a not-measured cell makes a back end unscorable',
      { webgpu: all(met(60)), webgl2: { paths100k: met(60) } },
      'webgpu',
    ],
    ['no scorable back end: no decision', { webgpu: all({ status: 'insufficient' }), webgl2: {} }, null],
  ];
  it.each(cases)('%s', (_name, m, want) => {
    expect(selectBackend(m)).toBe(want);
  });

  it('starts with the override, else the selection, else WebGL2 (SI-136)', () => {
    expect(startingBackend('webgpu', 'webgl2')).toBe('webgpu');
    expect(startingBackend(null, 'webgpu')).toBe('webgpu');
    expect(startingBackend(null, null)).toBe('webgl2');
  });
});

describe('the zoom bucket and the mapping', () => {
  it("matches Rust's bucket_of (crates/render/tests/c42_canvas.rs)", () => {
    expect([1, 1.5, 0.25, 64, 1e9, Number.NaN, 0, -1].map(bucketOf)).toEqual([0, 1, -2, 6, 8, 0, 0, 0]);
  });

  it('drawing → clip follows the announced matrix (the canvas derives nothing)', () => {
    const d2v: Affine = [2, 0, 0, -2, 960, 540]; // zoom 2, y up, centred in a 1920 × 1080 view
    const m = drawingToClip(d2v, 1920, 1080);
    const at = (k: number) => m[k] ?? Number.NaN;
    const clip = (x: number, y: number) => [at(0) * x + at(3) * y + at(6), at(1) * x + at(4) * y + at(7)];
    // The matrix is f32 (a GPU uniform): equal within its precision.
    const near = (got: number[], want: number[]) => {
      got.forEach((g, i) => {
        expect(g).toBeCloseTo(want[i] ?? Number.NaN, 6);
      });
    };
    near(clip(0, 0), [0, 0]);
    near(clip(480, 270), [1, 1]);
    near(clip(-480, -270), [-1, -1]);
    const v2d = invert(d2v);
    expect(v2d && apply(v2d, 960, 540)).toEqual([0, 0]);
  });
});

describe('the grid of 02 §10.1', () => {
  const lines = (g: Scene['grid'], d2v: Affine) => {
    const t = new ViewTris();
    gridGeometry(g, d2v, 1000, 1000, t);
    return t.vertexCount / 6;
  };
  it('normal: minor lines every 10 du only while 4 view pixels apart', () => {
    const g = { visible: true, type: 'normal' as const, overlay: false };
    // 100 % (10 du = 10 px): minor lines over 1000 du: ~101 per axis; at 25 % (2.5 px) majors only.
    expect(lines(g, [1, 0, 0, -1, 500, 500])).toBe(202);
    expect(lines(g, [0.25, 0, 0, -0.25, 500, 500])).toBe(82);
  });
  it('field guides: nested rectangles per field, the cross-hair and graduations', () => {
    expect(lines({ visible: true, type: 'field12', overlay: false }, [0.5, 0, 0, -0.5, 500, 500])).toBe(
      12 * 4 + 2 + 12 * 4,
    );
    expect(lines({ visible: true, type: 'field16', overlay: false }, [0.5, 0, 0, -0.5, 500, 500])).toBe(
      16 * 4 + 2 + 16 * 4,
    );
  });
});

describe('the scene validator', () => {
  const good = {
    generation: 2,
    scene: {
      viewId: 'drawingView',
      background: [240, 240, 240, 255],
      grid: { visible: false, type: 'normal', overlay: false },
      entries: [
        { drawingId: 'dr_X', rev: 3, role: 'current', opacity: 1, tint: null, shade: false, outline: false },
      ],
      colours: { c1: [1, 2, 3, 4] },
      paletteStamp: 9,
    },
  };
  it('accepts a well-formed answer, a null tint and a tint', () => {
    expect(parseSceneAnswer(good).scene.entries[0]?.tint).toBeNull();
    const tinted = structuredClone(good);
    (tinted.scene.entries[0] as Record<string, unknown>)['tint'] = [255, 0, 0, 255];
    expect(parseSceneAnswer(tinted).scene.entries[0]?.tint).toEqual([255, 0, 0, 255]);
  });
  // Absent and null are distinguished (D-7e): only `tint` may be null; every field must be present.
  type J = Record<string, unknown>;
  const sc = (v: J) => v['scene'] as J;
  const en = (v: J) => {
    const e = (sc(v)['entries'] as J[])[0];
    if (!e) throw new Error('no entry');
    return e;
  };
  const fields: [string, (v: J) => void][] = [
    [
      'tint absent',
      (v) => {
        Reflect.deleteProperty(en(v), 'tint');
      },
    ],
    [
      'rev null',
      (v) => {
        en(v)['rev'] = null;
      },
    ],
    [
      'role unknown',
      (v) => {
        en(v)['role'] = 'other';
      },
    ],
    [
      'grid type unknown',
      (v) => {
        (sc(v)['grid'] as J)['type'] = 'hex';
      },
    ],
    [
      'background out of range',
      (v) => {
        sc(v)['background'] = [256, 0, 0, 0];
      },
    ],
    [
      'colours null',
      (v) => {
        sc(v)['colours'] = null;
      },
    ],
    [
      'generation absent',
      (v) => {
        Reflect.deleteProperty(v, 'generation');
      },
    ],
    [
      'paletteStamp absent',
      (v) => {
        Reflect.deleteProperty(sc(v), 'paletteStamp');
      },
    ],
  ];
  it.each(fields)('refuses %s', (_name, edit) => {
    const v = structuredClone(good) as unknown as J;
    edit(v);
    expect(() => parseSceneAnswer(v)).toThrow(TypeError);
  });
});
