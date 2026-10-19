// @vitest-environment node
// C1.2 (IPC ticket): the generated command schemas (src/ipc/generated/commands.ts) keep the wire controls of
// ADR-0012 — lossless wide integers, finite doubles, strict text and depth, own-property and snapshot checks — and the
// front-end validator agrees with the executor on every declared example.
import { describe, expect, it } from 'vitest';

import { COMMAND_DEFS, COMMAND_INDEX, COMMANDS_REGISTRY_VERSION } from './generated/commands';
import { REGISTRY_VERSION } from './generated/wire';
import { certify, guard, MAX_DEPTH, structuredCloneable, violations, type Schema } from './schema';
import { certifyWireText, parseWireJson } from './wireJson';

function ref(name: string): Schema {
  return { $ref: `#/$defs/${name}` };
}

interface IndexEntry {
  readonly kind: string;
  readonly params: string;
  readonly result: string;
  readonly examples: readonly unknown[];
  readonly milestone?: string;
}

const entries = Object.entries(COMMAND_INDEX as Readonly<Record<string, IndexEntry>>);

const STATE =
  '{"projectId":"pr_1","path":null,"form":null,"formatVersion":"1.0","migratedFrom":null,' +
  '"documentRev":DOCREV,"savedRev":null,"dirty":true,"readOnly":false,"lock":null,"historyFromRev":1}';

describe('generated command schemas', () => {
  it('match the registry version and resolve for every typed declaration', () => {
    expect(COMMANDS_REGISTRY_VERSION).toBe(REGISTRY_VERSION);
    expect(entries.length).toBeGreaterThanOrEqual(130);
    for (const [id, c] of entries) {
      expect(Object.prototype.hasOwnProperty.call(COMMAND_DEFS, c.params), id).toBe(true);
      expect(Object.prototype.hasOwnProperty.call(COMMAND_DEFS, c.result), id).toBe(true);
    }
  });

  it('accept every declared example, as the executor does', () => {
    let n = 0;
    for (const [id, c] of entries) {
      for (const example of c.examples) {
        expect(violations(ref(c.params), example, '', COMMAND_DEFS), id).toEqual([]);
        n += 1;
      }
    }
    expect(n).toBeGreaterThanOrEqual(140);
  });

  it('reject unknown, missing and out-of-range parameters', () => {
    const frame = ref('app.frame.set.params');
    expect(violations(frame, { frame: 1 }, '', COMMAND_DEFS)).toEqual([]);
    expect(violations(frame, { frame: 1, extra: 1 }, '', COMMAND_DEFS)).not.toEqual([]);
    expect(violations(frame, {}, '', COMMAND_DEFS)).not.toEqual([]);
    expect(violations(frame, { frame: 0 }, '', COMMAND_DEFS)).not.toEqual([]);
    expect(violations(ref('window.fullScreen.set.params'), {}, '', COMMAND_DEFS)).not.toEqual([]);
  });

  it('carry 64-bit revisions losslessly and only in one representation', () => {
    const result = ref('doc.state.get.result');
    const wide = certifyWireText<{ documentRev: unknown }>(
      STATE.replace('DOCREV', '9007199254740993'),
      result,
      COMMAND_DEFS,
    );
    expect(wide?.documentRev).toBe(9007199254740993n);
    const max = certifyWireText<{ documentRev: unknown }>(
      STATE.replace('DOCREV', '18446744073709551615'),
      result,
      COMMAND_DEFS,
    );
    expect(max?.documentRev).toBe(18446744073709551615n);
    expect(
      certifyWireText(STATE.replace('DOCREV', '18446744073709551616'), result, COMMAND_DEFS),
    ).toBeUndefined();
    expect(certifyWireText(STATE.replace('DOCREV', '-1'), result, COMMAND_DEFS)).toBeUndefined();
    const parsed = JSON.parse(STATE.replace('DOCREV', '1')) as Record<string, unknown>;
    expect(violations(result, { ...parsed, documentRev: 9007199254740992 }, '', COMMAND_DEFS)).not.toEqual(
      [],
    );
    expect(violations(result, { ...parsed, documentRev: 12 }, '', COMMAND_DEFS)).toEqual([]);
  });

  it('accept only finite doubles and keep integral doubles as numbers', () => {
    const props = guard(ref('project.properties.get.result'), COMMAND_DEFS);
    expect(certify(props, { frameRate: 24, cameraWidth: 1920, cameraHeight: 1080 })).toBeDefined();
    for (const bad of [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
      expect(certify(props, { frameRate: bad, cameraWidth: 1920, cameraHeight: 1080 })).toBeUndefined();
    }
    const text = '{"frameRate":24,"cameraWidth":1920,"cameraHeight":1080}';
    expect(
      certifyWireText<{ frameRate: unknown }>(text, ref('project.properties.get.result'), COMMAND_DEFS)
        ?.frameRate,
    ).toBe(24);
  });

  it('validate free-form results by own properties, at bounded depth, as snapshots', () => {
    const get = guard(ref('doc.object.get.result'), COMMAND_DEFS);
    // `__proto__` from the wire is an own key of plain data, never a prototype.
    const parsed = parseWireJson('{"object":{"id":"x","type":"scene","__proto__":{"polluted":true}}}');
    const ok = certify(get, parsed) as { object: Record<string, unknown> } | undefined;
    expect(ok).toBeDefined();
    expect(Object.prototype.hasOwnProperty.call(ok?.object, '__proto__')).toBe(true);
    expect(({} as Record<string, unknown>)['polluted']).toBeUndefined();
    // Too deep: refused by the plain-data layer.
    let deep: unknown = 0;
    for (let i = 0; i < MAX_DEPTH + 2; i += 1) deep = { d: deep };
    expect(certify(get, { object: deep })).toBeUndefined();
    // Accessors, class instances and proxies are not plain data.
    const accessor = { object: Object.defineProperty({}, 'id', { get: () => 'x', enumerable: true }) };
    expect(certify(get, accessor)).toBeUndefined();
    expect(certify(get, { object: new Date() })).toBeUndefined();
    const proxy = new Proxy({ object: { id: 'x' } }, {});
    expect(structuredCloneable(proxy)).toBe(false);
    expect(certify(get, proxy)).toBeUndefined();
    // The certified value is a copy: later changes to the source do not reach it.
    const source = { object: { id: 'x', meta: { n: 1 } } };
    const copy = certify(get, source) as { object: { meta: { n: number } } };
    source.object.meta.n = 2;
    expect(copy.object.meta.n).toBe(1);
  });

  it('validate whole envelopes', () => {
    const env = ref('envelope.root');
    const text =
      '{"status":"ok","result":{"x":1},"changes":[{"id":"sc_1","type":"scene","op":"modified","rev":18446744073709551615,' +
      '"patch":[{"op":"add","path":"/meta","value":{}}]}],"undo":{"stepId":"u-1","label":"Set meta"},"warnings":[],"documentRev":18446744073709551615}';
    const ok = certifyWireText<{ documentRev: unknown }>(text, env, COMMAND_DEFS);
    expect(ok?.documentRev).toBe(18446744073709551615n);
    const error =
      '{"status":"error","changes":[],"warnings":[],"documentRev":null,"error":{"code":"command.notImplemented","message":"m","details":{"milestone":"M2 C2.3"}}}';
    expect(certifyWireText(error, env, COMMAND_DEFS)).toBeDefined();
    expect(certifyWireText(error.replace('"error"', '"oops"'), env, COMMAND_DEFS)).toBeUndefined();
  });
});
