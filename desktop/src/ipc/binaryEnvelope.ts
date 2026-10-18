// Envelopes of the binary host responses of the C0.3 canvas spike (M0 spike; not product API).
//
// A binary body cannot be certified by the JSON-schema validator directly, so the boundary does the next best
// thing: it decodes the fixed-size header into a plain object, certifies that object against its schema, and checks
// that the byte length the header implies is exactly the length received. Section contents (paths, points, verbs)
// are decoded and range-checked by the page's decoder, which never reads outside the certified length.
import { HostError } from './hostApi';
import { certify } from './schema';
import { isPixelFrameHeader, isRenderListHeader } from './spikeSchemas';
import type { CanvasSceneKind, PixelFrameHeader, RenderListHeader } from './types';

/** `TTRL` read as a little-endian u32. */
export const RENDER_LIST_MAGIC = 0x4c525454;
export const RENDER_LIST_HEADER_BYTES = 72;
const GROUP_BYTES = 24;
const PATH_BYTES = 28;
const KINDS: readonly CanvasSceneKind[] = ['paths100k', 'groups1k', 'brush10k'];

/** `TTPX` read as a little-endian u32. */
export const PIXEL_FRAME_MAGIC = 0x58505454;
export const PIXEL_FRAME_HEADER_BYTES = 16;

function invalid(message: string): HostError {
  return new HostError({ code: 'ipc.invalidResponse', message });
}

function magic(view: DataView): number {
  return view.getUint32(0, true);
}

function u64(view: DataView, offset: number): number {
  const v = view.getBigUint64(offset, true);
  if (v > BigInt(Number.MAX_SAFE_INTEGER)) throw invalid('64-bit field exceeds the safe integer range');
  return Number(v);
}

/** Expected byte length of a render list with these counts (renderlist.rs `encoded_len`). */
export function renderListLength(groups: number, paths: number, points: number, verbs: number): number {
  return (
    RENDER_LIST_HEADER_BYTES +
    GROUP_BYTES * groups +
    PATH_BYTES * paths +
    8 * points +
    Math.ceil(verbs / 4) * 4
  );
}

/** Decodes and certifies the header of a `TTRL` v1 render list; throws `ipc.invalidResponse` otherwise. */
export function certifyRenderList(bytes: ArrayBuffer): RenderListHeader {
  if (bytes.byteLength < RENDER_LIST_HEADER_BYTES) throw invalid('render list shorter than its header');
  const v = new DataView(bytes);
  if (magic(v) !== RENDER_LIST_MAGIC) throw invalid('render list magic is not TTRL');
  const kindCode = v.getUint32(8, true);
  const header = {
    version: v.getUint16(4, true),
    headerLength: v.getUint16(6, true),
    kind: KINDS[kindCode] ?? `unknown(${String(kindCode)})`,
    seed: v.getUint32(12, true),
    frame: v.getUint32(16, true),
    groups: v.getUint32(20, true),
    paths: v.getUint32(24, true),
    verbs: v.getUint32(28, true),
    points: v.getUint32(32, true),
    bounds: [v.getFloat32(36, true), v.getFloat32(40, true), v.getFloat32(44, true), v.getFloat32(48, true)],
    flags: v.getUint32(52, true),
    encodeNs: u64(v, 56),
    generateNs: u64(v, 64),
    byteLength: bytes.byteLength,
  };
  const certified = certify(isRenderListHeader, header);
  if (!certified) throw invalid('render list header does not match its schema');
  const [minX = Number.NaN, minY = Number.NaN, maxX = Number.NaN, maxY = Number.NaN] = certified.bounds;
  if (certified.bounds.length !== 4 || !(minX <= maxX && minY <= maxY)) {
    throw invalid('render list bounds are not ordered [min x, min y, max x, max y]');
  }
  const expected = renderListLength(certified.groups, certified.paths, certified.points, certified.verbs);
  if (expected !== bytes.byteLength) {
    throw invalid(`render list is ${String(bytes.byteLength)} bytes; its counts imply ${String(expected)}`);
  }
  return certified;
}

/** Decodes and certifies the header of a `TTPX` pixel frame. */
export function certifyPixelFrame(bytes: ArrayBuffer): PixelFrameHeader {
  if (bytes.byteLength < PIXEL_FRAME_HEADER_BYTES) throw invalid('pixel frame shorter than its header');
  const v = new DataView(bytes);
  if (magic(v) !== PIXEL_FRAME_MAGIC) throw invalid('pixel frame magic is not TTPX');
  const header = {
    width: v.getUint32(4, true),
    height: v.getUint32(8, true),
    fillNs: v.getUint32(12, true),
    byteLength: bytes.byteLength,
  };
  const certified = certify(isPixelFrameHeader, header);
  if (!certified) throw invalid('pixel frame header does not match its schema');
  if (PIXEL_FRAME_HEADER_BYTES + 4 * certified.width * certified.height !== bytes.byteLength) {
    throw invalid('pixel frame length does not match width × height × 4');
  }
  return certified;
}
