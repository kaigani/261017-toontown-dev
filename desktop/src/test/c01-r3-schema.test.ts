// @vitest-environment node
/// <reference types="node" />
import { runInNewContext } from 'node:vm';
import { describe, expect, it, vi } from 'vitest';
import { createHostApi } from '../ipc/hostApi';
import { ANY, certify, guard, MAX_DEPTH, plainDataViolations } from '../ipc/schema';
import type { HostTransport } from '../ipc/transport';
import { isMenuCommandEvent, isPingResponse } from '../ipc/wireSchemas';

const any = guard<unknown>(ANY);
function sparseWithExtra(): unknown[] {
  const value: unknown[] = new Array<unknown>(1);
  Object.defineProperty(value, 'extra', { value: 'lost in JSON', enumerable: true });
  return value;
}

describe('C01-R3-03: arrays must have every index and no extra named properties', () => {
  it('plain-data preflight rejects a hole compensated by an extra property', () => {
    expect(plainDataViolations(sparseWithExtra())).not.toEqual([]);
  });
  it('certify rejects the same shape nested under an unconstrained JSON property', () => {
    expect(certify(any, { data: sparseWithExtra() })).toBeUndefined();
  });
  it('the real menu-event path drops the malformed array', async () => {
    const event = {
      command: 'workspace.reset',
      params: { data: sparseWithExtra() },
      handler: 'frontend',
      milestone: null,
    };
    const handler = vi.fn();
    const transport: HostTransport = {
      kind: 'mock',
      invoke: () => Promise.resolve(null),
      listen: (_name, callback) => {
        callback(event);
        return Promise.resolve(() => undefined);
      },
    };
    await createHostApi(transport).onMenuCommand(handler);
    expect(handler).not.toHaveBeenCalled();
  });
});

describe('C01-R3-04: structured-clone failures must fail through the host boundary', () => {
  it('certify rejects a transparent object proxy without throwing', () => {
    expect(certify(any, new Proxy({ value: 1 }, {}))).toBeUndefined();
  });
  it('a ping with an uncloneable transport value yields ipc.invalidResponse', async () => {
    const response = new Proxy({ seq: 1, payload: null, engineUptimeNs: 1, handlerNs: 1 }, {});
    const transport: HostTransport = {
      kind: 'mock',
      invoke: () => Promise.resolve(response),
      listen: () => Promise.resolve(() => undefined),
    };
    await expect(createHostApi(transport).ping({ seq: 1 })).rejects.toMatchObject({
      code: 'ipc.invalidResponse',
    });
  });
  it('an uncloneable menu event is dropped without throwing through the listener', async () => {
    const event = new Proxy(
      { command: 'workspace.reset', params: {}, handler: 'frontend', milestone: null },
      {},
    );
    const handler = vi.fn();
    const transport: HostTransport = {
      kind: 'mock',
      invoke: () => Promise.resolve(null),
      listen: (_name, callback) => {
        callback(event);
        return Promise.resolve(() => undefined);
      },
    };
    await expect(createHostApi(transport).onMenuCommand(handler)).resolves.toBeTypeOf('function');
    expect(handler).not.toHaveBeenCalled();
  });
});

describe('C01-R3-05: declared Safari 15/macOS 12 runtime floor', () => {
  const response = { seq: 1, payload: null, engineUptimeNs: 1, handlerNs: 1 };
  it('valid transport values still work when structuredClone is unavailable', () => {
    let error: unknown;
    let value: unknown;
    vi.stubGlobal('structuredClone', undefined);
    try {
      value = certify(isPingResponse, response);
    } catch (caught: unknown) {
      error = caught;
    } finally {
      vi.unstubAllGlobals();
    }
    expect(error).toBeUndefined();
    expect(value).toEqual(response);
  });
  it('valid transport values still work when Object.hasOwn is unavailable', () => {
    const descriptor = Object.getOwnPropertyDescriptor(Object, 'hasOwn');
    if (!descriptor) throw new Error('test requires a modern host baseline');
    let error: unknown;
    let value: unknown;
    Object.defineProperty(Object, 'hasOwn', { ...descriptor, value: undefined });
    try {
      value = certify(isPingResponse, response);
    } catch (caught: unknown) {
      error = caught;
    } finally {
      Object.defineProperty(Object, 'hasOwn', descriptor);
    }
    expect(error).toBeUndefined();
    expect(value).toEqual(response);
  });
});

