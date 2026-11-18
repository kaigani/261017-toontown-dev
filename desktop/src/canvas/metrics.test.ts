// @vitest-environment node
// The canvas's measurements (plan §6, D-5): the statistics, and what each quantity measures from which event.
import { describe, expect, it } from 'vitest';

import { CanvasMetrics, stat } from './metrics';

describe('canvas metrics', () => {
  it('statistics: nearest-rank p50 and p95, max, and empty sets', () => {
    expect(stat([])).toEqual({ n: 0, p50: null, p95: null, max: null });
    const v = Array.from({ length: 100 }, (_, i) => i + 1);
    expect(stat(v)).toEqual({ n: 100, p50: 50, p95: 95, max: 100 });
    expect(stat([16.666, 16.7, 33.3])).toEqual({ n: 3, p50: 16.7, p95: 33.3, max: 33.3 });
  });

  it('frame time is the interval between frames; the work is per frame', () => {
    const m = new CanvasMetrics();
    for (let i = 0; i < 10; i++) m.frame(i * 16.7, 2);
    const s = m.snapshot();
    expect(s.frameMs.n).toBe(9);
    expect(s.frameMs.p95).toBeCloseTo(16.7, 6);
    expect(s.workMs.p50).toBe(2);
  });

  it('frames during a gesture are kept apart', () => {
    const m = new CanvasMetrics();
    m.frame(0, 1);
    m.frame(16, 1);
    m.frame(40, 1, true);
    m.frame(56, 1, true);
    expect(m.frames).toEqual([16]);
    expect(m.gestureFrames).toEqual([24, 16]);
  });

  it('the work of frames drawn during a gesture is kept apart from idle work (C43B-R1S-08)', () => {
    const m = new CanvasMetrics();
    m.frame(0, 1);
    m.frame(16, 2);
    m.frame(32, 7, true);
    m.frame(48, 9, true);
    m.frame(64, 3);
    expect(m.gestureWork).toEqual([7, 9]);
    expect(m.idleWork).toEqual([1, 2, 3]);
    const s = m.snapshot();
    expect(s.gestureWorkMs).toEqual({ n: 2, p50: 7, p95: 9, max: 9 });
    expect(s.idleWorkMs.n).toBe(3);
    expect(s.gestureWork).toEqual([7, 9]);
    // D-52: the idle work's population is exported with its summary.
    expect(s.idleWork).toEqual([1, 2, 3]);
    // Every frame's work is still in the overall figure.
    expect(s.workMs.n).toBe(5);
  });

  it('input-to-ink runs from the oldest undrawn sample to the frame that draws it', () => {
    const m = new CanvasMetrics();
    m.sample(100);
    m.sample(105); // a later sample in the same frame does not restart the clock
    m.inkDrawn(112);
    m.inkDrawn(130); // nothing pending: nothing recorded
    m.sample(140);
    m.inkDrawn(150);
    expect(m.ink).toEqual([12, 10]);
  });

  it('release-to-committed waits for a newer model version', () => {
    const m = new CanvasMetrics();
    m.released(1000, 7);
    m.drawn(7, 1010); // the same version: the commit is not drawn yet
    m.drawn(8, 1042);
    m.drawn(9, 1100); // no release pending
    expect(m.commit).toEqual([42]);
    m.released(2000, 9);
    m.answered(2030);
    m.drawn(10, 2045);
    expect(m.commitParts[1]).toEqual({ answerMs: 30, drawMs: 15 });
  });

  it('keeps bounded windows', () => {
    const m = new CanvasMetrics();
    for (let i = 0; i < 400; i++) m.frame(i, 0);
    for (let i = 0; i < 30; i++) m.load(`k${String(i)}`, 1, i);
    expect(m.frames.length).toBe(300);
    expect(m.loads.length).toBe(20);
    expect(m.loads[0]?.key).toBe('k10');
  });
});
