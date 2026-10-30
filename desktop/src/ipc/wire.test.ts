// @vitest-environment node
/// <reference types="node" />
// C1.1 (ADR-0012): lossless JSON text transport, generated schemas, proxy rejection on the platform floor.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it, vi } from 'vitest';

import { MODEL_DEFS, OBJECT_TYPE_DEFS } from './generated/model';
import { FORMAT_VERSION, WIRE_DEFS } from './generated/wire';
import { createHostApi, MENU_COMMAND_EVENT } from './hostApi';
import { createMockTransport } from './mockTransport';
import { ANY, certify, guard, MAX_DEPTH, structuredCloneable, violations, type Schema } from './schema';
import type { HostTransport } from './transport';
import { certifyWireText, coerceNumbers, parseWireJson, stringifyWireJson, WireJsonError } from './wireJson';

const repo = fileURLToPath(new URL('../../../', import.meta.url));
const any = guard<unknown>(ANY);

function ref(name: string): Schema {
  return { $ref: `#/$defs/${name}` };
}

describe('lossless wire JSON', () => {
  it('parses like JSON.parse for ordinary documents', () => {
    const texts = [
      '{"a":1,"b":[true,false,null],"c":{"d":"x\\u00e9\\n\\"q\\""},"e":-0.5,"f":1e-7,"g":[]}',
      '[1,2.5,-3,1e21,"\\ud83d\\ude00",{}]',
      ' \n\t{ "k" : [ 0 , -0 ] } ',
      '"just a string"',
      '123',
    ];
    for (const t of texts) expect(parseWireJson(t)).toEqual(JSON.parse(t));
  });

  it('keeps integers beyond 2^53 - 1 exactly as bigint, and only those', () => {
    const v = parseWireJson(
      '{"max":18446744073709551615,"safe":9007199254740991,"unsafe":9007199254740992,"neg":-9007199254740993}',
    ) as Record<string, unknown>;
    expect(v['max']).toBe(18446744073709551615n);
    expect(v['safe']).toBe(9007199254740991);
    expect(v['unsafe']).toBe(9007199254740992n);
    expect(v['neg']).toBe(-9007199254740993n);
    expect(stringifyWireJson(v)).toBe(
      '{"max":18446744073709551615,"safe":9007199254740991,"unsafe":9007199254740992,"neg":-9007199254740993}',
    );
  });

  it('rejects malformed text, duplicate keys and excessive depth', () => {
    for (const bad of [
      '',
      '{',
      '{"a":1,}',
      '[1,]',
      '{"a":1}x',
      "{'a':1}",
      '{"a":01}',
      '{"a":.5}',
      '{"a":1,"a":2}',
      '"\u0001"',
      'NaN',
      '[-]',
      '{"a":1e}',
      'tru',
      '"\\x"',
    ]) {
      expect(() => parseWireJson(bad), bad).toThrow(WireJsonError);
    }
    expect(() => parseWireJson('['.repeat(MAX_DEPTH) + ']'.repeat(MAX_DEPTH))).not.toThrow();
    expect(() => parseWireJson('['.repeat(MAX_DEPTH + 1) + ']'.repeat(MAX_DEPTH + 1))).toThrow(WireJsonError);
  });

  it('builds own data properties: __proto__ is a key, prototypes stay untouched', () => {
    const v = parseWireJson('{"__proto__":{"polluted":true},"constructor":1}') as object;
    expect(Object.getPrototypeOf(v)).toBe(Object.prototype);
    expect(Object.getOwnPropertyDescriptor(v, '__proto__')?.value).toEqual({ polluted: true });
    expect(Object.prototype.hasOwnProperty.call(Object.prototype, 'polluted')).toBe(false);
  });

  it('writes numbers exactly like the Rust canonical writer (2 000 shared vectors)', () => {
    const golden = JSON.parse(readFileSync(join(repo, 'crates/model/tests/golden/numbers.json'), 'utf8')) as {
      cases: [string, string][];
    };
    expect(golden.cases.length).toBeGreaterThan(1900);
    const view = new DataView(new ArrayBuffer(8));
    for (const [hex, text] of golden.cases) {
      view.setBigUint64(0, BigInt(`0x${hex}`));
      const x = view.getFloat64(0);
      // The canonical form is ECMAScript's except for integral doubles from 2^53 to 1e21, which both writers write
      // as their exact integer digits (C11-R1-07); there `JSON.stringify` pads the shortest digits with zeros.
      const large = Number.isInteger(x) && Math.abs(x) >= 2 ** 53 && Math.abs(x) < 1e21;
      expect(large ? BigInt(x).toString() : JSON.stringify(x), hex).toBe(text);
      expect(stringifyWireJson(x)).toBe(text);
      const back = parseWireJson(text);
      expect(typeof back === 'bigint' ? Number(back) : back).toBe(x === 0 ? 0 : x);
    }
  });

  it('coerces digit-only doubles back to numbers where the schema says number', () => {
    const schema: Schema = {
      type: 'object',
      properties: {
        d: { type: 'number' },
        i: { type: 'integer', minimum: 0, maximum: 18446744073709551615n },
        o: { anyOf: [{ type: 'number' }, { type: 'null' }] },
      },
      additionalProperties: false,
    };
    const v = coerceNumbers(
      parseWireJson('{"d":100000000000000000000,"i":10000000000000000000,"o":100000000000000000000}'),
      schema,
      {},
    ) as Record<string, unknown>;
    expect(v['d']).toBe(1e20);
    expect(v['i']).toBe(10000000000000000000n);
    expect(v['o']).toBe(1e20);
    expect(violations(schema, v)).toEqual([]);
  });
});

