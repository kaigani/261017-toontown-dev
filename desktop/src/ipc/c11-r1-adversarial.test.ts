// @vitest-environment node
/// <reference types="node" />
// Permanent C1.1 regressions; assertions describe required behaviour, without expected-failure markers.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { MODEL_DEFS } from './generated/model';
import { WIRE_DEFS } from './generated/wire';
import { certify, guard, type Schema } from './schema';
import { certifyWireText, parseWireJson, stringifyWireJson } from './wireJson';

const ref = (name: string): Schema => ({ $ref: `#/$defs/${name}` });
function golden(relative: string): Record<string, unknown> {
  return JSON.parse(
    readFileSync(
      fileURLToPath(new URL(`../../../crates/model/tests/golden/sample/${relative}`, import.meta.url)),
      'utf8',
    ),
  ) as Record<string, unknown>;
}

describe('C11 R1 text boundary', () => {
  it('coerces doubles declared inside generated conditional effect schemas', () => {
    // Drop-shadow transform coordinates are unrestricted f64 scene values.
    const shadow = golden('elements/Intro/Shadow.json');
    const p = shadow['params'] as Record<string, unknown>;
    const transform = p['transform'] as Record<string, unknown>;
    const origin = transform['origin'] as Record<string, unknown>;
    origin['x'] = 1e20;
    expect(certifyWireText(stringifyWireJson(shadow), ref('Element'), MODEL_DEFS)).toBeDefined();
  });

  it('rejects unpaired surrogate strings that the Rust UTF-8 reader cannot accept', () => {
    expect(
      certifyWireText('{"code":"arg.invalid","message":"\\ud800"}', ref('ErrorInfo'), WIRE_DEFS),
    ).toBeUndefined();
  });

  it('rejects malformed model tuples at every element position', () => {
    const stroke = {
      id: 's',
      type: 'stroke',
      rev: 1,
      kind: 'brush',
      closed: true,
      points: [[0, 0, 0, 0, 0, 0]],
      colourId: null,
      fillMatrix: null,
      lineStyle: null,
      spine: null,
      sides: { left: null, right: null },
    };
    expect(certifyWireText(stringifyWireJson(stroke), ref('Stroke'), MODEL_DEFS)).toBeDefined();
    for (let i = 0; i < 6; i++) {
      const tuple: unknown[] = [0, 0, 0, 0, 0, 0];
      tuple[i] = 'wrong';
      expect(
        certifyWireText(stringifyWireJson({ ...stroke, points: [tuple] }), ref('Stroke'), MODEL_DEFS),
      ).toBeUndefined();
    }
  });

  it('preserves u64, negative large integers, long strings, and canonical f64 values', () => {
    for (const text of [
      '9007199254740992',
      '9007199254740993',
      '18446744073709551615',
      '18446744073709551616',
      '-9007199254740993',
      '-18446744073709551616',
    ]) {
      expect(stringifyWireJson(parseWireJson(text))).toBe(text);
    }
    const s = 'é\u2028😀'.repeat(300_000);
    expect(parseWireJson(stringifyWireJson(s))).toBe(s);
    for (const n of [-0, 1e21, 0.1 + 0.2, 5e-324]) {
      expect(Number(parseWireJson(stringifyWireJson(n)))).toBe(n === 0 ? 0 : n);
    }
  });

  it('keeps proxy/getter/prototype rejection and never invokes a getter', () => {
    const any = guard<unknown>({});
    let calls = 0;
    const getter = Object.defineProperty({}, 'secret', {
      enumerable: true,
      get() {
        calls++;
        return 1;
      },
    });
    const transparent = new Proxy({ x: 1 }, {});
    const sparse = new Array<unknown>(3);
    sparse[2] = 1;
    const { proxy, revoke } = Proxy.revocable({}, {});
    revoke();
    for (const value of [
      getter,
      transparent,
      { nested: transparent },
      proxy,
      Object.create({ x: 1 }) as object,
      sparse,
    ]) {
      expect(certify(any, value)).toBeUndefined();
    }
    expect(calls).toBe(0);
    for (const text of ['{"x":1,"x":2}', '[1,,2]', '{"x":1}garbage'])
      expect(() => parseWireJson(text)).toThrow();
    const own = parseWireJson('{"__proto__":{"x":1},"constructor":2}') as object;
    expect(Object.getPrototypeOf(own)).toBe(Object.prototype);
    expect(Object.prototype.hasOwnProperty.call(own, '__proto__')).toBe(true);
  });

  it('validates a representative ten-megabyte drawing', () => {
    const stroke = {
      id: 's',
      type: 'stroke',
      rev: 1,
      kind: 'brush',
      closed: true,
      points: [
        [0, 0, 0, 0, 0, 0],
        [1, 1, 0, 0, 0, 0],
      ],
      colourId: null,
      fillMatrix: null,
      lineStyle: null,
      spine: null,
      sides: { left: null, right: null },
    };
    const strokes = Array.from({ length: 50_000 }, (_, i) => ({ ...stroke, id: `s${i}` }));
    const drawing = {
      id: 'd',
      type: 'drawing',
      rev: 1,
      name: '1',
      locked: false,
      pivot: { x: 0, y: 0 },
      mouthShape: null,
      nodes: [{ id: 'l', type: 'drawingNode', rev: 1, kind: 'layer', hint: 'brush', strokes }],
      colourRecovery: [],
      createdAt: '2026-10-17T09:30:12.345Z',
    };
    const text = stringifyWireJson(drawing);
    const start = performance.now();
    expect(certifyWireText(text, ref('Drawing'), MODEL_DEFS)).toBeDefined();
    console.log(
      JSON.stringify({
        review: 'drawing-validation',
        bytes: Buffer.byteLength(text),
        ms: performance.now() - start,
      }),
    );
  }, 30_000);
});
