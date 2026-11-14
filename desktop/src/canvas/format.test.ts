// @vitest-environment node
// T4 (design note 1 §7 and §8): the TypeScript decoder against the golden corpus the Rust encoder writes and checks
// byte for byte (`cargo test -p tt-render --test c42_canvas`, `cargo test -p tt-executor --test c42 canvas::`). Every
// valid file decodes to its expected JSON; every header field's file carries its value; every corrupt file is refused
// with exactly its rule, and the unmodified file decodes.
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

import { decodeDrawing, FormatError, vertexAt, type DecodedDrawing, type Rule } from './format';

const file = (name: string) => {
  const b = readFileSync(new URL(`./testdata/${name}`, import.meta.url));
  return new Uint8Array(b.buffer, b.byteOffset, b.byteLength);
};
const json = (name: string): unknown =>
  JSON.parse(readFileSync(new URL(`./testdata/${name}`, import.meta.url), 'utf8'));

/** The decoded drawing in the corpus's JSON shape. */
function asJson(d: DecodedDrawing): unknown {
  const verts = (m: { vertexCount: number } & Parameters<typeof vertexAt>[0]) =>
    Array.from({ length: m.vertexCount }, (_, i) => vertexAt(m, i));
  return {
    generation: d.generation.toString(),
    rev: d.rev.toString(),
    drawingId: d.drawingUlid,
    bucket: d.bucket,
    flags: d.flags,
    bounds: d.bounds,
    slots: d.slots,
    records: d.records.map((r) => {
      switch (r.kind) {
        case 'solid':
          return { kind: 'solid', vertices: verts(r), indices: Array.from(r.indices) };
        case 'gradient':
          return {
            kind: 'gradient',
            radial: r.radial,
            drawingToUnit: r.drawingToUnit,
            stops: r.stops,
            vertices: verts(r),
            indices: Array.from(r.indices),
          };
        case 'bitmap':
          return {
            kind: 'bitmap',
            width: r.width,
            height: r.height,
            quad: r.quad,
            imageId: r.imageId,
            slot: r.slot,
          };
      }
    }),
  };
}

describe('TTRD 1.0 decoder against the golden corpus', () => {
  it.each(['synthetic', 'drawing'])('%s.ttrd decodes to its expected JSON', (name) => {
    const d = decodeDrawing(file(`${name}.ttrd`));
    expect(asJson(d)).toEqual(json(`${name}.json`));
  });

  it('reaches every record kind', () => {
    const kinds = new Set(decodeDrawing(file('synthetic.ttrd')).records.map((r) => r.kind));
    expect([...kinds].sort()).toEqual(['bitmap', 'gradient', 'solid']);
  });

  it('carries every header field', () => {
    const cells = json('fields.json') as { file: string; field: string; expected: unknown }[];
    expect(cells.map((c) => c.field).sort()).toEqual([
      'bounds',
      'bucket',
      'drawingId',
      'flags',
      'generation',
      'rev',
      'slots',
    ]);
    const base = asJson(decodeDrawing(file('synthetic.ttrd')));
    for (const c of cells) {
      const got = asJson(decodeDrawing(file(c.file)));
      expect(got, c.field).toEqual(c.expected);
      expect(got, `${c.field} differs from the base drawing`).not.toEqual(base);
    }
  });

  it('refuses every corrupt file with its rule, and decodes the unmodified one', () => {
    const cells = json('corrupt.json') as { file: string; rule: Rule | null }[];
    const rules = new Set(cells.flatMap((c) => (c.rule ? [c.rule] : [])));
    expect(rules.size).toBe(10);
    for (const c of cells) {
      const bytes = file(c.file);
      if (c.rule === null) {
        expect(() => decodeDrawing(bytes)).not.toThrow();
        continue;
      }
      let err: unknown;
      try {
        decodeDrawing(bytes);
      } catch (e) {
        err = e;
      }
      expect(err, c.file).toBeInstanceOf(FormatError);
      expect((err as FormatError).rule, c.file).toBe(c.rule);
    }
  });

  it('decodes from an unaligned view by copying the indices', () => {
    const b = file('synthetic.ttrd');
    const shifted = new Uint8Array(b.byteLength + 1);
    shifted.set(b, 1);
    const d = decodeDrawing(shifted.subarray(1));
    expect(asJson(d)).toEqual(json('synthetic.json'));
  });
});
