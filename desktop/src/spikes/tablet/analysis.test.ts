import { describe, expect, it } from 'vitest';

import type { NativePacket, PenSimReport } from '../../ipc';
import {
  inferPressureScale,
  matchSimulation,
  summarizeNative,
  summarizeWeb,
  type FrameRecord,
  type WebSample,
} from './analysis';

const ws = (i: number, over: Partial<WebSample> = {}): WebSample => ({
  t: i * 4,
  handlerT: i * 4 + 2,
  kind: 'move',
  pointerType: 'pen',
  x: i,
  y: i,
  pressure: (1 + (i % 1024)) / 1024,
  tiltX: 10,
  tiltY: -5,
  twist: 0,
  dispatch: Math.floor(i / 4),
  coalescedCount: 4,
  ...over,
});

describe('web summary', () => {
  it('derives rate, pressure resolution, tilt and coalescing', () => {
    const samples = Array.from({ length: 1024 }, (_, i) => ws(i));
    const frames: FrameRecord[] = Array.from({ length: 256 }, (_, i) => ({
      t: i * 16,
      dispatched: 1,
      samples: 4,
    }));
    const s = summarizeWeb(samples, frames);
    expect(s.samples).toBe(1024);
    expect(s.pointerTypes).toEqual({ pen: 1024 });
    expect(s.rateHz).toBeCloseTo(250, 5);
    expect(s.medianIntervalMs).toBe(4);
    expect(s.gaps).toBe(0);
    expect(s.pressure.distinct).toBe(1024);
    expect(s.pressure.levelsImplied).toBe(1024);
    expect(s.tiltPresent).toBe(true);
    expect(s.twistPresent).toBe(false);
    expect(s.coalescedSupported).toBe(true);
    expect(s.coalescedPerDispatch?.p50).toBe(4);
    expect(s.samplesPerFrame?.p50).toBe(4);
    expect(s.coalescedObserved).toBe(true);
    expect(s.handlerLatencyMs.allSamples?.p50).toBe(2);
  });

  it('counts gaps longer than twice the median interval', () => {
    const samples = [0, 4, 8, 12, 30, 34].map((t, i) => ws(i, { t }));
    expect(summarizeWeb(samples, []).gaps).toBe(1);
  });

  it('handles an empty capture and mouse-only input', () => {
    expect(summarizeWeb([], []).rateHz).toBeNull();
    const mouse = [0, 1, 2].map((i) =>
      ws(i, { pointerType: 'mouse', pressure: 0.5, tiltX: 0, tiltY: 0, coalescedCount: 1 }),
    );
    const s = summarizeWeb(mouse, []);
    expect(s.tiltPresent).toBe(false);
    expect(s.pressure.distinct).toBe(1);
    expect(s.coalescedObserved).toBe(false);
  });
});

const packet = (i: number, over: Partial<NativePacket> = {}): NativePacket => ({
  kind: 'pose',
  hostEpochMs: 1_000_000 + i * 2,
  deviceMs: i * 2,
  x: i,
  y: i,
  pressure: (1 + i) / 1024,
  tiltX: 3,
  tiltY: 4,
  twist: 7,
  tool: 'pen',
  ...over,
});

describe('native summary', () => {
  it('uses device timestamps and only pose packets', () => {
    const packets = [packet(0, { kind: 'in' }), ...Array.from({ length: 500 }, (_, i) => packet(i))];
    const s = summarizeNative(packets);
    expect(s.samples).toBe(500);
    expect(s.medianRateHz).toBe(500);
    expect(s.twistPresent).toBe(true);
    expect(s.pressure.distinct).toBe(500);
  });
});

describe('simulation matching', () => {
  const report: PenSimReport = {
    method: 'InjectSyntheticPointerInput',
    frames: 1024,
    hz: 240,
    achievedHz: 240,
    injectEpochMs: Array.from({ length: 1024 }, (_, i) => 1_000_000 + i * 4),
    pressure: Array.from({ length: 1024 }, (_, i) => 1 + i),
    tiltX: Array.from({ length: 1024 }, () => 10),
    tiltY: Array.from({ length: 1024 }, () => -5),
    rotation: Array.from({ length: 1024 }, () => 0),
  };

  it('identifies frames by pressure and measures latency', () => {
    const samples = Array.from({ length: 1000 }, (_, i) => ({ k: i + 1, at: 1_000_000 + i * 4 + 3 }));
    const m = matchSimulation(report, samples, (s) => ({
      pressure: s.k / 1024,
      tiltX: 10,
      tiltY: -5,
      epochMs: s.at,
    }));
    expect(m.injected).toBe(1024);
    expect(m.received).toBe(1000);
    expect(m.missing).toBe(24);
    expect(m.tiltMatches).toBe(1000);
    expect(m.latencyMs?.p50).toBe(3);
  });

  it('ignores zero pressure, out-of-range values and duplicates', () => {
    const samples = [0, 0.5, 0.5, 2, 1 / 1024];
    const m = matchSimulation(report, samples, (p) => ({
      pressure: p,
      tiltX: null,
      tiltY: null,
      epochMs: 1_000_000,
    }));
    expect(m.received).toBe(2);
    expect(m.tiltMatches).toBe(0);
  });
});

