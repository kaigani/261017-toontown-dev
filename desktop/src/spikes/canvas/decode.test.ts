// @vitest-environment node
// Contract with the Rust encoder: testdata/tiny.ttrl is written and checked by the host test
// `spikes::canvas::renderlist::tests::tiny_fixture_matches_committed_bytes`.
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

import { createHostApi, createMockTransport, type HostTransport } from '../../ipc';
import { decodeRenderList } from './decode';

function fixture(): ArrayBuffer {
  const b = readFileSync(new URL('./testdata/tiny.ttrl', import.meta.url));
  return b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength);
}

function binaryHost(bytes: ArrayBuffer | (() => ArrayBuffer)) {
  const transport: HostTransport = {
    ...createMockTransport(),
    invokeBinary: () => Promise.resolve(typeof bytes === 'function' ? bytes() : bytes),
  };
  return createHostApi(transport);
}

const request = { scene: 'groups1k', seed: 9, frame: 0 } as const;

describe('render-list envelope (src/ipc)', () => {
  it('certifies the Rust-encoded fixture', async () => {
    const { header } = await binaryHost(fixture()).spike.canvasScene(request);
    expect(header).toMatchObject({
      version: 1,
      headerLength: 72,
      kind: 'groups1k',
      seed: 9,
      frame: 0,
      groups: 2,
      paths: 3,
      verbs: 11,
      points: 11,
      bounds: [0, 0, 100, 100],
      encodeNs: 0,
      generateNs: 0,
      byteLength: 304,
    });
  });

  it('rejects a wrong magic, an unknown version, a length that disagrees with the counts, and no transport', async () => {
    const mutate = (f: (v: DataView) => void) => () => {
      const b = fixture();
      f(new DataView(b));
      return b;
    };
    await expect(
      binaryHost(
        mutate((v) => {
          v.setUint8(0, 0x41);
        }),
      ).spike.canvasScene(request),
    ).rejects.toMatchObject({ code: 'ipc.invalidResponse' });
    await expect(
      binaryHost(
        mutate((v) => {
          v.setUint16(4, 2, true);
        }),
      ).spike.canvasScene(request),
    ).rejects.toMatchObject({ code: 'ipc.invalidResponse' });
    await expect(
      binaryHost(
        mutate((v) => {
          v.setUint32(24, 4, true);
        }),
      ).spike.canvasScene(request),
    ).rejects.toMatchObject({ code: 'ipc.invalidResponse' });
    await expect(binaryHost(fixture().slice(0, 100)).spike.canvasScene(request)).rejects.toMatchObject({
      code: 'ipc.invalidResponse',
    });
    await expect(createHostApi(createMockTransport()).spike.canvasScene(request)).rejects.toMatchObject({
      code: 'ipc.unsupported',
    });
  });
});

describe('render-list decoder', () => {
  it('decodes strokes, fills and groups of the fixture', async () => {
    const { header, bytes } = await binaryHost(fixture()).spike.canvasScene(request);
    const g = decodeRenderList(header, bytes, 0.5);
    expect(Array.from(g.groups)).toEqual([1, 0, 0, 1, 0, 0, 2, 0, 0, 2, 10, 20]);
    expect(g.stats).toMatchObject({ paths: 3, strokePaths: 1, fillPaths: 2, cubics: 1, triangles: 3 });
    expect(g.fillVertexCount).toBe(7);
    expect(g.strokeCount).toBeGreaterThan(1);
    const f32 = new Float32Array(g.strokes);
    const u32 = new Uint32Array(g.strokes);
    expect([f32[0], f32[1], f32[4]]).toEqual([0, 0, 1]); // starts at the move point, half width 1
    expect(u32[5]).toBe(0xff0000ff); // RGBA bytes 255,0,0,255 little-endian
    expect(u32[6]).toBe(0);
    // Last segment ends at the cubic's end point.
    const last = (g.strokeCount - 1) * 8;
    expect([f32[last + 2], f32[last + 3]]).toEqual([30, 30]);
    const fills = new Uint32Array(g.fillVertices);
    expect(fills[3]).toBe(1); // group of the first fill vertex
  });

  it('throws on malformed contents instead of reading out of range', async () => {
    const { header, bytes } = await binaryHost(fixture()).spike.canvasScene(request);
    const pathsOff = 72 + 2 * 24;
    const bad = (f: (v: DataView) => void) => {
      const b = bytes.slice(0);
      f(new DataView(b));
      return () => decodeRenderList(header, b);
    };
    expect(
      bad((v) => {
        v.setUint32(pathsOff + 12, 5, true);
      }),
    ).toThrow(/group/);
    expect(
      bad((v) => {
        v.setUint32(pathsOff + 20, 7, true);
      }),
    ).toThrow(/style/);
    expect(
      bad((v) => {
        v.setUint32(pathsOff + 4, 99, true);
      }),
    ).toThrow(/verbs out of range/);
    expect(
      bad((v) => {
        v.setUint32(pathsOff + 8, 10, true);
      }),
    ).toThrow(/points out of range/);
    const verbsOff = pathsOff + 3 * 28 + 11 * 8;
    expect(
      bad((v) => {
        v.setUint8(verbsOff, 9);
      }),
    ).toThrow(/unknown verb/);
  });
});
