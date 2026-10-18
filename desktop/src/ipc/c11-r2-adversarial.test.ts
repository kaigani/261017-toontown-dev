// @vitest-environment node
/// <reference types="node" />
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { MODEL_DEFS } from './generated/model';
import { type Schema, violations } from './schema';
import { certifyWireText, coerceNumbers, parseWireJson, stringifyWireJson } from './wireJson';

describe('C11 R2 wire closure', () => {
  it('coerces both branches of a discriminated conditional with doubles', () => {
    const schema: Schema = {
      type: 'object',
      properties: { kind: { type: 'string' }, x: {} },
      required: ['kind', 'x'],
      if: { properties: { kind: { const: 'double' } } },
      then: { properties: { x: { type: 'number' } } },
      else: { properties: { x: { type: 'integer' } } },
    };
    expect(certifyWireText('{"kind":"double","x":100000000000000000000}', schema, {})).toEqual({
      kind: 'double',
      x: 1e20,
    });
    expect(certifyWireText('{"kind":"integer","x":9007199254740993}', schema, {})).toEqual({
      kind: 'integer',
      x: 9007199254740993n,
    });
  });

  it('does not let an inapplicable oneOf double branch corrupt an integer branch', () => {
    const schema: Schema = {
      oneOf: [
        {
          type: 'object',
          properties: { kind: { const: 'integer' }, x: { type: 'integer' } },
          required: ['kind', 'x'],
        },
        {
          type: 'object',
          properties: { kind: { const: 'double' }, x: { type: 'number' } },
          required: ['kind', 'x'],
        },
      ],
    };
    const text = '{"kind":"integer","x":9007199254740993}';
    expect(violations(schema, parseWireJson(text), '', {})).toEqual([]);
    expect(certifyWireText(text, schema, {})).toEqual({ kind: 'integer', x: 9007199254740993n });
  });

  it('coerces scalar doubles declared through allOf', () => {
    const schema: Schema = {
      type: 'object',
      properties: { x: { allOf: [{ type: 'number' }, { minimum: 0 }] } },
      required: ['x'],
    };
    expect(certifyWireText('{"x":100000000000000000000}', schema, {})).toEqual({ x: 1e20 });
  });

  it('keeps opaque metadata large integers unchanged under generated conditional schemas', () => {
    const shadow = JSON.parse(
      readFileSync(
        fileURLToPath(
          new URL('../../../crates/model/tests/golden/sample/elements/Intro/Shadow.json', import.meta.url),
        ),
        'utf8',
      ),
    ) as Record<string, unknown>;
    shadow['meta'] = {
      type: 'element',
      rev: 18446744073709551615n,
      name: ' e\u0301 ',
      children: [{ id: 'a', type: 'scene' }],
    };
    const out = certifyWireText<Record<string, unknown>>(
      stringifyWireJson(shadow),
      { $ref: '#/$defs/Element' },
      MODEL_DEFS,
    );
    expect(out?.['meta']).toEqual(shadow['meta']);
  });

  it('accepts all integer edges or rejects inexact outside-range literals', () => {
    for (const token of [
      '18446744073709551614',
      '18446744073709551615',
      '18446744073709551616',
      '-9223372036854775808',
      '9007199254740991',
      '9007199254740992',
      '9007199254740993',
    ]) {
      expect(stringifyWireJson(parseWireJson(token))).toBe(token);
    }
    for (const token of ['18446744073709551617', '-9223372036854775809'])
      expect(() => parseWireJson(token)).toThrow();
  });

  it('enforces wire depth immediately above and below the limit', () => {
    for (const arrays of [126, 127, 128, 129]) {
      const text = '['.repeat(arrays) + '1' + ']'.repeat(arrays);
      if (arrays < 128) expect(stringifyWireJson(parseWireJson(text))).toBe(text);
      else expect(() => parseWireJson(text)).toThrow();
    }
  });

  it('rejects fresh malformed escaped keys and non-finite nested values', () => {
    for (const text of ['{"meta":{"x":1,"\\u0078":2}}', '{"\\ud83d":1}', '["\\udfff"]', '[1e9999]', '[1e+]'])
      expect(() => parseWireJson(text)).toThrow();
    for (const x of [NaN, Infinity, -Infinity])
      expect(() => stringifyWireJson({ nested: [{ x }] })).toThrow();
  });

  it('does not mutate parsed inputs while coercing conditionally typed numbers', () => {
    const input = { x: 100000000000000000000n };
    expect(coerceNumbers(input, { type: 'object', properties: { x: { type: 'number' } } }, {})).toEqual({
      x: 1e20,
    });
    expect(input.x).toBe(100000000000000000000n);
  });

  it('runs the TypeScript middle stage of the Rust to TS to Rust exchange', () => {
    const path = process.env['TT_R2_WIRE_EXPORT'];
    if (path === undefined) return;
    const text = readFileSync(path, 'utf8');
    const output = stringifyWireJson(parseWireJson(text));
    expect(output).toBe(text);
    const dest = process.env['TT_R2_WIRE_IMPORT'];
    if (dest !== undefined) writeFileSync(dest, output);
  });
});