describe('review round 1 (C0.2)', () => {
  const stroke = (start: number, n: number, pointerId = 1): WebSample[] => [
    ws(0, { t: start, kind: 'down', pointerId, dispatch: -1, coalescedCount: 1 }),
    ...Array.from({ length: n }, (_, i) =>
      ws(i + 1, { t: start + (i + 1) * 4, pointerId, dispatch: start + i, coalescedCount: 1 }),
    ),
    ws(0, { t: start + (n + 1) * 4, kind: 'up', pointerId, pressure: 0, dispatch: -2, coalescedCount: 1 }),
  ];

  it('segments web strokes at pointerdown/pointerup and reports each', () => {
    const s = summarizeWeb([...stroke(0, 10), ...stroke(5000, 20)], []);
    expect(s.strokes.map((k) => k.samples)).toEqual([11, 21]);
    expect(s.strokes.every((k) => k.rateHz === 250 && !k.truncated)).toBe(true);
    expect(s.gaps).toBe(0);
    expect(s.loss).toEqual({ basis: 'unknown', note: 'unknown, gap heuristic only' });
  });

  it('reports the three handler-latency estimators separately', () => {
    // One dispatch of three coalesced samples at t = 0, 4, 8 handled at 10.
    const samples = [0, 4, 8].map((t, i) =>
      ws(i, { t, handlerT: 10, dispatch: 1, coalescedCount: 3, coalescedIndex: i }),
    );
    const l = summarizeWeb(samples, []).handlerLatencyMs;
    expect(l.oldestCoalesced?.p50).toBe(10);
    expect(l.newestCoalesced?.p50).toBe(2);
    expect(l.allSamples?.p50).toBe(6);
  });

  it('splits native strokes on up, proximity-out and pressure returning to zero', () => {
    const packets = [
      packet(0, { kind: 'in' }),
      packet(1, { kind: 'down', pressure: null }),
      packet(2),
      packet(3),
      packet(4, { kind: 'up', pressure: null }),
      packet(5),
      packet(6),
      packet(7, { pressure: 0 }),
      packet(8),
      packet(9),
      packet(10, { kind: 'out' }),
    ];
    const s = summarizeNative(packets);
    expect(s.strokes.map((k) => k.samples)).toEqual([2, 2, 2]);
    expect(s.clock).toBe('device');
  });

  it('never mixes clocks: one packet without a device time switches the summary to host time', () => {
    const s = summarizeNative([packet(0), packet(1, { deviceMs: null }), packet(2)]);
    expect(s.clock).toBe('host');
    expect(s.medianIntervalMs).toBe(2);
  });

  it('counts host-to-page loss from sequence numbers, and nothing without them', () => {
    const withSeq = [0, 1, 2, 5, 6].map((seq, i) => packet(i, { seq }));
    expect(summarizeNative(withSeq).loss).toMatchObject({ basis: 'reference', expected: 7, missing: 2 });
    expect(summarizeNative([packet(0), packet(1)]).loss.basis).toBe('unknown');
  });

  it('lists the missing frame indices and handler latency of a simulation', () => {
    const report: PenSimReport = {
      method: 'test',
      frames: 4,
      hz: 240,
      achievedHz: 240,
      injectEpochMs: [0, 10, 20, 30],
      pressure: [1, 2, 3, 4],
      tiltX: [0, 0, 0, 0],
      tiltY: [0, 0, 0, 0],
      rotation: [0, 0, 0, 0],
    };
    const m = matchSimulation(report, [1, 2, 4], (k) => ({
      pressure: k / 1024,
      tiltX: 0,
      tiltY: 0,
      epochMs: (k - 1) * 10 + 1,
      handlerEpochMs: (k - 1) * 10 + 5,
    }));
    expect(m.missingFrames).toEqual([2]);
    expect(m.latencyMs?.p50).toBe(1);
    expect(m.handlerLatencyMs?.p50).toBe(5);
  });
});

describe('pressure scale', () => {
  it('infers the Ink range of the synthetic device and matches every frame with it', () => {
    const rts = Array.from({ length: 1024 }, (_, i) => (i + 1) / 1025);
    expect(inferPressureScale(rts)).toBe(1025);
    expect(inferPressureScale([0.5])).toBe(1024);
    expect(inferPressureScale(Array.from({ length: 10 }, (_, i) => i / 10))).toBe(1024);
    const report: PenSimReport = {
      method: 'test',
      frames: 1024,
      hz: 240,
      achievedHz: 240,
      injectEpochMs: Array.from({ length: 1024 }, (_, i) => i),
      pressure: Array.from({ length: 1024 }, (_, i) => i + 1),
      tiltX: Array.from({ length: 1024 }, () => 0),
      tiltY: Array.from({ length: 1024 }, () => 0),
      rotation: Array.from({ length: 1024 }, () => 0),
    };
    const pick = (p: number) => ({ pressure: p, tiltX: null, tiltY: null, epochMs: 0 });
    expect(matchSimulation(report, rts, pick).received).toBeLessThan(1024);
    expect(matchSimulation(report, rts, pick, inferPressureScale(rts)).received).toBe(1024);
  });
});
