// @vitest-environment node
/// <reference types="node" />
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { MODEL_DEFS } from './generated/model';
import { type Schema, violations } from './schema';
import { certifyWireText, parseWireJson, stringifyWireJson } from './wireJson';

describe('C11 R3 independent wire closure', () => {
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
      {
        type: 'object',
        properties: { kind: { const: 'mixed' }, x: { type: 'number' }, revision: { type: 'integer' } },
        required: ['kind', 'x', 'revision'],
      },
    ],
  };

  it('selects three numeric branches without changing unrelated integers', () => {
    for (const value of [
      { kind: 'integer', x: 9007199254740997n },
      { kind: 'double', x: 1e19 },
    ]) {
      expect(certifyWireText(stringifyWireJson(value), schema, {})).toEqual(value);
    }
  });

  it('coerces mixed doubles and wide integers inside one applicable alternative', () => {
    const expected = { kind: 'mixed', x: 1e19, revision: 9007199254740997n };
    expect(violations(schema, expected, '', {})).toEqual([]);
    expect(certifyWireText(stringifyWireJson(expected), schema, {})).toEqual(expected);
  });

  it('coerces a generated Element inside a nullable reply without rounding its revision', () => {
    const expected = JSON.parse(
      readFileSync(
        fileURLToPath(
          new URL('../../../crates/model/tests/golden/sample/elements/Intro/Shadow.json', import.meta.url),
        ),
        'utf8',
      ),
    ) as { rev: number | bigint; params: { transform: { origin: { x: number } } } };
    expected.rev = 9007199254740997n;
    expected.params.transform.origin.x = 1e19;
    const generated: Schema = { anyOf: [{ $ref: '#/$defs/Element' }, { type: 'null' }] };
    expect(violations(generated, expected, '', MODEL_DEFS)).toEqual([]);
    expect(certifyWireText(stringifyWireJson(expected), generated, MODEL_DEFS)).toEqual(expected);
  });

  it('reads every emitted value exactly at the mixed object-array depth limit', () => {
    for (const containers of [126, 127, 128]) {
      let value: unknown = 9007199254740997n;
      for (let i = 0; i < containers; i += 1) value = i % 2 === 0 ? [value] : { level: value };
      if (containers <= 127) {
        const text = stringifyWireJson(value);
        expect(stringifyWireJson(parseWireJson(text))).toBe(text);
      } else expect(() => stringifyWireJson(value)).toThrow();
    }
  });
});
