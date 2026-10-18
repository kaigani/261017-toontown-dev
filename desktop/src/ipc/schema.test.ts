import { describe, expect, it } from 'vitest';

import { createMockTransport, MOCK_ENGINE_INFO } from './mockTransport';
import { createHostApi, MENU_COMMAND_EVENT } from './hostApi';
import { certify, MAX_DEPTH, MAX_SAFE, plainDataViolations, U64, violations, type Schema } from './schema';
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
