import { describe, expect, it, vi } from 'vitest';

import { createHostApi, createMockTransport, HostError } from '.';
import type { HostTransport } from './transport';

const goodBatch = {
  packets: [
    {
      kind: 'pose',
      hostEpochMs: 1,
      deviceMs: null,
      x: 1,
      y: 2,
      pressure: 0.5,
      tiltX: null,
      tiltY: null,
      twist: null,
      tool: 'pen',
      seq: 0,
      rawTiltX: null,
      rawTiltY: null,
      deviceId: null,
      windowNumber: null,
    },
  ],
  hostDropped: 0,
  sendFailures: 0,
};
const startInfo = {
  backend: 'octotablet-ink',
  tablets: [],
  timestampGranularityMs: 1,
  deviceClock: 'test clock',
  window: null,
};

function streaming(messages: unknown[], result: unknown = startInfo): HostTransport {
  return {
    ...createMockTransport(),
    invokeWithChannel: (_command, _args, _channelArg, onMessage) => {
      for (const m of messages) onMessage(m);
      return Promise.resolve(result);
    },
  };
}

describe('spike API (C0.2)', () => {
  it('is disabled in a normal launch (mock host)', async () => {
    const host = createHostApi(createMockTransport());
    expect(await host.spike.mode()).toBeNull();
    await expect(host.spike.saveLog({})).rejects.toBeInstanceOf(HostError);
  });

  it('certifies every streamed batch and counts the invalid ones', async () => {
    const onBatch = vi.fn();
    const onInvalid = vi.fn();
    const bad = [
      { packets: [{ ...goodBatch.packets[0], pressure: 'high' }] },
      { packets: [{ ...goodBatch.packets[0], kind: 'teleport' }] },
      { packets: [{ ...goodBatch.packets[0], extra: 1 }] },
      { packets: [{ ...goodBatch.packets[0], x: Number.NaN }] },
      [],
    ];
    const host = createHostApi(streaming([goodBatch, ...bad]));
    const info = await host.spike.nativeStart(onBatch, onInvalid);
    expect(info.backend).toBe('octotablet-ink');
    expect(onBatch).toHaveBeenCalledTimes(1);
    expect(onBatch.mock.calls[0]?.[0]).toEqual(goodBatch);
    expect(onInvalid).toHaveBeenCalledTimes(bad.length);
  });

  it('rejects an invalid start result and transports without channels', async () => {
    const host = createHostApi(streaming([], { ...startInfo, backend: 'magic' }));
    await expect(host.spike.nativeStart(vi.fn())).rejects.toMatchObject({ code: 'ipc.invalidResponse' });
    const noChannels: HostTransport = {
      kind: 'mock',
      invoke: () => Promise.resolve(null),
      listen: () => Promise.resolve(() => undefined),
    };
    await expect(createHostApi(noChannels).spike.nativeStart(vi.fn())).rejects.toMatchObject({
      code: 'ipc.unsupported',
    });
  });

  it('validates the spike mode shape', async () => {
    const mode = {
      name: 'tablet',
      logPath: null,
      nativeBackend: 'appkit-nsevent',
      penSimulation: false,
      auto: false,
    };
    const ok: HostTransport = { ...createMockTransport(), invoke: () => Promise.resolve(mode) };
    expect(await createHostApi(ok).spike.mode()).toEqual(mode);
    const bad: HostTransport = {
      ...createMockTransport(),
      invoke: () => Promise.resolve({ ...mode, auto: 'yes' }),
    };
    await expect(createHostApi(bad).spike.mode()).rejects.toMatchObject({ code: 'ipc.invalidResponse' });
  });
});
