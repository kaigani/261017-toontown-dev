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

/** FNV-1a over UTF-16 code units: a cheap checksum to detect a corrupted or truncated echo. */
export function checksum(text: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h;
}

// Bursts send the same payload many times; its checksum is computed once.
let cachedPayload: string | undefined;
let cachedSum = 0;
function sentChecksum(payload: string): number {
  if (payload !== cachedPayload) {
    cachedPayload = payload;
    cachedSum = checksum(payload);
  }
  return cachedSum;
}

/**
 * One ping round trip in milliseconds, measured with the monotonic `performance.now()` clock. The echo (sequence
 * number, payload length and checksum) is verified after the clock stops, so verification is not timed; a mismatch
 * throws, which marks a benchmark report `ok: false`.
 */
export async function pingOnce(host: HostApi, seq: number, payload?: string): Promise<number> {
  const start = performance.now();
  const response = await host.ping(payload === undefined ? { seq } : { seq, payload });
  const elapsed = performance.now() - start;
  if (response.seq !== seq)
    throw new Error(`ping echo mismatch: sent seq ${seq}, got ${String(response.seq)}`);
  const echoed = response.payload ?? undefined;
  if (
    echoed?.length !== payload?.length ||
    (payload !== undefined && checksum(echoed ?? '') !== sentChecksum(payload))
  ) {
    throw new Error(
      `ping payload echo mismatch for seq ${seq}: sent ${payload?.length ?? 'none'} units, got ${echoed?.length ?? 'none'}`,
    );
  }
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
