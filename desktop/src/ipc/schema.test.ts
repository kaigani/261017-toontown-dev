import { describe, expect, it } from 'vitest';

import { MOCK_ENGINE_INFO } from './mockTransport';
import { MAX_SAFE, U64, violations, type Schema } from './schema';
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
