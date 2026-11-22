// The interactive canvas's render-list product format, TTRD 1.0: the decoder (M4 C4.2b; ADR-0007 decision 4). The
// layout, the validation rules V1-V10 and the order they are applied in, the version rule and the numeric rules are
// `docs/canvas-format.md` (ruling D-18), the single source this decoder and Rust's `tt_render::canvas` follow.
// Both are held to one golden corpus (`testdata/`): `cargo test` encodes it byte for byte and `format.test.ts` decodes
// it, including one corrupt file per validation rule (V1 to V10), each refused before any geometry is built.
//
// Vertex data stays a view into the payload (12-byte interleaved vertices: x, y as f32, slot and flags as u16), so the
// back ends upload it as it is; the decoder still reads every vertex and index once to validate it (V6, V7, V9).

export const MAGIC = 'TTRD';
export const MAJOR = 1;
export const HEADER_LEN = 80;
export const VERTEX_LEN = 12;
export const KIND_SOLID = 1;
export const KIND_GRADIENT = 2;
export const KIND_BITMAP = 3;
/** Vertex flag: the vertex belongs to a stroke (a pencil line). */
export const VERTEX_STROKE = 1;

export type Rule = 'V1' | 'V2' | 'V3' | 'V4' | 'V5' | 'V6' | 'V7' | 'V8' | 'V9' | 'V10';

/** A refusal naming the validation rule of design note 1 §8. */
export class FormatError extends Error {
  readonly rule: Rule;
  constructor(rule: Rule, message: string) {
    super(`${rule}: ${message}`);
    this.rule = rule;
    this.name = 'FormatError';
  }
}

export type SlotKind = 'solid' | 'gradient' | 'texture';
export type Rgba = [number, number, number, number];

export interface Slot {
  id: string;
  kind: SlotKind;
  rgba: Rgba;
}

/** A mesh: `vertexCount` interleaved vertices (a view into the payload) and triangle indices. */
export interface Mesh {
  vertexCount: number;
  /** `vertexCount * VERTEX_LEN` bytes: x f32, y f32, slot u16, flags u16. */
  vertices: Uint8Array;
  indices: Uint32Array;
}

export type DecodedRecord =
  | ({ kind: 'solid' } & Mesh)
  | ({
      kind: 'gradient';
      radial: boolean;
      /** Drawing → unit space, `[a, b, c, d, e, f]`. */
      drawingToUnit: number[];
      stops: [number, Rgba][];
    } & Mesh)
  | {
      kind: 'bitmap';
      width: number;
      height: number;
      /** Top-left, top-right, bottom-right, bottom-left of the image, in drawing units. */
      quad: number[];
      imageId: string;
      slot: number;
    };

export interface DecodedDrawing {
  generation: bigint;
  rev: bigint;
  /** The 26-character ULID; the drawing id is `dr_` + this. */
  drawingUlid: string;
  bucket: number;
  flags: number;
  /** Min x, min y, max x, max y in drawing units. */
  bounds: [number, number, number, number];
  slots: Slot[];
  records: DecodedRecord[];
  /** Payload bytes (the resident-memory account of design note 1 §5). */
  byteLength: number;
}

const ULID = /^[0-9A-HJKMNP-TV-Z]{26}$/;
const SLOT_KINDS: SlotKind[] = ['solid', 'gradient', 'texture'];
const utf8 = new TextDecoder('utf-8');

const padded = (n: number) => Math.ceil(n / 4) * 4;

