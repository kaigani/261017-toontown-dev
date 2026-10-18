import { describe, expect, it } from 'vitest';

import { createHostApi, createMockTransport } from '../ipc';
import type { HostTransport } from '../ipc';
import { checksum, formatMs, measureBurst, pingOnce, percentile, summarize } from './latency';

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

describe('ping echo verification', () => {
  function tampering(edit: (payload: string | null) => string | null): HostTransport {
    const mock = createMockTransport();
    return {
      ...mock,
      invoke: async (command, args) => {
        const value = (await mock.invoke(command, args)) as { payload: string | null };
        return command === 'ping' ? { ...value, payload: edit(value.payload) } : value;
      },
    };
  }

  it('accepts a faithful echo, with and without payload', async () => {
    const host = createHostApi(createMockTransport());
    await expect(pingOnce(host, 1, 'abc')).resolves.toBeGreaterThanOrEqual(0);
    await expect(pingOnce(host, 2)).resolves.toBeGreaterThanOrEqual(0);
  });

  it('rejects dropped, truncated and same-length corrupted payloads', async () => {
    await expect(pingOnce(createHostApi(tampering(() => null)), 1, 'abc')).rejects.toThrow(/echo mismatch/);
    await expect(pingOnce(createHostApi(tampering((p) => p?.slice(1) ?? null)), 1, 'abc')).rejects.toThrow();
    await expect(pingOnce(createHostApi(tampering(() => 'abd')), 1, 'abc')).rejects.toThrow();
    await expect(pingOnce(createHostApi(tampering(() => 'x')), 1)).rejects.toThrow();
  });

  it('checksum differs for permutations', () => {
    expect(checksum('ab')).not.toBe(checksum('ba'));
  });
});
