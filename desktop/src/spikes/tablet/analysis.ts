// C0.2 tablet spike (spike code): pure analysis of captured web pointer samples and native packets, so the numbers
// in the handoff are computed by tested code rather than read off a screen. `scripts/analyse-tablet-log.py`
// recomputes the same quantities from a saved log's raw samples (it never trusts the summaries stored in the log).
//
// Rules (C0.2 review round 1):
// * Rates and gaps are computed per contact episode (stroke), never across a lift: web episodes run from
//   pointerdown to pointerup/pointercancel of one pointer; native episodes from `down` (or the first pose with
//   pressure) to `up`, proximity-out, or a pose whose pressure returns to zero.
// * Loss is only reported against a reference (injected frames; host sequence numbers). Without one it is
//   "unknown, gap heuristic only": a long interval may be a slow hand, not a dropped sample.
// * Native times use one clock domain per summary (device clock if every contact packet has it, else host wall time).
// * Percentiles are nearest-rank (`lib/latency.ts`), the same definition as the analyzer script.
import type { NativePacket, PenSimReport } from '../../ipc';
import { percentile, summarize, type LatencySummary } from '../../lib/latency';

/** One pointer sample from the web path (a dispatched event or one of its coalesced events). */
export interface WebSample {
  /** Event timestamp (performance clock, ms). */
  t: number;
  /** performance.now() when the dispatched event's handler ran. */
  handlerT: number;
  kind: 'down' | 'move' | 'up';
  /** The `up` was a pointercancel. */
  cancelled?: boolean;
  pointerType: string;
  /** PointerEvent.pointerId (absent in logs before version 2). */
  pointerId?: number;
  /** Client (viewport) coordinates, CSS px. */
  x: number;
  y: number;
  pressure: number;
  tiltX: number;
  tiltY: number;
  twist: number;
  /** Index of the dispatched event this sample came from. */
  dispatch: number;
  /** Number of coalesced events in that dispatch (1 if none or unsupported). */
  coalescedCount: number;
  /** Position of this sample within its dispatch's coalesced list (0 = oldest). */
  coalescedIndex?: number;
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

/** Loss against a reference, or an explicit statement that there is none. */
export type LossEstimate =
  | { basis: 'reference'; reference: string; expected: number; received: number; missing: number }
  | { basis: 'unknown'; note: 'unknown, gap heuristic only' };

export const NO_REFERENCE: LossEstimate = { basis: 'unknown', note: 'unknown, gap heuristic only' };

/** One contact episode. */
export interface StrokeSummary {
  index: number;
  /** Pointer id (web) or tool (native). */
  pointer: string;
  /** Contact samples in the episode. */
  samples: number;
  durationMs: number;
  /** (samples - 1) / duration. */
  rateHz: number | null;
  medianIntervalMs: number | null;
  maxIntervalMs: number | null;
  /** Intervals longer than twice this stroke's median: a heuristic, not a loss count. */
  gapsOver2xMedian: number;
  pressure: PressureStats;
  tiltPresent: boolean;
  /** The episode began before the captured window (no down seen) or had not ended when the capture stopped. */
  truncated: boolean;
}

export interface StreamSummary {
  /** All samples or packets considered (contact and hover). */
  samples: number;
  pointerTypes: Record<string, number>;
  /** Samples inside contact episodes. */
  contactSamples: number;
  strokes: StrokeSummary[];
  /** Over all strokes: sum of (samples - 1) / sum of durations (lift time excluded). */
  rateHz: number | null;
  /** 1000 / median of the within-stroke intervals. */
  medianRateHz: number | null;
  medianIntervalMs: number | null;
  maxIntervalMs: number | null;
  /** Sum over strokes of intervals longer than twice the stroke median (heuristic only). */
  gaps: number;
  /** Loss against a reference; `unknown` when there is none. */
  loss: LossEstimate;
  pressure: PressureStats;
  tiltPresent: boolean;
  tiltRangeX: [number, number] | null;
  tiltRangeY: [number, number] | null;
  twistPresent: boolean;
}

/** Event-to-handler latency under the three estimators (C02-R1-01). */
export interface HandlerLatency {
  /** Handler time minus the oldest coalesced sample of each dispatch (worst case per dispatch). */
  oldestCoalesced: LatencySummary | null;
  /** Handler time minus the newest sample of each dispatch (the dispatched event itself). */
  newestCoalesced: LatencySummary | null;
  /** Handler time minus every sample. */
  allSamples: LatencySummary | null;
}

export interface WebSummary extends StreamSummary {
  /** `PointerEvent.prototype.getCoalescedEvents` exists in this webview (API availability). */
  coalescedSupported: boolean;
  /** At least one dispatch actually carried more than one coalesced sample (observed batching). */
  coalescedObserved: boolean;
  coalescedPerDispatch: LatencySummary | null;
  samplesPerFrame: LatencySummary | null;
  handlerLatencyMs: HandlerLatency;
}

export interface NativeSummary extends StreamSummary {
  /** Clock used for intervals: `device` (deviceMs) or `host` (hostEpochMs); never mixed. */
  clock: 'device' | 'host';
}

// lib.dom declares getCoalescedEvents as always present, but older WebKit lacks it: feature-detect it.
interface MaybeCoalescing {
  getCoalescedEvents?: unknown;
}

/** Whether this runtime's PointerEvent has `getCoalescedEvents` (API availability, not observed batching). */
export function coalescedApiAvailable(): boolean {
  if (typeof PointerEvent === 'undefined') return false;
  const proto = PointerEvent.prototype as unknown as MaybeCoalescing;
  return typeof proto.getCoalescedEvents === 'function';
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
  let lo = Infinity;
  let hi = -Infinity;
  for (const v of values) {
    if (v < lo) lo = v;
    if (v > hi) hi = v;
  }
  return [lo, hi];
}

interface Point {
  t: number;
  pressure: number | null;
  tiltX: number | null;
  tiltY: number | null;
  twist: number | null;
}

interface Episode {
  pointer: string;
  points: Point[];
  truncated: boolean;
}

function intervalsOf(points: Point[]): number[] {
  const sorted = [...points].sort((a, b) => a.t - b.t);
  const out: number[] = [];
  for (let i = 1; i < sorted.length; i++) out.push((sorted[i]?.t ?? 0) - (sorted[i - 1]?.t ?? 0));
  return out;
}

function strokeSummary(e: Episode, index: number): StrokeSummary {
  const intervals = intervalsOf(e.points);
  const sorted = [...intervals].sort((a, b) => a - b);
  const median = sorted.length > 0 ? percentile(sorted, 50) : null;
  const duration = intervals.reduce((a, b) => a + b, 0);
  return {
    index,
    pointer: e.pointer,
    samples: e.points.length,
    durationMs: duration,
    rateHz: duration > 0 ? (intervals.length * 1000) / duration : null,
    medianIntervalMs: median,
    maxIntervalMs: sorted.length > 0 ? (sorted[sorted.length - 1] ?? null) : null,
    gapsOver2xMedian: median ? intervals.filter((d) => d > 2 * median).length : 0,
    pressure: pressureStats(e.points.map((p) => p.pressure ?? 0)),
    tiltPresent: e.points.some((p) => (p.tiltX ?? 0) !== 0 || (p.tiltY ?? 0) !== 0),
    truncated: e.truncated,
  };
}

function summarizeEpisodes(
  episodes: Episode[],
  all: Point[],
  pointerTypes: Record<string, number>,
  loss: LossEstimate,
): StreamSummary {
  const strokes = episodes.map(strokeSummary);
  const intervals = episodes.flatMap((e) => intervalsOf(e.points));
  const sorted = [...intervals].sort((a, b) => a - b);
  const median = sorted.length > 0 ? percentile(sorted, 50) : null;
  const duration = intervals.reduce((a, b) => a + b, 0);
  const contact = episodes.flatMap((e) => e.points);
  const tiltX = all.map((p) => p.tiltX).filter((v): v is number => v !== null);
  const tiltY = all.map((p) => p.tiltY).filter((v): v is number => v !== null);
  return {
    samples: all.length,
    pointerTypes,
    contactSamples: contact.length,
    strokes,
    rateHz: duration > 0 ? (intervals.length * 1000) / duration : null,
    medianRateHz: median && median > 0 ? 1000 / median : null,
    medianIntervalMs: median,
    maxIntervalMs: sorted.length > 0 ? (sorted[sorted.length - 1] ?? null) : null,
    gaps: strokes.reduce((n, s) => n + s.gapsOver2xMedian, 0),
    loss,
    pressure: pressureStats(contact.map((p) => p.pressure ?? 0)),
    tiltPresent: [...tiltX, ...tiltY].some((v) => v !== 0),
    tiltRangeX: range(tiltX),
    tiltRangeY: range(tiltY),
    twistPresent: all.some((p) => (p.twist ?? 0) !== 0),
  };
}

/** Web episodes: pointerdown → pointerup/pointercancel per pointer id; a move with pressure while no episode is
 * open (its down fell outside the captured window) opens a truncated one. */
export function webEpisodes(samples: WebSample[]): Episode[] {
  const open = new Map<string, Episode>();
  const done: Episode[] = [];
  for (const s of samples) {
    const key = s.pointerId === undefined ? s.pointerType : `${s.pointerType}#${String(s.pointerId)}`;
    const point: Point = { t: s.t, pressure: s.pressure, tiltX: s.tiltX, tiltY: s.tiltY, twist: s.twist };
    const current = open.get(key);
    if (s.kind === 'down') {
      if (current) done.push({ ...current, truncated: true });
      open.set(key, { pointer: key, points: [point], truncated: false });
    } else if (s.kind === 'move') {
      if (current) current.points.push(point);
      else if (s.pressure > 0) open.set(key, { pointer: key, points: [point], truncated: true });
    } else if (current) {
      done.push(current);
      open.delete(key);
    }
  }
  for (const e of open.values()) done.push({ ...e, truncated: true });
  return done;
}

export function summarizeWeb(samples: WebSample[], frames: FrameRecord[]): WebSummary {
  const pointerTypes: Record<string, number> = {};
  for (const s of samples) pointerTypes[s.pointerType] = (pointerTypes[s.pointerType] ?? 0) + 1;
  const all = samples.map((s) => ({
    t: s.t,
    pressure: s.pressure,
    tiltX: s.tiltX,
    tiltY: s.tiltY,
    twist: s.twist,
  }));
  const base = summarizeEpisodes(webEpisodes(samples), all, pointerTypes, NO_REFERENCE);

  // Group samples by dispatch (down/up events are their own single-sample dispatches).
  const byDispatch = new Map<string, WebSample[]>();
  for (const s of samples) {
    const key =
      s.kind === 'move' ? `m${String(s.dispatch)}` : `${s.kind}${String(s.t)}#${String(s.dispatch)}`;
    const list = byDispatch.get(key);
    if (list) list.push(s);
    else byDispatch.set(key, [s]);
  }
  const oldest: number[] = [];
  const newest: number[] = [];
  const every: number[] = [];
  const moveCounts: number[] = [];
  for (const [key, list] of byDispatch) {
    // All samples of a dispatch share its handler time; each latency is that time minus a sample's own timestamp.
    let first = list[0];
    let last = list[0];
    for (const s of list) {
      if (first === undefined || s.t < first.t) first = s;
      if (last === undefined || s.t > last.t) last = s;
      every.push(s.handlerT - s.t);
    }
    if (first) oldest.push(first.handlerT - first.t);
    if (last) newest.push(last.handlerT - last.t);
    if (key.startsWith('m')) moveCounts.push(list[0]?.coalescedCount ?? list.length);
  }
  const active = frames.filter((f) => f.samples > 0);
  return {
    ...base,
    coalescedSupported: coalescedApiAvailable(),
    coalescedObserved: samples.some((s) => s.coalescedCount > 1),
    coalescedPerDispatch: summarize(moveCounts),
    samplesPerFrame: summarize(active.map((f) => f.samples)),
    handlerLatencyMs: {
      oldestCoalesced: summarize(oldest),
      newestCoalesced: summarize(newest),
      allSamples: summarize(every),
    },
  };
}

/** Whether a native packet is a contact sample (carries a position with non-zero pressure). */
function isContactSample(p: NativePacket): boolean {
  return (p.kind === 'pose' || p.kind === 'down') && (p.pressure ?? 0) > 0;
}

/** Native episodes: `down` or the first pose with pressure opens; `up`, `out` or a pose whose pressure returns to
 * zero closes (that pose is not part of the stroke). */
export function nativeEpisodes(packets: NativePacket[], clock: 'device' | 'host'): Episode[] {
  const done: Episode[] = [];
  let current: Episode | null = null;
  let first = true;
  for (const p of packets) {
    const point: Point = {
      t: clock === 'device' ? (p.deviceMs ?? 0) : p.hostEpochMs,
      pressure: p.pressure,
      tiltX: p.tiltX,
      tiltY: p.tiltY,
      twist: p.twist,
    };
    if (p.kind === 'down') {
      if (current) done.push(current);
      current = { pointer: p.tool, points: isContactSample(p) ? [point] : [], truncated: false };
    } else if (p.kind === 'pose') {
      if (isContactSample(p)) {
        if (current) current.points.push(point);
        else current = { pointer: p.tool, points: [point], truncated: first };
      } else if (current) {
        done.push(current);
        current = null;
      }
    } else if (p.kind === 'up' || p.kind === 'out') {
      if (current) done.push(current);
      current = null;
    }
    first = false;
  }
  if (current) done.push({ ...current, truncated: true });
  return done.filter((e) => e.points.length > 0);
}

/** Missing host sequence numbers between the first and last packet seen (host→page loss only). */
export function sequenceLoss(packets: NativePacket[]): LossEstimate {
  const seqs = packets.map((p) => p.seq).filter((s): s is number => s !== undefined);
  if (seqs.length === 0 || seqs.length !== packets.length) return NO_REFERENCE;
  const unique = new Set(seqs);
  let lo = Infinity;
  let hi = -Infinity;
  for (const s of unique) {
    if (s < lo) lo = s;
    if (s > hi) hi = s;
  }
  const expected = hi - lo + 1;
  return {
    basis: 'reference',
    reference: 'host sequence numbers (host-to-page channel loss only)',
    expected,
    received: unique.size,
    missing: expected - unique.size,
  };
}

export function summarizeNative(packets: NativePacket[]): NativeSummary {
  const contact = packets.filter(isContactSample);
  const clock = contact.length > 0 && contact.every((p) => p.deviceMs !== null) ? 'device' : 'host';
  const poses = packets.filter((p) => p.kind === 'pose' || p.kind === 'down');
  const pointerTypes: Record<string, number> = {};
  for (const p of poses) pointerTypes[p.tool] = (pointerTypes[p.tool] ?? 0) + 1;
  const all = poses.map((p) => ({
    t: clock === 'device' ? (p.deviceMs ?? 0) : p.hostEpochMs,
    pressure: p.pressure,
    tiltX: p.tiltX,
    tiltY: p.tiltY,
    twist: p.twist,
  }));
  const base = summarizeEpisodes(nativeEpisodes(packets, clock), all, pointerTypes, sequenceLoss(packets));
  return { ...base, clock };
}

/** How a path's samples line up with a synthetic stroke whose pressure value identifies each injected frame. */
export interface SimulationMatch {
  injected: number;
  /** Injected frames seen at least once (by their unique pressure value). */
  received: number;
  /** Reference minus received. */
  missing: number;
  /** 0-based indices of the injected frames never seen. */
  missingFrames: number[];
  /** Received samples whose tilt equals the injected tilt (±1 degree). */
  tiltMatches: number;
  /** Event (or packet) wall-clock time minus the time taken before the inject call, first sighting of each frame. */
  latencyMs: LatencySummary | null;
  /** Handler wall-clock time minus injection time (web only), first sighting of each frame. */
  handlerLatencyMs: LatencySummary | null;
}

/**
 * Normalised pressures are raw / scale: 1024 for web pointer events from the synthetic pen, the device range for
 * octotablet's Ink backend (1025 for the synthetic device). Inferred from the smallest step between distinct values
 * when it lies near 1024, else 1024 (same rule as the analyzer script).
 */
export function inferPressureScale(values: readonly (number | null)[]): number {
  const distinct = [...new Set(values.filter((v): v is number => v !== null && v > 0))].sort((a, b) => a - b);
  let step = Infinity;
  for (let i = 1; i < distinct.length; i++) {
    const d = (distinct[i] ?? 0) - (distinct[i - 1] ?? 0);
    if (d > 1e-9 && d < step) step = d;
  }
  if (!Number.isFinite(step)) return 1024;
  const scale = Math.round(1 / step);
  return scale >= 1000 && scale <= 1100 ? scale : 1024;
}

/**
 * Matches samples to injected frames by pressure: frame i was injected with pressure k = 1 + (i mod 1024), so k
 * identifies the frame within the first 1024 frames; a sample's k is round(pressure * scale). `pick` extracts a
 * sample's values and its wall-clock times in ms. Every sample with pressure counts, including the pointerdown
 * (C02-R1-05).
 */
export function matchSimulation<S>(
  report: PenSimReport,
  samples: S[],
  pick: (s: S) => {
    pressure: number | null;
    tiltX: number | null;
    tiltY: number | null;
    epochMs: number;
    handlerEpochMs?: number;
  },
  scale = 1024,
): SimulationMatch {
  const frames = Math.min(report.frames, 1024);
  const firstSeen = new Map<number, ReturnType<typeof pick>>();
  for (const s of samples) {
    const v = pick(s);
    if (v.pressure === null || v.pressure <= 0) continue;
    const k = Math.round(v.pressure * scale);
    if (k >= 1 && k <= frames && !firstSeen.has(k)) firstSeen.set(k, v);
  }
  const latencies: number[] = [];
  const handler: number[] = [];
  let tiltMatches = 0;
  for (const [k, v] of firstSeen) {
    const i = k - 1;
    const injectedAt = report.injectEpochMs[i];
    if (injectedAt !== undefined) {
      latencies.push(v.epochMs - injectedAt);
      if (v.handlerEpochMs !== undefined) handler.push(v.handlerEpochMs - injectedAt);
    }
    const tx = report.tiltX[i];
    const ty = report.tiltY[i];
    if (tx !== undefined && ty !== undefined && v.tiltX !== null && v.tiltY !== null) {
      if (Math.abs(v.tiltX - tx) <= 1 && Math.abs(v.tiltY - ty) <= 1) tiltMatches++;
    }
  }
  const missingFrames: number[] = [];
  for (let k = 1; k <= frames; k++) if (!firstSeen.has(k)) missingFrames.push(k - 1);
  return {
    injected: frames,
    received: firstSeen.size,
    missing: frames - firstSeen.size,
    missingFrames,
    tiltMatches,
    latencyMs: summarize(latencies),
    handlerLatencyMs: summarize(handler),
  };
}