class Reader {
  at: number;
  constructor(
    readonly view: DataView,
    at: number,
    readonly end: number,
    readonly overrun: Rule,
  ) {
    this.at = at;
  }
  need(n: number, what: string): number {
    const start = this.at;
    if (n < 0 || start + n > this.end) {
      throw new FormatError(
        this.overrun,
        `${what} at byte ${start} needs ${n} bytes; the section ends at ${this.end}`,
      );
    }
    this.at = start + n;
    return start;
  }
  u8(w: string) {
    return this.view.getUint8(this.need(1, w));
  }
  u16(w: string) {
    return this.view.getUint16(this.need(2, w), true);
  }
  i16(w: string) {
    return this.view.getInt16(this.need(2, w), true);
  }
  u32(w: string) {
    return this.view.getUint32(this.need(4, w), true);
  }
  u64(w: string) {
    return this.view.getBigUint64(this.need(8, w), true);
  }
  f32(w: string) {
    const v = this.view.getFloat32(this.need(4, w), true);
    if (!Number.isFinite(v)) throw new FormatError('V9', `${w}: a non-finite number`);
    return v;
  }
  bytes(n: number, w: string) {
    const at = this.need(n, w);
    return new Uint8Array(this.view.buffer, this.view.byteOffset + at, n);
  }
  rgba(w: string): Rgba {
    const b = this.bytes(4, w);
    return [b[0] ?? 0, b[1] ?? 0, b[2] ?? 0, b[3] ?? 0];
  }
  pad(n: number, w: string) {
    this.need(padded(n) - n, w);
  }
}

