import { describe, expect, it, vi } from 'vitest';

import { createMockTransport, MOCK_ENGINE_INFO } from './mockTransport';
import { createHostApi, MENU_COMMAND_EVENT } from './hostApi';
import {
  ANY,
  certify,
  copyPlainData,
  guard,
  MAX_DEPTH,
  MAX_SAFE,
  plainDataViolations,
  U64,
  violations,
  type Schema,
} from './schema';
import {
  ENGINE_INFO_FIELDS,
  isBenchPlanOrNull,
  isEngineInfo,
  isErrorInfo,
  isMenuCommandEvent,
  isPingResponse,
} from './wireSchemas';

const obj: Schema = {
  type: 'object',
  properties: { a: U64, b: { type: 'string', enum: ['x', 'y'] } },
  required: ['a'],
  additionalProperties: false,
};

describe('schema validator', () => {
  it('distinguishes objects from arrays and null', () => {
    expect(violations(obj, { a: 1 })).toEqual([]);
    expect(violations(obj, [])).not.toEqual([]);
    expect(violations(obj, null)).not.toEqual([]);
  });

  it('enforces required, additional properties, enums', () => {
    expect(violations(obj, {})).toEqual(['/a: required']);
    expect(violations(obj, { a: 1, c: 2 })).toEqual(['/c: not allowed']);
    expect(violations(obj, { a: 1, b: 'z' })).toEqual(['/b: not one of x|y']);
  });

  it('enforces integer ranges, including the JS safe-integer ceiling for u64', () => {
    expect(violations(U64, 0)).toEqual([]);
    expect(violations(U64, MAX_SAFE)).toEqual([]);
    expect(violations(U64, -1)).not.toEqual([]);
    expect(violations(U64, MAX_SAFE + 2)).not.toEqual([]);
    expect(violations(U64, 1.5)).not.toEqual([]);
    expect(violations(U64, '1')).not.toEqual([]);
  });

  it('reports nested JSON pointers', () => {
    expect(violations({ type: 'array', items: obj }, [{ a: 1 }, { a: -1 }])).toEqual(['/1/a: below 0']);
  });
});

describe('wire schemas', () => {
  it('engine info requires every declared field', () => {
    expect(isEngineInfo(MOCK_ENGINE_INFO)).toBe(true);
    expect([...ENGINE_INFO_FIELDS].sort()).toEqual(Object.keys(MOCK_ENGINE_INFO).sort());
    for (const field of ENGINE_INFO_FIELDS) {
      const copy = Object.fromEntries(Object.entries(MOCK_ENGINE_INFO).filter(([k]) => k !== field));
      expect(isEngineInfo(copy), field).toBe(false);
    }
    expect(isEngineInfo({ ...MOCK_ENGINE_INFO, runMode: 'telepathy' })).toBe(false);
    expect(isEngineInfo({ ...MOCK_ENGINE_INFO, formatVersion: '1' })).toBe(false);
  });

  it('accepts the shape the Rust host serializes', () => {
    // Taken from `toontown --ipc-bench` / engine unit tests (crates/engine/src/ping.rs).
    expect(isPingResponse({ seq: 42, payload: 'abc', engineUptimeNs: 123, handlerNs: 100 })).toBe(true);
    expect(isPingResponse({ seq: 42, payload: null, engineUptimeNs: 0, handlerNs: 0 })).toBe(true);
    expect(isPingResponse({ seq: 42, payload: 1, engineUptimeNs: 0, handlerNs: 0 })).toBe(false);
    expect(isErrorInfo({ code: 'limit.exceeded', message: 'm', details: { limit: 1 } })).toBe(true);
    expect(isErrorInfo({ code: 1, message: 'm' })).toBe(false);
  });

  it('menu events need a command id and object params', () => {
    const e = {
      command: 'workspace.reset',
      params: { name: 'Default' },
      handler: 'frontend',
      milestone: null,
    };
    expect(isMenuCommandEvent(e)).toBe(true);
    expect(isMenuCommandEvent({ ...e, params: [] })).toBe(false);
    expect(isMenuCommandEvent({ ...e, command: 'Quit' })).toBe(false);
    expect(isMenuCommandEvent({ ...e, handler: 'host' })).toBe(false);
  });

  it('bench plan', () => {
    expect(isBenchPlanOrNull(null)).toBe(true);
    expect(isBenchPlanOrNull({ count: 5, payloadBytes: [0, 1024] })).toBe(true);
    expect(isBenchPlanOrNull({ count: 0, payloadBytes: [] })).toBe(false);
    expect(isBenchPlanOrNull({ count: 5, payloadBytes: [-1] })).toBe(false);
  });
});

