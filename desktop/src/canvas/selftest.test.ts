// @vitest-environment node
// The product's back-end measurement (ADR-0007 rule set step 1; SI-136): the rule set's scenes, and each cell's
// status from a renderer's frame times (a fake renderer and clock: no GPU in tests).
import { describe, expect, it } from 'vitest';

import { selectBackend } from './backend';
import type { DecodedDrawing } from './format';
import { BackendUnavailable, type Renderer } from './renderer';
import { CELL_MS, measureBackends, measureCell, measureScene } from './selftest';

function fakeRenderer(
  clock: { t: number },
  frameMs: number | (() => number),
  uploadError?: string,
): Renderer & { held: Set<string> } {
  const held = new Set<string>();
  return {
    kind: 'webgl2',
    held,
    upload(id: string) {
      if (uploadError) throw new Error(uploadError);
      held.add(id);
    },
    drop(id: string) {
      held.delete(id);
    },
    setColours: () => undefined,
    setAntialias: () => undefined,
    setSmoothOverlays: () => undefined,
    frame() {
      clock.t += typeof frameMs === 'function' ? frameMs() : frameMs;
    },
    finish: () => Promise.resolve(),
    stats: () => ({
      kind: 'webgl2',
      residentBytes: 0,
      residentDrawings: held.size,
      drawn: 0,
      staticRedraws: 0,
      staticReuses: 0,
      lastError: null,
    }),
    destroy: () => undefined,
  };
}

const env = (clock: { t: number }) => ({ now: () => clock.t, yieldFrame: () => Promise.resolve() });

describe('the back-end measurement', () => {
  it('generates the rule set scenes: 100,000 paths, 1,000 groups, 10,000 brush outlines', () => {
    const paths = measureScene('paths100k');
    const groups = measureScene('groups1k');
    const brush = measureScene('brush10k');
    const vertices = (d: DecodedDrawing) =>
      d.records.reduce((n, r) => n + (r.kind === 'bitmap' ? 0 : r.vertexCount), 0);
    const one = (m: Map<string, DecodedDrawing>): DecodedDrawing => {
      const d = [...m.values()][0];
      if (!d) throw new Error('empty scene');
      return d;
    };
    expect(vertices(one(paths.drawings))).toBe(400_000);
    expect(groups.drawings.size).toBe(1000);
    expect(groups.scene.entries.length).toBe(1000);
    expect(vertices(one(brush.drawings))).toBe(10_000 * 18);
    // Deterministic: the same scene on every machine.
    expect([...measureScene('paths100k').drawings.values()][0]?.records).toEqual(
      [...paths.drawings.values()][0]?.records,
    );
  });

  it('a cell is met at 57 fps or more, missed below, and its drawings are dropped afterwards', async () => {
    const fast = { t: 0 };
    const r = fakeRenderer(fast, 5);
    const met = await measureCell(r, 'groups1k', env(fast));
    expect(met.status).toBe('met');
    expect(r.held.size).toBe(0);
    const slow = { t: 0 };
    const missed = await measureCell(fakeRenderer(slow, 25), 'brush10k', env(slow));
    expect(missed).toEqual({ status: 'missed', fps: 40 });
    expect(slow.t).toBeGreaterThanOrEqual(CELL_MS);
  });

  it('a frame over the abort bound ends the cell at once, missed at that rate', async () => {
    const clock = { t: 0 };
    const cell = await measureCell(fakeRenderer(clock, 1600), 'paths100k', env(clock));
    expect(cell).toEqual({ status: 'missed', fps: 1000 / 1600 });
    // The untimed warm-up frame, then one timed frame.
    expect(clock.t).toBe(3200);
  });

  it('the first frame after the upload is a warm-up: its one-time cost is not the scene rate', async () => {
    const clock = { t: 0 };
    let n = 0;
    const cell = await measureCell(
      fakeRenderer(clock, () => (n++ === 0 ? 900 : 10)),
      'groups1k',
      env(clock),
    );
    expect(cell).toEqual({ status: 'met', fps: 100 });
    expect(clock.t).toBeGreaterThanOrEqual(900 + CELL_MS);
  });

  it('a failing back end gives failed or unavailable cells, never selected over a measured one', async () => {
    const clock = { t: 0 };
    const m = await measureBackends((kind) => {
      if (kind === 'webgpu') return Promise.reject(new BackendUnavailable('webgpu', 'no adapter'));
      return Promise.resolve(fakeRenderer(clock, 10));
    }, env(clock));
    expect(m.webgpu.paths100k).toEqual({ status: 'unavailable' });
    expect(m.webgl2.groups1k?.status).toBe('met');
    expect(selectBackend(m)).toBe('webgl2');
    const broken = await measureCell(fakeRenderer({ t: 0 }, 10, 'device lost'), 'brush10k', env({ t: 0 }));
    expect(broken).toEqual({ status: 'failed' });
  });
});
