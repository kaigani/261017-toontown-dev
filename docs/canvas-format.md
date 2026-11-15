# TTRD 1.0: the interactive canvas's render-list format

Status: normative (ruling D-18, M4 C4.2b). This document is the single source for the byte layout, the validation
rules, the version rule and the numeric rules. The encoder (`crates/render/src/canvas.rs`, `encode`), the decoders
(`desktop/src/canvas/format.ts`, `decodeDrawing`; `crates/render/src/canvas.rs`, `decode`, used by the tests) and the
transport note (`reviews/M4-design-render-transport.md` §2, §8) cite it; where they differ from it, this document holds
and the difference is a defect.

One payload is one drawing at one revision, tessellated for one zoom bucket of one view (ADR-0007 decisions 1, 2 and
4). It travels as the binary answer of the host command `canvas_load` (transport note §9).

## 1. Conventions

- **Byte order:** every multi-byte integer and float is little endian.
- **Offsets** are from the first byte of the payload.
- **Types:** `u8`, `u16`, `u32`, `u64` unsigned integers; `i16` a two's-complement signed integer; `f32` an IEEE 754
  binary32 float. `bytes[n]` is `n` raw bytes.
- **Alignment:** the header is 80 bytes; every colour-table entry and every record is a multiple of 4 bytes long, so
  each section and record starts on a 4-byte boundary. Text (ids) is followed by zero bytes up to the next multiple of
  4 ("padded to 4"); a text of a length already a multiple of 4 has no padding.
- **Reserved** fields and bits are written as zero by the encoder and ignored by a reader (never refused), so a later
  minor version may give them a meaning (§7).
- **Padding** bytes are written as zero and are not checked by a reader.
- **Text** (colour ids, image ids) is UTF-8 as written by the encoder. Readers do not validate it; a reader that meets
  invalid UTF-8 decodes it with replacement characters. The drawing id is ASCII and is validated (V10).

## 2. Header (80 bytes)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0 | 4 | `bytes[4]` | magic | the ASCII bytes `TTRD` (`54 54 52 44`) |
| 4 | 2 | `u16` | major | 1 |
| 6 | 2 | `u16` | minor | 0 (§7: any minor is read by a 1.x reader) |
| 8 | 2 | `u16` | header length | 80 |
| 10 | 2 | `i16` | zoom bucket | log2 of the bucket's top zoom (§8.4); the host asks for buckets -8 to 8 |
| 12 | 4 | `u32` | colour slot count | the number of colour-table entries (§3) |
| 16 | 8 | `u64` | engine generation | the engine generation the payload was built on (0 at start, +1 per restart; D-7b) |
| 24 | 8 | `u64` | drawing revision | the drawing's revision (10 REV-02) the payload shows |
| 32 | 26 | `bytes[26]` | drawing id | the drawing's ULID: 26 characters of the Crockford base-32 alphabet `0-9 A-H J K M N P-T V-Z` (upper case); the drawing's id is `dr_` followed by these characters |
| 58 | 2 | `u16` | flags | bit 0 (1): the outline-only form (02 §10.2); bit 1 (2): approximate pencil strokes, round caps without round joins (`display.fastPencilRendering`, 01 §7.2); bits 2-15 reserved |
| 60 | 4 | `u32` | record count | the number of records (§4) |
| 64 | 16 | 4 × `f32` | bounds | min x, min y, max x, max y of every vertex and bitmap corner, in drawing units; `0, 0, 0, 0` when there are no records |

The colour table follows at offset 80.

## 3. Colour table

`colour slot count` entries, one per colour id the drawing uses, in slot order (slot 0 first). Each entry:

| Offset in entry | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0 | 2 | `u16` | id length `L` | the colour id's length in bytes |
| 2 | 1 | `u8` | kind | 0 solid, 1 gradient, 2 texture or bitmap; any other value is refused (V8) |
| 3 | 1 | `u8` | reserved | |
| 4 | 4 | 4 × `u8` | RGBA | straight (not premultiplied) 8-bit red, green, blue, alpha: a solid colour's value; a gradient's first stop; a texture's or bitmap's mean colour (SI-137) |
| 8 | `L` | `bytes[L]` | colour id | the palette colour's id, UTF-8 |
| 8 + `L` | 0-3 | | padding | to 4 |

An entry is `8 + L` bytes rounded up to a multiple of 4. A vertex or bitmap names its colour by slot number (0 to
`colour slot count` − 1), so a change of a solid colour's value changes the colour table only, not the geometry (06
§6.3).

## 4. Records

