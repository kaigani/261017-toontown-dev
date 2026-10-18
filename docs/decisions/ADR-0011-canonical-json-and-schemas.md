# ADR-0011 — Canonical JSON, schemas and change sets

Status: accepted (M1 C1.1, 2026-10-18); amended after review C1.1 round 1 (2026-10-18: decisions 1, 3, 5, 7, 8) and round 2
(2026-10-18: decisions 5, 7, 8).
Spec: 10 §2.6–§2.8, §6.5 (JSON-01…05), §9, §10.4.

## Context

10 §6.5 asks for deterministic, diff-friendly saves; 10 §2.8 for one set of JSON Schemas generated from the same
definitions the application validates with; 10 §2.6 for change sets that can be inverted. Each leaves details open.

## Decision

1. **Number form (JSON-03).** Numbers are written exactly as ECMAScript `Number.prototype.toString` (= `JSON.stringify`)
   writes them: shortest round-trip digits, `-0` as `0`, no fraction on integral values, exponent form outside
   1e-7 ≤ |x| < 1e21. Integers held as `i64`/`u64` are exact at any magnitude. One exception (C11-R1-07): an
   integral double with 2^53 ≤ |x| < 1e21 is written as its **exact** integer digits (2^64 as
   `18446744073709551616`, where ECMAScript writes `18446744073709552000`). Both forms are equally short and round-
   trip to the same double (JSON-03 is met), but every reader of these texts takes an integer literal as an exact
   integer, and the shortest digits name a different one. NaN and infinities are rejected on **every** serialisation
   path: `canonical::to_value` checks floats while serialising, because `serde_json` would silently write them as
   `null` (C11-R1-16); `object::to_json`, `to_canonical` and `wire::to_wire_json` all use it. The Rust writer and the
   front end's `stringifyWireJson` agree on 2 000 shared vectors (`crates/model/tests/golden/numbers.json`, checked
   by Rust and by Vitest). Integral doubles on input become integers (JSON has one number type). `serde_json` is
   built with `float_roundtrip`, without which parsing can be off by one ulp and canonical text would drift (found
   by the property tests).
