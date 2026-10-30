# Migration fixtures (10 §6.6 VER-03/VER-04)

One folder per step of the product chain `tt_persist::migrate::CHAIN`, named `<from>-<to>` (e.g. `1.0-1.1`):

- `before.json`: the JSON entries a step reads, as `{ "<entry path>": <entry JSON>, … }`;
- `after.json`: the entries the step must produce.

`crates/persist/tests/migrations.rs` checks three things:
- every product step has a folder;
- every folder here names a product step;
- each step turns `before.json` into `after.json` exactly.

Format 1.0 is the first version. The product chain is therefore empty, and there is no product folder yet.

`synthetic/` holds the fixtures of the test-only chain `0.9 → 1.0`, which renames the project's `fps` to
`frameRate`. They exercise the same checks and the migration engine. A full open of a migrated project is covered
separately, in `crates/executor/tests/c13_projects.rs`.
