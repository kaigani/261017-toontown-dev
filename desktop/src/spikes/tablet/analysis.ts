// C0.2 tablet spike (spike code): pure analysis of captured web pointer samples and native packets, so the numbers
// in the handoff are computed by tested code rather than read off a screen.
import type { NativePacket, PenSimReport } from '../../ipc';
import { percentile, summarize, type LatencySummary } from '../../lib/latency';

/** One pointer sample from the web path (a dispatched event or one of its coalesced events). */
export interface WebSample {
  /** Event timestamp (performance clock, ms). */
  t: number;
  /** performance.now() when the dispatched event's handler ran. */
  handlerT: number;
  kind: 'down' | 'move' | 'up';
  pointerType: string;
  /** Client (viewport) coordinates, CSS px. */
  x: number;
  y: number;
  pressure: number;
  tiltX: number;
  tiltY: number;
  twist: number;
  /** Index of the dispatched pointermove this sample came from. */
  dispatch: number;
  /** Number of coalesced events in that dispatch (1 if none or unsupported). */
  coalescedCount: number;
}

export interface FrameRecord {
  /** Animation frame time (performance clock). */
  t: number;
  /** pointermove events dispatched since the previous frame. */
  dispatched: number;
  /** Samples (coalesced events) received since the previous frame. */
  samples: number;
}

export interface PressureStats {
  distinct: number;
  min: number | null;
  max: number | null;
  /** Smallest difference between two distinct non-zero pressure values. */
  minStep: number | null;
  /** 1 / minStep: how many levels the observed step size implies. */
  levelsImplied: number | null;
}

export interface StreamSummary {
  samples: number;
  pointerTypes: Record<string, number>;
  /** Contact samples per second over the contact duration. */
  rateHz: number | null;
  /** 1000 / median interval between consecutive contact samples. */
  medianRateHz: number | null;
  medianIntervalMs: number | null;
  maxIntervalMs: number | null;
  /** Intervals longer than twice the median (suspected drops). */
  gaps: number;
  pressure: PressureStats;
  tiltPresent: boolean;
  tiltRangeX: [number, number] | null;
  tiltRangeY: [number, number] | null;
  twistPresent: boolean;
}

export interface WebSummary extends StreamSummary {
  coalescedSupported: boolean;
  coalescedPerDispatch: LatencySummary | null;
  samplesPerFrame: LatencySummary | null;
  /** performance.now() at handler minus event timestamp. */
  handlerLatencyMs: LatencySummary | null;
}

function pressureStats(values: number[]): PressureStats {
  const distinct = [...new Set(values.filter((v) => v > 0))].sort((a, b) => a - b);
  let minStep: number | null = null;
  for (let i = 1; i < distinct.length; i++) {
    const step = (distinct[i] ?? 0) - (distinct[i - 1] ?? 0);
    if (step > 0 && (minStep === null || step < minStep)) minStep = step;
  }
  return {
    distinct: distinct.length,
    min: distinct[0] ?? null,
    max: distinct[distinct.length - 1] ?? null,
    minStep,
    levelsImplied: minStep ? Math.round(1 / minStep) : null,
  };
}

function range(values: number[]): [number, number] | null {
  if (values.length === 0) return null;
  return [Math.min(...values), Math.max(...values)];
}

interface Point {
  t: number;
  pressure: number | null;
  tiltX: number | null;
  tiltY: number | null;
  twist: number | null;
  type: string;
  contact: boolean;
}

function summarizePoints(points: Point[]): StreamSummary {
  const pointerTypes: Record<string, number> = {};
  for (const p of points) pointerTypes[p.type] = (pointerTypes[p.type] ?? 0) + 1;
  const contact = points.filter((p) => p.contact).sort((a, b) => a.t - b.t);
  const intervals: number[] = [];
  for (let i = 1; i < contact.length; i++) intervals.push((contact[i]?.t ?? 0) - (contact[i - 1]?.t ?? 0));
  const sorted = [...intervals].sort((a, b) => a - b);
  const median = sorted.length > 0 ? percentile(sorted, 50) : null;
  const first = contact[0]?.t;
  const last = contact[contact.length - 1]?.t;
  const duration = first !== undefined && last !== undefined ? last - first : 0;
  const tiltX = points.map((p) => p.tiltX).filter((v): v is number => v !== null);
  const tiltY = points.map((p) => p.tiltY).filter((v): v is number => v !== null);
  return {
    samples: points.length,
    pointerTypes,
    rateHz: duration > 0 ? ((contact.length - 1) * 1000) / duration : null,
    medianRateHz: median && median > 0 ? 1000 / median : null,
    medianIntervalMs: median,
    maxIntervalMs: sorted.length > 0 ? (sorted[sorted.length - 1] ?? null) : null,
    gaps: median ? intervals.filter((d) => d > 2 * median).length : 0,
    pressure: pressureStats(points.map((p) => p.pressure ?? 0)),
    tiltPresent: [...tiltX, ...tiltY].some((v) => v !== 0),
    tiltRangeX: range(tiltX),
    tiltRangeY: range(tiltY),
    twistPresent: points.some((p) => (p.twist ?? 0) !== 0),
  };
}

