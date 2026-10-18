# ADR-0012 — Lossless IPC wire transport and generated front-end schemas

Status: accepted (M1 C1.1, 2026-10-18). Readiness bullet 1 of `reviews/M0-C0.4-r6-closure.md`; closes C01-R3-04.

## Context

M0's IPC boundary (ADR-0001) validated every host value against hand-written schemas, with own-property,
dense-array, depth and snapshot guarantees. Two gaps were accepted for M1: (a) values crossed as JS objects produced
by `JSON.parse`, so a 64-bit integer above 2^53 − 1 (a revision, a byte size, a seed) was silently rounded;
(b) a transparent `Proxy` could not be told apart from plain data on the Safari 15.0 floor (no `structuredClone`),
so `certify` copied it instead of rejecting it (C01-R3-04, three reviewer cases marked `it.fails`).

## Decision

1. **JSON text both ways.** Product commands take one argument `{json: <text>}` and answer with a UTF-8 JSON body
   (`tauri::ipc::Response`, an `ArrayBuffer` in the webview); errors reject with the JSON text of `ErrorInfo`; events
   (`menu-command`) carry JSON text. The host writes with `tt_engine::wire::to_wire_json` (canonical number form,
   exact integers, ADR-0011) and reads with `from_wire_json`.
2. **Lossless parser.** `desktop/src/ipc/wireJson.ts` parses RFC 8259 strictly (duplicate keys, trailing data,
   invalid escapes and depth > 64 are errors), builds every object and array itself (own enumerable data properties,
   defined not assigned, dense arrays — plain data by construction) and returns integers beyond ±(2^53 − 1) as
   `bigint`. Because the canonical writer prints doubles below 1e21 as digits, `coerceNumbers` converts such a
   `bigint` back to `number` wherever the schema says the value is a double.
3. **One representation per value.** The validator accepts a `number` for an integer schema only when it is a safe
   integer, and a `bigint` only beyond the safe range. `plainDataViolations` keeps rejecting `bigint`, so only the
   parser — never an object handed over by a transport — can produce one. Generated TypeScript types are
   `number | bigint` exactly for integers whose range exceeds the safe range.
4. **Proxy rejection on the floor.** `certify` runs the HTML structured-clone algorithm through
   `MessageChannel.postMessage` (available since Safari 5.1; serialisation is synchronous; both ports are closed so
   nothing is delivered). The algorithm refuses every Proxy (an exotic object), so transparent proxies, nested or not,
   are rejected on every supported runtime; without `MessageChannel` the probe fails closed. C01-R3-04's three cases
   now pass and are unmarked, bodies unchanged.
5. **Generated schemas and types.** `crates/engine/tests/frontend_schemas.rs` regenerates
   `desktop/src/ipc/generated/wire.ts` (engine wire types: `EngineInfo`, `ErrorInfo`, `PingRequest`, `PingResponse`,
   `MenuCommandEvent`) and `model.ts` (all object-model definitions) from the Rust types and fails on drift. The
   validator supports the draft 2020-12 keywords these use, `$ref` through a definitions table and `bigint` bounds.
   `wireSchemas.ts` keeps its exported guards (now built on the generated definitions); only the M0 diagnostics plan
   stays hand-written (not product API). `types.ts` spells `EngineInfo` out for the C0.1 regression and proves at
   compile time that it equals the generated type.
6. **Object path kept.** In-process transports (the mock, test doubles) may still hand over objects; those go
   through `certify` (plain data, clone probe, snapshot, validation). The mock answers with objects so the permanent
   tamper tests keep working; text-path tests use their own transports.

## Consequences

- Revisions, sizes and seeds reach the front end exactly; UI code must handle `bigint` where types say so.
- The front end can validate any object-model JSON with the same schemas the engine uses.
- The accepted runtime-string-assembly limitation of ADR-0001 is unchanged; the parser decodes string tokens with
  `JSON.parse` on the token, not with character-code assembly (which the boundary lint forbids).
- Workers stay banned (C01-R4-02 belongs to C1.2).
