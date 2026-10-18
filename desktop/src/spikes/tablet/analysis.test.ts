import { describe, expect, it } from 'vitest';

import type { NativePacket, PenSimReport } from '../../ipc';
import { matchSimulation, summarizeNative, summarizeWeb, type FrameRecord, type WebSample } from './analysis';

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
    expect(s.handlerLatencyMs?.p50).toBe(2);
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
    expect(s.coalescedSupported).toBe(false);
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
