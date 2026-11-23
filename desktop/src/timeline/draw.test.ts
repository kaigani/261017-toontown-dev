// The Timeline drawing (reviews/M5-design-timeline-view.md section 2): rows and frames in view only, counted on a
// recording 2-D context; the CI shape of the plan's section 6.2 (the 1,000-row scene costs at most three times the
// 10-row scene).
import { describe, expect, it } from 'vitest';

import {
  drawRuler,
  drawTracks,
  frameWindow,
  labelledFrames,
  rowsInView,
  type Ctx2D,
  type TracksModel,
} from './draw';
import type { RowKeyframe, TimelineRow } from './model';

/** A recording context: counts calls by name and keeps every rectangle drawn. */
function recorder() {
  const calls = new Map<string, number>();
  const rects: [number, number, number, number][] = [];
  const count = (name: string) => calls.set(name, (calls.get(name) ?? 0) + 1);
  const ctx: Ctx2D = {
    fillStyle: '',
    strokeStyle: '',
    lineWidth: 1,
    globalAlpha: 1,
    font: '',
    textBaseline: 'top',
    fillRect: (x, y, w, h) => {
      count('fillRect');
      rects.push([x, y, w, h]);
    },
    strokeRect: () => count('strokeRect'),
    clearRect: () => count('clearRect'),
    beginPath: () => count('beginPath'),
    moveTo: () => count('moveTo'),
    lineTo: () => count('lineTo'),
    closePath: () => count('closePath'),
    arc: () => count('arc'),
    fill: () => count('fill'),
    stroke: () => count('stroke'),
    fillText: () => count('fillText'),
    save: () => count('save'),
    restore: () => count('restore'),
  };
  const total = () => Array.from(calls.values()).reduce((a, b) => a + b, 0);
  return { ctx, calls, rects, total };
}

/** A scene of `n` rows: every other row a peg keyed every 4 frames over 9,999 frames, the rest drawings. */
function scene(n: number): TracksModel {
  const rows: TimelineRow[] = [];
  const runs = new Map();
  for (let i = 0; i < n; i++) {
    const peg = i % 2 === 0;
    const keyframes: RowKeyframe[] = peg
      ? Array.from({ length: 2500 }, (_, k) => ({
          frame: 1 + 4 * k,
          channels: ['position'],
          segmentAfter: 'interpolated',
        }))
      : [];
    rows.push({
      elementId: `e${String(i)}`,
      name: `Row ${String(i)}`,
      type: peg ? 'peg' : 'drawing',
      depth: 0,
      parentId: null,
      collapsed: false,
      visibleInScene: true,
      hiddenByParent: false,
      locked: false,
      bar: peg ? { start: 1, end: 9999, loops: 1 } : { start: 1, end: 9999, loops: 1 },
      keyframes,
      childKeyframeFrames: [],
    });
    if (!peg) runs.set(`e${String(i)}`, [{ drawing: '1', drawingId: 'd1', from: 1, to: 9999 }]);
  }
  return {
    rows,
    runs,
    display: new Map(),
    selectedRows: new Set(['e0']),
    cells: { elementIds: ['e1'], from: 3, to: 6 },
    currentElementId: 'e0',
    currentFrame: 5,
    background: '#303030FF',
  };
}

const geometry = { view: { zoomLevel: 6, firstFrame: 1 }, width: 960, height: 220, scrollTop: 0 };

describe('the Timeline drawing', () => {
  it('draws only the rows and frames in view', () => {
    expect(frameWindow(geometry)).toEqual({ first: 1, last: 61 });
    expect(rowsInView(geometry, 1000)).toEqual({ first: 0, last: 9 });
    const r = recorder();
    drawTracks(r.ctx, scene(1000), geometry);
    for (const [x, y, w, h] of r.rects) {
      expect(x + w, 'nothing right of the view').toBeLessThanOrEqual(geometry.width + 16);
      expect(y, 'nothing below the view').toBeLessThanOrEqual(geometry.height);
      expect(y + h, 'nothing above the view').toBeGreaterThanOrEqual(0);
    }
  });

  it('costs at most three times as much for 1,000 rows as for 10 (the CI shape of section 6.2)', () => {
    const small = recorder();
    drawTracks(small.ctx, scene(10), geometry);
    const large = recorder();
    drawTracks(large.ctx, scene(1000), geometry);
    expect(large.total()).toBeLessThanOrEqual(3 * small.total());
    // Scrolled to the middle of 1,000 rows and 9,999 frames: the same order of work.
    const middle = recorder();
    drawTracks(middle.ctx, scene(1000), {
      ...geometry,
      view: { zoomLevel: 6, firstFrame: 5000 },
      scrollTop: 22 * 500,
    });
    expect(middle.total()).toBeLessThanOrEqual(3 * small.total());
  });

  it('draws a keyframe glyph per channel, the selection and the playhead', () => {
    const r = recorder();
    const m = scene(2);
    drawTracks(r.ctx, m, geometry);
    // Peg e0: keys at 1, 5, ..., 61 in view (16), one circle each.
    expect(r.calls.get('arc')).toBe(16);
    // The current row's outline.
    expect(r.calls.get('strokeRect')).toBe(1);
  });

  it('labels the ruler at a step that fits the zoom and marks a free range', () => {
    expect(labelledFrames({ zoomLevel: 9, firstFrame: 1 }, 480).slice(0, 3)).toEqual([1, 2, 3]);
    expect(labelledFrames({ zoomLevel: 6, firstFrame: 1 }, 960).slice(0, 3)).toEqual([1, 5, 10]);
    expect(labelledFrames({ zoomLevel: 0, firstFrame: 1 }, 960).slice(0, 3)).toEqual([1, 50, 100]);
    const auto = recorder();
    drawRuler(
      auto.ctx,
      { currentFrame: 5, sceneLength: 48, range: { mode: 'auto', start: 1, end: 48 }, labels: new Map() },
      { zoomLevel: 6, firstFrame: 1 },
      960,
    );
    const free = recorder();
    drawRuler(
      free.ctx,
      { currentFrame: 5, sceneLength: 48, range: { mode: 'free', start: 4, end: 20 }, labels: new Map() },
      { zoomLevel: 6, firstFrame: 1 },
      960,
    );
    expect(auto.calls.get('fill') ?? 0).toBe(0);
    expect(free.calls.get('fill')).toBe(2);
  });
});
