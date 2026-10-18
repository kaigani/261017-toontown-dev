# ADR-0011 — Canonical JSON, schemas and change sets

Status: accepted (M1 C1.1, 2026-10-18). Spec: 10 §2.6–§2.8, §6.5 (JSON-01…05), §9, §10.4.

## Context

10 §6.5 asks for deterministic, diff-friendly saves; 10 §2.8 for one set of JSON Schemas generated from the same
definitions the application validates with; 10 §2.6 for change sets that can be inverted. Each leaves details open.

## Decision

1. **Number form (JSON-03).** Numbers are written exactly as ECMAScript `Number.prototype.toString` (= `JSON.stringify`)
   writes them: shortest round-trip digits, `-0` as `0`, no fraction on integral values, exponent form outside
   1e-7 ≤ |x| < 1e21. Integers held as `i64`/`u64` are exact at any magnitude. NaN and infinities are rejected. The
   Rust writer (`tt_model::canonical`) and `JSON.stringify` agree on 2 000 shared vectors
   (`crates/model/tests/golden/numbers.json`, checked by Rust and by Vitest). Integral doubles on input become
   integers (JSON has one number type). `serde_json` is built with `float_roundtrip`, without which parsing can be
   off by one ulp and canonical text would drift (found by the property tests).
2. **Layout (JSON-01, JSON-03).** Two-space indentation, `\n`, final newline, UTF-8 without BOM; only `"`, `\` and
   control characters are escaped. An array of 1–8 scalars is written on one line (`[x, y, inX, inY, outX, outY]`,
   `["dr_…", 6]`, matrices); everything else one element per line.
3. **Key order (JSON-02)** is schema order = struct declaration order (`serde_json` with `preserve_order`), envelope
   first and `meta`, `ext` last. Map-valued properties are written in a fixed order: swatch values in style order
   (normalised when a palette is parsed), frame-keyed maps numerically, channel maps by channel id; a scene's
   `paletteStyles` keeps its stored order (part of the document state, so equal documents write equal text).
4. **Omissions (JSON-04).** Only properties marked `x-omitDefault` may be omitted when default: `meta`, `ext`, an
   element's `note` and `cellNotes`, mouth-track `labels`, an unnamed drawing node's `name`, a texture value's
   informative `width`/`height`, an error's `details`.
5. **Schemas (10 §2.8).** Generated with `schemars` (draft 2020-12) from the model types and completed by one
   deterministic pass: `required` = every property not marked `x-omitDefault`/`x-kindSpecific`; kind-specific
   properties are absent, never `null`; one `if/then` rule per element type, drawing-node kind, stroke kind,
   channel kind and library-item kind; integer bounds of the Rust type (32-bit `int` unless stated; revisions, sizes
   and seeds 64-bit); per-entity name patterns and lengths (10 §1.3); child objects accept full objects or stubs
   (10 §2.2); query-only properties are declared with `x-derived`/`x-transient`; every object schema carries
   `x-since` and `x-objectType`; `x-unit`, `x-command` (checked against `spec/A_command_registry.md` by a test) and
   `x-ref` annotate fields. Validation uses the `jsonschema` crate pinned at 0.18.3 with its `draft202012` feature
   (later releases depend on an MIT-0 crate outside the ADR-0003 allow-list; open question in the C1.1 handoff).
6. **Publication.** `crates/model/schema/model-1.0.schema.json` (all definitions, kept current by a test),
   `tt_model::schema::object_schema(type)` (what `doc.schema.get` returns) and the generated front-end module
   `desktop/src/ipc/generated/model.ts` (ADR-0012).
7. **Change sets (10 §2.6).** Each addressable object is diffed by its **own view**: its JSON without `rev`, each
   child object reduced to `{id, type}`. A child's property change therefore never touches the parent (REV-02), while
   membership and order of a child collection produce `add`/`remove`/`move` operations on the parent. Every
   operation carries `oldValue`; `invert` reverses a patch exactly. Exposure and mouth-track edits use one range
   operation: `{op: "replace", path, range: {from, to}, value, oldValue}` where frames from `from` covered by
   `oldValue` are replaced by `value` and `to = from + max(|old|, |new|) − 1`. Entries shared by two snapshots (same
   `Arc`) are skipped, so a diff costs what changed. Values over 64 KiB are elided in events (`elide`).

## Consequences

- Saving the same document twice is byte-identical, verified by golden files of a sample project that uses every
  object and element type, and by property tests over generated entities.
- Front end and engine produce identical number text, so canonical JSON can be produced on either side.
- Upgrading `jsonschema` needs the MIT-0 licence approved first (or the dependency to change).