describe('integer schemas and bigint', () => {
  const u64: Schema = { type: 'integer', minimum: 0, maximum: 18446744073709551615n };
  it('accepts safe numbers and unsafe bigints within bounds, nothing else', () => {
    expect(violations(u64, 0)).toEqual([]);
    expect(violations(u64, Number.MAX_SAFE_INTEGER)).toEqual([]);
    expect(violations(u64, 18446744073709551615n)).toEqual([]);
    expect(violations(u64, 2 ** 53)).not.toEqual([]); // a number that may already be rounded
    expect(violations(u64, 5n)).not.toEqual([]); // one representation per value
    expect(violations(u64, 18446744073709551616n)).not.toEqual([]);
    expect(violations(u64, -9007199254740993n)).not.toEqual([]);
  });
  it('object transports still reject bigint (only the parser produces it)', () => {
    expect(certify(any, { seq: 18446744073709551615n })).toBeUndefined();
  });
});

describe('generated wire schemas', () => {
  it('describe the engine types the host sends', () => {
    expect(FORMAT_VERSION).toMatch(/^\d+\.\d+$/);
    const ping = { seq: 18446744073709551615n, payload: null, engineUptimeNs: 1, handlerNs: 2 };
    expect(violations(ref('PingResponse'), ping, '', WIRE_DEFS)).toEqual([]);
    expect(violations(ref('PingResponse'), { ...ping, extra: 1 }, '', WIRE_DEFS)).not.toEqual([]);
    expect(
      violations(
        ref('MenuCommandEvent'),
        { command: 'a.b', params: {}, handler: 'host', milestone: null },
        '',
        WIRE_DEFS,
      ),
    ).not.toEqual([]);
  });
});

function goldenFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    return statSync(p).isDirectory() ? goldenFiles(p) : [p];
  });
}

describe('generated model schemas (10 §2.8): the Rust goldens validate in the front end too', () => {
  const files = goldenFiles(join(repo, 'crates/model/tests/golden/sample'));

  it('every persisted entry of the sample project validates against its type', () => {
    expect(files.length).toBeGreaterThan(30);
    for (const file of files) {
      const value = parseWireJson(readFileSync(file, 'utf8')) as Record<string, unknown>;
      const type = value['type'] as string;
      const def = (OBJECT_TYPE_DEFS as Record<string, string>)[type];
      expect(def, file).toBeDefined();
      const schema = ref(def ?? '');
      const coerced = coerceNumbers(value, schema, MODEL_DEFS);
      expect(violations(schema, coerced, '', MODEL_DEFS), file).toEqual([]);
    }
  });

  it('rejects the same objects after hostile edits', () => {
    const element = files.find((f) => f.replaceAll('\\', '/').endsWith('elements/Intro/Bob.json')) ?? '';
    const base = parseWireJson(readFileSync(element, 'utf8')) as Record<string, unknown>;
    const schema = ref('Element');
    const bad: Record<string, unknown>[] = [
      { ...base, unexpected: 1 },
      { ...base, elementType: 'hologram' },
      { ...base, camera: { fovMode: 'static', fov: 40 } }, // a drawing element has no camera block
      { ...base, columnWidth: 5 },
      { ...base, name: 'Bob.1' }, // `.` is forbidden in element names
      { ...base, exposure: [['dr_x', 0]] }, // run counts are ≥ 1
      { ...base, rev: -1 },
    ];
    for (const b of bad)
      expect(violations(schema, b, '', MODEL_DEFS), JSON.stringify(Object.keys(b))).not.toEqual([]);
    const withoutTransform = { ...base };
    delete withoutTransform['transform'];
    expect(violations(schema, withoutTransform, '', MODEL_DEFS)).not.toEqual([]);
  });
});

