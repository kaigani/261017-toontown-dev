// @vitest-environment node
import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
import { decodeDrawing, FormatError } from '../canvas/format';
import { parseSceneAnswer } from '../canvas/scene';
import { parseWireJson, stringifyWireJson } from '../ipc/wireJson';

const original = new Uint8Array(readFileSync(new URL('../canvas/testdata/synthetic.ttrd', import.meta.url)));
const view = new DataView(original.buffer);
let first = 80;
for (let i = 0; i < view.getUint32(12, true); i++) {
  first += 8 + Math.ceil(view.getUint16(first, true) / 4) * 4;
}
const mutations: [string, (b: Uint8Array, v: DataView) => void][] = [
  [
    'magic',
    (b) => {
      b[0] = 0;
    },
  ],
  [
    'major',
    (_, v) => {
      v.setUint16(4, 65535, true);
    },
  ],
  [
    'header',
    (_, v) => {
      v.setUint16(8, 64, true);
    },
  ],
  [
    'count overflow',
    (_, v) => {
      v.setUint32(first + 8, 0xffffffff, true);
    },
  ],
  [
    'overlong record',
    (_, v) => {
      v.setUint32(first + 4, 0xffffffff, true);
    },
  ],
  [
    'unknown kind',
    (_, v) => {
      v.setUint16(first, 65535, true);
    },
  ],
  [
    'NaN',
    (_, v) => {
      v.setFloat32(first + 16, NaN, true);
    },
  ],
  [
    'infinity',
    (_, v) => {
      v.setFloat32(first + 16, Infinity, true);
    },
  ],
  [
    'slot',
    (_, v) => {
      v.setUint16(first + 24, 65535, true);
    },
  ],
  [
    'index',
    (_, v) => {
      v.setUint32(first + 16 + v.getUint32(first + 8, true) * 12, 0xffffffff, true);
    },
  ],
  [
    'id',
    (b) => {
      b[32] = 33;
    },
  ],
];
it.each(mutations)('refuses %s with a typed error', (_, mutate) => {
  const b = original.slice();
  mutate(b, new DataView(b.buffer));
  expect(() => decodeDrawing(b)).toThrow(FormatError);
});
it('refuses every truncated prefix and overlong payload', () => {
  for (let n = 0; n < original.length; n++)
    expect(() => decodeDrawing(original.slice(0, n))).toThrow(FormatError);
  const b = new Uint8Array(original.length + 4);
  b.set(original);
  expect(() => decodeDrawing(b)).toThrow(FormatError);
});
it('keeps integer revision precision in the decoder', () => {
  const b = original.slice();
  new DataView(b.buffer).setBigUint64(24, 9007199254740993n, true);
  expect(decodeDrawing(b).rev).toBe(9007199254740993n);
});
it('accepts a newer minor with the same layout', () => {
  const b = original.slice();
  new DataView(b.buffer).setUint16(6, 65535, true);
  expect(decodeDrawing(b).records.length).toBeGreaterThan(0);
});

it('accepts an exact u64 drawing revision through real wire parsing', () => {
  const answer = {
    generation: 1,
    scene: {
      viewId: 'drawingView',
      background: [0, 0, 0, 255],
      grid: { visible: false, type: 'normal', overlay: false },
      entries: [
        {
          drawingId: 'dr_01K7AAAAAAAAAAAAAAAAAAAAAA',
          rev: 9007199254740993n,
          role: 'current',
          opacity: 1,
          tint: null,
          shade: false,
          outline: false,
        },
      ],
      colours: {},
      paletteStamp: 1,
    },
  };
  expect(() => parseSceneAnswer(parseWireJson(stringifyWireJson(answer)))).not.toThrow();
});
