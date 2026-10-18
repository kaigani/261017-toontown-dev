// C0.3 canvas spike (spike code): summaries of one run, computed from its raw samples. The offline analyzer
// (scripts/analyse-canvas-log.py) recomputes the same quantities from the saved log and never reads these.
//
// Estimators (stated in the log and the readout):
// * frame interval = difference of consecutive requestAnimationFrame timestamps during the 10 s pan/zoom (the
//   presentation cadence the page achieved); fps = (frames − 1) · 1000 / (last − first);
// * display refresh = mean rAF interval during the 1 s idle phase before the run (nothing drawn): (last − first) /
//   (n − 1). Not the median: WebKit rounds timestamps to 1 ms, so a 60 Hz median reads 17 ms;
// * long frame = interval > 1.5 × display refresh (a missed vsync);
// * target "60 fps" is met when fps ≥ 57 (0.95 × 60); the display rate is reported next to it (a display below
//   60 Hz cannot meet it);
// * CPU frame time = JS time to update uniforms/groups and record + submit; GPU time (WebGPU only) = submission to
//   `onSubmittedWorkDone`;
// * transfer time = performance.now() around one `canvasScene` invoke (request, host encode, IPC, envelope check).
import { percentile } from '../../lib/latency';

export interface Stats {
  n: number;
  p50: number;
  p95: number;
  max: number;
  mean: number;
}

export function stats(values: readonly number[]): Stats | null {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return {
    n: sorted.length,
    p50: percentile(sorted, 50),
    p95: percentile(sorted, 95),
    max: sorted[sorted.length - 1] ?? Number.NaN,
    mean: sorted.reduce((s, v) => s + v, 0) / sorted.length,
  };
}

export interface TransferSample {
  i: number;
  frame: number;
  bytes: number;
  invokeMs: number;
  encodeNs: number;
  generateNs: number;
}

export interface FrameSample {
  /** rAF timestamp (performance clock). */
  t: number;
  cpuMs: number;
  /** WebGPU: submission to completion; null where unavailable. */
  gpuMs: number | null;
}

export interface RunSummary {
  transferMs: Stats | null;
  transferMBps: number | null;
  bytesPerTransfer: number | null;
  encodeMs: Stats | null;
  refreshMs: number | null;
  intervalMs: Stats | null;
  fps: number | null;
  longFrames: number;
  cpuMs: Stats | null;
  gpuMs: Stats | null;
  meetsTarget: boolean | null;
}

export function intervals(ts: readonly number[]): number[] {
  const out: number[] = [];
  for (let i = 1; i < ts.length; i++) out.push((ts[i] ?? 0) - (ts[i - 1] ?? 0));
  return out;
}

export function summarizeRun(run: {
  transfers: readonly TransferSample[];
  idle: readonly number[];
  frames: readonly FrameSample[];
}): RunSummary {
  const transferMs = stats(run.transfers.map((s) => s.invokeMs));
  const bytes = run.transfers.reduce((s, x) => s + x.bytes, 0);
  const ms = run.transfers.reduce((s, x) => s + x.invokeMs, 0);
  const idleFirst = run.idle[0];
  const idleLast = run.idle[run.idle.length - 1];
  const ts = run.frames.map((f) => f.t);
  const iv = intervals(ts);
  const first = ts[0];
  const last = ts[ts.length - 1];
  const fps =
    first !== undefined && last !== undefined && last > first
      ? ((ts.length - 1) * 1000) / (last - first)
      : null;
  const refresh =
    idleFirst !== undefined && idleLast !== undefined && run.idle.length > 1 && idleLast > idleFirst
      ? (idleLast - idleFirst) / (run.idle.length - 1)
      : null;
  const gpu = run.frames.map((f) => f.gpuMs).filter((v): v is number => v !== null);
  return {
    transferMs,
    transferMBps: ms > 0 ? bytes / 1e6 / (ms / 1000) : null,
    bytesPerTransfer: run.transfers.length > 0 ? bytes / run.transfers.length : null,
    encodeMs: stats(run.transfers.map((s) => s.encodeNs / 1e6)),
    refreshMs: refresh,
    intervalMs: stats(iv),
    fps,
    longFrames: refresh === null ? 0 : iv.filter((d) => d > 1.5 * refresh).length,
    cpuMs: stats(run.frames.map((f) => f.cpuMs)),
    gpuMs: stats(gpu),
    meetsTarget: fps === null ? null : fps >= 57,
  };
}