2. **Layout (JSON-01, JSON-03).** Two-space indentation, `\n`, final newline, UTF-8 without BOM; only `"`, `\` and
   control characters are escaped. An array of 1–8 scalars is written on one line (`[x, y, inX, inY, outX, outY]`,
   `["dr_…", 6]`, matrices); everything else one element per line.
3. **Key order (JSON-02)** is schema order = struct declaration order (`serde_json` with `preserve_order`), envelope
   first and `meta`, `ext` last. The keys of maps — `paletteStyles`, swatch `values`, element `channels`, effect
   `params` of no declared shape, and every object inside opaque `meta`/`ext`/`importOptions` values — are written in
   lexicographic (code point) order, recursively; frame-keyed maps numerically. The order is a pure function of
   content (10 AG-06, JSON-05; C11-R1-06): it is established by the input pass of every validated object
   (`tt_model::input`, ADR-0010 §15) and `Document::from_parts` revalidates every entry, so two equal documents write
   equal bytes whatever order their maps were built in. (Round 1 replaced the earlier rule that a stored map order
   is document state; spec 10 §4.3 defines a map. JSON-02 does not state map-key order; SI-014.)
4. **Omissions (JSON-04).** Only properties marked `x-omitDefault` may be omitted when default: `meta`, `ext`, an
   element's `note` and `cellNotes`, mouth-track `labels`, an unnamed drawing node's `name`, a texture value's
   informative `width`/`height`, an error's `details`.
5. **Schemas (10 §2.8).** Generated with `schemars` (draft 2020-12) from the model types and completed by one
   deterministic pass: `required` = every property not marked `x-omitDefault`/`x-kindSpecific`; kind-specific
   properties are absent, never `null`; one `if/then` rule per element type, drawing-node kind, stroke kind,
   channel kind and library-item kind; integer bounds as the spec states them (C11-R1-14): `int` is signed 32-bit
   (10 §1.1), so frames, durations, counts and loops are 0/1…2^31−1 even where the Rust field is `u32`; the stated
   exceptions are revisions and byte sizes (64-bit) and the particle seed (0–2^32−1, 06 §3.8.3); per-entity name
   patterns and lengths (10 §1.3). **Full object or stub** (10 §2.2 "Both forms validate", C11-R1-11): every object
   schema is `if {required: [rev]} then {full requirements and kind rules} else {only id/colourId, type, name}`, with
   its constant `type`, so a child slot accepts exactly its own type's full objects and stubs and a depth-0 result
   validates. Query-only properties are declared with `x-derived`/`x-transient`; every object schema carries
   `x-objectType` and **every property** carries `x-since` (C11-R1-13); the envelope `type` is `readOnly`; `x-unit`,
   `x-command` (checked against `spec/A_command_registry.md` by a test) and `x-ref` annotate fields.
   `schema::property_schema` resolves any JSON Pointer through `$ref`s, alternatives, conditional branches, tuple
   positions, array items, map and pattern-map values (C11-R1-19). It keeps every candidate schema down the path,
   so a field declared only in a kind rule (an effect's typed `params`) is found, and merges the final candidates
   with a declaration's own annotations before the referenced definition's keywords, so `readOnly`/`x-since`
   beside a `$ref` survive (C11-R2-06). Every property carries `x-since`, including those added after generation
   (`MouthTrack.stale`; the pass runs last, C11-R2-11). Frame-keyed maps (`cellNotes`, mouth-track `labels`)
   accept only keys 1–2 147 483 647 (`schema::FRAME_KEY_PATTERN`, 10 §1.1, §4.7, §4.14; C11-R2-08). Validation uses the `jsonschema` crate **0.59** (draft
   2020-12, default features off); MIT-0, which its dependency tree needs, was accepted on 2026-10-18 (ADR-0003,
   C11-R1-18). The upgrade fixes the reproduced integer `multipleOf` rounding of 0.18.3.
6. **Publication.** `crates/model/schema/model-1.0.schema.json` (all definitions, kept current by a test),
   `tt_model::schema::object_schema(type)` (what `doc.schema.get` returns) and the generated front-end module
   `desktop/src/ipc/generated/model.ts` (ADR-0012).
7. **Change sets (10 §2.6).** Each addressable object is diffed by its **own view**: its JSON without `rev`, each
   child object reduced to `{id, type}`. A child's property change therefore never touches the parent (REV-02), while
   membership and order of a child collection produce `add`/`remove`/`move` operations on the parent — only at the
   type's declared child-list slots; any other array, in particular inside opaque `meta`/`ext`, is an ordinary value
   (C11-R1-03). Every operation carries `oldValue`; an `add`, `move` or `copy` that overwrites an object member
   carries the overwritten value as `oldValue` and `apply` checks it (C11-R1-04). **Inversion is computed against
   the pre-state** (C11-R2-02): `change::apply_with_inverse` / `invert_against` apply each operation and record its
   inverse against the state that operation saw, with concrete paths only — an append `…/-` inverts to the index it
   created, a `move`'s destination is resolved after its source was removed and undone before the source is
   restored — and `oldValue` on every inverse operation, so inverses are themselves invertible. A property test
   checks `apply(after, invert_against(before, p)) = before` and the double inverse over random RFC 6902 sequences
   (appends, overwrites, nested-array moves, copies, tests) on random documents. Stateless `invert` remains for
   resolved patches (what `diff_json` and `invert_against` produce); for an append or a move into the source's
   ancestor it returns an operation `apply` refuses, never a guess. `apply` refuses an operation whose value is
   missing or elided (an elided value is unknown, never `null`: the consumer rereads), a `move`/`copy` without
   `from` (RFC 6902 §4.4/§4.5; C11-R2-04), and a malformed range replace: `value`/`oldValue` of the target's form
   (mouth letters A–G, X for a mouth string; runs for exposure), `from ≥ 1`, `to` consistent with the values, and
   `oldValue` equal to the window's current content (C11-R2-03). It supports RFC 6902 root `add`/`replace`;
   whether `doc.object.patch` may replace a whole object is a command decision (C1.2). Exposure and mouth-track
   edits use one range
   operation: `{op: "replace", path, range: {from, to}, value, oldValue}` where frames from `from` covered by
   `oldValue` are replaced by `value` and `to = from + max(|old|, |new|) − 1`. Entries shared by two snapshots (same
   `Arc`) are skipped, so a diff costs what changed. Values over 64 KiB are elided in events (`elide`).

8. **Text reading (C11-R1-07, R1-17).** JSON text from outside is read by `tt_model::json_text` before `serde_json`:
   RFC 8259 only, unique keys after unescaping, no unpaired surrogates, nesting ≤ 128, and an integer literal must
   fit `i64`/`u64` or be exactly a double — otherwise `arg.invalid`, never a rounded value. The front end's
   `parseWireJson` applies the same rules; both run `tests/fixtures/wire/hostile-json.json` (ADR-0012).

## Consequences

- Saving the same document twice is byte-identical, verified by golden files of a sample project that uses every
  object and element type, and by property tests over generated entities; equal documents built in different orders
  are byte-identical too.
- Front end and engine produce identical number text, so canonical JSON can be produced on either side.
- Nesting limits nest: objects ≤ 100 levels (model input); text ≤ 128 for both readers **and** both writers, all
  counting the root value as depth 1 and scalars included, so anything written can be read back (C11-R1-09,
  C11-R2-07). `canonical::to_value` (serialisation to a JSON value) checks nesting separately and is not a text
  writer.
