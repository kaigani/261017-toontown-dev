import { describe, expect, it } from 'vitest';

import { createHostApi, createMockTransport } from '../ipc';
import { maybeRunIpcBench, runIpcBench } from './ipcBench';

describe('IPC benchmark', () => {
  it('measures every requested payload size', async () => {
    const report = await runIpcBench(createHostApi(createMockTransport()), {
      count: 5,
      payloadBytes: [0, 64],
    });
    expect(report.ok).toBe(true);
    expect(report.bursts.map((b) => b.payloadBytes)).toEqual([0, 64]);
    expect(report.bursts.every((b) => b.roundTripMs?.n === 5)).toBe(true);
  });

  it('records failures instead of throwing', async () => {
    const report = await runIpcBench(createHostApi(createMockTransport()), {
      count: 1,
      payloadBytes: [(1 << 20) + 1],
    });
    expect(report.ok).toBe(false);
    expect(report.error).toMatch(/limit/);
  });

  it('does nothing when the host requested no benchmark', async () => {
    expect(await maybeRunIpcBench(createHostApi(createMockTransport()))).toBe(false);
  });
});