export function summarizeWeb(samples: WebSample[], frames: FrameRecord[]): WebSummary {
  const base = summarizePoints(
    samples.map((s) => ({
      t: s.t,
      pressure: s.pressure,
      tiltX: s.tiltX,
      tiltY: s.tiltY,
      twist: s.twist,
      type: s.pointerType,
      contact: s.kind === 'move' && s.pressure > 0,
    })),
  );
  const byDispatch = new Map<number, WebSample>();
  for (const s of samples)
    if (s.kind === 'move' && !byDispatch.has(s.dispatch)) byDispatch.set(s.dispatch, s);
  const dispatches = [...byDispatch.values()];
  const active = frames.filter((f) => f.samples > 0);
  return {
    ...base,
    coalescedSupported: samples.some((s) => s.coalescedCount > 1),
    coalescedPerDispatch: summarize(dispatches.map((d) => d.coalescedCount)),
    samplesPerFrame: summarize(active.map((f) => f.samples)),
    handlerLatencyMs: summarize(dispatches.map((d) => d.handlerT - d.t)),
  };
}

export function summarizeNative(packets: NativePacket[]): StreamSummary {
  return summarizePoints(
    packets
      .filter((p) => p.kind === 'pose')
      .map((p) => ({
        t: p.deviceMs ?? p.hostEpochMs,
        pressure: p.pressure,
        tiltX: p.tiltX,
        tiltY: p.tiltY,
        twist: p.twist,
        type: p.tool,
        contact: (p.pressure ?? 0) > 0,
      })),
  );
}

/** How a path's samples line up with a synthetic stroke whose pressure value identifies each injected frame. */
export interface SimulationMatch {
  injected: number;
  /** Injected frames seen at least once (by their unique pressure value). */
  received: number;
  missing: number;
  /** Received samples whose tilt equals the injected tilt (±1 degree). */
  tiltMatches: number;
  /** Event (or packet) wall-clock time minus injection wall-clock time, ms. */
  latencyMs: LatencySummary | null;
}

/**
 * Matches samples to injected frames by pressure: frame i was injected with pressure k = 1 + (i mod 1024) on the
 * 0..1024 scale, so k identifies the frame within the first 1024 frames. `pick` extracts a sample's values and its
 * wall-clock time in ms.
 */
export function matchSimulation<S>(
  report: PenSimReport,
  samples: S[],
  pick: (s: S) => { pressure: number | null; tiltX: number | null; tiltY: number | null; epochMs: number },
): SimulationMatch {
  const frames = Math.min(report.frames, 1024);
  const firstSeen = new Map<number, ReturnType<typeof pick>>();
  for (const s of samples) {
    const v = pick(s);
    if (v.pressure === null || v.pressure <= 0) continue;
    const k = Math.round(v.pressure * 1024);
    if (k >= 1 && k <= frames && !firstSeen.has(k)) firstSeen.set(k, v);
  }
  const latencies: number[] = [];
  let tiltMatches = 0;
  for (const [k, v] of firstSeen) {
    const i = k - 1;
    const injectedAt = report.injectEpochMs[i];
    if (injectedAt !== undefined) latencies.push(v.epochMs - injectedAt);
    const tx = report.tiltX[i];
    const ty = report.tiltY[i];
    if (tx !== undefined && ty !== undefined && v.tiltX !== null && v.tiltY !== null) {
      if (Math.abs(v.tiltX - tx) <= 1 && Math.abs(v.tiltY - ty) <= 1) tiltMatches++;
    }
  }
  return {
    injected: frames,
    received: firstSeen.size,
    missing: frames - firstSeen.size,
    tiltMatches,
    latencyMs: summarize(latencies),
  };
}