describe('round-3 schema closure controls', () => {
  it('rejects foreign-realm objects and arrays before cloning', () => {
    for (const value of [runInNewContext('({x: 1})') as unknown, runInNewContext('[1]') as unknown]) {
      expect(plainDataViolations(value)).not.toEqual([]);
      expect(certify(any, value)).toBeUndefined();
    }
  });
  it('accepts null-prototype own __proto__ data without altering any prototype', () => {
    const value: object = Object.create(null) as object;
    Object.defineProperty(value, '__proto__', { value: { polluted: true }, enumerable: true });
    expect(plainDataViolations(value)).toEqual([]);
    const snapshot = certify(any, value);
    expect(snapshot).toBeDefined();
    expect(Object.getOwnPropertyDescriptor(snapshot, '__proto__')?.value).toEqual({ polluted: true });
    expect(Object.hasOwn(Object.prototype, 'polluted')).toBe(false);
  });
  it('does not invoke array accessors or clone non-JSON types', () => {
    const getter = vi.fn(() => 1);
    const arr: unknown[] = [0];
    Object.defineProperty(arr, '0', { get: getter, enumerable: true });
    const clone = vi.spyOn(globalThis, 'structuredClone');
    try {
      for (const value of [arr, new Date(), new Map(), 1n, new Uint8Array([1])]) {
        expect(plainDataViolations(value)).not.toEqual([]);
        expect(certify(any, value)).toBeUndefined();
      }
      expect(getter).not.toHaveBeenCalled();
      expect(clone).not.toHaveBeenCalled();
    } finally {
      clone.mockRestore();
    }
  });
  it('enforces 64 object/array levels and accepts frozen plain data', () => {
    const nested = (n: number): unknown => {
      let value: unknown = 0;
      for (let i = 0; i < n; i++) value = { child: value };
      return value;
    };
    expect(plainDataViolations(nested(MAX_DEPTH))).toEqual([]);
    expect(certify(any, nested(MAX_DEPTH))).toBeDefined();
    expect(plainDataViolations(nested(MAX_DEPTH + 1))).not.toEqual([]);
    expect(certify(any, Object.freeze({ data: Object.freeze([1, 'x']) }))).toEqual({ data: [1, 'x'] });
  });
  it('own toJSON functions reject without invocation; plain toJSON data is just data', () => {
    const toJSON = vi.fn(() => ({}));
    expect(certify(any, { toJSON })).toBeUndefined();
    expect(toJSON).not.toHaveBeenCalled();
    expect(certify(any, { toJSON: 'data' })).toEqual({ toJSON: 'data' });
  });
  it('certifies 1 MiB ping strings promptly and snapshots the value', () => {
    const response = { seq: 1, payload: 'x'.repeat(1_048_576), engineUptimeNs: 1, handlerNs: 1 };
    const times: number[] = [];
    for (let i = 0; i < 30; i++) {
      const start = performance.now();
      expect(certify(isPingResponse, response)).toEqual(response);
      times.push(performance.now() - start);
    }
    times.sort((a, b) => a - b);
    console.log('schema 1MiB milliseconds (Node, not WebView2):', { p50: times[15], max: times[29] });
    expect(times[15]).toBeLessThan(100);
    const event = { command: 'workspace.reset', params: { x: 1 }, handler: 'frontend', milestone: null };
    const snapshot = certify(isMenuCommandEvent, event);
    event.params.x = 2;
    expect(snapshot?.params['x']).toBe(1);
  });
});
