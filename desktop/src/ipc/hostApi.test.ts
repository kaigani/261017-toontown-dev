import { describe, expect, it } from 'vitest';

import { createHostApi, createMockTransport, HostError, MENU_COMMAND_EVENT, toHostError } from '.';
import type { HostTransport } from './transport';

describe('host API over the mock transport', () => {
  it('pings and echoes seq and payload', async () => {
    const api = createHostApi(createMockTransport());
    const r = await api.ping({ seq: 7, payload: 'abc' });
    expect(r.seq).toBe(7);
    expect(r.payload).toBe('abc');
  });

  it('maps host error envelopes to HostError with the stable code', async () => {
    const api = createHostApi(createMockTransport());
    const err = await api.ping({ seq: 1, payload: 'x'.repeat((1 << 20) + 1) }).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(HostError);
    expect((err as HostError).code).toBe('limit.exceeded');
  });

  it('reports engine info', async () => {
    const info = await createHostApi(createMockTransport()).appInfo();
    expect(info.name).toBe('Toontown');
    expect(info.formatVersion).toMatch(/^\d+\.\d+$/);
  });

  it('rejects malformed responses as ipc.invalidResponse', async () => {
    const broken: HostTransport = {
      kind: 'mock',
      invoke: () => Promise.resolve({ nope: true }),
      listen: () => Promise.resolve(() => undefined),
    };
    const err = await createHostApi(broken)
      .ping({ seq: 1 })
      .catch((e: unknown) => e);
    expect((err as HostError).code).toBe('ipc.invalidResponse');
  });

  it('delivers valid menu commands, drops malformed ones, and unsubscribes', async () => {
    const transport = createMockTransport();
    const api = createHostApi(transport);
    const seen: string[] = [];
    const unlisten = await api.onMenuCommand((e) => seen.push(e.command));
    transport.emit(MENU_COMMAND_EVENT, { command: 'bad.event' });
    transport.emit(MENU_COMMAND_EVENT, {
      command: 'workspace.reset',
      params: {},
      handler: 'frontend',
      milestone: null,
    });
    unlisten();
    transport.emit(MENU_COMMAND_EVENT, { command: 'x.y', params: {}, handler: 'frontend', milestone: null });
    expect(seen).toEqual(['workspace.reset']);
    expect(transport.listenerCount(MENU_COMMAND_EVENT)).toBe(0);
  });
});

describe('toHostError', () => {
  it('wraps unknown errors as internal', () => {
    expect(toHostError(new Error('boom')).code).toBe('internal');
    expect(toHostError('text').message).toBe('text');
    expect(toHostError({ code: 'a.b', message: 'm' }).code).toBe('a.b');
  });
});