/** Decodes and validates one payload; throws [`FormatError`] naming the first rule it breaks. */
export function decodeDrawing(payload: ArrayBuffer | Uint8Array): DecodedDrawing {
  const bytes = payload instanceof Uint8Array ? payload : new Uint8Array(payload);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const len = bytes.byteLength;
  if (len < 4 || utf8.decode(bytes.subarray(0, 4)) !== MAGIC)
    throw new FormatError('V1', 'not a TTRD payload (magic)');
  if (len < 8) throw new FormatError('V3', `a payload of ${len} bytes is shorter than the header`);
  const major = view.getUint16(4, true);
  if (major !== MAJOR) throw new FormatError('V2', `major version ${major}; this decoder reads ${MAJOR}`);
  if (len < HEADER_LEN)
    throw new FormatError('V3', `a payload of ${len} bytes is shorter than the ${HEADER_LEN}-byte header`);
  const headerLen = view.getUint16(8, true);
  if (headerLen !== HEADER_LEN)
    throw new FormatError('V3', `header length ${headerLen}; 1.x headers are ${HEADER_LEN} bytes`);
  const r = new Reader(view, 10, len, 'V5');
  const bucket = r.i16('bucket');
  const slotCount = r.u32('slot count');
  const generation = r.u64('generation');
  const rev = r.u64('revision');
  const drawingUlid = utf8.decode(r.bytes(26, 'drawing id'));
  if (!ULID.test(drawingUlid))
    throw new FormatError('V10', 'the drawing id is not 26 Crockford base-32 characters');
  const flags = r.u16('flags');
  const recordCount = r.u32('record count');
  const bounds: [number, number, number, number] = [
    r.f32('bounds'),
    r.f32('bounds'),
    r.f32('bounds'),
    r.f32('bounds'),
  ];
  const slots: Slot[] = [];
  for (let i = 0; i < slotCount; i++) {
    const idLen = r.u16('colour id length');
    const kind = SLOT_KINDS[r.u8('slot kind')];
    if (kind === undefined) throw new FormatError('V8', 'an unknown colour slot kind');
    r.u8('reserved');
    const rgba = r.rgba('RGBA');
    const id = utf8.decode(r.bytes(idLen, 'colour id'));
    r.pad(idLen, 'colour id padding');
    slots.push({ id, kind, rgba });
  }
  const slotOk = (s: number) => {
    if (s >= slotCount) throw new FormatError('V7', `colour slot ${s} of ${slotCount}`);
    return s;
  };
  const records: DecodedRecord[] = [];
  for (let n = 0; n < recordCount; n++) {
    const start = r.at;
    const kind = r.u16('record kind');
    r.u16('record flags');
    const recLen = r.u32('record length');
    if (recLen < 8 || start + recLen > len)
      throw new FormatError('V5', `record ${n} of ${recLen} bytes overruns the payload`);
    const rr = new Reader(view, r.at, start + recLen, 'V4');
    const mesh = (): Mesh => {
      const vertexCount = rr.u32('vertex count');
      const indexCount = rr.u32('index count');
      if (vertexCount * VERTEX_LEN + indexCount * 4 !== rr.end - rr.at) {
        throw new FormatError(
          'V4',
          `record ${n}: ${vertexCount} vertices and ${indexCount} indices do not fill its length`,
        );
      }
      if (indexCount % 3 !== 0)
        throw new FormatError('V4', `record ${n}: ${indexCount} indices are not whole triangles`);
      const vAt = rr.at;
      for (let i = 0; i < vertexCount; i++) {
        rr.f32('vertex x');
        rr.f32('vertex y');
        slotOk(rr.u16('vertex slot'));
        rr.u16('vertex flags');
      }
      const iAt = rr.at;
      for (let i = 0; i < indexCount; i++) {
        const ix = rr.u32('index');
        if (ix >= vertexCount)
          throw new FormatError('V6', `record ${n}: index ${ix} of ${vertexCount} vertices`);
      }
      const abs = bytes.byteOffset + iAt;
      const indices =
        abs % 4 === 0
          ? new Uint32Array(bytes.buffer, abs, indexCount)
          : new Uint32Array(bytes.slice(iAt, iAt + indexCount * 4).buffer);
      return { vertexCount, vertices: bytes.subarray(vAt, vAt + vertexCount * VERTEX_LEN), indices };
    };
    if (kind === KIND_SOLID) {
      records.push({ kind: 'solid', ...mesh() });
    } else if (kind === KIND_GRADIENT) {
      const gradientKind = rr.u8('gradient kind');
      if (gradientKind > 1) throw new FormatError('V8', `record ${n}: unknown gradient kind ${gradientKind}`);
      const radial = gradientKind === 1;
      const stopCount = rr.u8('stop count');
      rr.u16('reserved');
      const drawingToUnit: number[] = [];
      for (let i = 0; i < 6; i++) drawingToUnit.push(rr.f32('gradient matrix'));
      const stops: [number, Rgba][] = [];
      for (let i = 0; i < stopCount; i++) stops.push([rr.f32('stop position'), rr.rgba('stop RGBA')]);
      records.push({ kind: 'gradient', radial, drawingToUnit, stops, ...mesh() });
    } else if (kind === KIND_BITMAP) {
      const width = rr.u16('bitmap width');
      const height = rr.u16('bitmap height');
      const quad: number[] = [];
      for (let i = 0; i < 8; i++) quad.push(rr.f32('bitmap quad'));
      const idLen = rr.u16('image id length');
      const slot = slotOk(rr.u16('bitmap slot'));
      const imageId = utf8.decode(rr.bytes(idLen, 'image id'));
      rr.pad(idLen, 'image id padding');
      rr.u32('reserved');
      if (rr.at !== rr.end)
        throw new FormatError('V4', `record ${n}: a bitmap reference of the wrong length`);
      records.push({ kind: 'bitmap', width, height, quad, imageId, slot });
    } else {
      throw new FormatError('V8', `record ${n}: unknown kind ${kind}`);
    }
    r.at = start + recLen;
  }
  if (r.at !== len) throw new FormatError('V5', `${len - r.at} bytes after the last record`);
  return { generation, rev, drawingUlid, bucket, flags, bounds, slots, records, byteLength: len };
}

/** One vertex of a mesh: `[x, y, slot, flags]`. */
export function vertexAt(m: Mesh, i: number): [number, number, number, number] {
  const v = new DataView(m.vertices.buffer, m.vertices.byteOffset + i * VERTEX_LEN, VERTEX_LEN);
  return [v.getFloat32(0, true), v.getFloat32(4, true), v.getUint16(8, true), v.getUint16(10, true)];
}
