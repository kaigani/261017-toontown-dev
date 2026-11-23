import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { createHostApi, createMockTransport } from '../ipc';
import type { HostApi } from '../ipc';
import { maybeRunIpcBench, resetWatchdogForTests, runIpcBench, watchStartup } from './ipcBench';

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

  describe('start-up watchdog (M5 C5.1b)', () => {
    beforeEach(() => {
      resetWatchdogForTests();
    });
    afterEach(() => {
      vi.useRealTimers();
      resetWatchdogForTests();
    });

    function benchHost(reports: unknown[]): HostApi {
      const api = createHostApi(createMockTransport());
      return Object.assign(Object.create(api) as HostApi, {
        benchPlan: () => Promise.resolve({ count: 2, payloadBytes: [0] }),
        benchReport: (r: unknown) => {
          reports.push(r);
          return Promise.resolve();
        },
      });
    }

    function fakeWindow() {
      const listeners = new Map<string, (e: Event) => void>();
      return {
        listeners,
        addEventListener: (name: string, fn: (e: Event) => void) => listeners.set(name, fn),
      } as unknown as Window & { listeners: Map<string, (e: Event) => void> };
    }

    it('reports the uncaught start-up errors when no benchmark report arrives in time', async () => {
      vi.useFakeTimers();
      const reports: unknown[] = [];
      const target = fakeWindow();
      watchStartup(benchHost(reports), target, 1000);
      await Promise.resolve();
      target.listeners.get('error')?.({
        error: new TypeError('x is undefined'),
        message: 'm',
      } as unknown as Event);
      target.listeners.get('unhandledrejection')?.({ reason: 'refused' } as unknown as Event);
      vi.advanceTimersByTime(1000);
      expect(reports).toHaveLength(1);
      const r = reports[0] as {
        ok: boolean;
        error: string;
        startup: { progress: string; errors: { kind: string; name: string | null; message: string }[] };
      };
      expect(r.ok).toBe(false);
      expect(r.error).toBe('no benchmark report after 1 s (2 uncaught at start-up)');
      expect(r.startup.progress).toBe('not started');
      expect(r.startup.errors.map(({ kind, name, message }) => ({ kind, name, message }))).toEqual([
        { kind: 'error', name: 'TypeError', message: 'x is undefined' },
        { kind: 'rejection', name: null, message: 'refused' },
      ]);
    });

    it('stays quiet when the benchmark reported first', async () => {
      vi.useFakeTimers();
      const reports: unknown[] = [];
      const host = benchHost(reports);
      watchStartup(host, fakeWindow(), 60_000);
      vi.useRealTimers();
      expect(await maybeRunIpcBench(host)).toBe(true);
      expect(reports).toHaveLength(1);
      expect((reports[0] as { ok: boolean }).ok).toBe(true);
      vi.useFakeTimers();
      vi.advanceTimersByTime(60_000);
      expect(reports).toHaveLength(1);
    });
  });
});