function textTransport(response: unknown, event?: unknown): HostTransport {
  return {
    kind: 'mock',
    invoke: () => Promise.resolve(response),
    listen: (_name, callback) => {
      if (event !== undefined) callback(event);
      return Promise.resolve(() => undefined);
    },
  };
}

describe('the host API over the text transport', () => {
  const text = '{"seq":18446744073709551615,"payload":"abc","engineUptimeNs":9007199254740993,"handlerNs":0}';

  it('delivers 64-bit integers exactly, from a string or a UTF-8 body', async () => {
    for (const body of [text, new TextEncoder().encode(text).buffer]) {
      const r = await createHostApi(textTransport(body)).ping({ seq: 18446744073709551615n });
      expect(r.seq).toBe(18446744073709551615n);
      expect(r.engineUptimeNs).toBe(9007199254740993n);
      expect(r.payload).toBe('abc');
    }
  });

  it('sends arguments as JSON text, losslessly', async () => {
    let sent: unknown;
    const transport: HostTransport = {
      kind: 'mock',
      invoke: (_command, args) => {
        sent = args;
        return Promise.resolve(text);
      },
      listen: () => Promise.resolve(() => undefined),
    };
    await createHostApi(transport).ping({ seq: 18446744073709551615n, payload: 'abc' });
    expect(sent).toEqual({ json: '{"seq":18446744073709551615,"payload":"abc"}' });
  });

  it('rejects malformed or invalid text as ipc.invalidResponse', async () => {
    for (const body of [
      '{"seq":1',
      '{"seq":-1,"payload":null,"engineUptimeNs":0,"handlerNs":0}',
      '{}',
      new Uint8Array([0xff, 0xfe]).buffer,
    ]) {
      await expect(createHostApi(textTransport(body)).ping({ seq: 1 })).rejects.toMatchObject({
        code: 'ipc.invalidResponse',
      });
    }
  });

  it('maps error text to HostError with the stable code and lossless details', async () => {
    const transport: HostTransport = {
      kind: 'mock',
      invoke: () =>
        Promise.reject(
          '{"code":"limit.exceeded","message":"too big","details":{"limit":18446744073709551615}}',
        ),
      listen: () => Promise.resolve(() => undefined),
    };
    await expect(createHostApi(transport).ping({ seq: 1 })).rejects.toMatchObject({
      code: 'limit.exceeded',
      details: { limit: 18446744073709551615n },
    });
  });

  it('delivers menu events sent as text and drops malformed ones', async () => {
    const seen: unknown[] = [];
    const ok =
      '{"command":"workspace.reset","params":{"name":"Default"},"handler":"frontend","milestone":null}';
    await createHostApi(textTransport(null, ok)).onMenuCommand((e) => seen.push(e));
    await createHostApi(textTransport(null, '{"command":"Quit"}')).onMenuCommand((e) => seen.push(e));
    expect(seen).toEqual([
      { command: 'workspace.reset', params: { name: 'Default' }, handler: 'frontend', milestone: null },
    ]);
  });

  it('still works over the object-returning mock', async () => {
    const transport = createMockTransport();
    const host = createHostApi(transport);
    expect((await host.appInfo()).formatVersion).toBe(FORMAT_VERSION);
    const seen: unknown[] = [];
    await host.onMenuCommand((e) => seen.push(e));
    transport.emit(MENU_COMMAND_EVENT, {
      command: 'workspace.reset',
      params: {},
      handler: 'frontend',
      milestone: null,
    });
    expect(seen).toHaveLength(1);
  });

  it('certifyWireText never throws', () => {
    expect(certifyWireText('{"a":', ANY, {})).toBeUndefined();
    expect(certifyWireText('[1,2]', ANY, {})).toEqual([1, 2]);
  });
});

describe('C01-R3-04 on the platform floor: proxies are rejected without structuredClone (Safari 15.0)', () => {
  it('rejects transparent object and array proxies, nested too, with structuredClone removed', () => {
    vi.stubGlobal('structuredClone', undefined);
    let results: unknown[] | undefined;
    try {
      results = [
        certify(any, new Proxy({ value: 1 }, {})),
        certify(any, new Proxy([1, 2], {})),
        certify(any, { inner: new Proxy({ x: 1 }, {}) }),
        certify(any, { ok: [1, { b: null }] }),
      ];
    } finally {
      vi.unstubAllGlobals();
    }
    expect(results).toEqual([undefined, undefined, undefined, { ok: [1, { b: null }] }]);
  });

  it('the probe fails closed when MessageChannel is missing', () => {
    vi.stubGlobal('MessageChannel', undefined);
    let cloneable: boolean | undefined;
    try {
      cloneable = structuredCloneable({ a: 1 });
    } finally {
      vi.unstubAllGlobals();
    }
    expect(cloneable).toBe(false);
  });
});