describe('own-property semantics (C01-R2-02)', () => {
  const inner: Schema = {
    type: 'object',
    properties: { seq: U64 },
    required: ['seq'],
    additionalProperties: false,
  };
  const outer: Schema = {
    type: 'object',
    properties: { child: inner, list: { type: 'array', items: inner } },
    required: ['child'],
    additionalProperties: false,
  };

  it('rejects prototype-named keys at every nesting level', () => {
    for (const key of ['__proto__', 'constructor', 'prototype', 'toString', 'hasOwnProperty', 'valueOf']) {
      const top = JSON.parse(`{"child":{"seq":1},"${key}":{}}`) as unknown;
      const nested = JSON.parse(`{"child":{"seq":1,"${key}":1}}`) as unknown;
      const inArray = JSON.parse(`{"child":{"seq":1},"list":[{"seq":2,"${key}":null}]}`) as unknown;
      expect(violations(outer, top), key).not.toEqual([]);
      expect(violations(outer, nested), key).not.toEqual([]);
      expect(violations(outer, inArray), key).not.toEqual([]);
    }
  });

  it('a schema that declares a prototype-named property still validates it as an own key', () => {
    const s: Schema = {
      type: 'object',
      properties: { constructor: U64 },
      required: ['constructor'],
      additionalProperties: false,
    };
    expect(violations(s, JSON.parse('{"constructor":3}'))).toEqual([]);
    expect(violations(s, JSON.parse('{"constructor":"x"}'))).not.toEqual([]);
    expect(violations(s, {})).toEqual(['/constructor: required']);
  });

  it('rejects Object.create(...) values with inherited fields, at the top and nested', () => {
    const inherited: unknown = Object.create({ seq: 1 });
    expect(violations(inner, inherited)).not.toEqual([]);
    expect(plainDataViolations(inherited)).not.toEqual([]);
    const withOwn: unknown = Object.assign(Object.create({ extra: 1 }), { seq: 1 });
    expect(plainDataViolations({ child: withOwn })).not.toEqual([]);
    expect(
      certify((v): v is unknown => violations(outer, v).length === 0, { child: withOwn }),
    ).toBeUndefined();
    // A null-prototype object holding only own data is plain.
    const bare = Object.assign(Object.create(null) as object, { seq: 1 });
    expect(plainDataViolations(bare)).toEqual([]);
    expect(violations(inner, bare)).toEqual([]);
  });

  it('rejects Object.create(...) transports through the real host API', async () => {
    const proto = { seq: 1, payload: null, engineUptimeNs: 0, handlerNs: 0 };
    const host = createHostApi({
      ...createMockTransport(),
      invoke: () => Promise.resolve(Object.create(proto)),
    });
    await expect(host.ping({ seq: 1 })).rejects.toMatchObject({ code: 'ipc.invalidResponse' });
    const transport = createMockTransport();
    const seen: unknown[] = [];
    await createHostApi(transport).onMenuCommand((e) => seen.push(e));
    transport.emit(
      MENU_COMMAND_EVENT,
      Object.create({ command: 'workspace.reset', params: {}, handler: 'frontend', milestone: null }),
    );
    expect(seen).toEqual([]);
  });

  it('rejects class instances, accessors, functions, symbols, holes, cycles and excessive depth', () => {
    class Ping {
      seq = 1;
    }
    expect(plainDataViolations(new Ping())).not.toEqual([]);
    expect(
      plainDataViolations(Object.defineProperty({}, 'seq', { get: () => 1, enumerable: true })),
    ).not.toEqual([]);
    expect(
      plainDataViolations(Object.defineProperty({}, 'seq', { value: 1, enumerable: false })),
    ).not.toEqual([]);
    expect(plainDataViolations({ f: () => 1 })).not.toEqual([]);
    expect(plainDataViolations({ [Symbol('s')]: 1 })).not.toEqual([]);
    const sparse: unknown[] = [1];
    sparse[2] = 3;
    expect(plainDataViolations(sparse)).not.toEqual([]);
    expect(plainDataViolations({ toJSON: () => ({ seq: 1 }) })).not.toEqual([]);
    const cyclic: Record<string, unknown> = {};
    cyclic['self'] = cyclic;
    expect(plainDataViolations(cyclic)).not.toEqual([]);
    let deep: unknown = 1;
    for (let i = 0; i <= MAX_DEPTH; i++) deep = [deep];
    expect(plainDataViolations(deep)).not.toEqual([]);
    expect(plainDataViolations({ a: [1, 'x', null, true, { b: {} }] })).toEqual([]);
  });

  it('returns a snapshot: changing the transport object afterwards does not change validated data', async () => {
    const original = { seq: 1, payload: 'abc', engineUptimeNs: 0, handlerNs: 0 };
    const host = createHostApi({ ...createMockTransport(), invoke: () => Promise.resolve(original) });
    const r = await host.ping({ seq: 1 });
    original.payload = 'tampered';
    expect(r.payload).toBe('abc');
    expect(r).not.toBe(original);
  });
});

