# Round 2 conformance registers

{
  "inventory": 796,
  "declared": 796,
  "handwritten": 134,
  "generated": 662,
  "generatedWithSchemaHoles": 597,
  "commandClassifications": {
    "deferred-with-owner": 120,
    "divergent": 621,
    "implemented": 55
  },
  "requirementClassifications": {
    "implemented": 47,
    "divergent": 18,
    "deferred-with-owner": 7
  },
  "random60": {
    "divergent": 49,
    "implemented": 4,
    "deferred-with-owner": 7
  },
  "previousImplemented40": {
    "implemented": 39,
    "divergent": 1
  },
  "examplesExecuted": 809,
  "exampleOutcomes": {
    "command.notImplemented": 660,
    "command.disabled": 77,
    "ok": 69,
    "edit.clipboardEmpty": 3
  }
}

All 796 IDs/kinds and verbatim appendix columns match. All declarations and parameter examples exist. Classification is evidence-bounded: implemented means exercised with no additional per-entry defect found; deferred-with-owner retains its actual future handler owner; divergent means a current declaration or behavior counterexample. Generic JSON Value and explicitly dynamic dictionaries in handwritten contracts are not automatically called placeholders. Generated holes are retained with exact schema pointers. Cross-cutting defects apply beyond per-command counts.

## Requirement closure (all 72)

| Row / requirement | R1 → R2 | Current source | Evidence | Adjudication |
|---|---|---|---|---|
| R01 Inventory IDs, namespace, kind and verbatim table columns (A) | implemented → implemented | `crates/registry/src/inventory.rs:8` | c12_registry; registry.json; this independent set comparison | Rechecked source and passing inherited suite in baseline/final logs.  |
| R02 Every inventory entry has constrained parameter/result schemas (plan 3.2; 11 14.2) | divergent → divergent | `scripts/gen-command-declarations.py:425` | c12_r2_registry | 597 generated entries retain schema holes; named-result shells, scopes and project rules also incomplete. C12-R2-02 |
| R03 Every command example executes and matches its result schema (11 14.2) | divergent → implemented | `crates/registry/tests/declared.rs:25` | independently_validate_every_declared_parameter_example | Rechecked source and passing inherited suite in baseline/final logs.  |
| R04 Every stub names a correct owner (plan 4) | divergent → implemented | `crates/registry/src/owners.rs:15` | registry_owners_follow_the_milestone_plan | Rechecked source and passing inherited suite in baseline/final logs.  |
| R05 Registry version and generated frontend schema freshness | implemented → implemented | `crates/registry/src/lib.rs:57` | frontend_schemas; c12_registry | Rechecked source and passing inherited suite in baseline/final logs.  |
| R06 Attribute macro rejects missing example/duplicate ID/wrong kind | implemented → implemented | `crates/registry-macros/src/lib.rs:6` | c12_r1_macro | Rechecked source and passing inherited suite in baseline/final logs.  |
| R07 Exclusive mutation capability (plan 3.2; 11 P2) | divergent → divergent | `crates/executor/src/run.rs:838` | safe_revision_reconstruction_cannot_install_future_object_revisions | Load capability and dirty-project policy fixed, but public revision reconstruction + load installs rev>documentRev. C12-R2-01 |
| R08 One serialized document queue (REV-06) | implemented → implemented | `crates/executor/src/lib.rs:272` | eight_fresh_clients_mix_object_guards_and_atomic_batches | Rechecked source and passing inherited suite in baseline/final logs.  |
| R09 Staged full integrity gate and rollback of document edits | implemented → implemented | `crates/model/src/document/mod.rs:674` | c12_transactions; staged_commit; c11_r4/r5/r6 | Rechecked source and passing inherited suite in baseline/final logs.  |
| R10 Atomic rollback covers every observable side effect (11 7.3) | divergent → implemented | `crates/executor/src/run.rs:133` | failed_batch_rolls_back_all_five_effect_classes_together | Rechecked source and passing inherited suite in baseline/final logs.  |
| R11 Atomic batch one revision, undo step and label | implemented → implemented | `crates/executor/src/run.rs:724` | c12_transactions | Rechecked source and passing inherited suite in baseline/final logs.  |
| R12 Query steps see prior staged changes (11 7.3) | divergent → divergent | `crates/executor/src/commands/checkpoint.rs:113` | checkpoint_is_visible_to_later_atomic_query | Later steps cannot read pending checkpoint creation. C12-R2-03 |
| R13 Numeric/named intermediate references | implemented → implemented | `crates/executor/src/run.rs:962` | c12_transactions | Rechecked source and passing inherited suite in baseline/final logs.  |
| R14 Long-running job-kind steps rejected in atomic batches | implemented → implemented | `crates/executor/src/run.rs:659` | c12_transactions | Rechecked source and passing inherited suite in baseline/final logs.  |
| R15 Ordinary edit dryRun changes/history/revisions | implemented → divergent | `crates/executor/src/commands/edit_ops.rs:80` | paste_dry_run_predicts_created_ids_exactly | Ordinary metadata previews fixed; ID-creating edits predict different IDs. C12-R2-04 |
| R16 Dry run covers clipboard events and cancellation | divergent → implemented | `crates/executor/src/run.rs:428` | failed_batch_rolls_back_all_five_effect_classes_together; c12_r1_contracts | Rechecked source and passing inherited suite in baseline/final logs.  |
| R17 Command-parameter dryRun in atomic batch | divergent → implemented | `crates/executor/src/run.rs:744` | parameter_dry_run_is_honoured_inside_atomic_batch | Rechecked source and passing inherited suite in baseline/final logs.  |
| R18 Sequential dry-run batch simulates preceding edits | divergent → divergent | `crates/executor/src/run.rs:632` | sequential_dry_run_simulates_checkpoints | Simulation suppresses checkpoint creation, so later query result differs. C12-R2-03 |
| R19 Common run_command envelope including job.wait | divergent → divergent | `crates/executor/src/run.rs:906` | pending_job_wait_reserves_request_id_against_other_commands; sequential_batch_job_wait_honours_wait_timeout_contract | Two-phase wait lacks in-flight reservation; batched wait uses polling-only handler. C12-R2-06 C12-R2-07 |
| R20 Undo/redo canonical content and revision stamps | implemented → implemented | `crates/executor/src/cx.rs:669` | edit_object_kinds_clipboard_and_history_roundtrip; standalone_m1_element_types_roundtrip_undo_redo | Rechecked source and passing inherited suite in baseline/final logs.  |
| R21 Failed batch history, redo invalidation, grouped labels | implemented → implemented | `crates/executor/src/run.rs:374` | c12_transactions; c12_r1_stress | Rechecked source and passing inherited suite in baseline/final logs.  |
| R22 Checkpoint create/list/delete/restore and history independence | implemented → divergent | `crates/executor/src/commands/checkpoint.rs:48` | checkpoint_create_then_delete_in_atomic_batch_succeeds | Standalone lifecycle works; create/delete composition fails. C12-R2-03 |
| R23 Checkpoint created from staged content has coherent rev | divergent → divergent | `crates/executor/src/run.rs:428` | checkpoint_between_edits_captures_its_staged_content | Checkpoint stores final batch content, not its staged point; before-edit prediction has wrong revision. C12-R2-03 |
| R24 Checkpoint cost at 100k strokes/10k drawings | implemented → implemented | `crates/model/src/document/mod.rs:104` | c12_r1_scale; large-documents.json | Rechecked source and passing inherited suite in baseline/final logs.  |
| R25 expectRev scalar and envelope guard | implemented → implemented | `crates/executor/src/run.rs:286` | mixed_expect_rev_guards_cover_paths_compound_ids_and_unmodified_project | Rechecked source and passing inherited suite in baseline/final logs.  |
| R26 expectRev map checks every listed object (REV-05) | divergent → implemented | `crates/executor/src/commands/doc.rs:679` | mixed_expect_rev_guards_cover_paths_compound_ids_and_unmodified_project | Rechecked source and passing inherited suite in baseline/final logs.  |
| R27 Conflict details.currentRev and current small JSON (REV-05) | divergent → implemented | `crates/executor/src/run.rs:299` | mixed_expect_rev_guards_cover_paths_compound_ids_and_unmodified_project | Rechecked source and passing inherited suite in baseline/final logs.  |
| R28 requestId retry same ID returns original without execution | divergent → divergent | `crates/executor/src/run.rs:906` | pending_job_wait_reserves_request_id_against_other_commands | Eviction and session isolation fixed; pending wait has no request-cache entry. C12-R2-06 |
| R29 requestId different parameters same ID | implemented → divergent | `crates/executor/src/run.rs:74` | pending_job_wait_reserves_request_id_against_other_commands | Completed collisions conform to R1-22; overlapping different command still executes. C12-R2-06 |
| R30 requestId persistence across process restart | deferred-with-owner → deferred-with-owner | `crates/executor/src/run.rs:57` | source audit | Retained deferral: Not required by 11 7.2 (within-session); durable session protocol is C1.4 decision, not a persistence defect  |
| R31 documentRev exactly +1 per edit/undo/redo (REV-01/03) | implemented → implemented | `crates/executor/src/cx.rs:502` | c12_transactions; c12_r1_stress | Rechecked source and passing inherited suite in baseline/final logs.  |
| R32 Revision overflow is controlled error, not thread panic | divergent → implemented | `crates/executor/src/cx.rs:507` | revision_max_blocks_fresh_paste_and_keeps_queries_alive | Rechecked source and passing inherited suite in baseline/final logs.  |
| R33 Drawing rev follows content edits (REV-02) | implemented → implemented | `crates/model/src/document/stamp.rs:22` | c12_transactions; changes_report_drawing_revision_when_a_stroke_changes | Rechecked source and passing inherited suite in baseline/final logs.  |
| R34 Every changed revision returned to caller (11 P6) | divergent → divergent | `crates/executor/src/commands/doc.rs:1298` | hundred_thousand_stroke_change_reports_drawing_in_envelope_event_and_diff | Envelope and event now include drawing rev; doc.diff still drops drawing invalidation. C12-R2-12 |
| R35 Persist revs through save/reopen (REV-04) | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:161` | typed declarations; handoff | Retained deferral: C1.3 persistence; cannot prove with C1.2 stubs  |
| R36 Change sets oldValue/concrete paths/numeric opaque keys | implemented → divergent | `crates/model/src/change.rs:890` | json04_materialised_parent_is_removed_by_raw_patch_inverse | JSON-04 synthesised parent is absent from inverse; opaque nested meta/ext containers invented. C12-R2-11 |
| R37 Historical doc.diff within retained 10k window | divergent → divergent | `crates/executor/src/commands/doc.rs:1194` | diff_reports_drawing_content_revision_invalidation | Historical reconstruction fixed; semantic diff omits drawing revision-only change. C12-R2-12 |
| R38 doc.diff retains last save even beyond 10k (AG-09) | divergent → implemented | `crates/executor/src/session.rs:92` | fresh_historical_diff_replays_and_agrees_with_checkpoint_across_batch_steps; diff_retains_the_last_save_even_after_ten_thousand_transactions | Rechecked source and passing inherited suite in baseline/final logs.  |
| R39 500-change envelope cap and oversized event elision | implemented → implemented | `crates/executor/src/run.rs:319` | c12_transactions; c12_policy | Rechecked source and passing inherited suite in baseline/final logs.  |
| R40 IDs/compound paths/special references/projections | implemented → implemented | `crates/executor/src/cx.rs:355` | c12_policy; c11_r1/r2/r3 | Rechecked source and passing inherited suite in baseline/final logs.  |
| R41 Derived fields already computable from model | implemented → implemented | `crates/executor/src/commands/derived.rs:14` | c12_policy | Rechecked source and passing inherited suite in baseline/final logs.  |
| R42 Geometry/render/media-derived fields not yet computable | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/derived.rs:5` | c12_policy; doc.derivedUnavailable | Retained deferral: Explicit M2/M3/M7/M8 owners and warnings; no pretend complete renderer  |
| R43 x-command / structural mutation routes and drawing aliases | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:737` | c12_registry::drawing_command_delegation_ruling; c12_policy | Retained deferral: Aliases mapped; specialized handlers intentionally M2/M3 etc; no hidden generic mutation  |
| R44 No-project enablement (01 1.3) | divergent → divergent | `scripts/gen-command-declarations.py:176` | library_close_has_no_project_rule_from_01_1_3 | All edit.* now gated; generated library declarations still exempt disallowed no-project commands. C12-R2-02 |
| R45 Dirty-project quit fail/discard/save policy | implemented → implemented | `crates/executor/src/commands/app.rs:479` | c12_kernel_state | Rechecked source and passing inherited suite in baseline/final logs.  |
| R46 Fullscreen explicit enabled parameter | implemented → implemented | `crates/executor/src/commands/window.rs:29` | c12_kernel_state; c12_r1_contracts | Rechecked source and passing inherited suite in baseline/final logs.  |
| R47 Mode/current view/fullscreen readback agreement | divergent → implemented | `crates/executor/src/commands/app.rs:380` | fresh_mode_workspace_and_renamed_builtin_reset_are_coherent | Rechecked source and passing inherited suite in baseline/final logs.  |
| R48 Workspace autoSave/default/save/reset rules | divergent → implemented | `crates/executor/src/ui.rs:922` | fresh_mode_workspace_and_renamed_builtin_reset_are_coherent | Rechecked source and passing inherited suite in baseline/final logs.  |
| R49 Queryable panels/layout/toolbars | implemented → implemented | `crates/executor/src/ui.rs:555` | c12_kernel_state | Rechecked source and passing inherited suite in baseline/final logs.  |
| R50 All edit.* functionality due C1.2 (plan 4) | missing → divergent | `crates/executor/src/commands/edit_ops.rs:1348` | c12_r2_contracts | Seven handlers exist; hierarchy, locks, opaque metadata, targets and deterministic prediction remain incorrect. C12-R2-04 C12-R2-05 C12-R2-08 C12-R2-09 C12-R2-13 C12-R2-14 |
| R51 Clipboard structured kind validation | divergent → divergent | `crates/executor/src/commands/edit.rs:705` | clipboard_element_properties_validates_values_not_only_names | elementProperties checks allowed field names, but accepts noninteger columnWidth. C12-R2-10 |
| R52 Clipboard return shape | divergent → implemented | `crates/executor/src/commands/edit.rs:823` | clipboard_get_returns_the_specified_clipboard_directly | Rechecked source and passing inherited suite in baseline/final logs.  |
| R53 Manifest forward parity; declared typed reverse coverage | implemented → implemented | `crates/executor/tests/c12_registry.rs:124` | c12_registry::ui_binding_manifest_parity | Rechecked source and passing inherited suite in baseline/final logs.  |
| R54 Full-inventory reverse parity build gate (11 14.1) | divergent → implemented | `crates/executor/tests/c12_registry.rs:124` | every_registry_entry_has_a_gui_binding_or_agent_only_justification | Reverse parity now visits all 796 entries; 662 GUI locations/agent-event justifications are declarative future bindings, not native dispatch proof.  |
| R55 Real native command dispatch/state parity | deferred-with-owner → deferred-with-owner | `desktop/src-tauri/src/menu.rs:35` | handoff; source audit | Retained deferral: M4 C4.1 per accepted readiness split; native fullscreen still toggles directly  |
| R56 Every registered command malformed inputs under watchdog | implemented → implemented | `crates/executor/src/lib.rs:174` | c12_r1_stress; c12_r1_scale | Rechecked source and passing inherited suite in baseline/final logs.  |
| R57 Watchdog bounds caller vs actual execution | implemented → implemented | `crates/executor/src/lib.rs:174` | c12_kernel_state::the_watchdog_answers_within_its_budget; job_wait_timeout_policy_is_explicitly_measured | Rechecked source and passing inherited suite in baseline/final logs.  |
| R58 Perpetually looping registered handler and modal interception proof | missing → implemented | `crates/executor/tests/c12_never_block.rs:33` | c12_never_block; ci-audit.json; baseline/gates/headless.log | Citation accepted: 11 P5 / actual section 14.6 prohibit modal/user waits, not forced Rust thread preemption. Test proves bounded caller timeout and recovery after release only. Process restart/real modal interception remain C1.4/M4.  |
| R59 Explicit scopes on implemented calls | implemented → implemented | `crates/executor/src/run.rs:224` | c12_policy | Rechecked source and passing inherited suite in baseline/final logs.  |
| R60 Job session isolation | divergent → implemented | `crates/executor/src/commands/job.rs:34` | fresh_job_cross_session_list_wait_cancel_are_isolated | Rechecked source and passing inherited suite in baseline/final logs.  |
| R61 File/prefs/network scopes on inventory stubs | divergent → divergent | `scripts/gen-command-declarations.py:207` | library_global_write_scope_is_declared | File/network examples improved; global-library item mutations still lack library.global. C12-R2-02 |
| R62 Protocol initialize/tools/resources/subscriptions/prompts/events/audit/confirmation | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:64` | agent.* typed declarations; bins/headless flags | Retained deferral: C1.4; no MCP inspector run possible at this checkpoint  |
| R63 Job wait/status/cancel/progress table | implemented → divergent | `crates/executor/src/commands/job.rs:119` | sequential_batch_job_wait_honours_wait_timeout_contract | Owned job table and event ordering work; sequential batch wait ignores timeout. C12-R2-07 |
| R64 No new unsafe/product unwrap/expect/panic allowances | implemented → implemented | `Cargo.toml:62` | source audit; clippy; CI | Rechecked source and passing inherited suite in baseline/final logs.  |
| R65 Existing three-layer IPC gate / worker ban / diagnostics gate | implemented → implemented | `desktop/eslint.config.js:10` | baseline build; Vitest boundary 69+46; CI default/diagnostics builds | Rechecked source and passing inherited suite in baseline/final logs.  |
| R66 Composite command-result boundary controls | implemented → implemented | `desktop/src/ipc/commands.test.ts:30` | 7 command schema tests; frontend_schemas | Rechecked source and passing inherited suite in baseline/final logs.  |
| R67 Touched-object diff and 100k full-commit costs | implemented → implemented | `crates/executor/src/cx.rs:433` | commit-scale.log; large-documents.json | Rechecked source and passing inherited suite in baseline/final logs.  |
| R68 10k-drawing query performance | divergent → implemented | `crates/model/src/document/paths.rs:113` | c12_r1_scale; logs/release-scale.log; large-documents.json | Rechecked source and passing inherited suite in baseline/final logs.  |
| R69 Dependency/licence and lockfile changes | implemented → implemented | `Cargo.lock:3` | cargo deny; CI; git diff | Rechecked source and passing inherited suite in baseline/final logs.  |
| R70 Reviewer test preservation and CI coverage accuracy | implemented → implemented | `.github/workflows/ci.yml:88` | ci-audit.json; static-audit.json; final/gates/test.log | Rechecked source and passing inherited suite in baseline/final logs.  |
| R71 Fresh-seed randomized coverage and shrinking | implemented → implemented | `crates/executor/tests/c12_r2_contracts.rs:834` | eight_fresh_clients_mix_object_guards_and_atomic_batches; random-history.json; c11_r3_random | Rechecked source and passing inherited suite in baseline/final logs.  |
| R72 Persistence unchecked assembly confinement | deferred-with-owner → deferred-with-owner | `crates/model/src/document/mod.rs:232` | C1.1 closure carryover | Retained deferral: C1.3 load/repair; open capability must first close C12-R1-01  |

