import type { HostApi } from '../ipc';

export interface LatencySummary {
  n: number;
  min: number;
  p50: number;
  p95: number;
  p99: number;
  max: number;
  mean: number;
}

/** Nearest-rank percentile of an ascending-sorted, non-empty array (p in (0, 100]). */
export function percentile(sorted: readonly number[], p: number): number {
  const rank = Math.max(1, Math.ceil((p / 100) * sorted.length));
  return sorted[Math.min(rank, sorted.length) - 1] ?? Number.NaN;
}

export function summarize(samples: readonly number[]): LatencySummary | null {
  if (samples.length === 0) return null;
  const sorted = [...samples].sort((a, b) => a - b);
  const sum = sorted.reduce((acc, v) => acc + v, 0);
  return {
    n: sorted.length,
    min: sorted[0] ?? Number.NaN,
    p50: percentile(sorted, 50),
    p95: percentile(sorted, 95),
    p99: percentile(sorted, 99),
    max: sorted[sorted.length - 1] ?? Number.NaN,
    mean: sum / sorted.length,
  };
}

/** One ping round trip in milliseconds, measured with the monotonic `performance.now()` clock. */
export async function pingOnce(host: HostApi, seq: number, payload?: string): Promise<number> {
  const start = performance.now();
  const response = await host.ping(payload === undefined ? { seq } : { seq, payload });
  const elapsed = performance.now() - start;
  if (response.seq !== seq) throw new Error(`ping echo mismatch: sent ${seq}, got ${response.seq}`);
  return elapsed;
}

/** `count` sequential pings (each waits for the previous), after `warmup` unrecorded ones. */
export async function measureBurst(
  host: HostApi,
  count: number,
  options: { payloadBytes?: number; warmup?: number } = {},
): Promise<LatencySummary | null> {
  const payload = options.payloadBytes ? 'x'.repeat(options.payloadBytes) : undefined;
  const warmup = options.warmup ?? 10;
  const samples: number[] = [];
  for (let i = 0; i < warmup + count; i++) {
    const rtt = await pingOnce(host, i, payload);
    if (i >= warmup) samples.push(rtt);
  }
  return summarize(samples);
}

export function formatMs(ms: number): string {
  return ms < 10 ? ms.toFixed(2) : ms.toFixed(1);
}
