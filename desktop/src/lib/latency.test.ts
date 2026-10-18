import { describe, expect, it } from 'vitest';

import { createHostApi, createMockTransport } from '../ipc';
import { formatMs, measureBurst, percentile, summarize } from './latency';

describe('latency statistics', () => {
  it('nearest-rank percentiles', () => {
    const s = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    expect(percentile(s, 50)).toBe(5);
    expect(percentile(s, 95)).toBe(10);
    expect(percentile(s, 10)).toBe(1);
    expect(percentile([42], 99)).toBe(42);
  });

  it('summarizes unsorted samples', () => {
    const r = summarize([3, 1, 2]);
    expect(r).toMatchObject({ n: 3, min: 1, max: 3, p50: 2, mean: 2 });
    expect(summarize([])).toBeNull();
  });

  it('measures a burst against the host', async () => {
    const r = await measureBurst(createHostApi(createMockTransport()), 20, { warmup: 2, payloadBytes: 16 });
    expect(r?.n).toBe(20);
    expect(r?.min).toBeGreaterThanOrEqual(0);
  });

  it('formats milliseconds', () => {
    expect(formatMs(0.4567)).toBe('0.46');
    expect(formatMs(12.34)).toBe('12.3');
  });
});