describe('platform floor: Safari 15.0 / macOS 12.0 (ADR-0005, C01-R3-05)', () => {
  it('certifies host responses with BOTH Object.hasOwn and structuredClone removed', async () => {
    const hasOwnDescriptor = Object.getOwnPropertyDescriptor(Object, 'hasOwn');
    if (!hasOwnDescriptor) throw new Error('test host lacks Object.hasOwn');
    const response = { seq: 3, payload: 'abc', engineUptimeNs: 1, handlerNs: 1 };
    const host = createHostApi({ ...createMockTransport(), invoke: () => Promise.resolve(response) });
    let pinged: unknown;
    let nested: unknown;
    let error: unknown;
    // Only product code runs while the APIs are missing (Vitest's own expect() uses Object.hasOwn).
    Object.defineProperty(Object, 'hasOwn', { ...hasOwnDescriptor, value: undefined });
    vi.stubGlobal('structuredClone', undefined);
    try {
      pinged = await host.ping({ seq: 3 });
      nested = certify(guard<unknown>(ANY), JSON.parse('{"__proto__":{"a":[1,{"b":null}]}}'));
    } catch (caught: unknown) {
      error = caught;
    } finally {
      vi.unstubAllGlobals();
      Object.defineProperty(Object, 'hasOwn', hasOwnDescriptor);
    }
    expect(error).toBeUndefined();
    expect(pinged).toEqual(response);
    expect(nested).toBeDefined();
  });
});

describe('exact-index arrays and own copies (C01-R3-03)', () => {
  it('rejects holes compensated by named properties, and out-of-order or extra keys', () => {
    const holeAndName: unknown[] = new Array<unknown>(2);
    holeAndName[0] = 1;
    Object.defineProperty(holeAndName, 'x', { value: 1, enumerable: true });
    expect(plainDataViolations(holeAndName)).not.toEqual([]);
    const named = Object.assign([1, 2], { extra: 3 });
    expect(plainDataViolations(named)).not.toEqual([]);
    expect(plainDataViolations([1, 2, 3])).toEqual([]);
    expect(plainDataViolations([])).toEqual([]);
  });

  it('copyPlainData keeps prototypes, own __proto__ keys and values, and shares nothing', () => {
    const source = JSON.parse('{"__proto__":{"x":1},"list":[1,{"y":[2]}]}') as Record<string, unknown>;
    const copy = copyPlainData(source) as Record<string, unknown>;
    expect(Object.getPrototypeOf(copy)).toBe(Object.prototype);
    expect(Object.getOwnPropertyDescriptor(copy, '__proto__')?.value).toEqual({ x: 1 });
    expect(copy['list']).toEqual([1, { y: [2] }]);
    expect(copy['list']).not.toBe(source['list']);
    const bare = Object.assign(Object.create(null) as object, { a: 1 });
    expect(Object.getPrototypeOf(copyPlainData(bare))).toBeNull();
  });

  it('certify never throws, even for a Proxy whose traps throw', () => {
    const hostile = new Proxy(
      {},
      {
        ownKeys() {
          throw new Error('trap');
        },
      },
    );
    expect(() => certify(guard<unknown>(ANY), hostile)).not.toThrow();
    expect(certify(guard<unknown>(ANY), hostile)).toBeUndefined();
  });
});