`record count` records follow the colour table, in painter order: the first record is drawn first (back), the last on
top (06 §6.1, the final renderer's walk). Each record starts with an 8-byte record header:

| Offset in record | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0 | 2 | `u16` | kind | 1 solid mesh, 2 gradient mesh, 3 bitmap reference; any other value is refused (V8) |
| 2 | 2 | `u16` | record flags | reserved |
| 4 | 4 | `u32` | record length | the whole record's length in bytes, its header included (at least 8) |

The record body follows at offset 8. The next record starts at the record's first byte plus its record length.

### 4.1 Mesh body (used by kinds 1 and 2)

| Offset in mesh body | Size | Type | Field |
|---|---|---|---|
| 0 | 4 | `u32` | vertex count `V` |
| 4 | 4 | `u32` | index count `I` |
| 8 | 12 × `V` | | vertices, each 12 bytes: `x` (`f32`), `y` (`f32`), slot (`u16`), vertex flags (`u16`) |
| 8 + 12 `V` | 4 × `I` | `u32` each | indices |

- `x, y` are in drawing units (02 §2: y up).
- The slot is the vertex's colour slot (§3). All vertices of one triangle carry the same slot.
- Vertex flags: bit 0 (1) the vertex belongs to a stroke (a pencil line); bits 1-15 reserved.
- Indices name vertices of this record (0 to `V` − 1); every three consecutive indices are one triangle. `I` is a
  multiple of 3. Triangle winding carries no meaning (the meshes are drawn without face culling).
- The mesh body fills the rest of its record exactly: `8 + 12 V + 4 I` bytes.

### 4.2 Kind 1: solid mesh

Record header, then a mesh body (§4.1). Each triangle is filled with its slot's RGBA.

Record length = 8 + 8 + 12 `V` + 4 `I`.

### 4.3 Kind 2: gradient mesh

| Offset in record | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 8 | 1 | `u8` | gradient kind | 0 linear, 1 radial; any other value is refused (V8) |
| 9 | 1 | `u8` | stop count `S` | 0 to 255 |
| 10 | 2 | `u16` | reserved | |
| 12 | 24 | 6 × `f32` | drawing → unit space `a, b, c, d, e, f` | a point `(x, y)` in drawing units maps to `u = a x + c y + e`, `v = b x + d y + f` |
| 36 | 8 × `S` | | stops, each: position (`f32`), RGBA (4 × `u8`, straight) | |
| 36 + 8 `S` | | | mesh body (§4.1) | the area the gradient fills |

Record length = 36 + 8 `S` + 8 + 12 `V` + 4 `I`.

Each fragment of the mesh is coloured per fragment from its unit-space point `(u, v)`: `t = u` for a linear gradient,
`t = 2 · sqrt((u − 0.5)² + (v − 0.5)²)` for a radial one; `t` is clamped to 0-1 and the stops are interpolated
linearly between neighbours in their stored order (before the first stop: the first stop's colour; after the last:
the last's). The vertices' slots name the gradient's colour slot (kind 1). The stored positions are not required to be
sorted or in 0-1; the product writes them as the palette stores them. The back ends evaluate the first 16 stops (a
preview limit; 06 §6.10).

### 4.4 Kind 3: bitmap reference

| Offset in record | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 8 | 2 | `u16` | width | the image's width in pixels |
| 10 | 2 | `u16` | height | the image's height in pixels |
| 12 | 32 | 8 × `f32` | quad | the image's corners in drawing units: top-left `x, y`, top-right, bottom-right, bottom-left |
| 44 | 2 | `u16` | image id length `L` | |
| 46 | 2 | `u16` | slot | the colour slot holding the image's mean colour (§3) |
| 48 | `L` | `bytes[L]` | image id | UTF-8 |
| 48 + `L` | 0-3 | | padding | to 4 |
| 48 + pad4(`L`) | 4 | `u32` | reserved | |

Record length = 52 + pad4(`L`), exactly (V4). Until bitmap pixels are transported, the quad is drawn with its slot's
colour (SI-137).

## 5. Payload length

The payload is exactly the header, the colour table and the records: nothing may follow the last record (V5).

## 6. Numeric rules

- **Floats:** every `f32` field (bounds, vertex coordinates, gradient matrix and stop positions, bitmap corners) is
  finite; NaN and infinities are refused (V9).
- **64-bit integers:** the format has exactly two, the engine generation and the drawing revision (header offsets 16
  and 24). They are identities, compared for equality, never used as numbers:
  - the TypeScript decoder returns them as `bigint` (`DataView.getBigUint64`), Rust as `u64`;
  - the same two identities also travel in the JSON answer of `canvas_scene` (`generation`, and each entry's `rev`):
    the host writes them as JSON integers, the window parses that answer losslessly (`parseWireJson`: an integer
    beyond 2^53 − 1 stays a `bigint`) and validates both as unsigned 64-bit integers (`scene.ts`, `u64`), so the canvas
    compares a payload's generation and revision with the scene's exactly (T1, T6; C42B-R1-20).
  - No other 64-bit value crosses the canvas transport: the scene's `paletteStamp` is masked to 53 bits by the kernel,
    and coordinates, opacities and the view mapping are floating numbers.
- **Counts** (`u32`) are checked against the bytes present without overflow: a count whose bytes would run past its
  section is refused, however large (V4 inside a record, V5 for the header and the colour table).

## 7. Versions

- **Major** (offset 4): a reader reads exactly one major version, 1, and refuses any other (V2). A change of layout,
  of a field's size, offset, type or meaning, of the header length, or of a validation rule is a new major version.
- **Minor** (offset 6): a 1.x reader accepts every minor version and reads the payload as 1.0. A later minor version
  may only give a meaning to what 1.0 reserves (reserved fields, header flag bits 2-15, vertex flag bits 1-15, record
  flags), each with a value of zero meaning 1.0's behaviour; a 1.0 reader ignores them and draws the 1.0 picture.
- **Trailing fields:** there are none in 1.x. The header length is 80 in every 1.x payload (V3), a record's length is
  its kind's exact length (V4 for meshes and bitmap references), and nothing follows the last record (V5); so a reader
  never meets unknown trailing bytes it should skip: any it meets are refused under those rules.

## 8. Validation

A reader validates the whole payload before giving any of it to a renderer, and refuses it with the first rule it
breaks, in the order below (the order fixes which rule a payload that breaks several is refused under).

1. Fewer than 4 bytes, or the first 4 bytes are not `TTRD`: **V1**.
2. Fewer than 8 bytes: **V3**.
3. Major version (offset 4) other than 1: **V2**.
4. Fewer than 80 bytes, or header length (offset 8) other than 80: **V3**.
5. The header's fields are read in offset order. The drawing id (offset 32) is not 26 characters of the alphabet of §2:
   **V10**. A bounds value is not finite: **V9**.
6. The colour table is read entry by entry. An entry, its id or its padding runs past the payload: **V5**. A kind other
   than 0, 1 or 2: **V8**.
7. The records are read one by one. For each:
   1. its 8-byte record header runs past the payload: **V5**;
   2. its record length is less than 8, or the record would end past the payload: **V5**;
   3. its kind is not 1, 2 or 3: **V8**;
   4. inside the record, a field runs past the record's end: **V4**;
   5. gradient mesh: a gradient kind other than 0 or 1: **V8**; a matrix value or stop position not finite: **V9**;
   6. mesh body: `12 V + 4 I` differs from the bytes left in the record after the two counts, or `I` is not a multiple
      of 3: **V4**; then each vertex in order: a coordinate not finite: **V9**; a slot not less than the colour slot
      count: **V7**; then each index in order: not less than `V`: **V6**;
   7. bitmap reference: a corner not finite: **V9**; a slot not less than the colour slot count: **V7**; the record's
      length differs from 52 + pad4(`L`): **V4**.
8. Bytes follow the last record: **V5**.

| Rule | What it refuses |
|---|---|
| V1 | not a TTRD payload (magic) |
| V2 | another major version |
| V3 | a payload shorter than the header, or another header length |
| V4 | a record whose content does not match its length: counts against bytes, a field past the record's end, an index count not a multiple of 3, a bitmap reference of the wrong length |
| V5 | the sections against the payload length: a truncated header field, colour table or record, a record length under 8 or past the payload, bytes after the last record |
| V6 | an index out of range |
| V7 | a colour slot out of range |
| V8 | an unknown colour-slot kind, record kind or gradient kind |
| V9 | a non-finite float |
| V10 | a drawing id that is not 26 Crockford base-32 characters |

## 9. Meaning for a renderer

- Coordinates are drawing units; the view's drawing → view mapping (`draw.viewMapping.get`, `shell.view`) places them.
- **Zoom bucket:** curves are flattened to 0.25 view pixels at the bucket's top zoom, `2^bucket` (SI-137): the
  tolerance in drawing units is `0.25 / 2^bucket`. Under a layer transform, curves and round caps and joins are
  flattened in the item's own coordinates to the tolerance that maps to that one (C42B-R1-01).
- **Painter order inside a record:** triangles are drawn in index order, each over what is already drawn (source-over
  with its slot's colour). One record may hold the triangles of several consecutive items of the walk.
- **Strokes** are already triangulated: the encoder unions a stroke's segment quads and round caps and joins (non-zero)
  before triangulating, so one stroke's triangles do not overlap and a translucent stroke never blends twice (ADR-0007
  decision 7).
- **Outline-only form** (header flag 1): brush outlines and pencil centre lines as strokes one view pixel wide at the
  bucket's top zoom, planar fills left out (02 §10.2's outline style; SI-139); the same layout.

## 10. Golden corpus

`desktop/src/canvas/testdata/`: valid payloads with the JSON of what they decode to, payloads that vary every header
field, and one corrupt payload per rule V1-V10 (`corrupt.json` lists each file and the rule it must be refused
under). `cargo test -p tt-render --test c42_canvas` encodes the valid ones byte for byte and refuses the corrupt ones;
`desktop/src/canvas/format.test.ts` decodes the same files.
