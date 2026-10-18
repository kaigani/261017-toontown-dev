// @vitest-environment node
// Permanent review regressions, exercising invokeBinary through the typed API and decoder.
import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { expect, it } from 'vitest';
import { createHostApi, createMockTransport } from '../ipc';
import { decodeRenderList } from '../spikes/canvas/decode';

const request = { scene: 'groups1k', seed: 9, frame: 0 } as const;
function fixture(): ArrayBuffer {
  const b = readFileSync(new URL('../spikes/canvas/testdata/tiny.ttrl', import.meta.url));
  return b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength);
}
async function decode(bytes: ArrayBuffer) {
  const converted = spawnSync(
    process.execPath,
    [fileURLToPath(new URL('../../../tests/adversarial/c03-r1-real-transport.mjs', import.meta.url))],
    { input: Buffer.from(bytes) },
  );
  expect(converted.status, converted.stderr.toString()).toBe(0);
  const response = converted.stdout.buffer.slice(
    converted.stdout.byteOffset,
    converted.stdout.byteOffset + converted.stdout.byteLength,
  );
  const p = await createHostApi({
    ...createMockTransport(),
    invokeBinary: () => Promise.resolve(response),
  }).spike.canvasScene(request);
  return decodeRenderList(p.header, p.bytes);
}

it('valid binary control travels through real invokeBinary', async () => {
  expect((await decode(fixture())).stats.paths).toBe(3);
});

for (const [name, mutation] of [
  [
    'NaN move coordinate',
    (v: DataView) => {
      v.setFloat32(204, NaN, true);
    },
  ],
  [
    'infinite cubic control',
    (v: DataView) => {
      v.setFloat32(212, Infinity, true);
    },
  ],
] as const) {
  it(`C03-R1-02: rejects ${name}`, async () => {
    const b = fixture();
    mutation(new DataView(b));
    await expect(decode(b)).rejects.toThrow();
  });
}

for (const [name, mutation] of [
  [
    'wrong counts',
    (v: DataView) => {
      v.setUint32(24, 4, true);
    },
  ],
  [
    '2^31 counts',
    (v: DataView) => {
      v.setUint32(24, 2 ** 31, true);
    },
  ],
  [
    'out of range group',
    (v: DataView) => {
      v.setUint32(132, 5, true);
    },
  ],
  [
    'negative width',
    (v: DataView) => {
      v.setFloat32(144, -1, true);
    },
  ],
  [
    'out of range points',
    (v: DataView) => {
      v.setUint32(128, 2 ** 31, true);
    },
  ],
] as const) {
  it(`malformed control: ${name}`, async () => {
    const b = fixture();
    mutation(new DataView(b));
    await expect(decode(b)).rejects.toThrow();
  });
}

it('malformed control: truncation', async () => {
  await expect(decode(fixture().slice(0, 290))).rejects.toThrow();
});