## Inventory (all 796)

| ID | R1 → R2 | Declaration / appendix | Examples run | Findings / owner |
|---|---|---|---|---|
| `agent.activity.export` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:405`; `spec/A_command_registry.md:1035` | 1 | M1 C1.4 |
| `agent.activity.list` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:342`; `spec/A_command_registry.md:1036` | 1 | M1 C1.4 |
| `agent.activity.persist.set` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:426`; `spec/A_command_registry.md:1037` | 1 | M1 C1.4 |
| `agent.confirmation.requested` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:465`; `spec/A_command_registry.md:1038` | 1 | M1 C1.4 |
| `agent.confirmation.respond` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:313`; `spec/A_command_registry.md:1039` | 1 | M1 C1.4 |
| `agent.policy.defaults.set` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:224`; `spec/A_command_registry.md:1040` | 1 | M1 C1.4 |
| `agent.server.set` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:64`; `spec/A_command_registry.md:1041` | 1 | M1 C1.4 |
| `agent.server.status` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:77`; `spec/A_command_registry.md:1042` | 1 | M1 C1.4 |
| `agent.session.connected` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:439`; `spec/A_command_registry.md:1043` | 1 | M1 C1.4 |
| `agent.session.disconnect` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:128`; `spec/A_command_registry.md:1044` | 1 | M1 C1.4 |
| `agent.session.disconnected` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:452`; `spec/A_command_registry.md:1045` | 1 | M1 C1.4 |
| `agent.session.grant` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:107`; `spec/A_command_registry.md:1046` | 1 | M1 C1.4 |
| `agent.session.pause` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:157`; `spec/A_command_registry.md:1047` | 1 | M1 C1.4 |
| `agent.session.resume` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:177`; `spec/A_command_registry.md:1048` | 1 | M1 C1.4 |
| `agent.token.regenerate` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:248`; `spec/A_command_registry.md:1049` | 1 | M1 C1.4 |
| `agent.typedTools.set` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/agent.rs:280`; `spec/A_command_registry.md:1050` | 1 | M1 C1.4 |
| `anim.bone.create` | divergent → divergent | `crates/registry/src/declared.rs:6386`; `spec/A_command_registry.md:517` | 1 | C12-R2-02 |
| `anim.bone.delete` | divergent → divergent | `crates/registry/src/declared.rs:6411`; `spec/A_command_registry.md:518` | 1 | C12-R2-02 |
| `anim.bone.influence.set` | divergent → divergent | `crates/registry/src/declared.rs:6433`; `spec/A_command_registry.md:519` | 1 | C12-R2-02 |
| `anim.bone.parent.set` | divergent → divergent | `crates/registry/src/declared.rs:6458`; `spec/A_command_registry.md:520` | 1 | C12-R2-02 |
| `anim.bone.pose.set` | divergent → divergent | `crates/registry/src/declared.rs:6483`; `spec/A_command_registry.md:521` | 1 | C12-R2-02 |
| `anim.bone.reach.apply` | divergent → divergent | `crates/registry/src/declared.rs:6508`; `spec/A_command_registry.md:522` | 1 | C12-R2-02 |
| `anim.bone.rest.set` | divergent → divergent | `crates/registry/src/declared.rs:6533`; `spec/A_command_registry.md:523` | 1 | C12-R2-02 |
| `anim.camera.create` | divergent → divergent | `crates/registry/src/declared.rs:6558`; `spec/A_command_registry.md:524` | 1 | C12-R2-02 |
| `anim.camera.current.get` | divergent → divergent | `crates/registry/src/declared.rs:6580`; `spec/A_command_registry.md:525` | 1 | C12-R2-02 |
| `anim.camera.current.set` | divergent → divergent | `crates/registry/src/declared.rs:6602`; `spec/A_command_registry.md:526` | 1 | C12-R2-02 |
| `anim.camera.currentChanged` | divergent → divergent | `crates/registry/src/declared.rs:6627`; `spec/A_command_registry.md:527` | 1 | C12-R2-02 |
| `anim.camera.delete` | divergent → divergent | `crates/registry/src/declared.rs:6646`; `spec/A_command_registry.md:528` | 1 | C12-R2-02 |
| `anim.camera.get` | divergent → divergent | `crates/registry/src/declared.rs:6671`; `spec/A_command_registry.md:529` | 1 | C12-R2-02 |
| `anim.camera.list` | divergent → divergent | `crates/registry/src/declared.rs:6696`; `spec/A_command_registry.md:530` | 1 | C12-R2-02 |
| `anim.camera.select` | divergent → divergent | `crates/registry/src/declared.rs:6718`; `spec/A_command_registry.md:531` | 1 | C12-R2-02 |
| `anim.camera.set` | divergent → divergent | `crates/registry/src/declared.rs:6740`; `spec/A_command_registry.md:532` | 1 | C12-R2-02 |
| `anim.changed` | divergent → divergent | `crates/registry/src/declared.rs:6765`; `spec/A_command_registry.md:533` | 1 | C12-R2-02 |
| `anim.channel.copy` | divergent → divergent | `crates/registry/src/declared.rs:6784`; `spec/A_command_registry.md:534` | 1 | C12-R2-02 |
| `anim.channel.cut` | divergent → divergent | `crates/registry/src/declared.rs:6809`; `spec/A_command_registry.md:535` | 1 | C12-R2-02 |
| `anim.channel.paste` | divergent → divergent | `crates/registry/src/declared.rs:6834`; `spec/A_command_registry.md:536` | 1 | C12-R2-02 |
| `anim.depth.move` | divergent → divergent | `crates/registry/src/declared.rs:6859`; `spec/A_command_registry.md:537` | 1 | C12-R2-02 |
| `anim.display.allPegs.set` | divergent → divergent | `crates/registry/src/declared.rs:6884`; `spec/A_command_registry.md:538` | 1 | C12-R2-02 |
| `anim.display.antialiasToolOverlays.set` | divergent → divergent | `crates/registry/src/declared.rs:6909`; `spec/A_command_registry.md:539` | 1 | C12-R2-02 |
| `anim.display.arms.set` | divergent → divergent | `crates/registry/src/declared.rs:6934`; `spec/A_command_registry.md:540` | 1 | C12-R2-02 |
| `anim.display.changed` | divergent → divergent | `crates/registry/src/declared.rs:6959`; `spec/A_command_registry.md:541` | 1 | C12-R2-02 |
| `anim.display.envelopes.set` | divergent → divergent | `crates/registry/src/declared.rs:6978`; `spec/A_command_registry.md:542` | 1 | C12-R2-02 |
| `anim.display.frameMarkers.set` | divergent → divergent | `crates/registry/src/declared.rs:7003`; `spec/A_command_registry.md:543` | 1 | C12-R2-02 |
| `anim.display.get` | divergent → divergent | `crates/registry/src/declared.rs:7028`; `spec/A_command_registry.md:544` | 1 | C12-R2-02 |
| `anim.display.pegGhosts.set` | divergent → divergent | `crates/registry/src/declared.rs:7050`; `spec/A_command_registry.md:545` | 1 | C12-R2-02 |
| `anim.display.pivotPath.set` | divergent → divergent | `crates/registry/src/declared.rs:7072`; `spec/A_command_registry.md:546` | 1 | C12-R2-02 |
| `anim.display.smoothPaths.set` | divergent → divergent | `crates/registry/src/declared.rs:7094`; `spec/A_command_registry.md:547` | 1 | C12-R2-02 |
| `anim.evaluate` | divergent → divergent | `crates/registry/src/declared.rs:7119`; `spec/A_command_registry.md:548` | 1 | C12-R2-02 |
| `anim.flip.apply` | divergent → divergent | `crates/registry/src/declared.rs:7144`; `spec/A_command_registry.md:549` | 1 | C12-R2-02 |
| `anim.function.curveType.set` | divergent → divergent | `crates/registry/src/declared.rs:7169`; `spec/A_command_registry.md:550` | 1 | C12-R2-02 |
| `anim.function.get` | divergent → divergent | `crates/registry/src/declared.rs:7194`; `spec/A_command_registry.md:551` | 1 | C12-R2-02 |
| `anim.function.key.add` | divergent → divergent | `crates/registry/src/declared.rs:7219`; `spec/A_command_registry.md:552` | 1 | C12-R2-02 |
| `anim.function.key.duplicate` | divergent → divergent | `crates/registry/src/declared.rs:7244`; `spec/A_command_registry.md:553` | 1 | C12-R2-02 |
| `anim.function.key.remove` | divergent → divergent | `crates/registry/src/declared.rs:7269`; `spec/A_command_registry.md:554` | 1 | C12-R2-02 |
| `anim.function.key.set` | divergent → divergent | `crates/registry/src/declared.rs:7294`; `spec/A_command_registry.md:555` | 1 | C12-R2-02 |
| `anim.function.list` | divergent → divergent | `crates/registry/src/declared.rs:7319`; `spec/A_command_registry.md:556` | 1 | C12-R2-02 |
| `anim.function.preset.apply` | divergent → divergent | `crates/registry/src/declared.rs:7344`; `spec/A_command_registry.md:557` | 1 | C12-R2-02 |
| `anim.function.sample` | divergent → divergent | `crates/registry/src/declared.rs:7369`; `spec/A_command_registry.md:558` | 1 | C12-R2-02 |
| `anim.function.set` | divergent → divergent | `crates/registry/src/declared.rs:7394`; `spec/A_command_registry.md:559` | 1 | C12-R2-02 |
| `anim.functionEditor.get` | divergent → divergent | `crates/registry/src/declared.rs:7419`; `spec/A_command_registry.md:560` | 1 | C12-R2-02 |
| `anim.functionEditor.open` | divergent → divergent | `crates/registry/src/declared.rs:7441`; `spec/A_command_registry.md:561` | 1 | C12-R2-02 |
| `anim.functionEditor.syncFrame.set` | divergent → divergent | `crates/registry/src/declared.rs:7466`; `spec/A_command_registry.md:562` | 1 | C12-R2-02 |
| `anim.functionEditor.view.set` | divergent → divergent | `crates/registry/src/declared.rs:7491`; `spec/A_command_registry.md:563` | 1 | C12-R2-02 |
| `anim.hierarchy.attach` | divergent → divergent | `crates/registry/src/declared.rs:7516`; `spec/A_command_registry.md:564` | 1 | C12-R2-02 |
| `anim.hierarchy.changed` | divergent → divergent | `crates/registry/src/declared.rs:7541`; `spec/A_command_registry.md:565` | 1 | C12-R2-02 |
| `anim.hierarchy.detach` | divergent → divergent | `crates/registry/src/declared.rs:7560`; `spec/A_command_registry.md:566` | 1 | C12-R2-02 |
| `anim.hierarchy.get` | divergent → divergent | `crates/registry/src/declared.rs:7585`; `spec/A_command_registry.md:567` | 1 | C12-R2-02 |
| `anim.hierarchy.select` | divergent → divergent | `crates/registry/src/declared.rs:7607`; `spec/A_command_registry.md:568` | 1 | C12-R2-02 |
| `anim.key.add` | divergent → divergent | `crates/registry/src/declared.rs:7632`; `spec/A_command_registry.md:569` | 1 | C12-R2-02 |
| `anim.key.list` | divergent → divergent | `crates/registry/src/declared.rs:7657`; `spec/A_command_registry.md:570` | 1 | C12-R2-02 |
| `anim.key.move` | divergent → divergent | `crates/registry/src/declared.rs:7682`; `spec/A_command_registry.md:571` | 1 | C12-R2-02 |
| `anim.key.remove` | divergent → divergent | `crates/registry/src/declared.rs:7707`; `spec/A_command_registry.md:572` | 1 | C12-R2-02 |
| `anim.loop.set` | divergent → divergent | `crates/registry/src/declared.rs:7732`; `spec/A_command_registry.md:573` | 1 | C12-R2-02 |
| `anim.node.get` | divergent → divergent | `crates/registry/src/declared.rs:7757`; `spec/A_command_registry.md:574` | 1 | C12-R2-02 |
| `anim.node.options.set` | divergent → divergent | `crates/registry/src/declared.rs:7782`; `spec/A_command_registry.md:575` | 1 | C12-R2-02 |
| `anim.node.pathDisplay.set` | divergent → divergent | `crates/registry/src/declared.rs:7804`; `spec/A_command_registry.md:576` | 1 | C12-R2-02 |
| `anim.path.get` | divergent → divergent | `crates/registry/src/declared.rs:7829`; `spec/A_command_registry.md:577` | 1 | C12-R2-02 |
| `anim.path.lockDepth.set` | divergent → divergent | `crates/registry/src/declared.rs:7854`; `spec/A_command_registry.md:578` | 1 | C12-R2-02 |
| `anim.path.point.add` | divergent → divergent | `crates/registry/src/declared.rs:7879`; `spec/A_command_registry.md:579` | 1 | C12-R2-02 |
| `anim.path.point.remove` | divergent → divergent | `crates/registry/src/declared.rs:7904`; `spec/A_command_registry.md:580` | 1 | C12-R2-02 |
| `anim.path.point.select` | divergent → divergent | `crates/registry/src/declared.rs:7929`; `spec/A_command_registry.md:581` | 1 | C12-R2-02 |
| `anim.path.point.set` | divergent → divergent | `crates/registry/src/declared.rs:7954`; `spec/A_command_registry.md:582` | 1 | C12-R2-02 |
| `anim.path.sample` | divergent → divergent | `crates/registry/src/declared.rs:7979`; `spec/A_command_registry.md:583` | 1 | C12-R2-02 |
| `anim.path.set` | divergent → divergent | `crates/registry/src/declared.rs:8004`; `spec/A_command_registry.md:584` | 1 | C12-R2-02 |
| `anim.peg.create` | divergent → divergent | `crates/registry/src/declared.rs:8029`; `spec/A_command_registry.md:585` | 1 | C12-R2-02 |
| `anim.pegOnlyMode.set` | divergent → divergent | `crates/registry/src/declared.rs:8054`; `spec/A_command_registry.md:586` | 1 | C12-R2-02 |
| `anim.point.copy` | divergent → divergent | `crates/registry/src/declared.rs:8079`; `spec/A_command_registry.md:587` | 1 | C12-R2-02 |
| `anim.point.paste` | divergent → divergent | `crates/registry/src/declared.rs:8104`; `spec/A_command_registry.md:588` | 1 | C12-R2-02 |
| `anim.prefs.get` | divergent → divergent | `crates/registry/src/declared.rs:8129`; `spec/A_command_registry.md:589` | 1 | C12-R2-02 |
| `anim.prefs.set` | divergent → divergent | `crates/registry/src/declared.rs:8151`; `spec/A_command_registry.md:590` | 1 | C12-R2-02 |
| `anim.projection.get` | divergent → divergent | `crates/registry/src/declared.rs:8173`; `spec/A_command_registry.md:591` | 1 | C12-R2-02 |
| `anim.record.completed` | divergent → divergent | `crates/registry/src/declared.rs:8198`; `spec/A_command_registry.md:592` | 1 | C12-R2-02 |
| `anim.record.options.get` | divergent → divergent | `crates/registry/src/declared.rs:8217`; `spec/A_command_registry.md:593` | 1 | C12-R2-02 |
| `anim.record.options.set` | divergent → divergent | `crates/registry/src/declared.rs:8239`; `spec/A_command_registry.md:594` | 1 | C12-R2-02 |
| `anim.record.run` | divergent → divergent | `crates/registry/src/declared.rs:8264`; `spec/A_command_registry.md:595` | 1 | C12-R2-02 |
| `anim.reset.apply` | divergent → divergent | `crates/registry/src/declared.rs:8289`; `spec/A_command_registry.md:596` | 1 | C12-R2-02 |
| `anim.segment.set` | divergent → divergent | `crates/registry/src/declared.rs:8314`; `spec/A_command_registry.md:597` | 1 | C12-R2-02 |
| `anim.segment.uniform.apply` | divergent → divergent | `crates/registry/src/declared.rs:8339`; `spec/A_command_registry.md:598` | 1 | C12-R2-02 |
| `anim.skeleton.create` | divergent → divergent | `crates/registry/src/declared.rs:8364`; `spec/A_command_registry.md:599` | 1 | C12-R2-02 |
| `anim.skeleton.deform.get` | divergent → divergent | `crates/registry/src/declared.rs:8389`; `spec/A_command_registry.md:600` | 1 | C12-R2-02 |
| `anim.skeleton.delete` | divergent → divergent | `crates/registry/src/declared.rs:8414`; `spec/A_command_registry.md:601` | 1 | C12-R2-02 |
| `anim.skeleton.get` | divergent → divergent | `crates/registry/src/declared.rs:8439`; `spec/A_command_registry.md:602` | 1 | C12-R2-02 |
| `anim.skeleton.restPose.apply` | divergent → divergent | `crates/registry/src/declared.rs:8464`; `spec/A_command_registry.md:603` | 1 | C12-R2-02 |
| `anim.skeleton.root.select` | divergent → divergent | `crates/registry/src/declared.rs:8489`; `spec/A_command_registry.md:604` | 1 | C12-R2-02 |
| `anim.skeleton.set` | divergent → divergent | `crates/registry/src/declared.rs:8514`; `spec/A_command_registry.md:605` | 1 | C12-R2-02 |
| `anim.skeleton.skin.add` | divergent → divergent | `crates/registry/src/declared.rs:8539`; `spec/A_command_registry.md:606` | 1 | C12-R2-02 |
| `anim.skeleton.skin.remove` | divergent → divergent | `crates/registry/src/declared.rs:8564`; `spec/A_command_registry.md:607` | 1 | C12-R2-02 |
| `anim.snapLastKey.set` | divergent → divergent | `crates/registry/src/declared.rs:8589`; `spec/A_command_registry.md:608` | 1 | C12-R2-02 |
| `anim.stretch.apply` | divergent → divergent | `crates/registry/src/declared.rs:8614`; `spec/A_command_registry.md:609` | 1 | C12-R2-02 |
| `anim.tool.changed` | divergent → divergent | `crates/registry/src/declared.rs:8639`; `spec/A_command_registry.md:610` | 1 | C12-R2-02 |
| `anim.tool.get` | divergent → divergent | `crates/registry/src/declared.rs:8658`; `spec/A_command_registry.md:611` | 1 | C12-R2-02 |
| `anim.tool.set` | divergent → divergent | `crates/registry/src/declared.rs:8680`; `spec/A_command_registry.md:612` | 1 | C12-R2-02 |
| `anim.transform.set` | divergent → divergent | `crates/registry/src/declared.rs:8705`; `spec/A_command_registry.md:613` | 1 | C12-R2-02 |
| `anim.view.coords.get` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:8730`; `spec/A_command_registry.md:614` | 1 | M6 C6.1 |
| `anim.view.get` | divergent → divergent | `crates/registry/src/declared.rs:8755`; `spec/A_command_registry.md:615` | 1 | C12-R2-02 |
| `anim.view.slot.select` | divergent → divergent | `crates/registry/src/declared.rs:8780`; `spec/A_command_registry.md:616` | 1 | C12-R2-02 |
| `anim.view.type.set` | divergent → divergent | `crates/registry/src/declared.rs:8805`; `spec/A_command_registry.md:617` | 1 | C12-R2-02 |
| `app.context.set` | implemented → implemented | `crates/executor/src/commands/app.rs:63`; `spec/A_command_registry.md:53` | 2 | M1 C1.2 |
| `app.currentElement.move` | implemented → implemented | `crates/executor/src/commands/app.rs:178`; `spec/A_command_registry.md:54` | 1 | M1 C1.2 |
| `app.currentElement.set` | implemented → implemented | `crates/executor/src/commands/app.rs:129`; `spec/A_command_registry.md:55` | 1 | M1 C1.2 |
| `app.frame.move` | implemented → implemented | `crates/executor/src/commands/app.rs:293`; `spec/A_command_registry.md:56` | 2 | M1 C1.2 |
| `app.frame.set` | implemented → implemented | `crates/executor/src/commands/app.rs:238`; `spec/A_command_registry.md:57` | 1 | M1 C1.2 |
| `app.info.get` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/app.rs:431`; `spec/A_command_registry.md:58` | 1 | M1 C1.4 |
| `app.mode.set` | divergent → implemented | `crates/executor/src/commands/app.rs:372`; `spec/A_command_registry.md:59` | 1 | M1 C1.2 |
| `app.quit` | implemented → implemented | `crates/executor/src/commands/app.rs:473`; `spec/A_command_registry.md:60` | 1 | M1 C1.2 |
| `app.state.changed` | implemented → implemented | `crates/executor/src/commands/app.rs:501`; `spec/A_command_registry.md:61` | 1 | M1 C1.2 |
| `app.state.get` | divergent → implemented | `crates/executor/src/commands/app.rs:32`; `spec/A_command_registry.md:62` | 1 | M1 C1.2 |
| `capture.auto.start` | divergent → divergent | `crates/registry/src/declared.rs:15031`; `spec/A_command_registry.md:956` | 1 | C12-R2-02 |
| `capture.auto.stop` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:15056`; `spec/A_command_registry.md:957` | 1 | M10 C10.3 |
| `capture.device.added` | divergent → divergent | `crates/registry/src/declared.rs:15078`; `spec/A_command_registry.md:958` | 1 | C12-R2-02 |
| `capture.device.dialog.open` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:15097`; `spec/A_command_registry.md:959` | 1 | M10 C10.3 |
| `capture.device.get` | divergent → divergent | `crates/registry/src/declared.rs:15122`; `spec/A_command_registry.md:960` | 1 | C12-R2-02 |
| `capture.device.list` | divergent → divergent | `crates/registry/src/declared.rs:15147`; `spec/A_command_registry.md:961` | 1 | C12-R2-02 |
| `capture.device.removed` | divergent → divergent | `crates/registry/src/declared.rs:15169`; `spec/A_command_registry.md:962` | 1 | C12-R2-02 |
| `capture.device.select` | divergent → divergent | `crates/registry/src/declared.rs:15188`; `spec/A_command_registry.md:963` | 1 | C12-R2-02 |
| `capture.device.settings.set` | divergent → divergent | `crates/registry/src/declared.rs:15213`; `spec/A_command_registry.md:964` | 1 | C12-R2-02 |
| `capture.display.set` | divergent → divergent | `crates/registry/src/declared.rs:15238`; `spec/A_command_registry.md:965` | 1 | C12-R2-02 |
| `capture.frame.add` | divergent → divergent | `crates/registry/src/declared.rs:15260`; `spec/A_command_registry.md:966` | 1 | C12-R2-02 |
| `capture.frame.addFromFile` | divergent → divergent | `crates/registry/src/declared.rs:15285`; `spec/A_command_registry.md:967` | 1 | C12-R2-02 |
| `capture.frame.captured` | divergent → divergent | `crates/registry/src/declared.rs:15310`; `spec/A_command_registry.md:968` | 1 | C12-R2-02 |
| `capture.inputLock.set` | divergent → divergent | `crates/registry/src/declared.rs:15329`; `spec/A_command_registry.md:969` | 1 | C12-R2-02 |
| `capture.keyColour.set` | divergent → divergent | `crates/registry/src/declared.rs:15354`; `spec/A_command_registry.md:970` | 1 | C12-R2-02 |
| `capture.keying.set` | divergent → divergent | `crates/registry/src/declared.rs:15379`; `spec/A_command_registry.md:971` | 1 | C12-R2-02 |
| `capture.lightTable.references.list` | divergent → divergent | `crates/registry/src/declared.rs:15404`; `spec/A_command_registry.md:972` | 1 | C12-R2-02 |
| `capture.lightTable.set` | divergent → divergent | `crates/registry/src/declared.rs:15429`; `spec/A_command_registry.md:973` | 1 | C12-R2-02 |
| `capture.preview.get` | divergent → divergent | `crates/registry/src/declared.rs:15454`; `spec/A_command_registry.md:974` | 1 | C12-R2-02 |
| `capture.session.close` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:15479`; `spec/A_command_registry.md:975` | 1 | M10 C10.3 |
| `capture.session.error` | divergent → divergent | `crates/registry/src/declared.rs:15501`; `spec/A_command_registry.md:976` | 1 | C12-R2-02 |
| `capture.session.get` | divergent → divergent | `crates/registry/src/declared.rs:15520`; `spec/A_command_registry.md:977` | 1 | C12-R2-02 |
| `capture.session.open` | divergent → divergent | `crates/registry/src/declared.rs:15542`; `spec/A_command_registry.md:978` | 1 | C12-R2-02 |
| `capture.source.set` | divergent → divergent | `crates/registry/src/declared.rs:15567`; `spec/A_command_registry.md:979` | 1 | C12-R2-02 |
| `capture.target.set` | divergent → divergent | `crates/registry/src/declared.rs:15592`; `spec/A_command_registry.md:980` | 1 | C12-R2-02 |
| `cell.blank.add` | divergent → divergent | `crates/registry/src/declared.rs:5228`; `spec/A_command_registry.md:448` | 1 | C12-R2-02 |
| `cell.blank.remove` | divergent → divergent | `crates/registry/src/declared.rs:5253`; `spec/A_command_registry.md:449` | 1 | C12-R2-02 |
| `cell.clipboard.apply` | divergent → divergent | `crates/registry/src/declared.rs:5278`; `spec/A_command_registry.md:450` | 1 | C12-R2-02 |
| `cell.clipboard.set` | divergent → divergent | `crates/registry/src/declared.rs:5303`; `spec/A_command_registry.md:451` | 1 | C12-R2-02 |
| `cell.columnate.apply` | divergent → divergent | `crates/registry/src/declared.rs:5328`; `spec/A_command_registry.md:452` | 1 | C12-R2-02 |
| `cell.cycle.create` | divergent → divergent | `crates/registry/src/declared.rs:5353`; `spec/A_command_registry.md:453` | 1 | C12-R2-02 |
| `cell.drawing.duplicate` | divergent → divergent | `crates/registry/src/declared.rs:5378`; `spec/A_command_registry.md:454` | 1 | C12-R2-02 |
| `cell.exposure.add` | divergent → divergent | `crates/registry/src/declared.rs:5403`; `spec/A_command_registry.md:455` | 1 | C12-R2-02 |
| `cell.exposure.remove` | divergent → divergent | `crates/registry/src/declared.rs:5428`; `spec/A_command_registry.md:456` | 1 | C12-R2-02 |
| `cell.exposure.set` | divergent → divergent | `crates/registry/src/declared.rs:5453`; `spec/A_command_registry.md:457` | 1 | C12-R2-02 |
| `cell.exposures.changed` | divergent → divergent | `crates/registry/src/declared.rs:5478`; `spec/A_command_registry.md:458` | 1 | C12-R2-02 |
| `cell.fill.apply` | divergent → divergent | `crates/registry/src/declared.rs:5497`; `spec/A_command_registry.md:459` | 1 | C12-R2-02 |
| `cell.label.set` | divergent → divergent | `crates/registry/src/declared.rs:5522`; `spec/A_command_registry.md:460` | 1 | C12-R2-02 |
| `cell.note.set` | divergent → divergent | `crates/registry/src/declared.rs:5547`; `spec/A_command_registry.md:461` | 1 | C12-R2-02 |
| `cell.range.add` | divergent → divergent | `crates/registry/src/declared.rs:5572`; `spec/A_command_registry.md:462` | 1 | C12-R2-02 |
| `cell.range.delete` | divergent → divergent | `crates/registry/src/declared.rs:5597`; `spec/A_command_registry.md:463` | 1 | C12-R2-02 |
| `cell.range.move` | divergent → divergent | `crates/registry/src/declared.rs:5622`; `spec/A_command_registry.md:464` | 1 | C12-R2-02 |
| `cell.select` | divergent → divergent | `crates/registry/src/declared.rs:5647`; `spec/A_command_registry.md:465` | 1 | C12-R2-02 |
| `cell.selection.changed` | divergent → divergent | `crates/registry/src/declared.rs:5672`; `spec/A_command_registry.md:466` | 1 | C12-R2-02 |
| `cell.selection.get` | divergent → divergent | `crates/registry/src/declared.rs:5691`; `spec/A_command_registry.md:467` | 1 | C12-R2-02 |
| `checkpoint.create` | divergent → divergent | `crates/executor/src/commands/checkpoint.rs:40`; `spec/A_command_registry.md:1071` | 1 | C12-R2-03 |
| `checkpoint.delete` | implemented → divergent | `crates/executor/src/commands/checkpoint.rs:149`; `spec/A_command_registry.md:1072` | 1 | C12-R2-03 |
| `checkpoint.list` | divergent → divergent | `crates/executor/src/commands/checkpoint.rs:105`; `spec/A_command_registry.md:1073` | 1 | C12-R2-03 |
| `checkpoint.restore` | implemented → implemented | `crates/executor/src/commands/checkpoint.rs:128`; `spec/A_command_registry.md:1074` | 1 | M1 C1.2 |
| `colour.autoColourUpdate.set` | divergent → divergent | `crates/registry/src/declared.rs:3019`; `spec/A_command_registry.md:343` | 1 | C12-R2-02 |
| `colour.current.changed` | divergent → divergent | `crates/registry/src/declared.rs:3044`; `spec/A_command_registry.md:344` | 1 | C12-R2-02 |
| `colour.current.get` | divergent → divergent | `crates/registry/src/declared.rs:3063`; `spec/A_command_registry.md:345` | 1 | C12-R2-02 |
| `colour.current.set` | divergent → divergent | `crates/registry/src/declared.rs:3085`; `spec/A_command_registry.md:346` | 1 | C12-R2-02 |
| `colour.editor.close` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:3110`; `spec/A_command_registry.md:347` | 1 | M7 C7.1 |
| `colour.editor.open` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:3135`; `spec/A_command_registry.md:348` | 1 | M7 C7.1 |
| `colour.fillMapping.get` | divergent → divergent | `crates/registry/src/declared.rs:3160`; `spec/A_command_registry.md:349` | 1 | C12-R2-02 |
| `colour.fillMapping.set` | divergent → divergent | `crates/registry/src/declared.rs:3185`; `spec/A_command_registry.md:350` | 1 | C12-R2-02 |
| `colour.gap.close` | divergent → divergent | `crates/registry/src/declared.rs:3210`; `spec/A_command_registry.md:351` | 1 | C12-R2-02 |
| `colour.gapClosing.get` | divergent → divergent | `crates/registry/src/declared.rs:3235`; `spec/A_command_registry.md:352` | 1 | C12-R2-02 |
| `colour.gapClosing.set` | divergent → divergent | `crates/registry/src/declared.rs:3257`; `spec/A_command_registry.md:353` | 1 | C12-R2-02 |
| `colour.missing.list` | divergent → divergent | `crates/registry/src/declared.rs:3282`; `spec/A_command_registry.md:354` | 1 | C12-R2-02 |
| `colour.paint.apply` | divergent → divergent | `crates/registry/src/declared.rs:3307`; `spec/A_command_registry.md:355` | 1 | C12-R2-02 |
| `colour.paint.remove` | divergent → divergent | `crates/registry/src/declared.rs:3332`; `spec/A_command_registry.md:356` | 1 | C12-R2-02 |
| `colour.paintUnpainted.apply` | divergent → divergent | `crates/registry/src/declared.rs:3357`; `spec/A_command_registry.md:357` | 1 | C12-R2-02 |
| `colour.palette.changed` | divergent → divergent | `crates/registry/src/declared.rs:3382`; `spec/A_command_registry.md:358` | 1 | C12-R2-02 |
| `colour.palette.create` | divergent → divergent | `crates/registry/src/declared.rs:3401`; `spec/A_command_registry.md:359` | 1 | C12-R2-02 |
| `colour.palette.delete` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:3423`; `spec/A_command_registry.md:360` | 1 | M7 C7.1 |
| `colour.palette.duplicate` | divergent → divergent | `crates/registry/src/declared.rs:3448`; `spec/A_command_registry.md:361` | 1 | C12-R2-02 |
| `colour.palette.export` | divergent → divergent | `crates/registry/src/declared.rs:3473`; `spec/A_command_registry.md:362` | 1 | C12-R2-02 |
| `colour.palette.get` | divergent → divergent | `crates/registry/src/declared.rs:3498`; `spec/A_command_registry.md:363` | 1 | C12-R2-02 |
| `colour.palette.import` | divergent → divergent | `crates/registry/src/declared.rs:3523`; `spec/A_command_registry.md:364` | 1 | C12-R2-02 |
| `colour.palette.list` | divergent → divergent | `crates/registry/src/declared.rs:3548`; `spec/A_command_registry.md:365` | 1 | C12-R2-02 |
| `colour.palette.rename` | divergent → divergent | `crates/registry/src/declared.rs:3570`; `spec/A_command_registry.md:366` | 1 | C12-R2-02 |
| `colour.palette.select` | divergent → divergent | `crates/registry/src/declared.rs:3595`; `spec/A_command_registry.md:367` | 1 | C12-R2-02 |
| `colour.palette.set` | divergent → divergent | `crates/registry/src/declared.rs:3620`; `spec/A_command_registry.md:368` | 1 | C12-R2-02 |
| `colour.pick` | divergent → divergent | `crates/registry/src/declared.rs:3645`; `spec/A_command_registry.md:369` | 1 | C12-R2-02 |
| `colour.recolour.apply` | divergent → divergent | `crates/registry/src/declared.rs:3670`; `spec/A_command_registry.md:370` | 1 | C12-R2-02 |
| `colour.recovery.performed` | divergent → divergent | `crates/registry/src/declared.rs:3695`; `spec/A_command_registry.md:371` | 1 | C12-R2-02 |
| `colour.recovery.run` | divergent → divergent | `crates/registry/src/declared.rs:3714`; `spec/A_command_registry.md:372` | 1 | C12-R2-02 |
| `colour.sceneStyle.changed` | divergent → divergent | `crates/registry/src/declared.rs:3739`; `spec/A_command_registry.md:373` | 1 | C12-R2-02 |
| `colour.sceneStyles.get` | divergent → divergent | `crates/registry/src/declared.rs:3758`; `spec/A_command_registry.md:374` | 1 | C12-R2-02 |
| `colour.selection.apply` | divergent → divergent | `crates/registry/src/declared.rs:3780`; `spec/A_command_registry.md:375` | 1 | C12-R2-02 |
| `colour.style.changed` | divergent → divergent | `crates/registry/src/declared.rs:3805`; `spec/A_command_registry.md:376` | 1 | C12-R2-02 |
| `colour.style.delete` | divergent → divergent | `crates/registry/src/declared.rs:3824`; `spec/A_command_registry.md:377` | 1 | C12-R2-02 |
| `colour.style.duplicate` | divergent → divergent | `crates/registry/src/declared.rs:3849`; `spec/A_command_registry.md:378` | 1 | C12-R2-02 |
| `colour.style.list` | divergent → divergent | `crates/registry/src/declared.rs:3874`; `spec/A_command_registry.md:379` | 1 | C12-R2-02 |
| `colour.style.rename` | divergent → divergent | `crates/registry/src/declared.rs:3899`; `spec/A_command_registry.md:380` | 1 | C12-R2-02 |
| `colour.style.select` | divergent → divergent | `crates/registry/src/declared.rs:3924`; `spec/A_command_registry.md:381` | 1 | C12-R2-02 |
| `colour.swatch.changed` | divergent → divergent | `crates/registry/src/declared.rs:3949`; `spec/A_command_registry.md:382` | 1 | C12-R2-02 |
| `colour.swatch.create` | divergent → divergent | `crates/registry/src/declared.rs:3968`; `spec/A_command_registry.md:383` | 1 | C12-R2-02 |
| `colour.swatch.delete` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:3993`; `spec/A_command_registry.md:384` | 1 | M7 C7.1 |
| `colour.swatch.get` | divergent → divergent | `crates/registry/src/declared.rs:4018`; `spec/A_command_registry.md:385` | 1 | C12-R2-02 |
| `colour.swatch.list` | divergent → divergent | `crates/registry/src/declared.rs:4043`; `spec/A_command_registry.md:386` | 1 | C12-R2-02 |
| `colour.swatch.move` | divergent → divergent | `crates/registry/src/declared.rs:4068`; `spec/A_command_registry.md:387` | 1 | C12-R2-02 |
| `colour.swatch.set` | divergent → divergent | `crates/registry/src/declared.rs:4093`; `spec/A_command_registry.md:388` | 1 | C12-R2-02 |
| `colour.swatchNames.set` | divergent → divergent | `crates/registry/src/declared.rs:4118`; `spec/A_command_registry.md:389` | 1 | C12-R2-02 |
| `colour.tintBlend.apply` | divergent → divergent | `crates/registry/src/declared.rs:4143`; `spec/A_command_registry.md:390` | 1 | C12-R2-02 |
| `colour.tintDialog.open` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:4168`; `spec/A_command_registry.md:391` | 1 | M7 C7.1 |
| `colour.tintOffset.apply` | divergent → divergent | `crates/registry/src/declared.rs:4193`; `spec/A_command_registry.md:392` | 1 | C12-R2-02 |
| `colour.usage.list` | divergent → divergent | `crates/registry/src/declared.rs:4218`; `spec/A_command_registry.md:393` | 1 | C12-R2-02 |
| `doc.asset.embed` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:1902`; `spec/A_command_registry.md:1002` | 1 | M1 C1.3 |
| `doc.asset.export` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:1931`; `spec/A_command_registry.md:1003` | 1 | M1 C1.3 |
| `doc.asset.list` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:1875`; `spec/A_command_registry.md:1004` | 1 | M1 C1.3 |
| `doc.asset.relink` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:1980`; `spec/A_command_registry.md:1005` | 1 | M1 C1.3 |
| `doc.asset.remove` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:2011`; `spec/A_command_registry.md:1006` | 1 | M1 C1.3 |
| `doc.asset.status.changed` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:2099`; `spec/A_command_registry.md:1007` | 1 | M1 C1.3 |
| `doc.blob.get` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:1808`; `spec/A_command_registry.md:1008` | 1 | M1 C1.3 |
| `doc.changed` | implemented → implemented | `crates/executor/src/commands/doc.rs:1349`; `spec/A_command_registry.md:1009` | 1 | M1 C1.2 |
| `doc.diff` | divergent → divergent | `crates/executor/src/commands/doc.rs:1241`; `spec/A_command_registry.md:1010` | 1 | C12-R2-12 |
| `doc.export.json` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:1674`; `spec/A_command_registry.md:1011` | 1 | M1 C1.3 |
| `doc.import.json` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:1758`; `spec/A_command_registry.md:1012` | 1 | M1 C1.3 |
| `doc.loadReport.get` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:1591`; `spec/A_command_registry.md:1013` | 1 | M1 C1.3 |
| `doc.object.find` | divergent → implemented | `crates/executor/src/commands/doc.rs:445`; `spec/A_command_registry.md:1014` | 3 | M1 C1.2 |
| `doc.object.get` | implemented → implemented | `crates/executor/src/commands/doc.rs:137`; `spec/A_command_registry.md:1015` | 3 | M1 C1.2 |
| `doc.object.patch` | divergent → divergent | `crates/executor/src/commands/doc.rs:860`; `spec/A_command_registry.md:1016` | 4 | C12-R2-11 |
| `doc.object.refs` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:1420`; `spec/A_command_registry.md:1017` | 1 | M1 C1.3 |
| `doc.object.resolve` | implemented → implemented | `crates/executor/src/commands/doc.rs:566`; `spec/A_command_registry.md:1018` | 1 | M1 C1.2 |
| `doc.package.changedOnDisk` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:2112`; `spec/A_command_registry.md:1019` | 1 | M1 C1.3 |
| `doc.package.export` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:2085`; `spec/A_command_registry.md:1020` | 1 | M1 C1.3 |
| `doc.package.get` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:2038`; `spec/A_command_registry.md:1021` | 1 | M1 C1.3 |
| `doc.repair` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:1576`; `spec/A_command_registry.md:1022` | 1 | M1 C1.3 |
| `doc.schema.get` | implemented → implemented | `crates/executor/src/commands/doc.rs:1065`; `spec/A_command_registry.md:1023` | 2 | M1 C1.2 |
| `doc.schema.list` | implemented → implemented | `crates/executor/src/commands/doc.rs:1016`; `spec/A_command_registry.md:1024` | 1 | M1 C1.2 |
| `doc.schema.validate` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:1469`; `spec/A_command_registry.md:1025` | 1 | M1 C1.3 |
| `doc.state.get` | implemented → implemented | `crates/executor/src/commands/doc.rs:1115`; `spec/A_command_registry.md:1026` | 1 | M1 C1.2 |
| `doc.validate` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/doc.rs:1529`; `spec/A_command_registry.md:1027` | 1 | M1 C1.3 |
| `draw.arrange.apply` | divergent → divergent | `crates/registry/src/declared.rs:758`; `spec/A_command_registry.md:227` | 1 | C12-R2-02 |
| `draw.bounds.get` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:783`; `spec/A_command_registry.md:228` | 1 | M2 C2.3 |
| `draw.breakApart.apply` | divergent → divergent | `crates/registry/src/declared.rs:808`; `spec/A_command_registry.md:229` | 1 | C12-R2-02 |
| `draw.centreLine.apply` | divergent → divergent | `crates/registry/src/declared.rs:833`; `spec/A_command_registry.md:230` | 1 | C12-R2-02 |
| `draw.content.add` | divergent → divergent | `crates/registry/src/declared.rs:858`; `spec/A_command_registry.md:231` | 1 | C12-R2-02 |
| `draw.current.get` | divergent → divergent | `crates/registry/src/declared.rs:883`; `spec/A_command_registry.md:232` | 1 | C12-R2-02 |
| `draw.cut.apply` | divergent → divergent | `crates/registry/src/declared.rs:905`; `spec/A_command_registry.md:233` | 1 | C12-R2-02 |
| `draw.drawing.changed` | divergent → divergent | `crates/registry/src/declared.rs:930`; `spec/A_command_registry.md:234` | 1 | C12-R2-02 |
| `draw.drawing.get` | divergent → divergent | `crates/registry/src/declared.rs:949`; `spec/A_command_registry.md:235` | 1 | C12-R2-02 |
| `draw.drawing.rename` | divergent → divergent | `crates/registry/src/declared.rs:971`; `spec/A_command_registry.md:236` | 1 | C12-R2-02 |
| `draw.drawing.render` | divergent → divergent | `crates/registry/src/declared.rs:996`; `spec/A_command_registry.md:237` | 1 | C12-R2-02 |
| `draw.ellipse.add` | divergent → divergent | `crates/registry/src/declared.rs:1021`; `spec/A_command_registry.md:238` | 1 | C12-R2-02 |
| `draw.envelope.apply` | divergent → divergent | `crates/registry/src/declared.rs:1046`; `spec/A_command_registry.md:239` | 1 | C12-R2-02 |
| `draw.erase.apply` | divergent → divergent | `crates/registry/src/declared.rs:1071`; `spec/A_command_registry.md:240` | 1 | C12-R2-02 |
| `draw.feather.apply` | divergent → divergent | `crates/registry/src/declared.rs:1096`; `spec/A_command_registry.md:241` | 1 | C12-R2-02 |
| `draw.flatten.apply` | divergent → divergent | `crates/registry/src/declared.rs:1121`; `spec/A_command_registry.md:242` | 1 | C12-R2-02 |
| `draw.flip.apply` | divergent → divergent | `crates/registry/src/declared.rs:1146`; `spec/A_command_registry.md:243` | 1 | C12-R2-02 |
| `draw.group.create` | divergent → divergent | `crates/registry/src/declared.rs:1171`; `spec/A_command_registry.md:244` | 1 | C12-R2-02 |
| `draw.group.delete` | divergent → divergent | `crates/registry/src/declared.rs:1193`; `spec/A_command_registry.md:245` | 1 | C12-R2-02 |
| `draw.hitTest.get` | divergent → divergent | `crates/registry/src/declared.rs:1218`; `spec/A_command_registry.md:246` | 1 | C12-R2-02 |
| `draw.inspectorPreview.set` | divergent → divergent | `crates/registry/src/declared.rs:1243`; `spec/A_command_registry.md:247` | 1 | C12-R2-02 |
| `draw.line.add` | divergent → divergent | `crates/registry/src/declared.rs:1268`; `spec/A_command_registry.md:248` | 1 | C12-R2-02 |
| `draw.linesToBrush.apply` | divergent → divergent | `crates/registry/src/declared.rs:1293`; `spec/A_command_registry.md:249` | 1 | C12-R2-02 |
| `draw.lock.set` | divergent → divergent | `crates/registry/src/declared.rs:1318`; `spec/A_command_registry.md:250` | 1 | C12-R2-02 |
| `draw.object.delete` | divergent → divergent | `crates/registry/src/declared.rs:1343`; `spec/A_command_registry.md:251` | 1 | C12-R2-02 |
| `draw.optimize.apply` | divergent → divergent | `crates/registry/src/declared.rs:1365`; `spec/A_command_registry.md:252` | 1 | C12-R2-02 |
| `draw.pivot.set` | divergent → divergent | `crates/registry/src/declared.rs:1390`; `spec/A_command_registry.md:253` | 1 | C12-R2-02 |
| `draw.point.add` | divergent → divergent | `crates/registry/src/declared.rs:1415`; `spec/A_command_registry.md:254` | 1 | C12-R2-02 |
| `draw.point.delete` | divergent → divergent | `crates/registry/src/declared.rs:1440`; `spec/A_command_registry.md:255` | 1 | C12-R2-02 |
| `draw.point.move` | divergent → divergent | `crates/registry/src/declared.rs:1465`; `spec/A_command_registry.md:256` | 1 | C12-R2-02 |
| `draw.point.set` | divergent → divergent | `crates/registry/src/declared.rs:1490`; `spec/A_command_registry.md:257` | 1 | C12-R2-02 |
| `draw.pointSelection.set` | divergent → divergent | `crates/registry/src/declared.rs:1515`; `spec/A_command_registry.md:258` | 1 | C12-R2-02 |
| `draw.polyline.add` | divergent → divergent | `crates/registry/src/declared.rs:1540`; `spec/A_command_registry.md:259` | 1 | C12-R2-02 |
| `draw.rectangle.add` | divergent → divergent | `crates/registry/src/declared.rs:1565`; `spec/A_command_registry.md:260` | 1 | C12-R2-02 |
| `draw.repositionAll.apply` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:1590`; `spec/A_command_registry.md:261` | 1 | M2 C2.3 |
| `draw.rotate.apply` | divergent → divergent | `crates/registry/src/declared.rs:1615`; `spec/A_command_registry.md:262` | 1 | C12-R2-02 |
| `draw.segment.move` | divergent → divergent | `crates/registry/src/declared.rs:1640`; `spec/A_command_registry.md:263` | 1 | C12-R2-02 |
| `draw.selection.changed` | divergent → divergent | `crates/registry/src/declared.rs:1665`; `spec/A_command_registry.md:264` | 1 | C12-R2-02 |
| `draw.selection.get` | divergent → divergent | `crates/registry/src/declared.rs:1684`; `spec/A_command_registry.md:265` | 1 | C12-R2-02 |
| `draw.selection.set` | divergent → divergent | `crates/registry/src/declared.rs:1706`; `spec/A_command_registry.md:266` | 1 | C12-R2-02 |
| `draw.smooth.apply` | divergent → divergent | `crates/registry/src/declared.rs:1731`; `spec/A_command_registry.md:267` | 1 | C12-R2-02 |
| `draw.stroke.add` | divergent → divergent | `crates/registry/src/declared.rs:1756`; `spec/A_command_registry.md:268` | 1 | C12-R2-02 |
| `draw.stroke.set` | divergent → divergent | `crates/registry/src/declared.rs:1781`; `spec/A_command_registry.md:269` | 1 | C12-R2-02 |
| `draw.text.add` | divergent → divergent | `crates/registry/src/declared.rs:1806`; `spec/A_command_registry.md:270` | 1 | C12-R2-02 |
| `draw.text.set` | divergent → divergent | `crates/registry/src/declared.rs:1831`; `spec/A_command_registry.md:271` | 1 | C12-R2-02 |
| `draw.textFormat.set` | divergent → divergent | `crates/registry/src/declared.rs:1856`; `spec/A_command_registry.md:272` | 1 | C12-R2-02 |
| `draw.transform.apply` | divergent → divergent | `crates/registry/src/declared.rs:1881`; `spec/A_command_registry.md:273` | 1 | C12-R2-02 |
| `draw.vectorFile.import` | divergent → divergent | `crates/registry/src/declared.rs:1906`; `spec/A_command_registry.md:274` | 1 | C12-R2-02 |
| `draw.vectorize.preview` | divergent → divergent | `crates/registry/src/declared.rs:1931`; `spec/A_command_registry.md:275` | 1 | C12-R2-02 |
| `draw.vectorize.run` | divergent → divergent | `crates/registry/src/declared.rs:1953`; `spec/A_command_registry.md:276` | 1 | C12-R2-02 |
| `draw.viewMapping.get` | divergent → divergent | `crates/registry/src/declared.rs:1978`; `spec/A_command_registry.md:277` | 1 | C12-R2-02 |
| `drawAid.autoLightTable.set` | divergent → divergent | `crates/registry/src/declared.rs:2630`; `spec/A_command_registry.md:319` | 1 | C12-R2-02 |
| `drawAid.background.set` | divergent → divergent | `crates/registry/src/declared.rs:2652`; `spec/A_command_registry.md:320` | 1 | C12-R2-02 |
| `drawAid.changed` | divergent → divergent | `crates/registry/src/declared.rs:2674`; `spec/A_command_registry.md:321` | 1 | C12-R2-02 |
| `drawAid.currentOnTop.set` | divergent → divergent | `crates/registry/src/declared.rs:2693`; `spec/A_command_registry.md:322` | 1 | C12-R2-02 |
| `drawAid.grid.set` | divergent → divergent | `crates/registry/src/declared.rs:2718`; `spec/A_command_registry.md:323` | 1 | C12-R2-02 |
| `drawAid.lightTableShade.set` | divergent → divergent | `crates/registry/src/declared.rs:2740`; `spec/A_command_registry.md:324` | 1 | C12-R2-02 |
| `drawAid.onionSkin.set` | divergent → divergent | `crates/registry/src/declared.rs:2762`; `spec/A_command_registry.md:325` | 1 | C12-R2-02 |
| `drawAid.onionSkinCycle.run` | divergent → divergent | `crates/registry/src/declared.rs:2784`; `spec/A_command_registry.md:326` | 1 | C12-R2-02 |
| `drawAid.onionSkinStyle.set` | divergent → divergent | `crates/registry/src/declared.rs:2809`; `spec/A_command_registry.md:327` | 1 | C12-R2-02 |
| `drawAid.selectionDisplay.set` | divergent → divergent | `crates/registry/src/declared.rs:2831`; `spec/A_command_registry.md:328` | 1 | C12-R2-02 |
| `drawAid.state.get` | divergent → divergent | `crates/registry/src/declared.rs:2853`; `spec/A_command_registry.md:329` | 1 | C12-R2-02 |
| `drawAid.staticLightTable.add` | divergent → divergent | `crates/registry/src/declared.rs:2875`; `spec/A_command_registry.md:330` | 1 | C12-R2-02 |
| `drawAid.staticLightTable.list` | divergent → divergent | `crates/registry/src/declared.rs:2900`; `spec/A_command_registry.md:331` | 1 | C12-R2-02 |
| `drawAid.staticLightTable.remove` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:2922`; `spec/A_command_registry.md:332` | 1 | M5 C5.2 |
| `drawAid.staticLightTable.set` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:2944`; `spec/A_command_registry.md:333` | 1 | M5 C5.2 |
| `drawAid.staticLightTablePanel.set` | divergent → divergent | `crates/registry/src/declared.rs:2969`; `spec/A_command_registry.md:334` | 1 | C12-R2-02 |
| `drawAid.strokes.set` | divergent → divergent | `crates/registry/src/declared.rs:2994`; `spec/A_command_registry.md:335` | 1 | C12-R2-02 |
| `edit.clipboard.changed` | divergent → implemented | `crates/executor/src/commands/edit.rs:880`; `spec/A_command_registry.md:98` | 1 | M1 C1.2 |
| `edit.clipboard.get` | divergent → implemented | `crates/executor/src/commands/edit.rs:815`; `spec/A_command_registry.md:99` | 1 | M1 C1.2 |
| `edit.clipboard.set` | divergent → divergent | `crates/executor/src/commands/edit.rs:864`; `spec/A_command_registry.md:100` | 1 | C12-R2-10 |
| `edit.copy` | divergent → divergent | `crates/executor/src/commands/edit_ops.rs:1341`; `spec/A_command_registry.md:101` | 1 | C12-R2-05, C12-R2-08, C12-R2-13 |
| `edit.cut` | divergent → divergent | `crates/executor/src/commands/edit_ops.rs:1323`; `spec/A_command_registry.md:102` | 1 | C12-R2-05, C12-R2-08, C12-R2-13 |
| `edit.cutSpecial` | divergent → divergent | `crates/executor/src/commands/edit_ops.rs:1432`; `spec/A_command_registry.md:103` | 1 | C12-R2-05, C12-R2-08 |
| `edit.delete` | divergent → divergent | `crates/executor/src/commands/edit_ops.rs:1365`; `spec/A_command_registry.md:104` | 1 | C12-R2-05, C12-R2-08 |
| `edit.deselectAll` | implemented → implemented | `crates/executor/src/commands/edit.rs:572`; `spec/A_command_registry.md:105` | 1 | M1 C1.2 |
| `edit.history.changed` | implemented → implemented | `crates/executor/src/commands/edit.rs:280`; `spec/A_command_registry.md:106` | 1 | M1 C1.2 |
| `edit.history.get` | implemented → implemented | `crates/executor/src/commands/edit.rs:182`; `spec/A_command_registry.md:107` | 1 | M1 C1.2 |
| `edit.history.move` | implemented → implemented | `crates/executor/src/commands/edit.rs:235`; `spec/A_command_registry.md:108` | 1 | M1 C1.2 |
| `edit.paste` | divergent → divergent | `crates/executor/src/commands/edit_ops.rs:1394`; `spec/A_command_registry.md:109` | 1 | C12-R2-04, C12-R2-05, C12-R2-08, C12-R2-09, C12-R2-13, C12-R2-14 |
| `edit.pasteAsNewElement` | divergent → divergent | `crates/executor/src/commands/edit_ops.rs:1550`; `spec/A_command_registry.md:110` | 1 | C12-R2-04, C12-R2-08 |
| `edit.pasteSpecial` | divergent → divergent | `crates/executor/src/commands/edit_ops.rs:1491`; `spec/A_command_registry.md:111` | 1 | C12-R2-04, C12-R2-05, C12-R2-08, C12-R2-09, C12-R2-13, C12-R2-14 |
| `edit.redo` | implemented → implemented | `crates/executor/src/commands/edit.rs:112`; `spec/A_command_registry.md:112` | 1 | M1 C1.2 |
| `edit.selectAll` | implemented → implemented | `crates/executor/src/commands/edit.rs:550`; `spec/A_command_registry.md:113` | 1 | M1 C1.2 |
| `edit.selection.changed` | implemented → implemented | `crates/executor/src/commands/edit.rs:587`; `spec/A_command_registry.md:114` | 1 | M1 C1.2 |
| `edit.selection.get` | implemented → implemented | `crates/executor/src/commands/edit.rs:324`; `spec/A_command_registry.md:115` | 1 | M1 C1.2 |
| `edit.selection.set` | implemented → implemented | `crates/executor/src/commands/edit.rs:398`; `spec/A_command_registry.md:116` | 1 | M1 C1.2 |
| `edit.undo` | implemented → implemented | `crates/executor/src/commands/edit.rs:69`; `spec/A_command_registry.md:117` | 1 | M1 C1.2 |
| `element.annotation.set` | divergent → divergent | `crates/registry/src/declared.rs:4243`; `spec/A_command_registry.md:401` | 1 | C12-R2-02 |
| `element.childExposure.apply` | divergent → divergent | `crates/registry/src/declared.rs:4268`; `spec/A_command_registry.md:402` | 1 | C12-R2-02 |
| `element.clone` | divergent → divergent | `crates/registry/src/declared.rs:4293`; `spec/A_command_registry.md:403` | 1 | C12-R2-02 |
| `element.collapsed.set` | divergent → divergent | `crates/registry/src/declared.rs:4318`; `spec/A_command_registry.md:404` | 1 | C12-R2-02 |
| `element.colour.set` | divergent → divergent | `crates/registry/src/declared.rs:4343`; `spec/A_command_registry.md:405` | 1 | C12-R2-02 |
| `element.create` | divergent → divergent | `crates/registry/src/declared.rs:4368`; `spec/A_command_registry.md:406` | 1 | C12-R2-02 |
| `element.current.move` | divergent → divergent | `crates/registry/src/declared.rs:4393`; `spec/A_command_registry.md:407` | 1 | C12-R2-02 |
| `element.current.set` | divergent → divergent | `crates/registry/src/declared.rs:4418`; `spec/A_command_registry.md:408` | 1 | C12-R2-02 |
| `element.delete` | divergent → divergent | `crates/registry/src/declared.rs:4443`; `spec/A_command_registry.md:409` | 1 | C12-R2-02 |
| `element.drawing.changed` | divergent → divergent | `crates/registry/src/declared.rs:4468`; `spec/A_command_registry.md:410` | 1 | C12-R2-02 |
| `element.drawing.create` | divergent → divergent | `crates/registry/src/declared.rs:4487`; `spec/A_command_registry.md:411` | 1 | C12-R2-02 |
| `element.drawing.delete` | divergent → divergent | `crates/registry/src/declared.rs:4512`; `spec/A_command_registry.md:412` | 1 | C12-R2-02 |
| `element.drawing.duplicate` | divergent → divergent | `crates/registry/src/declared.rs:4537`; `spec/A_command_registry.md:413` | 1 | C12-R2-02 |
| `element.drawing.list` | divergent → divergent | `crates/registry/src/declared.rs:4562`; `spec/A_command_registry.md:414` | 1 | C12-R2-02 |
| `element.drawing.rename` | divergent → divergent | `crates/registry/src/declared.rs:4587`; `spec/A_command_registry.md:415` | 1 | C12-R2-02 |
| `element.drawingContent.delete` | divergent → divergent | `crates/registry/src/declared.rs:4612`; `spec/A_command_registry.md:416` | 1 | C12-R2-02 |
| `element.drawingLock.set` | divergent → divergent | `crates/registry/src/declared.rs:4637`; `spec/A_command_registry.md:417` | 1 | C12-R2-02 |
| `element.duplicate` | divergent → divergent | `crates/registry/src/declared.rs:4662`; `spec/A_command_registry.md:418` | 1 | C12-R2-02 |
| `element.get` | divergent → divergent | `crates/registry/src/declared.rs:4687`; `spec/A_command_registry.md:419` | 1 | C12-R2-02 |
| `element.hierarchy.get` | divergent → divergent | `crates/registry/src/declared.rs:4712`; `spec/A_command_registry.md:420` | 1 | C12-R2-02 |
| `element.hierarchy.select` | divergent → divergent | `crates/registry/src/declared.rs:4737`; `spec/A_command_registry.md:421` | 1 | C12-R2-02 |
| `element.list` | divergent → divergent | `crates/registry/src/declared.rs:4762`; `spec/A_command_registry.md:422` | 1 | C12-R2-02 |
| `element.lock.set` | divergent → divergent | `crates/registry/src/declared.rs:4787`; `spec/A_command_registry.md:423` | 1 | C12-R2-02 |
| `element.mediaLength.set` | divergent → divergent | `crates/registry/src/declared.rs:4812`; `spec/A_command_registry.md:424` | 1 | C12-R2-02 |
| `element.note.set` | divergent → divergent | `crates/registry/src/declared.rs:4837`; `spec/A_command_registry.md:425` | 1 | C12-R2-02 |
| `element.order.move` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:4862`; `spec/A_command_registry.md:426` | 1 | M3 C3.1 |
| `element.order.set` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:4887`; `spec/A_command_registry.md:427` | 1 | M3 C3.1 |
| `element.parent.remove` | divergent → divergent | `crates/registry/src/declared.rs:4912`; `spec/A_command_registry.md:428` | 1 | C12-R2-02 |
| `element.parent.set` | divergent → divergent | `crates/registry/src/declared.rs:4937`; `spec/A_command_registry.md:429` | 1 | C12-R2-02 |
| `element.parentPeg.create` | divergent → divergent | `crates/registry/src/declared.rs:4962`; `spec/A_command_registry.md:430` | 1 | C12-R2-02 |
| `element.properties.set` | divergent → divergent | `crates/registry/src/declared.rs:4987`; `spec/A_command_registry.md:431` | 1 | C12-R2-02 |
| `element.rename` | divergent → divergent | `crates/registry/src/declared.rs:5012`; `spec/A_command_registry.md:432` | 1 | C12-R2-02 |
| `element.select` | divergent → divergent | `crates/registry/src/declared.rs:5037`; `spec/A_command_registry.md:433` | 1 | C12-R2-02 |
| `element.solo.set` | divergent → divergent | `crates/registry/src/declared.rs:5062`; `spec/A_command_registry.md:434` | 1 | C12-R2-02 |
| `element.state.changed` | divergent → divergent | `crates/registry/src/declared.rs:5087`; `spec/A_command_registry.md:435` | 1 | C12-R2-02 |
| `element.timing.set` | divergent → divergent | `crates/registry/src/declared.rs:5106`; `spec/A_command_registry.md:436` | 1 | C12-R2-02 |
| `element.typeColour.get` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:5131`; `spec/A_command_registry.md:437` | 1 | M3 C3.1 |
| `element.typeColour.set` | divergent → divergent | `crates/registry/src/declared.rs:5153`; `spec/A_command_registry.md:438` | 1 | C12-R2-02 |
| `element.unusedDrawing.list` | divergent → divergent | `crates/registry/src/declared.rs:5178`; `spec/A_command_registry.md:439` | 1 | C12-R2-02 |
| `element.visibility.set` | divergent → divergent | `crates/registry/src/declared.rs:5203`; `spec/A_command_registry.md:440` | 1 | C12-R2-02 |
| `export.drawingPdf.run` | divergent → divergent | `crates/registry/src/declared.rs:14584`; `spec/A_command_registry.md:931` | 1 | C12-R2-02 |
| `export.formats.list` | divergent → divergent | `crates/registry/src/declared.rs:14609`; `spec/A_command_registry.md:932` | 1 | C12-R2-02 |
| `export.job.finished` | divergent → divergent | `crates/registry/src/declared.rs:14634`; `spec/A_command_registry.md:933` | 1 | C12-R2-02 |
| `export.movie.defaults.get` | divergent → divergent | `crates/registry/src/declared.rs:14653`; `spec/A_command_registry.md:934` | 1 | C12-R2-02 |
| `export.movie.defaults.set` | divergent → divergent | `crates/registry/src/declared.rs:14678`; `spec/A_command_registry.md:935` | 1 | C12-R2-02 |
| `export.movie.run` | divergent → divergent | `crates/registry/src/declared.rs:14703`; `spec/A_command_registry.md:936` | 1 | C12-R2-02 |
| `export.movie.validate` | divergent → divergent | `crates/registry/src/declared.rs:14728`; `spec/A_command_registry.md:937` | 1 | C12-R2-02 |
| `export.publish.account.close` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:14753`; `spec/A_command_registry.md:938` | 1 | M10 C10.2 |
| `export.publish.account.get` | divergent → divergent | `crates/registry/src/declared.rs:14778`; `spec/A_command_registry.md:939` | 1 | C12-R2-02 |
| `export.publish.account.open` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:14803`; `spec/A_command_registry.md:940` | 1 | M10 C10.2 |
| `export.publish.categories.list` | divergent → divergent | `crates/registry/src/declared.rs:14828`; `spec/A_command_registry.md:941` | 1 | C12-R2-02 |
| `export.publish.run` | divergent → divergent | `crates/registry/src/declared.rs:14853`; `spec/A_command_registry.md:942` | 1 | C12-R2-02 |
| `export.publish.service.list` | divergent → divergent | `crates/registry/src/declared.rs:14881`; `spec/A_command_registry.md:943` | 1 | C12-R2-02 |
| `export.publish.validate` | divergent → divergent | `crates/registry/src/declared.rs:14906`; `spec/A_command_registry.md:944` | 1 | C12-R2-02 |
| `export.report.get` | divergent → divergent | `crates/registry/src/declared.rs:14931`; `spec/A_command_registry.md:945` | 1 | C12-R2-02 |
| `export.settings.get` | divergent → divergent | `crates/registry/src/declared.rs:14956`; `spec/A_command_registry.md:946` | 1 | C12-R2-02 |
| `export.settings.set` | divergent → divergent | `crates/registry/src/declared.rs:14981`; `spec/A_command_registry.md:947` | 1 | C12-R2-02 |
| `export.snapshot.run` | divergent → divergent | `crates/registry/src/declared.rs:15006`; `spec/A_command_registry.md:948` | 1 | C12-R2-02 |
| `fx.allEffects.changed` | divergent → divergent | `crates/registry/src/declared.rs:8830`; `spec/A_command_registry.md:625` | 1 | C12-R2-02 |
| `fx.allEffects.get` | divergent → divergent | `crates/registry/src/declared.rs:8849`; `spec/A_command_registry.md:626` | 1 | C12-R2-02 |
| `fx.allEffects.set` | divergent → divergent | `crates/registry/src/declared.rs:8871`; `spec/A_command_registry.md:627` | 1 | C12-R2-02 |
| `fx.block.add` | divergent → divergent | `crates/registry/src/declared.rs:8896`; `spec/A_command_registry.md:628` | 1 | C12-R2-02 |
| `fx.block.defaults.apply` | divergent → divergent | `crates/registry/src/declared.rs:8921`; `spec/A_command_registry.md:629` | 1 | C12-R2-02 |
| `fx.block.get` | divergent → divergent | `crates/registry/src/declared.rs:8946`; `spec/A_command_registry.md:630` | 1 | C12-R2-02 |
| `fx.block.list` | divergent → divergent | `crates/registry/src/declared.rs:8971`; `spec/A_command_registry.md:631` | 1 | C12-R2-02 |
| `fx.block.remove` | divergent → divergent | `crates/registry/src/declared.rs:8996`; `spec/A_command_registry.md:632` | 1 | C12-R2-02 |
| `fx.block.seed.set` | divergent → divergent | `crates/registry/src/declared.rs:9021`; `spec/A_command_registry.md:633` | 1 | C12-R2-02 |
| `fx.block.set` | divergent → divergent | `crates/registry/src/declared.rs:9046`; `spec/A_command_registry.md:634` | 1 | C12-R2-02 |
| `fx.block.type.set` | divergent → divergent | `crates/registry/src/declared.rs:9071`; `spec/A_command_registry.md:635` | 1 | C12-R2-02 |
| `fx.changed` | divergent → divergent | `crates/registry/src/declared.rs:9096`; `spec/A_command_registry.md:636` | 1 | C12-R2-02 |
| `fx.create` | divergent → divergent | `crates/registry/src/declared.rs:9115`; `spec/A_command_registry.md:637` | 1 | C12-R2-02 |
| `fx.curve.get` | divergent → divergent | `crates/registry/src/declared.rs:9140`; `spec/A_command_registry.md:638` | 1 | C12-R2-02 |
| `fx.dropShadow.transform.set` | divergent → divergent | `crates/registry/src/declared.rs:9165`; `spec/A_command_registry.md:639` | 1 | C12-R2-02 |
| `fx.get` | divergent → divergent | `crates/registry/src/declared.rs:9190`; `spec/A_command_registry.md:640` | 1 | C12-R2-02 |
| `fx.hitTest.run` | divergent → divergent | `crates/registry/src/declared.rs:9215`; `spec/A_command_registry.md:641` | 1 | C12-R2-02 |
| `fx.keyframe.add` | divergent → divergent | `crates/registry/src/declared.rs:9240`; `spec/A_command_registry.md:642` | 1 | C12-R2-02 |
| `fx.keyframe.list` | divergent → divergent | `crates/registry/src/declared.rs:9265`; `spec/A_command_registry.md:643` | 1 | C12-R2-02 |
| `fx.keyframe.remove` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:9290`; `spec/A_command_registry.md:644` | 1 | M7 C7.2 |
| `fx.list` | divergent → divergent | `crates/registry/src/declared.rs:9315`; `spec/A_command_registry.md:645` | 1 | C12-R2-02 |
| `fx.paramElement.add` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:9337`; `spec/A_command_registry.md:646` | 1 | M7 C7.2 |
| `fx.paramElement.list` | divergent → divergent | `crates/registry/src/declared.rs:9362`; `spec/A_command_registry.md:647` | 1 | C12-R2-02 |
| `fx.paramElement.remove` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:9387`; `spec/A_command_registry.md:648` | 1 | M7 C7.2 |
| `fx.paramRows.get` | divergent → divergent | `crates/registry/src/declared.rs:9412`; `spec/A_command_registry.md:649` | 1 | C12-R2-02 |
| `fx.paramRows.set` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:9434`; `spec/A_command_registry.md:650` | 1 | M7 C7.2 |
| `fx.params.get` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:9459`; `spec/A_command_registry.md:651` | 1 | M7 C7.2 |
| `fx.params.set` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:9484`; `spec/A_command_registry.md:652` | 1 | M7 C7.2 |
| `fx.particles.get` | divergent → divergent | `crates/registry/src/declared.rs:9509`; `spec/A_command_registry.md:653` | 1 | C12-R2-02 |
| `fx.preset.apply` | divergent → divergent | `crates/registry/src/declared.rs:9534`; `spec/A_command_registry.md:654` | 1 | C12-R2-02 |
| `fx.preset.changed` | divergent → divergent | `crates/registry/src/declared.rs:9559`; `spec/A_command_registry.md:655` | 1 | C12-R2-02 |
| `fx.preset.create` | divergent → divergent | `crates/registry/src/declared.rs:9578`; `spec/A_command_registry.md:656` | 1 | C12-R2-02 |
| `fx.preset.delete` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:9603`; `spec/A_command_registry.md:657` | 1 | M7 C7.2 |
| `fx.preset.export` | divergent → divergent | `crates/registry/src/declared.rs:9628`; `spec/A_command_registry.md:658` | 1 | C12-R2-02 |
| `fx.preset.get` | divergent → divergent | `crates/registry/src/declared.rs:9653`; `spec/A_command_registry.md:659` | 1 | C12-R2-02 |
| `fx.preset.import` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:9678`; `spec/A_command_registry.md:660` | 1 | M7 C7.2 |
| `fx.preset.list` | divergent → divergent | `crates/registry/src/declared.rs:9703`; `spec/A_command_registry.md:661` | 1 | C12-R2-02 |
| `fx.preset.rename` | divergent → divergent | `crates/registry/src/declared.rs:9725`; `spec/A_command_registry.md:662` | 1 | C12-R2-02 |
| `fx.preview.cache.delete` | divergent → divergent | `crates/registry/src/declared.rs:9750`; `spec/A_command_registry.md:663` | 1 | C12-R2-02 |
| `fx.preview.close` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:9772`; `spec/A_command_registry.md:664` | 1 | M7 C7.3 |
| `fx.preview.frame.get` | divergent → divergent | `crates/registry/src/declared.rs:9794`; `spec/A_command_registry.md:665` | 1 | C12-R2-02 |
| `fx.preview.frameRendered` | divergent → divergent | `crates/registry/src/declared.rs:9816`; `spec/A_command_registry.md:666` | 1 | C12-R2-02 |
| `fx.preview.invalidated` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:9835`; `spec/A_command_registry.md:667` | 1 | M7 C7.3 |
| `fx.preview.open` | divergent → divergent | `crates/registry/src/declared.rs:9854`; `spec/A_command_registry.md:668` | 1 | C12-R2-02 |
| `fx.preview.options.get` | divergent → divergent | `crates/registry/src/declared.rs:9876`; `spec/A_command_registry.md:669` | 1 | C12-R2-02 |
| `fx.preview.options.set` | divergent → divergent | `crates/registry/src/declared.rs:9898`; `spec/A_command_registry.md:670` | 1 | C12-R2-02 |
| `fx.preview.render` | divergent → divergent | `crates/registry/src/declared.rs:9920`; `spec/A_command_registry.md:671` | 1 | C12-R2-02 |
| `fx.preview.status.get` | divergent → divergent | `crates/registry/src/declared.rs:9942`; `spec/A_command_registry.md:672` | 1 | C12-R2-02 |
| `fx.preview.stop` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:9964`; `spec/A_command_registry.md:673` | 1 | M7 C7.3 |
| `fx.renderList.get` | divergent → divergent | `crates/registry/src/declared.rs:9989`; `spec/A_command_registry.md:674` | 1 | C12-R2-02 |
| `fx.renderer.capabilities.get` | divergent → divergent | `crates/registry/src/declared.rs:10014`; `spec/A_command_registry.md:675` | 1 | C12-R2-02 |
| `fx.renderer.options.changed` | divergent → divergent | `crates/registry/src/declared.rs:10039`; `spec/A_command_registry.md:676` | 1 | C12-R2-02 |
| `fx.renderer.options.get` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:10058`; `spec/A_command_registry.md:677` | 1 | M7 C7.2 |
| `fx.renderer.options.set` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:10080`; `spec/A_command_registry.md:678` | 1 | M7 C7.2 |
| `fx.shake.displacement.get` | divergent → divergent | `crates/registry/src/declared.rs:10105`; `spec/A_command_registry.md:679` | 1 | C12-R2-02 |
| `fx.simulation.run` | divergent → divergent | `crates/registry/src/declared.rs:10130`; `spec/A_command_registry.md:680` | 1 | C12-R2-02 |
| `fx.spriteSet.get` | divergent → divergent | `crates/registry/src/declared.rs:10155`; `spec/A_command_registry.md:681` | 1 | C12-R2-02 |
| `fx.spriteSet.list` | divergent → divergent | `crates/registry/src/declared.rs:10180`; `spec/A_command_registry.md:682` | 1 | C12-R2-02 |
| `fx.timing.set` | divergent → divergent | `crates/registry/src/declared.rs:10202`; `spec/A_command_registry.md:683` | 1 | C12-R2-02 |
| `fx.types.get` | divergent → divergent | `crates/registry/src/declared.rs:10227`; `spec/A_command_registry.md:684` | 1 | C12-R2-02 |
| `fx.types.list` | divergent → divergent | `crates/registry/src/declared.rs:10252`; `spec/A_command_registry.md:685` | 1 | C12-R2-02 |
| `import.completed` | divergent → divergent | `crates/registry/src/declared.rs:14327`; `spec/A_command_registry.md:913` | 1 | C12-R2-02 |
| `import.file.get` | divergent → divergent | `crates/registry/src/declared.rs:14346`; `spec/A_command_registry.md:914` | 1 | C12-R2-02 |
| `import.file.run` | divergent → divergent | `crates/registry/src/declared.rs:14371`; `spec/A_command_registry.md:915` | 1 | C12-R2-02 |
| `import.formats.list` | divergent → divergent | `crates/registry/src/declared.rs:14396`; `spec/A_command_registry.md:916` | 1 | C12-R2-02 |
| `import.keyedImage.get` | divergent → divergent | `crates/registry/src/declared.rs:14418`; `spec/A_command_registry.md:917` | 1 | C12-R2-02 |
| `import.keyedImages.run` | divergent → divergent | `crates/registry/src/declared.rs:14443`; `spec/A_command_registry.md:918` | 1 | C12-R2-02 |
| `import.movie.run` | divergent → divergent | `crates/registry/src/declared.rs:14468`; `spec/A_command_registry.md:919` | 1 | C12-R2-02 |
| `import.options.get` | divergent → divergent | `crates/registry/src/declared.rs:14493`; `spec/A_command_registry.md:920` | 1 | C12-R2-02 |
| `import.options.set` | divergent → divergent | `crates/registry/src/declared.rs:14515`; `spec/A_command_registry.md:921` | 1 | C12-R2-02 |
| `import.vectorize.settings.get` | divergent → divergent | `crates/registry/src/declared.rs:14537`; `spec/A_command_registry.md:922` | 1 | C12-R2-02 |
| `import.vectorize.settings.set` | divergent → divergent | `crates/registry/src/declared.rs:14559`; `spec/A_command_registry.md:923` | 1 | C12-R2-02 |
| `inspect.contactSheet.render` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/inspect.rs:202`; `spec/A_command_registry.md:1058` | 1 | M3 C3.3 |
| `inspect.coordinates.convert` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/inspect.rs:166`; `spec/A_command_registry.md:1059` | 1 | M3 C3.3 |
| `inspect.frame.describe` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/inspect.rs:49`; `spec/A_command_registry.md:1060` | 1 | M3 C3.3 |
| `inspect.hitTest` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/inspect.rs:106`; `spec/A_command_registry.md:1061` | 1 | M3 C3.3 |
| `inspect.screenshot` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/inspect.rs:257`; `spec/A_command_registry.md:1062` | 1 | M4 C4.2 |
| `inspect.ui.tree` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/inspect.rs:232`; `spec/A_command_registry.md:1063` | 1 | M4 C4.2 |
| `job.cancel` | divergent → implemented | `crates/executor/src/commands/job.rs:136`; `spec/A_command_registry.md:1082` | 1 | M1 C1.2 |
| `job.finished` | implemented → implemented | `crates/executor/src/commands/job.rs:164`; `spec/A_command_registry.md:1083` | 1 | M1 C1.2 |
| `job.list` | divergent → implemented | `crates/executor/src/commands/job.rs:52`; `spec/A_command_registry.md:1084` | 1 | M1 C1.2 |
| `job.progress` | implemented → implemented | `crates/executor/src/commands/job.rs:152`; `spec/A_command_registry.md:1085` | 1 | M1 C1.2 |
| `job.wait` | divergent → divergent | `crates/executor/src/commands/job.rs:112`; `spec/A_command_registry.md:1086` | 1 | C12-R2-06, C12-R2-07 |
| `keymap.changed` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:336`; `spec/A_command_registry.md:193` | 1 | M4 C4.1 |
| `keymap.export` | divergent → divergent | `crates/registry/src/declared.rs:355`; `spec/A_command_registry.md:194` | 1 | C12-R2-02 |
| `keymap.get` | divergent → divergent | `crates/registry/src/declared.rs:377`; `spec/A_command_registry.md:195` | 1 | C12-R2-02 |
| `keymap.import` | divergent → divergent | `crates/registry/src/declared.rs:399`; `spec/A_command_registry.md:196` | 1 | C12-R2-02 |
| `keymap.list` | divergent → divergent | `crates/registry/src/declared.rs:421`; `spec/A_command_registry.md:197` | 1 | C12-R2-02 |
| `keymap.remove` | divergent → divergent | `crates/registry/src/declared.rs:440`; `spec/A_command_registry.md:198` | 1 | C12-R2-02 |
| `keymap.reset` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:462`; `spec/A_command_registry.md:199` | 1 | M4 C4.1 |
| `keymap.set` | divergent → divergent | `crates/registry/src/declared.rs:484`; `spec/A_command_registry.md:200` | 1 | C12-R2-02 |
| `library.changed` | divergent → divergent | `crates/registry/src/declared.rs:12058`; `spec/A_command_registry.md:788` | 1 | C12-R2-02 |
| `library.close` | divergent → divergent | `crates/registry/src/declared.rs:12077`; `spec/A_command_registry.md:789` | 1 | C12-R2-02 |
| `library.closed` | divergent → divergent | `crates/registry/src/declared.rs:12102`; `spec/A_command_registry.md:790` | 1 | C12-R2-02 |
| `library.create` | divergent → divergent | `crates/registry/src/declared.rs:12121`; `spec/A_command_registry.md:791` | 1 | C12-R2-02 |
| `library.drawing.insert` | divergent → divergent | `crates/registry/src/declared.rs:12143`; `spec/A_command_registry.md:792` | 1 | C12-R2-02 |
| `library.element.duplicate` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:12168`; `spec/A_command_registry.md:793` | 1 | M9 C9.1 |
| `library.folder.create` | divergent → divergent | `crates/registry/src/declared.rs:12193`; `spec/A_command_registry.md:794` | 1 | C12-R2-02 |
| `library.folder.delete` | divergent → divergent | `crates/registry/src/declared.rs:12218`; `spec/A_command_registry.md:795` | 1 | C12-R2-02 |
| `library.folder.rename` | divergent → divergent | `crates/registry/src/declared.rs:12243`; `spec/A_command_registry.md:796` | 1 | C12-R2-02 |
| `library.item.delete` | divergent → divergent | `crates/registry/src/declared.rs:12268`; `spec/A_command_registry.md:797` | 1 | C12-R2-02 |
| `library.item.duplicate` | divergent → divergent | `crates/registry/src/declared.rs:12290`; `spec/A_command_registry.md:798` | 1 | C12-R2-02 |
| `library.item.get` | divergent → divergent | `crates/registry/src/declared.rs:12315`; `spec/A_command_registry.md:799` | 1 | C12-R2-02 |
| `library.item.import` | divergent → divergent | `crates/registry/src/declared.rs:12340`; `spec/A_command_registry.md:800` | 1 | C12-R2-02 |
| `library.item.list` | divergent → divergent | `crates/registry/src/declared.rs:12365`; `spec/A_command_registry.md:801` | 1 | C12-R2-02 |
| `library.item.move` | divergent → divergent | `crates/registry/src/declared.rs:12390`; `spec/A_command_registry.md:802` | 1 | C12-R2-02 |
| `library.item.rename` | divergent → divergent | `crates/registry/src/declared.rs:12415`; `spec/A_command_registry.md:803` | 1 | C12-R2-02 |
| `library.item.thumbnail.get` | divergent → divergent | `crates/registry/src/declared.rs:12440`; `spec/A_command_registry.md:804` | 1 | C12-R2-02 |
| `library.list` | divergent → divergent | `crates/registry/src/declared.rs:12465`; `spec/A_command_registry.md:805` | 1 | C12-R2-02 |
| `library.node.get` | divergent → divergent | `crates/registry/src/declared.rs:12484`; `spec/A_command_registry.md:806` | 1 | C12-R2-02 |
| `library.open` | divergent → divergent | `crates/registry/src/declared.rs:12509`; `spec/A_command_registry.md:807` | 1 | C12-R2-02 |
| `library.opened` | divergent → divergent | `crates/registry/src/declared.rs:12531`; `spec/A_command_registry.md:808` | 1 | C12-R2-02 |
| `library.preview.get` | divergent → divergent | `crates/registry/src/declared.rs:12550`; `spec/A_command_registry.md:809` | 1 | C12-R2-02 |
| `library.preview.set` | divergent → divergent | `crates/registry/src/declared.rs:12572`; `spec/A_command_registry.md:810` | 1 | C12-R2-02 |
| `library.project.get` | divergent → divergent | `crates/registry/src/declared.rs:12597`; `spec/A_command_registry.md:811` | 1 | C12-R2-02 |
| `library.refresh.run` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:12622`; `spec/A_command_registry.md:812` | 1 | M9 C9.1 |
| `library.save` | divergent → divergent | `crates/registry/src/declared.rs:12647`; `spec/A_command_registry.md:813` | 1 | C12-R2-02 |
| `library.selection.get` | divergent → divergent | `crates/registry/src/declared.rs:12669`; `spec/A_command_registry.md:814` | 1 | C12-R2-02 |
| `library.selection.set` | divergent → divergent | `crates/registry/src/declared.rs:12691`; `spec/A_command_registry.md:815` | 1 | C12-R2-02 |
| `library.starter.open` | divergent → divergent | `crates/registry/src/declared.rs:12716`; `spec/A_command_registry.md:816` | 1 | C12-R2-02 |
| `library.template.changed` | divergent → divergent | `crates/registry/src/declared.rs:12741`; `spec/A_command_registry.md:817` | 1 | C12-R2-02 |
| `library.template.create` | divergent → divergent | `crates/registry/src/declared.rs:12760`; `spec/A_command_registry.md:818` | 1 | C12-R2-02 |
| `library.template.edit.changed` | divergent → divergent | `crates/registry/src/declared.rs:12785`; `spec/A_command_registry.md:819` | 1 | C12-R2-02 |
| `library.template.edit.get` | divergent → divergent | `crates/registry/src/declared.rs:12804`; `spec/A_command_registry.md:820` | 1 | C12-R2-02 |
| `library.template.edit.start` | divergent → divergent | `crates/registry/src/declared.rs:12826`; `spec/A_command_registry.md:821` | 1 | C12-R2-02 |
| `library.template.edit.stop` | divergent → divergent | `crates/registry/src/declared.rs:12851`; `spec/A_command_registry.md:822` | 1 | C12-R2-02 |
| `library.template.export` | divergent → divergent | `crates/registry/src/declared.rs:12876`; `spec/A_command_registry.md:823` | 1 | C12-R2-02 |
| `library.template.frame.render` | divergent → divergent | `crates/registry/src/declared.rs:12901`; `spec/A_command_registry.md:824` | 1 | C12-R2-02 |
| `library.template.get` | divergent → divergent | `crates/registry/src/declared.rs:12926`; `spec/A_command_registry.md:825` | 1 | C12-R2-02 |
| `library.template.insert` | divergent → divergent | `crates/registry/src/declared.rs:12951`; `spec/A_command_registry.md:826` | 1 | C12-R2-02 |
| `library.template.search` | divergent → divergent | `crates/registry/src/declared.rs:12976`; `spec/A_command_registry.md:827` | 1 | C12-R2-02 |
| `library.templateUpdate.apply` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:13001`; `spec/A_command_registry.md:828` | 1 | M9 C9.1 |
| `library.templateUpdate.list` | divergent → divergent | `crates/registry/src/declared.rs:13026`; `spec/A_command_registry.md:829` | 1 | C12-R2-02 |
| `library.tree.get` | divergent → divergent | `crates/registry/src/declared.rs:13051`; `spec/A_command_registry.md:830` | 1 | C12-R2-02 |
| `library.view.get` | divergent → divergent | `crates/registry/src/declared.rs:13070`; `spec/A_command_registry.md:831` | 1 | C12-R2-02 |
| `library.view.set` | divergent → divergent | `crates/registry/src/declared.rs:13092`; `spec/A_command_registry.md:832` | 1 | C12-R2-02 |
| `library.writeAccess.set` | divergent → divergent | `crates/registry/src/declared.rs:13117`; `spec/A_command_registry.md:833` | 1 | C12-R2-02 |
| `lipsync.apply` | divergent → divergent | `crates/registry/src/declared.rs:11244`; `spec/A_command_registry.md:740` | 1 | C12-R2-02 |
| `lipsync.detect` | divergent → divergent | `crates/registry/src/declared.rs:11269`; `spec/A_command_registry.md:741` | 1 | C12-R2-02 |
| `lipsync.display.set` | divergent → divergent | `crates/registry/src/declared.rs:11294`; `spec/A_command_registry.md:742` | 1 | C12-R2-02 |
| `lipsync.mapping.get` | divergent → divergent | `crates/registry/src/declared.rs:11319`; `spec/A_command_registry.md:743` | 1 | C12-R2-02 |
| `lipsync.mapping.mouth.set` | divergent → divergent | `crates/registry/src/declared.rs:11344`; `spec/A_command_registry.md:744` | 1 | C12-R2-02 |
| `lipsync.mapping.set` | divergent → divergent | `crates/registry/src/declared.rs:11369`; `spec/A_command_registry.md:745` | 1 | C12-R2-02 |
| `lipsync.mouth.set` | divergent → divergent | `crates/registry/src/declared.rs:11394`; `spec/A_command_registry.md:746` | 1 | C12-R2-02 |
| `lipsync.panel.close` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:11419`; `spec/A_command_registry.md:747` | 1 | M8 C8.2 |
| `lipsync.panel.open` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:11441`; `spec/A_command_registry.md:748` | 1 | M8 C8.2 |
| `lipsync.panel.state.set` | divergent → divergent | `crates/registry/src/declared.rs:11463`; `spec/A_command_registry.md:749` | 1 | C12-R2-02 |
| `lipsync.settings.get` | divergent → divergent | `crates/registry/src/declared.rs:11488`; `spec/A_command_registry.md:750` | 1 | C12-R2-02 |
| `lipsync.settings.set` | divergent → divergent | `crates/registry/src/declared.rs:11510`; `spec/A_command_registry.md:751` | 1 | C12-R2-02 |
| `lipsync.shapes.list` | divergent → divergent | `crates/registry/src/declared.rs:11535`; `spec/A_command_registry.md:752` | 1 | C12-R2-02 |
| `lipsync.template.import` | divergent → divergent | `crates/registry/src/declared.rs:11560`; `spec/A_command_registry.md:753` | 1 | C12-R2-02 |
| `lipsync.template.list` | divergent → divergent | `crates/registry/src/declared.rs:11585`; `spec/A_command_registry.md:754` | 1 | C12-R2-02 |
| `lipsync.track.changed` | divergent → divergent | `crates/registry/src/declared.rs:11607`; `spec/A_command_registry.md:755` | 1 | C12-R2-02 |
| `lipsync.track.clear` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:11626`; `spec/A_command_registry.md:756` | 1 | M8 C8.2 |
| `lipsync.track.get` | divergent → divergent | `crates/registry/src/declared.rs:11651`; `spec/A_command_registry.md:757` | 1 | C12-R2-02 |
| `lipsync.track.set` | divergent → divergent | `crates/registry/src/declared.rs:11673`; `spec/A_command_registry.md:758` | 1 | C12-R2-02 |
| `lipsync.track.stale` | divergent → divergent | `crates/registry/src/declared.rs:11698`; `spec/A_command_registry.md:759` | 1 | C12-R2-02 |
| `media.movie.import` | divergent → divergent | `crates/registry/src/declared.rs:11717`; `spec/A_command_registry.md:767` | 1 | C12-R2-02 |
| `media.movie.probe` | divergent → divergent | `crates/registry/src/declared.rs:11742`; `spec/A_command_registry.md:768` | 1 | C12-R2-02 |
| `media.object.list` | divergent → divergent | `crates/registry/src/declared.rs:11767`; `spec/A_command_registry.md:769` | 1 | C12-R2-02 |
| `media.object.relink` | divergent → divergent | `crates/registry/src/declared.rs:11792`; `spec/A_command_registry.md:770` | 1 | C12-R2-02 |
| `media.sequence.add` | divergent → divergent | `crates/registry/src/declared.rs:11817`; `spec/A_command_registry.md:771` | 1 | C12-R2-02 |
| `media.sequence.get` | divergent → divergent | `crates/registry/src/declared.rs:11842`; `spec/A_command_registry.md:772` | 1 | C12-R2-02 |
| `media.sequence.list` | divergent → divergent | `crates/registry/src/declared.rs:11867`; `spec/A_command_registry.md:773` | 1 | C12-R2-02 |
| `media.sequence.move` | divergent → divergent | `crates/registry/src/declared.rs:11892`; `spec/A_command_registry.md:774` | 1 | C12-R2-02 |
| `media.sequence.naturalTiming.apply` | divergent → divergent | `crates/registry/src/declared.rs:11917`; `spec/A_command_registry.md:775` | 1 | C12-R2-02 |
| `media.sequence.remove` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:11942`; `spec/A_command_registry.md:776` | 1 | M8 C8.3 |
| `media.sequence.set` | divergent → divergent | `crates/registry/src/declared.rs:11967`; `spec/A_command_registry.md:777` | 1 | C12-R2-02 |
| `media.sound.play` | divergent → divergent | `crates/registry/src/declared.rs:11992`; `spec/A_command_registry.md:778` | 1 | C12-R2-02 |
| `media.sound.stop` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:12017`; `spec/A_command_registry.md:779` | 1 | M8 C8.3 |
| `media.source.missing` | divergent → divergent | `crates/registry/src/declared.rs:12039`; `spec/A_command_registry.md:780` | 1 | C12-R2-02 |
| `playback.forceRate.set` | divergent → divergent | `crates/registry/src/declared.rs:13857`; `spec/A_command_registry.md:879` | 1 | C12-R2-02 |
| `playback.loop.set` | divergent → divergent | `crates/registry/src/declared.rs:13882`; `spec/A_command_registry.md:880` | 1 | C12-R2-02 |
| `playback.player.close` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:13907`; `spec/A_command_registry.md:881` | 1 | M10 C10.1 |
| `playback.player.get` | divergent → divergent | `crates/registry/src/declared.rs:13929`; `spec/A_command_registry.md:882` | 1 | C12-R2-02 |
| `playback.player.open` | divergent → divergent | `crates/registry/src/declared.rs:13951`; `spec/A_command_registry.md:883` | 1 | C12-R2-02 |
| `playback.player.set` | divergent → divergent | `crates/registry/src/declared.rs:13976`; `spec/A_command_registry.md:884` | 1 | C12-R2-02 |
| `playback.preview.run` | divergent → divergent | `crates/registry/src/declared.rs:14001`; `spec/A_command_registry.md:885` | 1 | C12-R2-02 |
| `playback.range.changed` | divergent → divergent | `crates/registry/src/declared.rs:14026`; `spec/A_command_registry.md:886` | 1 | C12-R2-02 |
| `playback.range.get` | divergent → divergent | `crates/registry/src/declared.rs:14045`; `spec/A_command_registry.md:887` | 1 | C12-R2-02 |
| `playback.range.mode.set` | divergent → divergent | `crates/registry/src/declared.rs:14067`; `spec/A_command_registry.md:888` | 1 | C12-R2-02 |
| `playback.range.set` | divergent → divergent | `crates/registry/src/declared.rs:14092`; `spec/A_command_registry.md:889` | 1 | C12-R2-02 |
| `playback.rate.set` | divergent → divergent | `crates/registry/src/declared.rs:14117`; `spec/A_command_registry.md:890` | 1 | C12-R2-02 |
| `playback.start` | divergent → divergent | `crates/registry/src/declared.rs:14142`; `spec/A_command_registry.md:891` | 1 | C12-R2-02 |
| `playback.started` | divergent → divergent | `crates/registry/src/declared.rs:14167`; `spec/A_command_registry.md:892` | 1 | C12-R2-02 |
| `playback.state.get` | divergent → divergent | `crates/registry/src/declared.rs:14186`; `spec/A_command_registry.md:893` | 1 | C12-R2-02 |
| `playback.stop` | divergent → divergent | `crates/registry/src/declared.rs:14208`; `spec/A_command_registry.md:894` | 1 | C12-R2-02 |
| `playback.stopped` | divergent → divergent | `crates/registry/src/declared.rs:14233`; `spec/A_command_registry.md:895` | 1 | C12-R2-02 |
| `prefs.changed` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:232`; `spec/A_command_registry.md:181` | 1 | M4 C4.1 |
| `prefs.get` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:251`; `spec/A_command_registry.md:182` | 1 | M4 C4.1 |
| `prefs.list` | divergent → divergent | `crates/registry/src/declared.rs:273`; `spec/A_command_registry.md:183` | 1 | C12-R2-02 |
| `prefs.reset` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:292`; `spec/A_command_registry.md:184` | 1 | M4 C4.1 |
| `prefs.set` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:314`; `spec/A_command_registry.md:185` | 1 | M4 C4.1 |
| `print.pageSetup.get` | divergent → divergent | `crates/registry/src/declared.rs:15617`; `spec/A_command_registry.md:988` | 1 | C12-R2-02 |
| `print.pageSetup.set` | divergent → divergent | `crates/registry/src/declared.rs:15639`; `spec/A_command_registry.md:989` | 1 | C12-R2-02 |
| `print.preview.get` | divergent → divergent | `crates/registry/src/declared.rs:15661`; `spec/A_command_registry.md:990` | 1 | C12-R2-02 |
| `print.printer.list` | divergent → divergent | `crates/registry/src/declared.rs:15686`; `spec/A_command_registry.md:991` | 1 | C12-R2-02 |
| `print.run` | divergent → divergent | `crates/registry/src/declared.rs:15708`; `spec/A_command_registry.md:992` | 1 | C12-R2-02 |
| `print.settings.get` | divergent → divergent | `crates/registry/src/declared.rs:15736`; `spec/A_command_registry.md:993` | 1 | C12-R2-02 |
| `print.settings.set` | divergent → divergent | `crates/registry/src/declared.rs:15758`; `spec/A_command_registry.md:994` | 1 | C12-R2-02 |
| `project.autosaved` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:461`; `spec/A_command_registry.md:70` | 1 | M1 C1.3 |
| `project.backup.list` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:416`; `spec/A_command_registry.md:71` | 1 | M1 C1.3 |
| `project.backup.open` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:448`; `spec/A_command_registry.md:72` | 1 | M1 C1.3 |
| `project.close` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:136`; `spec/A_command_registry.md:73` | 1 | M1 C1.3 |
| `project.closed` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:474`; `spec/A_command_registry.md:74` | 1 | M1 C1.3 |
| `project.create` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:74`; `spec/A_command_registry.md:75` | 1 | M1 C1.3 |
| `project.dirty.changed` | implemented → implemented | `crates/executor/src/commands/project.rs:497`; `spec/A_command_registry.md:76` | 1 | M1 C1.2 |
| `project.info.get` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:204`; `spec/A_command_registry.md:77` | 1 | M1 C1.3 |
| `project.open` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:108`; `spec/A_command_registry.md:78` | 1 | M1 C1.3 |
| `project.opened` | implemented → implemented | `crates/executor/src/commands/project.rs:521`; `spec/A_command_registry.md:79` | 1 | M1 C1.2 |
| `project.presets.list` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:280`; `spec/A_command_registry.md:80` | 1 | M1 C1.3 |
| `project.properties.get` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:230`; `spec/A_command_registry.md:81` | 1 | M1 C1.3 |
| `project.properties.set` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:265`; `spec/A_command_registry.md:82` | 1 | M1 C1.3 |
| `project.recent.list` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:293`; `spec/A_command_registry.md:83` | 1 | M1 C1.3 |
| `project.recent.remove` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:325`; `spec/A_command_registry.md:84` | 1 | M1 C1.3 |
| `project.recovery.delete` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:394`; `spec/A_command_registry.md:85` | 1 | M1 C1.3 |
| `project.recovery.list` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:338`; `spec/A_command_registry.md:86` | 1 | M1 C1.3 |
| `project.recovery.open` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:362`; `spec/A_command_registry.md:87` | 1 | M1 C1.3 |
| `project.save` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:161`; `spec/A_command_registry.md:88` | 1 | M1 C1.3 |
| `project.saveAs` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:191`; `spec/A_command_registry.md:89` | 1 | M1 C1.3 |
| `project.saved` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/project.rs:533`; `spec/A_command_registry.md:90` | 1 | M1 C1.3 |
| `render.frame.run` | divergent → divergent | `crates/registry/src/declared.rs:14252`; `spec/A_command_registry.md:903` | 1 | C12-R2-02 |
| `render.limits.get` | divergent → divergent | `crates/registry/src/declared.rs:14277`; `spec/A_command_registry.md:904` | 1 | C12-R2-02 |
| `render.range.run` | divergent → divergent | `crates/registry/src/declared.rs:14302`; `spec/A_command_registry.md:905` | 1 | C12-R2-02 |
| `sound.asset.get` | divergent → divergent | `crates/registry/src/declared.rs:10277`; `spec/A_command_registry.md:693` | 1 | C12-R2-02 |
| `sound.asset.import` | divergent → divergent | `crates/registry/src/declared.rs:10302`; `spec/A_command_registry.md:694` | 1 | C12-R2-02 |
| `sound.asset.list` | divergent → divergent | `crates/registry/src/declared.rs:10327`; `spec/A_command_registry.md:695` | 1 | C12-R2-02 |
| `sound.asset.missing` | divergent → divergent | `crates/registry/src/declared.rs:10349`; `spec/A_command_registry.md:696` | 1 | C12-R2-02 |
| `sound.asset.probe` | divergent → divergent | `crates/registry/src/declared.rs:10368`; `spec/A_command_registry.md:697` | 1 | C12-R2-02 |
| `sound.asset.relink` | divergent → divergent | `crates/registry/src/declared.rs:10393`; `spec/A_command_registry.md:698` | 1 | C12-R2-02 |
| `sound.editor.close` | divergent → divergent | `crates/registry/src/declared.rs:10418`; `spec/A_command_registry.md:699` | 1 | C12-R2-02 |
| `sound.editor.open` | divergent → divergent | `crates/registry/src/declared.rs:10443`; `spec/A_command_registry.md:700` | 1 | C12-R2-02 |
| `sound.editor.state.get` | divergent → divergent | `crates/registry/src/declared.rs:10468`; `spec/A_command_registry.md:701` | 1 | C12-R2-02 |
| `sound.editor.state.set` | divergent → divergent | `crates/registry/src/declared.rs:10490`; `spec/A_command_registry.md:702` | 1 | C12-R2-02 |
| `sound.element.changed` | divergent → divergent | `crates/registry/src/declared.rs:10515`; `spec/A_command_registry.md:703` | 1 | C12-R2-02 |
| `sound.element.streamed.set` | divergent → divergent | `crates/registry/src/declared.rs:10534`; `spec/A_command_registry.md:704` | 1 | C12-R2-02 |
| `sound.envelope.clear` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:10559`; `spec/A_command_registry.md:705` | 1 | M8 C8.1 |
| `sound.envelope.get` | divergent → divergent | `crates/registry/src/declared.rs:10584`; `spec/A_command_registry.md:706` | 1 | C12-R2-02 |
| `sound.envelope.point.add` | divergent → divergent | `crates/registry/src/declared.rs:10609`; `spec/A_command_registry.md:707` | 1 | C12-R2-02 |
| `sound.envelope.point.move` | divergent → divergent | `crates/registry/src/declared.rs:10634`; `spec/A_command_registry.md:708` | 1 | C12-R2-02 |
| `sound.envelope.point.remove` | divergent → divergent | `crates/registry/src/declared.rs:10659`; `spec/A_command_registry.md:709` | 1 | C12-R2-02 |
| `sound.envelope.set` | divergent → divergent | `crates/registry/src/declared.rs:10684`; `spec/A_command_registry.md:710` | 1 | C12-R2-02 |
| `sound.fade.apply` | divergent → divergent | `crates/registry/src/declared.rs:10709`; `spec/A_command_registry.md:711` | 1 | C12-R2-02 |
| `sound.import` | divergent → divergent | `crates/registry/src/declared.rs:10734`; `spec/A_command_registry.md:712` | 1 | C12-R2-02 |
| `sound.placement.add` | divergent → divergent | `crates/registry/src/declared.rs:10759`; `spec/A_command_registry.md:713` | 1 | C12-R2-02 |
| `sound.placement.get` | divergent → divergent | `crates/registry/src/declared.rs:10784`; `spec/A_command_registry.md:714` | 1 | C12-R2-02 |
| `sound.placement.list` | divergent → divergent | `crates/registry/src/declared.rs:10809`; `spec/A_command_registry.md:715` | 1 | C12-R2-02 |
| `sound.placement.move` | divergent → divergent | `crates/registry/src/declared.rs:10831`; `spec/A_command_registry.md:716` | 1 | C12-R2-02 |
| `sound.placement.remove` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:10856`; `spec/A_command_registry.md:717` | 1 | M8 C8.1 |
| `sound.placement.set` | divergent → divergent | `crates/registry/src/declared.rs:10881`; `spec/A_command_registry.md:718` | 1 | C12-R2-02 |
| `sound.placement.split` | divergent → divergent | `crates/registry/src/declared.rs:10906`; `spec/A_command_registry.md:719` | 1 | C12-R2-02 |
| `sound.placement.trim` | divergent → divergent | `crates/registry/src/declared.rs:10931`; `spec/A_command_registry.md:720` | 1 | C12-R2-02 |
| `sound.play` | divergent → divergent | `crates/registry/src/declared.rs:10956`; `spec/A_command_registry.md:721` | 1 | C12-R2-02 |
| `sound.playback.set` | divergent → divergent | `crates/registry/src/declared.rs:10981`; `spec/A_command_registry.md:722` | 1 | C12-R2-02 |
| `sound.range.delete` | divergent → divergent | `crates/registry/src/declared.rs:11006`; `spec/A_command_registry.md:723` | 1 | C12-R2-02 |
| `sound.range.select` | divergent → divergent | `crates/registry/src/declared.rs:11031`; `spec/A_command_registry.md:724` | 1 | C12-R2-02 |
| `sound.recent.list` | divergent → divergent | `crates/registry/src/declared.rs:11056`; `spec/A_command_registry.md:725` | 1 | C12-R2-02 |
| `sound.scrubbing.set` | divergent → divergent | `crates/registry/src/declared.rs:11081`; `spec/A_command_registry.md:726` | 1 | C12-R2-02 |
| `sound.settings.get` | divergent → divergent | `crates/registry/src/declared.rs:11106`; `spec/A_command_registry.md:727` | 1 | C12-R2-02 |
| `sound.silence.list` | divergent → divergent | `crates/registry/src/declared.rs:11131`; `spec/A_command_registry.md:728` | 1 | C12-R2-02 |
| `sound.stop` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:11153`; `spec/A_command_registry.md:729` | 1 | M8 C8.1 |
| `sound.waveform.get` | divergent → divergent | `crates/registry/src/declared.rs:11175`; `spec/A_command_registry.md:730` | 1 | C12-R2-02 |
| `sound.waveform.ready` | divergent → divergent | `crates/registry/src/declared.rs:11200`; `spec/A_command_registry.md:731` | 1 | C12-R2-02 |
| `sound.waveforms.set` | divergent → divergent | `crates/registry/src/declared.rs:11219`; `spec/A_command_registry.md:732` | 1 | C12-R2-02 |
| `storyline.changed` | divergent → divergent | `crates/registry/src/declared.rs:13145`; `spec/A_command_registry.md:841` | 1 | C12-R2-02 |
| `storyline.currentScene.changed` | divergent → divergent | `crates/registry/src/declared.rs:13164`; `spec/A_command_registry.md:842` | 1 | C12-R2-02 |
| `storyline.currentScene.get` | divergent → divergent | `crates/registry/src/declared.rs:13183`; `spec/A_command_registry.md:843` | 1 | C12-R2-02 |
| `storyline.currentScene.set` | divergent → divergent | `crates/registry/src/declared.rs:13205`; `spec/A_command_registry.md:844` | 1 | C12-R2-02 |
| `storyline.frame.get` | divergent → divergent | `crates/registry/src/declared.rs:13230`; `spec/A_command_registry.md:845` | 1 | C12-R2-02 |
| `storyline.frame.render` | divergent → divergent | `crates/registry/src/declared.rs:13255`; `spec/A_command_registry.md:846` | 1 | C12-R2-02 |
| `storyline.get` | divergent → divergent | `crates/registry/src/declared.rs:13280`; `spec/A_command_registry.md:847` | 1 | C12-R2-02 |
| `storyline.play.changed` | divergent → divergent | `crates/registry/src/declared.rs:13305`; `spec/A_command_registry.md:848` | 1 | C12-R2-02 |
| `storyline.play.start` | divergent → divergent | `crates/registry/src/declared.rs:13324`; `spec/A_command_registry.md:849` | 1 | C12-R2-02 |
| `storyline.play.stop` | divergent → divergent | `crates/registry/src/declared.rs:13349`; `spec/A_command_registry.md:850` | 1 | C12-R2-02 |
| `storyline.playhead.get` | divergent → divergent | `crates/registry/src/declared.rs:13371`; `spec/A_command_registry.md:851` | 1 | C12-R2-02 |
| `storyline.playhead.set` | divergent → divergent | `crates/registry/src/declared.rs:13393`; `spec/A_command_registry.md:852` | 1 | C12-R2-02 |
| `storyline.scene.create` | divergent → divergent | `crates/registry/src/declared.rs:13415`; `spec/A_command_registry.md:853` | 1 | C12-R2-02 |
| `storyline.scene.delete` | divergent → divergent | `crates/registry/src/declared.rs:13437`; `spec/A_command_registry.md:854` | 1 | C12-R2-02 |
| `storyline.scene.get` | divergent → divergent | `crates/registry/src/declared.rs:13462`; `spec/A_command_registry.md:855` | 1 | C12-R2-02 |
| `storyline.scene.list` | divergent → divergent | `crates/registry/src/declared.rs:13487`; `spec/A_command_registry.md:856` | 1 | C12-R2-02 |
| `storyline.scene.move` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:13509`; `spec/A_command_registry.md:857` | 1 | M9 C9.2 |
| `storyline.scene.rename` | divergent → divergent | `crates/registry/src/declared.rs:13534`; `spec/A_command_registry.md:858` | 1 | C12-R2-02 |
| `storyline.scene.set` | divergent → divergent | `crates/registry/src/declared.rs:13559`; `spec/A_command_registry.md:859` | 1 | C12-R2-02 |
| `storyline.scene.visible.set` | divergent → divergent | `crates/registry/src/declared.rs:13584`; `spec/A_command_registry.md:860` | 1 | C12-R2-02 |
| `storyline.selection.changed` | divergent → divergent | `crates/registry/src/declared.rs:13609`; `spec/A_command_registry.md:861` | 1 | C12-R2-02 |
| `storyline.selection.get` | divergent → divergent | `crates/registry/src/declared.rs:13628`; `spec/A_command_registry.md:862` | 1 | C12-R2-02 |
| `storyline.selection.set` | divergent → divergent | `crates/registry/src/declared.rs:13650`; `spec/A_command_registry.md:863` | 1 | C12-R2-02 |
| `storyline.transition.get` | divergent → divergent | `crates/registry/src/declared.rs:13675`; `spec/A_command_registry.md:864` | 1 | C12-R2-02 |
| `storyline.transition.list` | divergent → divergent | `crates/registry/src/declared.rs:13697`; `spec/A_command_registry.md:865` | 1 | C12-R2-02 |
| `storyline.transition.remove` | divergent → divergent | `crates/registry/src/declared.rs:13722`; `spec/A_command_registry.md:866` | 1 | C12-R2-02 |
| `storyline.transition.set` | divergent → divergent | `crates/registry/src/declared.rs:13744`; `spec/A_command_registry.md:867` | 1 | C12-R2-02 |
| `storyline.transition.types.list` | divergent → divergent | `crates/registry/src/declared.rs:13769`; `spec/A_command_registry.md:868` | 1 | C12-R2-02 |
| `storyline.view.get` | divergent → divergent | `crates/registry/src/declared.rs:13794`; `spec/A_command_registry.md:869` | 1 | C12-R2-02 |
| `storyline.view.set` | divergent → divergent | `crates/registry/src/declared.rs:13816`; `spec/A_command_registry.md:870` | 1 | C12-R2-02 |
| `storyline.warning.changed` | divergent → divergent | `crates/registry/src/declared.rs:13838`; `spec/A_command_registry.md:871` | 1 | C12-R2-02 |
| `timeline.backgroundColour.set` | divergent → divergent | `crates/registry/src/declared.rs:5716`; `spec/A_command_registry.md:475` | 1 | C12-R2-02 |
| `timeline.currentFrame.changed` | divergent → divergent | `crates/registry/src/declared.rs:5741`; `spec/A_command_registry.md:476` | 1 | C12-R2-02 |
| `timeline.currentFrame.move` | divergent → divergent | `crates/registry/src/declared.rs:5760`; `spec/A_command_registry.md:477` | 1 | C12-R2-02 |
| `timeline.currentFrame.set` | divergent → divergent | `crates/registry/src/declared.rs:5785`; `spec/A_command_registry.md:478` | 1 | C12-R2-02 |
| `timeline.effectParameterRows.set` | divergent → divergent | `crates/registry/src/declared.rs:5810`; `spec/A_command_registry.md:479` | 1 | C12-R2-02 |
| `timeline.frameLabel.get` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:5835`; `spec/A_command_registry.md:480` | 1 | M5 C5.1 |
| `timeline.playbackRange.changed` | divergent → divergent | `crates/registry/src/declared.rs:5860`; `spec/A_command_registry.md:481` | 1 | C12-R2-02 |
| `timeline.playbackRange.get` | divergent → divergent | `crates/registry/src/declared.rs:5879`; `spec/A_command_registry.md:482` | 1 | C12-R2-02 |
| `timeline.playbackRange.set` | divergent → divergent | `crates/registry/src/declared.rs:5901`; `spec/A_command_registry.md:483` | 1 | C12-R2-02 |
| `timeline.rows.list` | divergent → divergent | `crates/registry/src/declared.rs:5926`; `spec/A_command_registry.md:484` | 1 | C12-R2-02 |
| `timeline.singleCellSelection.set` | divergent → divergent | `crates/registry/src/declared.rs:5951`; `spec/A_command_registry.md:485` | 1 | C12-R2-02 |
| `timeline.soundWaveforms.set` | divergent → divergent | `crates/registry/src/declared.rs:5976`; `spec/A_command_registry.md:486` | 1 | C12-R2-02 |
| `timeline.timeDisplay.set` | divergent → divergent | `crates/registry/src/declared.rs:6001`; `spec/A_command_registry.md:487` | 1 | C12-R2-02 |
| `timeline.transferOptions.get` | divergent → divergent | `crates/registry/src/declared.rs:6026`; `spec/A_command_registry.md:488` | 1 | C12-R2-02 |
| `timeline.transferOptions.set` | divergent → divergent | `crates/registry/src/declared.rs:6051`; `spec/A_command_registry.md:489` | 1 | C12-R2-02 |
| `timeline.view.set` | divergent → divergent | `crates/registry/src/declared.rs:6076`; `spec/A_command_registry.md:490` | 1 | C12-R2-02 |
| `tool.active.changed` | divergent → divergent | `crates/registry/src/declared.rs:2003`; `spec/A_command_registry.md:285` | 1 | C12-R2-02 |
| `tool.active.get` | divergent → divergent | `crates/registry/src/declared.rs:2022`; `spec/A_command_registry.md:286` | 1 | C12-R2-02 |
| `tool.active.set` | divergent → divergent | `crates/registry/src/declared.rs:2044`; `spec/A_command_registry.md:287` | 1 | C12-R2-02 |
| `tool.brush.options.set` | divergent → divergent | `crates/registry/src/declared.rs:2069`; `spec/A_command_registry.md:288` | 1 | C12-R2-02 |
| `tool.drawTopLayer.set` | divergent → divergent | `crates/registry/src/declared.rs:2094`; `spec/A_command_registry.md:289` | 1 | C12-R2-02 |
| `tool.eraser.options.set` | divergent → divergent | `crates/registry/src/declared.rs:2119`; `spec/A_command_registry.md:290` | 1 | C12-R2-02 |
| `tool.group.select` | divergent → divergent | `crates/registry/src/declared.rs:2144`; `spec/A_command_registry.md:291` | 1 | C12-R2-02 |
| `tool.list` | divergent → divergent | `crates/registry/src/declared.rs:2169`; `spec/A_command_registry.md:292` | 1 | C12-R2-02 |
| `tool.options.changed` | divergent → divergent | `crates/registry/src/declared.rs:2194`; `spec/A_command_registry.md:293` | 1 | C12-R2-02 |
| `tool.options.get` | divergent → divergent | `crates/registry/src/declared.rs:2213`; `spec/A_command_registry.md:294` | 1 | C12-R2-02 |
| `tool.options.set` | divergent → divergent | `crates/registry/src/declared.rs:2238`; `spec/A_command_registry.md:295` | 1 | C12-R2-02 |
| `tool.pen.changed` | divergent → divergent | `crates/registry/src/declared.rs:2263`; `spec/A_command_registry.md:296` | 1 | C12-R2-02 |
| `tool.pen.create` | divergent → divergent | `crates/registry/src/declared.rs:2282`; `spec/A_command_registry.md:297` | 1 | C12-R2-02 |
| `tool.pen.delete` | divergent → divergent | `crates/registry/src/declared.rs:2307`; `spec/A_command_registry.md:298` | 1 | C12-R2-02 |
| `tool.pen.list` | divergent → divergent | `crates/registry/src/declared.rs:2332`; `spec/A_command_registry.md:299` | 1 | C12-R2-02 |
| `tool.pen.rename` | divergent → divergent | `crates/registry/src/declared.rs:2354`; `spec/A_command_registry.md:300` | 1 | C12-R2-02 |
| `tool.pen.select` | divergent → divergent | `crates/registry/src/declared.rs:2379`; `spec/A_command_registry.md:301` | 1 | C12-R2-02 |
| `tool.pen.set` | divergent → divergent | `crates/registry/src/declared.rs:2404`; `spec/A_command_registry.md:302` | 1 | C12-R2-02 |
| `tool.pencil.options.set` | divergent → divergent | `crates/registry/src/declared.rs:2429`; `spec/A_command_registry.md:303` | 1 | C12-R2-02 |
| `tool.placeBehind.set` | divergent → divergent | `crates/registry/src/declared.rs:2451`; `spec/A_command_registry.md:304` | 1 | C12-R2-02 |
| `tool.polyline.options.set` | divergent → divergent | `crates/registry/src/declared.rs:2473`; `spec/A_command_registry.md:305` | 1 | C12-R2-02 |
| `tool.select.options.set` | divergent → divergent | `crates/registry/src/declared.rs:2495`; `spec/A_command_registry.md:306` | 1 | C12-R2-02 |
| `tool.settings.get` | divergent → divergent | `crates/registry/src/declared.rs:2517`; `spec/A_command_registry.md:307` | 1 | C12-R2-02 |
| `tool.snapToContour.set` | divergent → divergent | `crates/registry/src/declared.rs:2539`; `spec/A_command_registry.md:308` | 1 | C12-R2-02 |
| `tool.symmetry.set` | divergent → divergent | `crates/registry/src/declared.rs:2564`; `spec/A_command_registry.md:309` | 1 | C12-R2-02 |
| `tool.tablet.set` | divergent → divergent | `crates/registry/src/declared.rs:2586`; `spec/A_command_registry.md:310` | 1 | C12-R2-02 |
| `tool.text.options.set` | divergent → divergent | `crates/registry/src/declared.rs:2608`; `spec/A_command_registry.md:311` | 1 | C12-R2-02 |
| `ui.contextHelpMode.set` | divergent → divergent | `crates/registry/src/declared.rs:506`; `spec/A_command_registry.md:208` | 1 | C12-R2-02 |
| `ui.help.get` | divergent → divergent | `crates/registry/src/declared.rs:528`; `spec/A_command_registry.md:209` | 1 | C12-R2-02 |
| `ui.help.open` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:550`; `spec/A_command_registry.md:210` | 1 | M4 C4.1 |
| `ui.message.shown` | divergent → divergent | `crates/registry/src/declared.rs:572`; `spec/A_command_registry.md:211` | 1 | C12-R2-02 |
| `ui.preferences.open` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:591`; `spec/A_command_registry.md:212` | 1 | M4 C4.1 |
| `ui.startScreen.set` | divergent → divergent | `crates/registry/src/declared.rs:613`; `spec/A_command_registry.md:213` | 1 | C12-R2-02 |
| `ui.statusBar.get` | divergent → divergent | `crates/registry/src/declared.rs:635`; `spec/A_command_registry.md:214` | 1 | C12-R2-02 |
| `ui.statusBar.set` | divergent → divergent | `crates/registry/src/declared.rs:654`; `spec/A_command_registry.md:215` | 1 | C12-R2-02 |
| `ui.theme.list` | divergent → divergent | `crates/registry/src/declared.rs:676`; `spec/A_command_registry.md:216` | 1 | C12-R2-02 |
| `ui.tips.list` | divergent → divergent | `crates/registry/src/declared.rs:698`; `spec/A_command_registry.md:217` | 1 | C12-R2-02 |
| `ui.tips.reset` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:717`; `spec/A_command_registry.md:218` | 1 | M4 C4.1 |
| `ui.tips.set` | divergent → divergent | `crates/registry/src/declared.rs:736`; `spec/A_command_registry.md:219` | 1 | C12-R2-02 |
| `view.active.set` | divergent → divergent | `crates/registry/src/declared.rs:11`; `spec/A_command_registry.md:125` | 1 | C12-R2-02 |
| `view.coordinates.convert` | divergent → divergent | `crates/registry/src/declared.rs:36`; `spec/A_command_registry.md:126` | 1 | C12-R2-02 |
| `view.list` | divergent → divergent | `crates/registry/src/declared.rs:61`; `spec/A_command_registry.md:127` | 1 | C12-R2-02 |
| `view.pan.set` | divergent → divergent | `crates/registry/src/declared.rs:86`; `spec/A_command_registry.md:128` | 1 | C12-R2-02 |
| `view.rotation.set` | divergent → divergent | `crates/registry/src/declared.rs:111`; `spec/A_command_registry.md:129` | 1 | C12-R2-02 |
| `view.transform.get` | divergent → divergent | `crates/registry/src/declared.rs:135`; `spec/A_command_registry.md:130` | 1 | C12-R2-02 |
| `view.transform.reset` | divergent → divergent | `crates/registry/src/declared.rs:157`; `spec/A_command_registry.md:131` | 1 | C12-R2-02 |
| `view.type.set` | divergent → divergent | `crates/registry/src/declared.rs:182`; `spec/A_command_registry.md:132` | 1 | C12-R2-02 |
| `view.zoom.set` | divergent → divergent | `crates/registry/src/declared.rs:207`; `spec/A_command_registry.md:133` | 1 | C12-R2-02 |
| `window.floatingPanels.set` | implemented → implemented | `crates/executor/src/commands/window.rs:171`; `spec/A_command_registry.md:141` | 1 | M1 C1.2 |
| `window.fullScreen.set` | divergent → implemented | `crates/executor/src/commands/window.rs:22`; `spec/A_command_registry.md:142` | 2 | M1 C1.2 |
| `window.geometry.get` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/window.rs:434`; `spec/A_command_registry.md:143` | 1 | M4 C4.1 |
| `window.geometry.set` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/window.rs:471`; `spec/A_command_registry.md:144` | 1 | M4 C4.1 |
| `window.minimized.set` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/window.rs:492`; `spec/A_command_registry.md:145` | 1 | M4 C4.1 |
| `window.panel.list` | implemented → implemented | `crates/executor/src/commands/window.rs:35`; `spec/A_command_registry.md:146` | 1 | M1 C1.2 |
| `window.panel.move` | implemented → implemented | `crates/executor/src/commands/window.rs:137`; `spec/A_command_registry.md:147` | 1 | M1 C1.2 |
| `window.panel.set` | implemented → implemented | `crates/executor/src/commands/window.rs:71`; `spec/A_command_registry.md:148` | 2 | M1 C1.2 |
| `window.raiseAll.run` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/window.rs:505`; `spec/A_command_registry.md:149` | 1 | M4 C4.1 |
| `window.sidePanelsExtended.set` | implemented → implemented | `crates/executor/src/commands/window.rs:184`; `spec/A_command_registry.md:150` | 1 | M1 C1.2 |
| `window.toolbar.items.reset` | implemented → implemented | `crates/executor/src/commands/window.rs:385`; `spec/A_command_registry.md:151` | 1 | M1 C1.2 |
| `window.toolbar.items.set` | implemented → implemented | `crates/executor/src/commands/window.rs:349`; `spec/A_command_registry.md:152` | 1 | M1 C1.2 |
| `window.toolbar.list` | implemented → implemented | `crates/executor/src/commands/window.rs:248`; `spec/A_command_registry.md:153` | 1 | M1 C1.2 |
| `window.toolbar.set` | implemented → implemented | `crates/executor/src/commands/window.rs:307`; `spec/A_command_registry.md:154` | 1 | M1 C1.2 |
| `workspace.changed` | implemented → implemented | `crates/executor/src/commands/workspace.rs:322`; `spec/A_command_registry.md:162` | 1 | M1 C1.2 |
| `workspace.create` | implemented → implemented | `crates/executor/src/commands/workspace.rs:120`; `spec/A_command_registry.md:163` | 1 | M1 C1.2 |
| `workspace.delete` | implemented → implemented | `crates/executor/src/commands/workspace.rs:171`; `spec/A_command_registry.md:164` | 1 | M1 C1.2 |
| `workspace.export` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/workspace.rs:262`; `spec/A_command_registry.md:165` | 1 | M4 C4.1 |
| `workspace.get` | implemented → implemented | `crates/executor/src/commands/workspace.rs:38`; `spec/A_command_registry.md:166` | 2 | M1 C1.2 |
| `workspace.import` | deferred-with-owner → deferred-with-owner | `crates/executor/src/commands/workspace.rs:300`; `spec/A_command_registry.md:167` | 1 | M4 C4.1 |
| `workspace.layout.set` | implemented → implemented | `crates/executor/src/commands/workspace.rs:226`; `spec/A_command_registry.md:168` | 1 | M1 C1.2 |
| `workspace.list` | implemented → implemented | `crates/executor/src/commands/workspace.rs:17`; `spec/A_command_registry.md:169` | 1 | M1 C1.2 |
| `workspace.rename` | divergent → implemented | `crates/executor/src/commands/workspace.rs:143`; `spec/A_command_registry.md:170` | 1 | M1 C1.2 |
| `workspace.reset` | divergent → implemented | `crates/executor/src/commands/workspace.rs:206`; `spec/A_command_registry.md:171` | 1 | M1 C1.2 |
| `workspace.save` | implemented → implemented | `crates/executor/src/commands/workspace.rs:83`; `spec/A_command_registry.md:172` | 1 | M1 C1.2 |
| `workspace.select` | divergent → implemented | `crates/executor/src/commands/workspace.rs:70`; `spec/A_command_registry.md:173` | 1 | M1 C1.2 |
| `xsheet.columnWidth.set` | divergent → divergent | `crates/registry/src/declared.rs:6101`; `spec/A_command_registry.md:498` | 1 | C12-R2-02 |
| `xsheet.currentCell.set` | divergent → divergent | `crates/registry/src/declared.rs:6123`; `spec/A_command_registry.md:499` | 1 | C12-R2-02 |
| `xsheet.display.set` | divergent → divergent | `crates/registry/src/declared.rs:6148`; `spec/A_command_registry.md:500` | 1 | C12-R2-02 |
| `xsheet.exposures.get` | divergent → divergent | `crates/registry/src/declared.rs:6173`; `spec/A_command_registry.md:501` | 1 | C12-R2-02 |
| `xsheet.exposures.set` | divergent → divergent | `crates/registry/src/declared.rs:6198`; `spec/A_command_registry.md:502` | 1 | C12-R2-02 |
| `xsheet.lightTable.add` | divergent → divergent | `crates/registry/src/declared.rs:6223`; `spec/A_command_registry.md:503` | 1 | C12-R2-02 |
| `xsheet.lightTable.changed` | divergent → divergent | `crates/registry/src/declared.rs:6248`; `spec/A_command_registry.md:504` | 1 | C12-R2-02 |
| `xsheet.lightTable.list` | divergent → divergent | `crates/registry/src/declared.rs:6267`; `spec/A_command_registry.md:505` | 1 | C12-R2-02 |
| `xsheet.lightTable.move` | divergent → deferred-with-owner | `crates/registry/src/declared.rs:6289`; `spec/A_command_registry.md:506` | 1 | M5 C5.2 |
| `xsheet.lightTable.remove` | divergent → divergent | `crates/registry/src/declared.rs:6314`; `spec/A_command_registry.md:507` | 1 | C12-R2-02 |
| `xsheet.lightTable.set` | divergent → divergent | `crates/registry/src/declared.rs:6336`; `spec/A_command_registry.md:508` | 1 | C12-R2-02 |
| `xsheet.panel.set` | divergent → divergent | `crates/registry/src/declared.rs:6361`; `spec/A_command_registry.md:509` | 1 | C12-R2-02 |
