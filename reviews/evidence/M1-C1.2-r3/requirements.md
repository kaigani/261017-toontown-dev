# Round 3 requirements

| Row | Status | Owner | Findings |
|---|---|---|---|
| R01 — Inventory IDs, namespace, kind and verbatim table columns (A) | implemented | M1 C1.2 | — |
| R02 — Every inventory entry has constrained parameter/result schemas (plan 3.2; 11 14.2) | divergent | M1 C1.2 | C12-R3-01 |
| R03 — Every command example executes and matches its result schema (11 14.2) | implemented | M1 C1.2 | — |
| R04 — Every stub names a correct owner (plan 4) | implemented | M1 C1.2 | — |
| R05 — Registry version and generated frontend schema freshness | implemented | M1 C1.2 | — |
| R06 — Attribute macro rejects missing example/duplicate ID/wrong kind | implemented | M1 C1.2 | — |
| R07 — Exclusive mutation capability (plan 3.2; 11 P2) | implemented | M1 C1.2 | — |
| R08 — One serialized document queue (REV-06) | implemented | M1 C1.2 | — |
| R09 — Staged full integrity gate and rollback of document edits | implemented | M1 C1.2 | — |
| R10 — Atomic rollback covers every observable side effect (11 7.3) | implemented | M1 C1.2 | — |
| R11 — Atomic batch one revision, undo step and label | implemented | M1 C1.2 | — |
| R12 — Query steps see prior staged changes (11 7.3) | divergent | M1 C1.2 | C12-R3-04 |
| R13 — Numeric/named intermediate references | implemented | M1 C1.2 | — |
| R14 — Long-running job-kind steps rejected in atomic batches | implemented | M1 C1.2 | — |
| R15 — Ordinary edit dryRun changes/history/revisions | implemented | M1 C1.2 | — |
| R16 — Dry run covers clipboard events and cancellation | implemented | M1 C1.2 | — |
| R17 — Command-parameter dryRun in atomic batch | implemented | M1 C1.2 | — |
| R18 — Sequential dry-run batch simulates preceding edits | implemented | M1 C1.2 | — |
| R19 — Common run_command envelope including job.wait | divergent | M1 C1.2 | C12-R3-03 |
| R20 — Undo/redo canonical content and revision stamps | implemented | M1 C1.2 | — |
| R21 — Failed batch history, redo invalidation, grouped labels | implemented | M1 C1.2 | — |
| R22 — Checkpoint create/list/delete/restore and history independence | implemented | M1 C1.2 | — |
| R23 — Checkpoint created from staged content has coherent rev | divergent | M1 C1.2 | C12-R3-02 |
| R24 — Checkpoint cost at 100k strokes/10k drawings | implemented | M1 C1.2 | — |
| R25 — expectRev scalar and envelope guard | implemented | M1 C1.2 | — |
| R26 — expectRev map checks every listed object (REV-05) | implemented | M1 C1.2 | — |
| R27 — Conflict details.currentRev and current small JSON (REV-05) | implemented | M1 C1.2 | — |
| R28 — requestId retry same ID returns original without execution | divergent | M1 C1.2 | C12-R3-03 |
| R29 — requestId different parameters same ID | implemented | M1 C1.2 | — |
| R30 — requestId persistence across process restart | deferred-with-owner | M1 C1.4 | — |
| R31 — documentRev exactly +1 per edit/undo/redo (REV-01/03) | implemented | M1 C1.2 | — |
| R32 — Revision overflow is controlled error, not thread panic | implemented | M1 C1.2 | — |
| R33 — Drawing rev follows content edits (REV-02) | implemented | M1 C1.2 | — |
| R34 — Every changed revision returned to caller (11 P6) | divergent | M1 C1.2 | C12-R3-04 |
| R35 — Persist revs through save/reopen (REV-04) | deferred-with-owner | M1 C1.3 | — |
| R36 — Change sets oldValue/concrete paths/numeric opaque keys | implemented | M1 C1.2 | — |
| R37 — Historical doc.diff within retained 10k window | divergent | M1 C1.2 | C12-R3-02, C12-R3-04 |
| R38 — doc.diff retains last save even beyond 10k (AG-09) | implemented | M1 C1.2 | — |
| R39 — 500-change envelope cap and oversized event elision | implemented | M1 C1.2 | — |
| R40 — IDs/compound paths/special references/projections | implemented | M1 C1.2 | — |
| R41 — Derived fields already computable from model | implemented | M1 C1.2 | — |
| R42 — Geometry/render/media-derived fields not yet computable | deferred-with-owner | M2 C2.3; M3 C3.2/C3.3; M7 C7.2; M8 C8.1 | — |
| R43 — x-command / structural mutation routes and drawing aliases | deferred-with-owner | M2 C2.3; M3 C3.1/C3.2; owning specialty checkpoints | — |
| R44 — No-project enablement (01 1.3) | implemented | M1 C1.2 | — |
| R45 — Dirty-project quit fail/discard/save policy | implemented | M1 C1.2 | — |
| R46 — Fullscreen explicit enabled parameter | implemented | M1 C1.2 | — |
| R47 — Mode/current view/fullscreen readback agreement | implemented | M1 C1.2 | — |
| R48 — Workspace autoSave/default/save/reset rules | implemented | M1 C1.2 | — |
| R49 — Queryable panels/layout/toolbars | implemented | M1 C1.2 | — |
| R50 — All edit.* functionality due C1.2 (plan 4) | implemented | M1 C1.2 | — |
| R51 — Clipboard structured kind validation | implemented | M1 C1.2 | — |
| R52 — Clipboard return shape | implemented | M1 C1.2 | — |
| R53 — Manifest forward parity; declared typed reverse coverage | implemented | M1 C1.2 | — |
| R54 — Full-inventory reverse parity build gate (11 14.1) | implemented | M1 C1.2 | — |
| R55 — Real native command dispatch/state parity | deferred-with-owner | M4 C4.1 | — |
| R56 — Every registered command malformed inputs under watchdog | implemented | M1 C1.2 | — |
| R57 — Watchdog bounds caller vs actual execution | implemented | M1 C1.2 | — |
| R58 — Perpetually looping registered handler and modal interception proof | implemented | M1 C1.2 | — |
| R59 — Explicit scopes on implemented calls | implemented | M1 C1.2 | — |
| R60 — Job session isolation | implemented | M1 C1.2 | — |
| R61 — File/prefs/network scopes on inventory stubs | implemented | M1 C1.2 | — |
| R62 — Protocol initialize/tools/resources/subscriptions/prompts/events/audit/confirmation | deferred-with-owner | M1 C1.4 | — |
| R63 — Job wait/status/cancel/progress table | divergent | M1 C1.2 | C12-R3-03 |
| R64 — No new unsafe/product unwrap/expect/panic allowances | implemented | M1 C1.2 | — |
| R65 — Existing three-layer IPC gate / worker ban / diagnostics gate | implemented | M1 C1.2 | — |
| R66 — Composite command-result boundary controls | implemented | M1 C1.2 | — |
| R67 — Touched-object diff and 100k full-commit costs | implemented | M1 C1.2 | — |
| R68 — 10k-drawing query performance | implemented | M1 C1.2 | — |
| R69 — Dependency/licence and lockfile changes | implemented | M1 C1.2 | — |
| R70 — Reviewer test preservation and CI coverage accuracy | implemented | M1 C1.2 | — |
| R71 — Fresh-seed randomized coverage and shrinking | implemented | M1 C1.2 | — |
| R72 — Persistence unchecked assembly confinement | deferred-with-owner | M1 C1.3 | — |
