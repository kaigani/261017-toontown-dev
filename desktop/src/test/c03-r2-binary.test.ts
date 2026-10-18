// @vitest-environment node
import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
import { createHostApi, createMockTransport } from '../ipc';
import { decodeRenderList } from '../spikes/canvas/decode';

function fixture() {
  const b = readFileSync(new URL('../spikes/canvas/testdata/tiny.ttrl', import.meta.url));
  return b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength);
}
async function decode(b: ArrayBuffer) {
  const p = await createHostApi({
    ...createMockTransport(),
    invokeBinary: () => Promise.resolve(b),
  }).spike.canvasScene({ scene: 'groups1k', seed: 9, frame: 0 });
  return decodeRenderList(p.header, p.bytes);
}
for (const v of [NaN, Infinity, -Infinity, -(2 ** -149), 100 + 2 ** -17]) {
  it(`refuses nonfinite or adjacent out of bounds coordinate ${String(v)}`, async () => {
    const b = fixture();
    new DataView(b).setFloat32(204, v, true);
    await expect(decode(b)).rejects.toThrow();
  });
}
it('accepts subnormal coordinates inside bounds', async () => {
  const b = fixture();
  new DataView(b).setFloat32(204, 2 ** -149, true);
  expect((await decode(b)).stats.paths).toBe(3);
});
it('rejects NaN header bounds', async () => {
  const b = fixture();
  new DataView(b).setFloat32(36, NaN, true);
  await expect(decode(b)).rejects.toThrow();
});
it('does not wrap 2^31 points times 8 to match a header-only packet', async () => {
  const b = fixture().slice(0, 72);
  const v = new DataView(b);
  v.setUint32(20, 0, true);
  v.setUint32(24, 0, true);
  v.setUint32(28, 0, true);
  v.setUint32(32, 2 ** 31, true);
  await expect(decode(b)).rejects.toThrow();
});
it('accepts an empty verb range with no geometry from that path', async () => {
  const b = fixture();
  new DataView(b).setUint32(124, 0, true);
  expect((await decode(b)).strokeCount).toBe(0);
});
