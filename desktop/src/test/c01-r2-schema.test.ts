import { describe, expect, it } from 'vitest';

import { createHostApi, createMockTransport, MOCK_ENGINE_INFO } from '../ipc';
import { U64, violations, type Schema } from '../ipc/schema';
import { isBenchPlanOrNull, isEngineInfo, isMenuCommandEvent, isPingResponse } from '../ipc/wireSchemas';

describe('C01-R2-02: schema validation must use own properties', () => {
  for (const key of ['__proto__', 'constructor', 'toString', 'hasOwnProperty']) {
    it(`rejects an undeclared JSON key named ${key}`, () => {
      // JSON round-trip proves this is a wire-representable own key, not prototype mutation by a literal.
      const value: unknown = JSON.parse(JSON.stringify({ ...MOCK_ENGINE_INFO, [key]: 'unexpected' }));
      expect(isEngineInfo(value)).toBe(false);
    });
  }

  it('rejects the extra key through the real appInfo API', async () => {
    const value: unknown = JSON.parse(JSON.stringify({ ...MOCK_ENGINE_INFO, ['__proto__']: {} }));
    const host = createHostApi({ ...createMockTransport(), invoke: () => Promise.resolve(value) });
    await expect(host.appInfo()).rejects.toMatchObject({ code: 'ipc.invalidResponse' });
  });

  it('does not certify inherited, incorrectly typed required ping fields', async () => {
    const value: unknown = Object.create({
      seq: 'not a u64',
      payload: [],
      engineUptimeNs: 'not a u64',
      handlerNs: -1,
    });
    const host = createHostApi({ ...createMockTransport(), invoke: () => Promise.resolve(value) });
    await expect(host.ping({ seq: 1 })).rejects.toMatchObject({ code: 'ipc.invalidResponse' });
  });

  it('checks prototype-named extra properties at nested levels too', () => {
    const nested: Schema = {
      type: 'object',
      properties: {
        child: { type: 'object', properties: { seq: U64 }, required: ['seq'], additionalProperties: false },
      },
      required: ['child'],
      additionalProperties: false,
    };
    const value: unknown = JSON.parse('{"child":{"seq":1,"constructor":[]}}');
    expect(violations(nested, value)).not.toEqual([]);
  });
});

describe('round-2 schema attack controls', () => {
  it('rejects non-finite, unsafe, negative, fractional and string u64 values', () => {
    for (const value of [NaN, Infinity, -Infinity, 2 ** 53, -1, 0.5, '1', [], {}]) {
      expect(violations(U64, value)).not.toEqual([]);
    }
    // JSON has no distinct negative-zero integer; this is not a loss of integer magnitude.
    expect(violations(U64, -0)).toEqual([]);
    expect(violations(U64, Number.MAX_SAFE_INTEGER)).toEqual([]);
  });

  it('rejects nested arrays, ordinary extra keys and prototype-named enum values', () => {
    expect(isBenchPlanOrNull({ count: 1, payloadBytes: [[1]] })).toBe(false);
    expect(isBenchPlanOrNull({ count: 1, payloadBytes: [1], extra: 2 })).toBe(false);
    expect(isPingResponse({ seq: 1, payload: [], engineUptimeNs: 0, handlerNs: 0 })).toBe(false);
    for (const runMode of ['__proto__', 'constructor', 'toString']) {
      expect(isEngineInfo({ ...MOCK_ENGINE_INFO, runMode })).toBe(false);
    }
    expect(
      isMenuCommandEvent({ command: 'workspace.reset', params: [], handler: 'frontend', milestone: null }),
    ).toBe(false);
  });

  it('anyOf checks another branch after a failure and rejects when all fail', () => {
    const schema: Schema = { anyOf: [U64, { type: 'string', enum: ['ok'] }] };
    expect(violations(schema, 'ok')).toEqual([]);
    expect(violations(schema, -1)).not.toEqual([]);
    expect(violations(schema, {})).not.toEqual([]);
  });

  it('invalid-response errors do not include rejected payload values', async () => {
    const host = createHostApi({
      ...createMockTransport(),
      invoke: () => Promise.resolve({ ...MOCK_ENGINE_INFO, runMode: 'secret-payload-r2' }),
    });
    await expect(host.appInfo()).rejects.toMatchObject({
      code: 'ipc.invalidResponse',
      message: 'unexpected response from app_info',
    });
  });

  it('current wire patterns reject long adversarial suffixes', () => {
    expect(isEngineInfo({ ...MOCK_ENGINE_INFO, formatVersion: '1'.repeat(100_000) + '!' })).toBe(false);
    expect(
      isMenuCommandEvent({
        command: 'a.'.repeat(50_000) + '!',
        params: {},
        handler: 'frontend',
        milestone: null,
      }),
    ).toBe(false);
  });
});
