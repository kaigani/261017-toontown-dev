# M1-C1.2 round 1 conformance register

Reviewed 23ca3d5. Rebuild with `python reviews/evidence/M1-C1.2-r1/build-register.py` after recording the Rust sweeps. `registry.json` contains the complete schemas and verbatim spec columns; `command-register.json` contains each field assessment. This file is an index, not a substitute for the failing tests.

## Counts

```json
{
  "inventory": 796,
  "typed": 134,
  "typedImplemented": 62,
  "typedDeferred": 72,
  "untyped": 662,
  "events": 77,
  "implementedEvents": 10,
  "examples": 147,
  "commandClassifications": {
    "deferred-with-owner": 65,
    "divergent": 686,
    "implemented": 45
  },
  "requirementClassifications": {
    "implemented": 36,
    "divergent": 27,
    "deferred-with-owner": 7,
    "missing": 2
  },
  "untypedBoundInManifest": 3,
  "unreachableUnannotatedEntries": 659,
  "typedMissingTopLevelParameters": {}
}
```

Implemented means exercised with no additional per-row defect found; cross-cutting executor findings still apply. Deferred-with-owner means delivery is later per plan, not that its placeholder can satisfy current registry metadata requirements. In particular all 662 untyped entries are divergent metadata, while their handlers remain future work. Event examples are checked for non-invocability, not counted as successful actions. Native GUI and MCP coverage are explicitly separate.

## Cross-cutting requirements

| Requirement | Ruling | Implementation | Evidence/test | Qualification |
|---|---|---|---|---|
| R01 Inventory IDs, namespace, kind and verbatim table columns (A) | implemented | `crates/registry/src/inventory.rs:8` | c12_registry; registry.json; this independent set comparison | 796 exact; no added/missing ID |
| R02 Every inventory entry has constrained parameter/result schemas (plan 3.2; 11 14.2) | divergent | `crates/registry/src/lib.rs:464` | registry_stubs_have_actual_parameter_constraints_and_examples | C12-R1-12: 662 untyped placeholders |
| R03 Every command example executes and matches its result schema (11 14.2) | divergent | `crates/executor/src/commands/job.rs:63` | every_executable_typed_example_also_validates_its_result_schema | 147 typed examples; one result-schema violation; 662 entries lack examples; C12-R1-12,13 |
| R04 Every stub names a correct owner (plan 4) | divergent | `crates/registry/src/owners.rs:14` | registry_owners_follow_the_milestone_plan | C12-R1-21; typed deferred owners in command-register.json |
| R05 Registry version and generated frontend schema freshness | implemented | `crates/registry/src/lib.rs:52` | frontend_schemas; c12_registry | 0.1.0; generated snapshots remain current |
| R06 Attribute macro rejects missing example/duplicate ID/wrong kind | implemented | `crates/registry-macros/src/lib.rs:176` | c12_r1_macro | Missing example compile error; duplicates/wrong kind assembly errors |
| R07 Exclusive mutation capability (plan 3.2; 11 P2) | divergent | `crates/executor/src/lib.rs:180` | external_safe_code_cannot_install_a_mutated_stale_snapshot; compile_fail | Private Cx fields enforced; safe snapshot/edit/open replacement bypass remains; C12-R1-01 |
| R08 One serialized document queue (REV-06) | implemented | `crates/executor/src/lib.rs:249` | eight_clients_cas_commands_and_batches_have_no_lost_updates | 8 clients, 200 commits; every rev 2..201 once; no lost updates |
| R09 Staged full integrity gate and rollback of document edits | implemented | `crates/executor/src/cx.rs:326` | c12_transactions; staged_commit; c11_r4/r5/r6 | Multi-entry writes use checked staging; inherited structural bounds retained |
| R10 Atomic rollback covers every observable side effect (11 7.3) | divergent | `crates/executor/src/run.rs:48` | failed_atomic_batch_cannot_cancel_a_running_job; failed_batch_clipboard_emits_no_change_event | C12-R1-02: shared jobs and pending events escape rollback |
| R11 Atomic batch one revision, undo step and label | implemented | `crates/executor/src/run.rs:512` | c12_transactions | One commit, batch label, rollback on model failure |
| R12 Query steps see prior staged changes (11 7.3) | divergent | `crates/executor/src/commands/doc.rs:1215` | doc_diff_sees_prior_uncommitted_atomic_steps | Most queries do; doc.diff reads live project instead; C12-R1-09 |
| R13 Numeric/named intermediate references | implemented | `crates/executor/src/run.rs:622` | c12_transactions | 1-based SI-017 interpretation; missing/forward references rejected |
| R14 Long-running job-kind steps rejected in atomic batches | implemented | `crates/executor/src/run.rs:138` | c12_transactions | Kind=job rejected; job.cancel is kind=ui and still must roll back |
| R15 Ordinary edit dryRun changes/history/revisions | implemented | `crates/executor/src/run.rs:370` | ordinary_dry_run_preserves_exact_history_and_matches_real_result | Exact snapshot/history unchanged; result and changes match real call |
| R16 Dry run covers clipboard events and cancellation | divergent | `crates/executor/src/run.rs:48` | dry_run_cannot_cancel_a_running_job; dry_run_clipboard_emits_no_change_event | C12-R1-02 |
| R17 Command-parameter dryRun in atomic batch | divergent | `crates/executor/src/run.rs:512` | parameter_dry_run_is_honoured_inside_atomic_batch | C12-R1-03 |
| R18 Sequential dry-run batch simulates preceding edits | divergent | `crates/executor/src/run.rs:471` | sequential_batch_dry_run_queries_see_previous_simulated_edits | C12-R1-03 |
| R19 Common run_command envelope including job.wait | divergent | `crates/executor/src/lib.rs:218` | job_wait_obeys_request_id_and_retains_document_revision | job.wait bypasses queue/options/rev/cache; C12-R1-04 |
| R20 Undo/redo canonical content and revision stamps | implemented | `crates/executor/src/cx.rs:450` | ten_thousand_history_operations_match_independent_content_oracle_and_replay; c11_r6 | Fresh seed; 10k operations + fresh 10k replay byte-identical |
| R21 Failed batch history, redo invalidation, grouped labels | implemented | `crates/executor/src/run.rs:231` | c12_transactions; c12_r1_stress | No failed-document batch step retained; new edits invalidate redo |
| R22 Checkpoint create/list/delete/restore and history independence | implemented | `crates/executor/src/commands/checkpoint.rs:1` | c12_transactions; c12_r1_stress | Normal checkpoints restore undoably; copy-on-write, no full serialized copy |
| R23 Checkpoint created from staged content has coherent rev | divergent | `crates/executor/src/commands/checkpoint.rs:60` | checkpoint_in_atomic_batch_has_the_committed_content_revision | C12-R1-10 |
| R24 Checkpoint cost at 100k strokes/10k drawings | implemented | `crates/model/src/document/mod.rs:159` | c12_r1_scale; large-documents.json | 20 creates measured; per-entry Arc sharing, index sharing, map clone cost remains |
| R25 expectRev scalar and envelope guard | implemented | `crates/executor/src/run.rs:173` | c12_transactions; eight_clients_cas_commands_and_batches_have_no_lost_updates | Normal CAS conflicts prevent mutations |
| R26 expectRev map checks every listed object (REV-05) | divergent | `crates/executor/src/commands/doc.rs:874` | expect_rev_map_checks_every_listed_object_not_only_patch_targets | C12-R1-07 |
| R27 Conflict details.currentRev and current small JSON (REV-05) | divergent | `crates/executor/src/run.rs:189` | expect_rev_error_reports_current_rev_and_small_current_object | C12-R1-07 |
| R28 requestId retry same ID returns original without execution | divergent | `crates/executor/src/run.rs:22` | session_idempotency_survives_more_than_ten_thousand_other_requests | Eviction permits destructive retry in same session; job.wait bypass; C12-R1-04,06 |
| R29 requestId different parameters same ID | implemented | `crates/executor/src/run.rs:356` | request_id_collision_returns_original_result_per_literal_spec | Literal 11 7.2 met; stricter review-charter rule conflicts/needs ruling; C12-R1-22 P2 |
| R30 requestId persistence across process restart | deferred-with-owner | `crates/executor/src/run.rs:22` | source audit | Not required by 11 7.2 (within-session); durable session protocol is C1.4 decision, not a persistence defect |
| R31 documentRev exactly +1 per edit/undo/redo (REV-01/03) | implemented | `crates/executor/src/cx.rs:388` | c12_transactions; c12_r1_stress | Normal revisions checked on every random operation |
| R32 Revision overflow is controlled error, not thread panic | divergent | `crates/executor/src/cx.rs:388` | revision_exhaustion_is_an_error_and_does_not_kill_the_document_thread | C12-R1-19 |
| R33 Drawing rev follows content edits (REV-02) | implemented | `crates/model/src/document/stamp.rs:1` | c12_transactions; changes_report_drawing_revision_when_a_stroke_changes | Stamp correct; notification incomplete in next row |
| R34 Every changed revision returned to caller (11 P6) | divergent | `crates/executor/src/cx.rs:617` | changes_report_drawing_revision_when_a_stroke_changes | C12-R1-11: changed drawing omitted |
| R35 Persist revs through save/reopen (REV-04) | deferred-with-owner | `crates/executor/src/commands/project.rs:1` | typed declarations; handoff | C1.3 persistence; cannot prove with C1.2 stubs |
| R36 Change sets oldValue/concrete paths/numeric opaque keys | implemented | `crates/model/src/document/mod.rs:535` | staged_commit; staged_inverse; c11_r6; c12_r1_stress | Checked canonical before/after diffs; no stateless inversion resurrected |
| R37 Historical doc.diff within retained 10k window | divergent | `crates/executor/src/commands/doc.rs:1166` | diff_older_than_snapshot_window_replays_repeated_entry_batch; historical_diff_reports_the_revision_of_its_requested_end_state | C12-R1-08: repeated-entry reconstruction fails and old revs drift |
| R38 doc.diff retains last save even beyond 10k (AG-09) | divergent | `crates/executor/src/session.rs:104` | diff_retains_the_last_save_even_after_ten_thousand_transactions | C12-R1-08 |
| R39 500-change envelope cap and oversized event elision | implemented | `crates/executor/src/run.rs:184` | c12_transactions; c12_policy | ChangesSince/truncated and >64 KiB elision; query for full values |
| R40 IDs/compound paths/special references/projections | implemented | `crates/executor/src/cx.rs:1` | c12_policy; c11_r1/r2/r3 | SI-018 current-element path choice qualified; known active-view defect C12-R1-15 |
| R41 Derived fields already computable from model | implemented | `crates/executor/src/commands/derived.rs:1` | c12_policy | Exposures/frame counts/drawing content projections covered |
| R42 Geometry/render/media-derived fields not yet computable | deferred-with-owner | `crates/executor/src/commands/derived.rs:1` | c12_policy; doc.derivedUnavailable | Explicit M2/M3/M7/M8 owners and warnings; no pretend complete renderer |
| R43 x-command / structural mutation routes and drawing aliases | deferred-with-owner | `crates/executor/src/commands/doc.rs:837` | c12_registry::drawing_command_delegation_ruling; c12_policy | Aliases mapped; specialized handlers intentionally M2/M3 etc; no hidden generic mutation |
| R44 No-project enablement (01 1.3) | divergent | `crates/executor/src/commands/edit.rs:635` | edit_clipboard_is_disabled_without_a_project | Clipboard get/set omit project requirement; C12-R1-20 |
| R45 Dirty-project quit fail/discard/save policy | implemented | `crates/executor/src/commands/app.rs:481` | c12_kernel_state | Fail and discard implemented; save returns owned C1.3 stub; native quit routing later |
| R46 Fullscreen explicit enabled parameter | implemented | `crates/executor/src/commands/window.rs:31` | c12_kernel_state; c12_r1_contracts | Explicit setting/repeat idempotent; active context divergence next row |
| R47 Mode/current view/fullscreen readback agreement | divergent | `crates/executor/src/commands/app.rs:381` | setting_mode_activates_a_view_of_the_requested_type; fullscreen_state_and_active_view_agree_with_visible_central_panel | C12-R1-15 |
| R48 Workspace autoSave/default/save/reset rules | divergent | `crates/executor/src/ui.rs:917` | workspace_switch_defaults_to_autosaving_current_arrangement; explicitly_saving_and_reselecting_current_workspace_keeps_layout; renamed_builtin_workspace_can_still_be_restored | C12-R1-16 |
| R49 Queryable panels/layout/toolbars | implemented | `crates/executor/src/ui.rs:1` | c12_kernel_state | Kernel state only; no native rendering assertion |
| R50 All edit.* functionality due C1.2 (plan 4) | missing | `crates/executor/src/commands/edit.rs:1` | edit_commands_promised_for_c12_are_implemented | Seven commands return M4 stubs without a plan exception; C12-R1-14 |
| R51 Clipboard structured kind validation | divergent | `crates/executor/src/commands/edit.rs:608` | clipboard_set_validates_the_content_for_its_kind | C12-R1-17 |
| R52 Clipboard return shape | divergent | `crates/executor/src/commands/edit.rs:630` | clipboard_get_returns_the_specified_clipboard_directly | C12-R1-18 |
| R53 Manifest forward parity; declared typed reverse coverage | implemented | `crates/executor/tests/c12_registry.rs:138` | c12_registry::ui_binding_manifest_parity | Existing menu IDs present; strings describe intended GUI locations |
| R54 Full-inventory reverse parity build gate (11 14.1) | divergent | `crates/executor/tests/c12_registry.rs:166` | every_registry_entry_has_a_gui_binding_or_agent_only_justification | 662 skipped; C12-R1-12 |
| R55 Real native command dispatch/state parity | deferred-with-owner | `desktop/src-tauri/src/menu.rs:1` | handoff; source audit | M4 C4.1 per accepted readiness split; native fullscreen still toggles directly |
| R56 Every registered command malformed inputs under watchdog | implemented | `crates/executor/src/lib.rs:149` | c12_r1_stress; c12_r1_scale | 2388 small +1592 >1MiB calls; no internal/panic on these fixtures |
| R57 Watchdog bounds caller vs actual execution | implemented | `crates/executor/src/lib.rs:149` | c12_kernel_state::the_watchdog_answers_within_its_budget; job_wait_timeout_policy_is_explicitly_measured | Caller timeout only; no cancellation/loop recovery claim; job.wait uses own timeout |
| R58 Perpetually looping registered handler and modal interception proof | missing | `crates/executor/src/lib.rs:249` | source audit; no injectable Core/registry hook | Cannot inject a handler through public API without product changes; no catch/unwind/interrupt; headless examples alone do not prove recovery |
| R59 Explicit scopes on implemented calls | implemented | `crates/executor/src/run.rs:114` | c12_policy | Agent read/edit/ui checks; confirmation engine still C1.4 |
| R60 Job session isolation | divergent | `crates/executor/src/commands/job.rs:37` | agent_cannot_read_another_sessions_jobs_with_all_true; agent_cannot_cancel_another_sessions_job | C12-R1-05 |
| R61 File/prefs/network scopes on inventory stubs | divergent | `crates/registry/src/lib.rs:450` | registry_file_and_network_commands_declare_required_scopes | C12-R1-12 |
| R62 Protocol initialize/tools/resources/subscriptions/prompts/events/audit/confirmation | deferred-with-owner | `crates/executor/src/commands/agent.rs:1` | agent.* typed declarations; bins/headless flags | C1.4; no MCP inspector run possible at this checkpoint |
| R63 Job wait/status/cancel/progress table | implemented | `crates/executor/src/jobs.rs:1` | c12_kernel_state | Background progress and cooperative cancellation exist; isolation/transaction exceptions above |
| R64 No new unsafe/product unwrap/expect/panic allowances | implemented | `crates/executor/src/lib.rs:1` | source audit; clippy; CI | New unsafe absent; test/tool allowances scoped; overflow panic is separate finding |
| R65 Existing three-layer IPC gate / worker ban / diagnostics gate | implemented | `desktop/eslint.config.js:1` | baseline build; Vitest boundary 69+46; CI default/diagnostics builds | No worker exception introduced; accepted runtime-string limitation unchanged |
| R66 Composite command-result boundary controls | implemented | `desktop/src/ipc/commands.test.ts:1` | 7 command schema tests; frontend_schemas | Wide ints, finite doubles, own keys, snapshots, depth, accessors/proxies retained |
| R67 Touched-object diff and 100k full-commit costs | implemented | `crates/executor/src/cx.rs:617` | commit-scale.log; large-documents.json | Touched path implemented; whole-entry parse/diff/inverse integrity cost remains; M2 optimization |
| R68 10k-drawing query performance | divergent | `crates/executor/src/commands/doc.rs:485` | c12_r1_scale; large-documents.json | 6.372 s release find100 on empty drawings; P2 C12-R1-23 |
| R69 Dependency/licence and lockfile changes | implemented | `Cargo.lock:1` | cargo deny; CI; git diff | Only local executor/registry plus trybuild support additions; no new unapproved licence found |
| R70 Reviewer test preservation and CI coverage accuracy | implemented | `.github/workflows/ci.yml:1` | ci-audit.json; both raw run logs; git diff | Existing reviewer tests untouched; Windows/macOS check runs include suites |
| R71 Fresh-seed randomized coverage and shrinking | implemented | `crates/executor/tests/c12_r1_stress.rs:1` | random-history.json; existing proptest suites | Fresh deterministic sequence recorded; new oracle reports operation but does not shrink; existing properties shrink |
| R72 Persistence unchecked assembly confinement | deferred-with-owner | `crates/model/src/document/mod.rs:1` | C1.1 closure carryover | C1.3 load/repair; open capability must first close C12-R1-01 |

## Complete spec A inventory

All 796 IDs, namespace/kind and verbatim parameters/result/undo columns match the frozen appendix; no extra ID exists. `Typed` below means real generated schemas (with the job.wait exception), examples and declaration metadata. `Placeholder` means unconstrained schemas, zero examples, default needsProject=false, undoLabel=null, guessed kind-based scopes and no reverse parity. Scope/undo/enablement values for **every** row are retained in the companion JSON. Owners are claimed owners, with identified plan disagreements flagged. M1 typed fields were compared with the area tables and exercised by the example/contract suites; future handler behavior is not claimed verified.

| ID / kind | Ruling | Declaration / spec A line | Contract / examples executed | Owner / findings |
|---|---|---|---|---|
| `agent.activity.export` (action) | deferred-with-owner | `crates/executor/src/commands/agent.rs:405`; A:1035 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.activity.list` (query) | deferred-with-owner | `crates/executor/src/commands/agent.rs:342`; A:1036 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.activity.persist.set` (toggle) | deferred-with-owner | `crates/executor/src/commands/agent.rs:426`; A:1037 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.confirmation.requested` (event) | deferred-with-owner | `crates/executor/src/commands/agent.rs:465`; A:1038 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.confirmation.respond` (ui) | deferred-with-owner | `crates/executor/src/commands/agent.rs:313`; A:1039 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.policy.defaults.set` (ui) | deferred-with-owner | `crates/executor/src/commands/agent.rs:224`; A:1040 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.server.set` (ui) | deferred-with-owner | `crates/executor/src/commands/agent.rs:64`; A:1041 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.server.status` (query) | deferred-with-owner | `crates/executor/src/commands/agent.rs:77`; A:1042 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.session.connected` (event) | deferred-with-owner | `crates/executor/src/commands/agent.rs:439`; A:1043 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.session.disconnect` (ui) | deferred-with-owner | `crates/executor/src/commands/agent.rs:128`; A:1044 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.session.disconnected` (event) | deferred-with-owner | `crates/executor/src/commands/agent.rs:452`; A:1045 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.session.grant` (ui) | deferred-with-owner | `crates/executor/src/commands/agent.rs:107`; A:1046 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.session.pause` (ui) | deferred-with-owner | `crates/executor/src/commands/agent.rs:157`; A:1047 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.session.resume` (ui) | deferred-with-owner | `crates/executor/src/commands/agent.rs:177`; A:1048 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.token.regenerate` (ui) | deferred-with-owner | `crates/executor/src/commands/agent.rs:248`; A:1049 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `agent.typedTools.set` (ui) | deferred-with-owner | `crates/executor/src/commands/agent.rs:280`; A:1050 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `anim.bone.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:517 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.bone.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:518 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.bone.influence.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:519 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.bone.parent.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:520 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.bone.pose.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:521 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.bone.reach.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:522 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.bone.rest.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:523 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.camera.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:524 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.camera.current.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:525 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.camera.current.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:526 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.camera.currentChanged` (event) | divergent | `crates/registry/src/lib.rs:440`; A:527 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.camera.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:528 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.camera.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:529 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.camera.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:530 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.camera.select` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:531 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.camera.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:532 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:533 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.channel.copy` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:534 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.channel.cut` (action) | divergent | `crates/registry/src/lib.rs:440`; A:535 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.channel.paste` (action) | divergent | `crates/registry/src/lib.rs:440`; A:536 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.depth.move` (action) | divergent | `crates/registry/src/lib.rs:440`; A:537 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.display.allPegs.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:538 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.display.antialiasToolOverlays.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:539 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.display.arms.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:540 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.display.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:541 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.display.envelopes.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:542 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.display.frameMarkers.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:543 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.display.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:544 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.display.pegGhosts.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:545 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.display.pivotPath.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:546 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.display.smoothPaths.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:547 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.evaluate` (query) | divergent | `crates/registry/src/lib.rs:440`; A:548 | Placeholder; 0 | M3 C3.2; C12-R1-12 |
| `anim.flip.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:549 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.function.curveType.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:550 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.function.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:551 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.function.key.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:552 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.function.key.duplicate` (action) | divergent | `crates/registry/src/lib.rs:440`; A:553 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.function.key.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:554 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.function.key.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:555 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.function.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:556 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.function.preset.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:557 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.function.sample` (query) | divergent | `crates/registry/src/lib.rs:440`; A:558 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.function.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:559 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.functionEditor.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:560 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.functionEditor.open` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:561 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.functionEditor.syncFrame.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:562 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.functionEditor.view.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:563 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.hierarchy.attach` (action) | divergent | `crates/registry/src/lib.rs:440`; A:564 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.hierarchy.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:565 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.hierarchy.detach` (action) | divergent | `crates/registry/src/lib.rs:440`; A:566 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.hierarchy.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:567 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.hierarchy.select` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:568 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.key.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:569 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.key.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:570 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.key.move` (action) | divergent | `crates/registry/src/lib.rs:440`; A:571 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.key.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:572 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.loop.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:573 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.node.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:574 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.node.options.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:575 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.node.pathDisplay.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:576 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.path.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:577 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.path.lockDepth.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:578 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.path.point.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:579 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.path.point.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:580 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.path.point.select` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:581 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.path.point.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:582 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.path.sample` (query) | divergent | `crates/registry/src/lib.rs:440`; A:583 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.path.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:584 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.peg.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:585 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.pegOnlyMode.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:586 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.point.copy` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:587 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.point.paste` (action) | divergent | `crates/registry/src/lib.rs:440`; A:588 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.prefs.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:589 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.prefs.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:590 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.projection.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:591 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.record.completed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:592 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.record.options.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:593 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.record.options.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:594 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.record.run` (action) | divergent | `crates/registry/src/lib.rs:440`; A:595 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.reset.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:596 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.segment.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:597 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.segment.uniform.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:598 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.skeleton.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:599 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.skeleton.deform.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:600 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.skeleton.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:601 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.skeleton.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:602 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.skeleton.restPose.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:603 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.skeleton.root.select` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:604 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.skeleton.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:605 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.skeleton.skin.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:606 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.skeleton.skin.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:607 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.snapLastKey.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:608 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.stretch.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:609 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.tool.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:610 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.tool.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:611 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.tool.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:612 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.transform.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:613 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.view.coords.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:614 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.view.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:615 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.view.slot.select` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:616 | Placeholder; 0 | M6; C12-R1-12 |
| `anim.view.type.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:617 | Placeholder; 0 | M6; C12-R1-12 |
| `app.context.set` (ui) | implemented | `crates/executor/src/commands/app.rs:64`; A:53 | Typed; 2 | C1.2; no additional per-command defect found |
| `app.currentElement.move` (ui) | implemented | `crates/executor/src/commands/app.rs:179`; A:54 | Typed; 1 | C1.2; no additional per-command defect found |
| `app.currentElement.set` (ui) | implemented | `crates/executor/src/commands/app.rs:130`; A:55 | Typed; 1 | C1.2; no additional per-command defect found |
| `app.frame.move` (ui) | implemented | `crates/executor/src/commands/app.rs:294`; A:56 | Typed; 2 | C1.2; no additional per-command defect found |
| `app.frame.set` (ui) | implemented | `crates/executor/src/commands/app.rs:239`; A:57 | Typed; 1 | C1.2; no additional per-command defect found |
| `app.info.get` (query) | deferred-with-owner | `crates/executor/src/commands/app.rs:420`; A:58 | Typed; 1 | M1 C1.4; no additional per-command defect found |
| `app.mode.set` (ui) | divergent | `crates/executor/src/commands/app.rs:373`; A:59 | Typed; 1 | C1.2; C12-R1-15 |
| `app.quit` (action) | implemented | `crates/executor/src/commands/app.rs:462`; A:60 | Typed; 1 | C1.2; no additional per-command defect found |
| `app.state.changed` (event) | implemented | `crates/executor/src/commands/app.rs:490`; A:61 | Typed; 1 | C1.2; no additional per-command defect found |
| `app.state.get` (query) | divergent | `crates/executor/src/commands/app.rs:33`; A:62 | Typed; 1 | C1.2; C12-R1-15 |
| `capture.auto.start` (job) | divergent | `crates/registry/src/lib.rs:440`; A:956 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.auto.stop` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:957 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.device.added` (event) | divergent | `crates/registry/src/lib.rs:440`; A:958 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.device.dialog.open` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:959 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.device.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:960 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.device.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:961 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.device.removed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:962 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.device.select` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:963 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.device.settings.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:964 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.display.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:965 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.frame.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:966 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.frame.addFromFile` (action) | divergent | `crates/registry/src/lib.rs:440`; A:967 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.frame.captured` (event) | divergent | `crates/registry/src/lib.rs:440`; A:968 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.inputLock.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:969 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.keyColour.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:970 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.keying.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:971 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.lightTable.references.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:972 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.lightTable.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:973 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.preview.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:974 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.session.close` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:975 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.session.error` (event) | divergent | `crates/registry/src/lib.rs:440`; A:976 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.session.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:977 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.session.open` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:978 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.source.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:979 | Placeholder; 0 | M10; C12-R1-12 |
| `capture.target.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:980 | Placeholder; 0 | M10; C12-R1-12 |
| `cell.blank.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:448 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.blank.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:449 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.clipboard.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:450 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.clipboard.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:451 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.columnate.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:452 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.cycle.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:453 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.drawing.duplicate` (action) | divergent | `crates/registry/src/lib.rs:440`; A:454 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.exposure.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:455 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.exposure.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:456 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.exposure.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:457 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.exposures.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:458 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.fill.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:459 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.label.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:460 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.note.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:461 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.range.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:462 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.range.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:463 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.range.move` (action) | divergent | `crates/registry/src/lib.rs:440`; A:464 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.select` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:465 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.selection.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:466 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `cell.selection.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:467 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `checkpoint.create` (ui) | divergent | `crates/executor/src/commands/checkpoint.rs:41`; A:1071 | Typed; 1 | C1.2; C12-R1-10 |
| `checkpoint.delete` (ui) | implemented | `crates/executor/src/commands/checkpoint.rs:145`; A:1072 | Typed; 1 | C1.2; no additional per-command defect found |
| `checkpoint.list` (query) | divergent | `crates/executor/src/commands/checkpoint.rs:101`; A:1073 | Typed; 1 | C1.2; C12-R1-10 |
| `checkpoint.restore` (action) | implemented | `crates/executor/src/commands/checkpoint.rs:124`; A:1074 | Typed; 1 | C1.2; no additional per-command defect found |
| `colour.autoColourUpdate.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:343 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.current.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:344 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.current.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:345 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.current.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:346 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.editor.close` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:347 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.editor.open` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:348 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.fillMapping.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:349 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.fillMapping.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:350 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.gap.close` (action) | divergent | `crates/registry/src/lib.rs:440`; A:351 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.gapClosing.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:352 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.gapClosing.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:353 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.missing.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:354 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.paint.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:355 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `colour.paint.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:356 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `colour.paintUnpainted.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:357 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `colour.palette.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:358 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.palette.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:359 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.palette.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:360 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.palette.duplicate` (action) | divergent | `crates/registry/src/lib.rs:440`; A:361 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.palette.export` (action) | divergent | `crates/registry/src/lib.rs:440`; A:362 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.palette.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:363 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.palette.import` (action) | divergent | `crates/registry/src/lib.rs:440`; A:364 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.palette.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:365 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.palette.rename` (action) | divergent | `crates/registry/src/lib.rs:440`; A:366 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.palette.select` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:367 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.palette.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:368 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.pick` (query) | divergent | `crates/registry/src/lib.rs:440`; A:369 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.recolour.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:370 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.recovery.performed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:371 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.recovery.run` (action) | divergent | `crates/registry/src/lib.rs:440`; A:372 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.sceneStyle.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:373 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.sceneStyles.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:374 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.selection.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:375 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.style.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:376 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.style.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:377 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.style.duplicate` (action) | divergent | `crates/registry/src/lib.rs:440`; A:378 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.style.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:379 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.style.rename` (action) | divergent | `crates/registry/src/lib.rs:440`; A:380 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.style.select` (action) | divergent | `crates/registry/src/lib.rs:440`; A:381 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.swatch.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:382 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.swatch.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:383 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.swatch.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:384 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.swatch.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:385 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.swatch.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:386 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.swatch.move` (action) | divergent | `crates/registry/src/lib.rs:440`; A:387 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.swatch.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:388 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.swatchNames.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:389 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.tintBlend.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:390 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.tintDialog.open` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:391 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.tintOffset.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:392 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `colour.usage.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:393 | Placeholder; 0 | M7 C7.1; C12-R1-12 |
| `doc.asset.embed` (action) | deferred-with-owner | `crates/executor/src/commands/doc.rs:1856`; A:1002 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.asset.export` (query) | deferred-with-owner | `crates/executor/src/commands/doc.rs:1885`; A:1003 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.asset.list` (query) | deferred-with-owner | `crates/executor/src/commands/doc.rs:1829`; A:1004 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.asset.relink` (action) | deferred-with-owner | `crates/executor/src/commands/doc.rs:1934`; A:1005 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.asset.remove` (action) | deferred-with-owner | `crates/executor/src/commands/doc.rs:1965`; A:1006 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.asset.status.changed` (event) | deferred-with-owner | `crates/executor/src/commands/doc.rs:2053`; A:1007 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.blob.get` (query) | deferred-with-owner | `crates/executor/src/commands/doc.rs:1762`; A:1008 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.changed` (event) | implemented | `crates/executor/src/commands/doc.rs:1303`; A:1009 | Typed; 1 | C1.2; no additional per-command defect found |
| `doc.diff` (query) | divergent | `crates/executor/src/commands/doc.rs:1203`; A:1010 | Typed; 1 | C1.2; C12-R1-08,09 |
| `doc.export.json` (query) | deferred-with-owner | `crates/executor/src/commands/doc.rs:1628`; A:1011 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.import.json` (action) | deferred-with-owner | `crates/executor/src/commands/doc.rs:1712`; A:1012 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.loadReport.get` (query) | deferred-with-owner | `crates/executor/src/commands/doc.rs:1545`; A:1013 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.object.find` (query) | divergent | `crates/executor/src/commands/doc.rs:445`; A:1014 | Typed; 3 | C1.2; C12-R1-23 |
| `doc.object.get` (query) | implemented | `crates/executor/src/commands/doc.rs:137`; A:1015 | Typed; 3 | C1.2; no additional per-command defect found |
| `doc.object.patch` (action) | divergent | `crates/executor/src/commands/doc.rs:852`; A:1016 | Typed; 4 | C1.2; C12-R1-03,07,11,19 |
| `doc.object.refs` (query) | deferred-with-owner | `crates/executor/src/commands/doc.rs:1374`; A:1017 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.object.resolve` (query) | implemented | `crates/executor/src/commands/doc.rs:558`; A:1018 | Typed; 1 | C1.2; no additional per-command defect found |
| `doc.package.changedOnDisk` (event) | deferred-with-owner | `crates/executor/src/commands/doc.rs:2066`; A:1019 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.package.export` (job) | deferred-with-owner | `crates/executor/src/commands/doc.rs:2039`; A:1020 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.package.get` (query) | deferred-with-owner | `crates/executor/src/commands/doc.rs:1992`; A:1021 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.repair` (action) | deferred-with-owner | `crates/executor/src/commands/doc.rs:1530`; A:1022 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.schema.get` (query) | implemented | `crates/executor/src/commands/doc.rs:1038`; A:1023 | Typed; 2 | C1.2; no additional per-command defect found |
| `doc.schema.list` (query) | implemented | `crates/executor/src/commands/doc.rs:989`; A:1024 | Typed; 1 | C1.2; no additional per-command defect found |
| `doc.schema.validate` (query) | deferred-with-owner | `crates/executor/src/commands/doc.rs:1423`; A:1025 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `doc.state.get` (query) | implemented | `crates/executor/src/commands/doc.rs:1088`; A:1026 | Typed; 1 | C1.2; no additional per-command defect found |
| `doc.validate` (query) | deferred-with-owner | `crates/executor/src/commands/doc.rs:1483`; A:1027 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `draw.arrange.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:227 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.bounds.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:228 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.breakApart.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:229 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.centreLine.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:230 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.content.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:231 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.current.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:232 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.cut.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:233 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.drawing.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:234 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.drawing.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:235 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.drawing.rename` (action) | divergent | `crates/registry/src/lib.rs:440`; A:236 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.drawing.render` (query) | divergent | `crates/registry/src/lib.rs:440`; A:237 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.ellipse.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:238 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.envelope.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:239 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.erase.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:240 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.feather.apply` (job) | divergent | `crates/registry/src/lib.rs:440`; A:241 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.flatten.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:242 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.flip.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:243 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.group.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:244 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.group.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:245 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.hitTest.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:246 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.inspectorPreview.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:247 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.line.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:248 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.linesToBrush.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:249 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.lock.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:250 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.object.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:251 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.optimize.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:252 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.pivot.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:253 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.point.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:254 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.point.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:255 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.point.move` (action) | divergent | `crates/registry/src/lib.rs:440`; A:256 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.point.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:257 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.pointSelection.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:258 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.polyline.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:259 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.rectangle.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:260 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.repositionAll.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:261 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.rotate.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:262 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.segment.move` (action) | divergent | `crates/registry/src/lib.rs:440`; A:263 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.selection.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:264 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.selection.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:265 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.selection.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:266 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.smooth.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:267 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.stroke.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:268 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.stroke.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:269 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.text.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:270 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.text.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:271 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.textFormat.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:272 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.transform.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:273 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.vectorFile.import` (job) | divergent | `crates/registry/src/lib.rs:440`; A:274 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.vectorize.preview` (query) | divergent | `crates/registry/src/lib.rs:440`; A:275 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.vectorize.run` (job) | divergent | `crates/registry/src/lib.rs:440`; A:276 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `draw.viewMapping.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:277 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.autoLightTable.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:319 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.background.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:320 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:321 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.currentOnTop.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:322 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.grid.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:323 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.lightTableShade.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:324 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.onionSkin.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:325 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.onionSkinCycle.run` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:326 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.onionSkinStyle.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:327 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.selectionDisplay.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:328 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.state.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:329 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.staticLightTable.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:330 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.staticLightTable.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:331 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.staticLightTable.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:332 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.staticLightTable.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:333 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.staticLightTablePanel.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:334 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `drawAid.strokes.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:335 | Placeholder; 0 | M2 C2.3; C12-R1-12 |
| `edit.clipboard.changed` (event) | divergent | `crates/executor/src/commands/edit.rs:701`; A:98 | Typed; 1 | C1.2; C12-R1-02 |
| `edit.clipboard.get` (query) | divergent | `crates/executor/src/commands/edit.rs:636`; A:99 | Typed; 1 | C1.2; C12-R1-18,20 |
| `edit.clipboard.set` (ui) | divergent | `crates/executor/src/commands/edit.rs:677`; A:100 | Typed; 1 | C1.2; C12-R1-02,17,20 |
| `edit.copy` (ui) | divergent | `crates/executor/src/commands/edit.rs:750`; A:101 | Typed; 1 | M4 C4.1; C12-R1-14 |
| `edit.cut` (action) | divergent | `crates/executor/src/commands/edit.rs:736`; A:102 | Typed; 1 | M4 C4.1; C12-R1-14 |
| `edit.cutSpecial` (action) | divergent | `crates/executor/src/commands/edit.rs:844`; A:103 | Typed; 1 | M4 C4.1; C12-R1-14 |
| `edit.delete` (action) | divergent | `crates/executor/src/commands/edit.rs:771`; A:104 | Typed; 1 | M4 C4.1; C12-R1-14 |
| `edit.deselectAll` (ui) | implemented | `crates/executor/src/commands/edit.rs:540`; A:105 | Typed; 1 | C1.2; no additional per-command defect found |
| `edit.history.changed` (event) | implemented | `crates/executor/src/commands/edit.rs:279`; A:106 | Typed; 1 | C1.2; no additional per-command defect found |
| `edit.history.get` (query) | implemented | `crates/executor/src/commands/edit.rs:181`; A:107 | Typed; 1 | C1.2; no additional per-command defect found |
| `edit.history.move` (action) | implemented | `crates/executor/src/commands/edit.rs:234`; A:108 | Typed; 1 | C1.2; no additional per-command defect found |
| `edit.paste` (action) | divergent | `crates/executor/src/commands/edit.rs:810`; A:109 | Typed; 1 | M4 C4.1; C12-R1-14 |
| `edit.pasteAsNewElement` (action) | divergent | `crates/executor/src/commands/edit.rs:915`; A:110 | Typed; 1 | M4 C4.1; C12-R1-14 |
| `edit.pasteSpecial` (action) | divergent | `crates/executor/src/commands/edit.rs:878`; A:111 | Typed; 1 | M4 C4.1; C12-R1-14 |
| `edit.redo` (action) | implemented | `crates/executor/src/commands/edit.rs:111`; A:112 | Typed; 1 | C1.2; no additional per-command defect found |
| `edit.selectAll` (ui) | implemented | `crates/executor/src/commands/edit.rs:516`; A:113 | Typed; 1 | C1.2; no additional per-command defect found |
| `edit.selection.changed` (event) | implemented | `crates/executor/src/commands/edit.rs:558`; A:114 | Typed; 1 | C1.2; no additional per-command defect found |
| `edit.selection.get` (query) | implemented | `crates/executor/src/commands/edit.rs:324`; A:115 | Typed; 1 | C1.2; no additional per-command defect found |
| `edit.selection.set` (ui) | implemented | `crates/executor/src/commands/edit.rs:398`; A:116 | Typed; 1 | C1.2; no additional per-command defect found |
| `edit.undo` (action) | implemented | `crates/executor/src/commands/edit.rs:68`; A:117 | Typed; 1 | C1.2; no additional per-command defect found |
| `element.annotation.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:401 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.childExposure.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:402 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.clone` (action) | divergent | `crates/registry/src/lib.rs:440`; A:403 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.collapsed.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:404 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.colour.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:405 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:406 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.current.move` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:407 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.current.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:408 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:409 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.drawing.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:410 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.drawing.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:411 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.drawing.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:412 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.drawing.duplicate` (action) | divergent | `crates/registry/src/lib.rs:440`; A:413 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.drawing.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:414 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.drawing.rename` (action) | divergent | `crates/registry/src/lib.rs:440`; A:415 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.drawingContent.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:416 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.drawingLock.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:417 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.duplicate` (action) | divergent | `crates/registry/src/lib.rs:440`; A:418 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:419 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.hierarchy.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:420 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.hierarchy.select` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:421 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:422 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.lock.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:423 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.mediaLength.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:424 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.note.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:425 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.order.move` (action) | divergent | `crates/registry/src/lib.rs:440`; A:426 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.order.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:427 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.parent.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:428 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.parent.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:429 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.parentPeg.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:430 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.properties.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:431 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.rename` (action) | divergent | `crates/registry/src/lib.rs:440`; A:432 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.select` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:433 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.solo.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:434 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.state.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:435 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.timing.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:436 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.typeColour.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:437 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.typeColour.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:438 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.unusedDrawing.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:439 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `element.visibility.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:440 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `export.drawingPdf.run` (job) | divergent | `crates/registry/src/lib.rs:440`; A:931 | Placeholder; 0 | M10; C12-R1-12 |
| `export.formats.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:932 | Placeholder; 0 | M10; C12-R1-12 |
| `export.job.finished` (event) | divergent | `crates/registry/src/lib.rs:440`; A:933 | Placeholder; 0 | M10; C12-R1-12 |
| `export.movie.defaults.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:934 | Placeholder; 0 | M10; C12-R1-12 |
| `export.movie.defaults.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:935 | Placeholder; 0 | M10; C12-R1-12 |
| `export.movie.run` (job) | divergent | `crates/registry/src/lib.rs:440`; A:936 | Placeholder; 0 | M10; C12-R1-12 |
| `export.movie.validate` (query) | divergent | `crates/registry/src/lib.rs:440`; A:937 | Placeholder; 0 | M10; C12-R1-12 |
| `export.publish.account.close` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:938 | Placeholder; 0 | M10; C12-R1-12 |
| `export.publish.account.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:939 | Placeholder; 0 | M10; C12-R1-12 |
| `export.publish.account.open` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:940 | Placeholder; 0 | M10; C12-R1-12 |
| `export.publish.categories.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:941 | Placeholder; 0 | M10; C12-R1-12 |
| `export.publish.run` (job) | divergent | `crates/registry/src/lib.rs:440`; A:942 | Placeholder; 0 | M10; C12-R1-12 |
| `export.publish.service.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:943 | Placeholder; 0 | M10; C12-R1-12 |
| `export.publish.validate` (query) | divergent | `crates/registry/src/lib.rs:440`; A:944 | Placeholder; 0 | M10; C12-R1-12 |
| `export.report.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:945 | Placeholder; 0 | M10; C12-R1-12 |
| `export.settings.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:946 | Placeholder; 0 | M10; C12-R1-12 |
| `export.settings.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:947 | Placeholder; 0 | M10; C12-R1-12 |
| `export.snapshot.run` (job) | divergent | `crates/registry/src/lib.rs:440`; A:948 | Placeholder; 0 | M10; C12-R1-12 |
| `fx.allEffects.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:625 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.allEffects.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:626 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.allEffects.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:627 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.block.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:628 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.block.defaults.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:629 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.block.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:630 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.block.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:631 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.block.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:632 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.block.seed.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:633 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.block.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:634 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.block.type.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:635 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:636 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:637 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.curve.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:638 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.dropShadow.transform.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:639 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:640 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.hitTest.run` (query) | divergent | `crates/registry/src/lib.rs:440`; A:641 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.keyframe.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:642 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.keyframe.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:643 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.keyframe.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:644 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:645 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.paramElement.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:646 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.paramElement.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:647 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.paramElement.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:648 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.paramRows.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:649 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.paramRows.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:650 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.params.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:651 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.params.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:652 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.particles.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:653 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preset.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:654 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preset.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:655 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preset.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:656 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preset.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:657 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preset.export` (action) | divergent | `crates/registry/src/lib.rs:440`; A:658 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preset.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:659 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preset.import` (action) | divergent | `crates/registry/src/lib.rs:440`; A:660 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preset.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:661 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preset.rename` (action) | divergent | `crates/registry/src/lib.rs:440`; A:662 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preview.cache.delete` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:663 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preview.close` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:664 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preview.frame.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:665 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preview.frameRendered` (event) | divergent | `crates/registry/src/lib.rs:440`; A:666 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preview.invalidated` (event) | divergent | `crates/registry/src/lib.rs:440`; A:667 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preview.open` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:668 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preview.options.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:669 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preview.options.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:670 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preview.render` (job) | divergent | `crates/registry/src/lib.rs:440`; A:671 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preview.status.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:672 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.preview.stop` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:673 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.renderList.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:674 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.renderer.capabilities.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:675 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.renderer.options.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:676 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.renderer.options.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:677 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.renderer.options.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:678 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.shake.displacement.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:679 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.simulation.run` (job) | divergent | `crates/registry/src/lib.rs:440`; A:680 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.spriteSet.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:681 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.spriteSet.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:682 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.timing.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:683 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.types.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:684 | Placeholder; 0 | M7; C12-R1-12 |
| `fx.types.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:685 | Placeholder; 0 | M7; C12-R1-12 |
| `import.completed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:913 | Placeholder; 0 | M10; C12-R1-12 |
| `import.file.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:914 | Placeholder; 0 | M10; C12-R1-12 |
| `import.file.run` (job) | divergent | `crates/registry/src/lib.rs:440`; A:915 | Placeholder; 0 | M10; C12-R1-12 |
| `import.formats.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:916 | Placeholder; 0 | M10; C12-R1-12 |
| `import.keyedImage.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:917 | Placeholder; 0 | M10; C12-R1-12 |
| `import.keyedImages.run` (job) | divergent | `crates/registry/src/lib.rs:440`; A:918 | Placeholder; 0 | M10; C12-R1-12 |
| `import.movie.run` (job) | divergent | `crates/registry/src/lib.rs:440`; A:919 | Placeholder; 0 | M10; C12-R1-12,21 |
| `import.options.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:920 | Placeholder; 0 | M10; C12-R1-12 |
| `import.options.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:921 | Placeholder; 0 | M10; C12-R1-12 |
| `import.vectorize.settings.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:922 | Placeholder; 0 | M10; C12-R1-12 |
| `import.vectorize.settings.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:923 | Placeholder; 0 | M10; C12-R1-12 |
| `inspect.contactSheet.render` (query) | deferred-with-owner | `crates/executor/src/commands/inspect.rs:202`; A:1058 | Typed; 1 | M3 C3.3; no additional per-command defect found |
| `inspect.coordinates.convert` (query) | deferred-with-owner | `crates/executor/src/commands/inspect.rs:166`; A:1059 | Typed; 1 | M3 C3.3; no additional per-command defect found |
| `inspect.frame.describe` (query) | deferred-with-owner | `crates/executor/src/commands/inspect.rs:49`; A:1060 | Typed; 1 | M3 C3.3; no additional per-command defect found |
| `inspect.hitTest` (query) | deferred-with-owner | `crates/executor/src/commands/inspect.rs:106`; A:1061 | Typed; 1 | M3 C3.3; no additional per-command defect found |
| `inspect.screenshot` (query) | deferred-with-owner | `crates/executor/src/commands/inspect.rs:257`; A:1062 | Typed; 1 | M4 C4.2; no additional per-command defect found |
| `inspect.ui.tree` (query) | deferred-with-owner | `crates/executor/src/commands/inspect.rs:232`; A:1063 | Typed; 1 | M4 C4.2; no additional per-command defect found |
| `job.cancel` (ui) | divergent | `crates/executor/src/commands/job.rs:95`; A:1082 | Typed; 1 | C1.2; C12-R1-02,05 |
| `job.finished` (event) | implemented | `crates/executor/src/commands/job.rs:119`; A:1083 | Typed; 1 | C1.2; no additional per-command defect found |
| `job.list` (query) | divergent | `crates/executor/src/commands/job.rs:28`; A:1084 | Typed; 1 | C1.2; C12-R1-05 |
| `job.progress` (event) | implemented | `crates/executor/src/commands/job.rs:107`; A:1085 | Typed; 1 | C1.2; no additional per-command defect found |
| `job.wait` (query) | divergent | `crates/executor/src/commands/job.rs:73`; A:1086 | Typed; 1 | C1.2; C12-R1-04,05,13 |
| `keymap.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:193 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `keymap.export` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:194 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `keymap.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:195 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `keymap.import` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:196 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `keymap.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:197 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `keymap.remove` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:198 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `keymap.reset` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:199 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `keymap.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:200 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `library.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:788 | Placeholder; 0 | M9; C12-R1-12 |
| `library.close` (action) | divergent | `crates/registry/src/lib.rs:440`; A:789 | Placeholder; 0 | M9; C12-R1-12 |
| `library.closed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:790 | Placeholder; 0 | M9; C12-R1-12 |
| `library.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:791 | Placeholder; 0 | M9; C12-R1-12 |
| `library.drawing.insert` (action) | divergent | `crates/registry/src/lib.rs:440`; A:792 | Placeholder; 0 | M9; C12-R1-12 |
| `library.element.duplicate` (action) | divergent | `crates/registry/src/lib.rs:440`; A:793 | Placeholder; 0 | M9; C12-R1-12 |
| `library.folder.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:794 | Placeholder; 0 | M9; C12-R1-12 |
| `library.folder.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:795 | Placeholder; 0 | M9; C12-R1-12 |
| `library.folder.rename` (action) | divergent | `crates/registry/src/lib.rs:440`; A:796 | Placeholder; 0 | M9; C12-R1-12 |
| `library.item.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:797 | Placeholder; 0 | M9; C12-R1-12 |
| `library.item.duplicate` (action) | divergent | `crates/registry/src/lib.rs:440`; A:798 | Placeholder; 0 | M9; C12-R1-12 |
| `library.item.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:799 | Placeholder; 0 | M9; C12-R1-12 |
| `library.item.import` (action) | divergent | `crates/registry/src/lib.rs:440`; A:800 | Placeholder; 0 | M9; C12-R1-12 |
| `library.item.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:801 | Placeholder; 0 | M9; C12-R1-12 |
| `library.item.move` (action) | divergent | `crates/registry/src/lib.rs:440`; A:802 | Placeholder; 0 | M9; C12-R1-12 |
| `library.item.rename` (action) | divergent | `crates/registry/src/lib.rs:440`; A:803 | Placeholder; 0 | M9; C12-R1-12 |
| `library.item.thumbnail.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:804 | Placeholder; 0 | M9; C12-R1-12 |
| `library.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:805 | Placeholder; 0 | M9; C12-R1-12 |
| `library.node.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:806 | Placeholder; 0 | M9; C12-R1-12 |
| `library.open` (action) | divergent | `crates/registry/src/lib.rs:440`; A:807 | Placeholder; 0 | M9; C12-R1-12 |
| `library.opened` (event) | divergent | `crates/registry/src/lib.rs:440`; A:808 | Placeholder; 0 | M9; C12-R1-12 |
| `library.preview.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:809 | Placeholder; 0 | M9; C12-R1-12 |
| `library.preview.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:810 | Placeholder; 0 | M9; C12-R1-12 |
| `library.project.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:811 | Placeholder; 0 | M9; C12-R1-12 |
| `library.refresh.run` (action) | divergent | `crates/registry/src/lib.rs:440`; A:812 | Placeholder; 0 | M9; C12-R1-12 |
| `library.save` (action) | divergent | `crates/registry/src/lib.rs:440`; A:813 | Placeholder; 0 | M9; C12-R1-12 |
| `library.selection.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:814 | Placeholder; 0 | M9; C12-R1-12 |
| `library.selection.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:815 | Placeholder; 0 | M9; C12-R1-12 |
| `library.starter.open` (action) | divergent | `crates/registry/src/lib.rs:440`; A:816 | Placeholder; 0 | M9; C12-R1-12 |
| `library.template.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:817 | Placeholder; 0 | M9; C12-R1-12 |
| `library.template.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:818 | Placeholder; 0 | M9; C12-R1-12 |
| `library.template.edit.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:819 | Placeholder; 0 | M9; C12-R1-12 |
| `library.template.edit.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:820 | Placeholder; 0 | M9; C12-R1-12 |
| `library.template.edit.start` (action) | divergent | `crates/registry/src/lib.rs:440`; A:821 | Placeholder; 0 | M9; C12-R1-12 |
| `library.template.edit.stop` (action) | divergent | `crates/registry/src/lib.rs:440`; A:822 | Placeholder; 0 | M9; C12-R1-12 |
| `library.template.export` (action) | divergent | `crates/registry/src/lib.rs:440`; A:823 | Placeholder; 0 | M9; C12-R1-12 |
| `library.template.frame.render` (query) | divergent | `crates/registry/src/lib.rs:440`; A:824 | Placeholder; 0 | M9; C12-R1-12 |
| `library.template.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:825 | Placeholder; 0 | M9; C12-R1-12 |
| `library.template.insert` (action) | divergent | `crates/registry/src/lib.rs:440`; A:826 | Placeholder; 0 | M9; C12-R1-12 |
| `library.template.search` (query) | divergent | `crates/registry/src/lib.rs:440`; A:827 | Placeholder; 0 | M9; C12-R1-12 |
| `library.templateUpdate.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:828 | Placeholder; 0 | M9; C12-R1-12 |
| `library.templateUpdate.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:829 | Placeholder; 0 | M9; C12-R1-12 |
| `library.tree.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:830 | Placeholder; 0 | M9; C12-R1-12 |
| `library.view.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:831 | Placeholder; 0 | M9; C12-R1-12 |
| `library.view.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:832 | Placeholder; 0 | M9; C12-R1-12 |
| `library.writeAccess.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:833 | Placeholder; 0 | M9; C12-R1-12 |
| `lipsync.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:740 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.detect` (job) | divergent | `crates/registry/src/lib.rs:440`; A:741 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.display.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:742 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.mapping.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:743 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.mapping.mouth.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:744 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.mapping.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:745 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.mouth.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:746 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.panel.close` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:747 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.panel.open` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:748 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.panel.state.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:749 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.settings.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:750 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.settings.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:751 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.shapes.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:752 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.template.import` (action) | divergent | `crates/registry/src/lib.rs:440`; A:753 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.template.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:754 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.track.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:755 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.track.clear` (action) | divergent | `crates/registry/src/lib.rs:440`; A:756 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.track.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:757 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.track.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:758 | Placeholder; 0 | M8; C12-R1-12 |
| `lipsync.track.stale` (event) | divergent | `crates/registry/src/lib.rs:440`; A:759 | Placeholder; 0 | M8; C12-R1-12 |
| `media.movie.import` (job) | divergent | `crates/registry/src/lib.rs:440`; A:767 | Placeholder; 0 | M8; C12-R1-12 |
| `media.movie.probe` (query) | divergent | `crates/registry/src/lib.rs:440`; A:768 | Placeholder; 0 | M8; C12-R1-12 |
| `media.object.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:769 | Placeholder; 0 | M8; C12-R1-12 |
| `media.object.relink` (action) | divergent | `crates/registry/src/lib.rs:440`; A:770 | Placeholder; 0 | M8; C12-R1-12 |
| `media.sequence.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:771 | Placeholder; 0 | M8; C12-R1-12 |
| `media.sequence.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:772 | Placeholder; 0 | M8; C12-R1-12 |
| `media.sequence.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:773 | Placeholder; 0 | M8; C12-R1-12 |
| `media.sequence.move` (action) | divergent | `crates/registry/src/lib.rs:440`; A:774 | Placeholder; 0 | M8; C12-R1-12 |
| `media.sequence.naturalTiming.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:775 | Placeholder; 0 | M8; C12-R1-12 |
| `media.sequence.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:776 | Placeholder; 0 | M8; C12-R1-12 |
| `media.sequence.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:777 | Placeholder; 0 | M8; C12-R1-12 |
| `media.sound.play` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:778 | Placeholder; 0 | M8; C12-R1-12 |
| `media.sound.stop` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:779 | Placeholder; 0 | M8; C12-R1-12 |
| `media.source.missing` (event) | divergent | `crates/registry/src/lib.rs:440`; A:780 | Placeholder; 0 | M8; C12-R1-12 |
| `playback.forceRate.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:879 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.loop.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:880 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.player.close` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:881 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.player.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:882 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.player.open` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:883 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.player.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:884 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.preview.run` (job) | divergent | `crates/registry/src/lib.rs:440`; A:885 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.range.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:886 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.range.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:887 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.range.mode.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:888 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.range.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:889 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.rate.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:890 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.start` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:891 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.started` (event) | divergent | `crates/registry/src/lib.rs:440`; A:892 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.state.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:893 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.stop` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:894 | Placeholder; 0 | M5; C12-R1-12 |
| `playback.stopped` (event) | divergent | `crates/registry/src/lib.rs:440`; A:895 | Placeholder; 0 | M5; C12-R1-12 |
| `prefs.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:181 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `prefs.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:182 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `prefs.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:183 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `prefs.reset` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:184 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `prefs.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:185 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `print.pageSetup.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:988 | Placeholder; 0 | M10; C12-R1-12 |
| `print.pageSetup.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:989 | Placeholder; 0 | M10; C12-R1-12 |
| `print.preview.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:990 | Placeholder; 0 | M10; C12-R1-12 |
| `print.printer.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:991 | Placeholder; 0 | M10; C12-R1-12 |
| `print.run` (job) | divergent | `crates/registry/src/lib.rs:440`; A:992 | Placeholder; 0 | M10; C12-R1-12 |
| `print.settings.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:993 | Placeholder; 0 | M10; C12-R1-12 |
| `print.settings.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:994 | Placeholder; 0 | M10; C12-R1-12 |
| `project.autosaved` (event) | deferred-with-owner | `crates/executor/src/commands/project.rs:461`; A:70 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.backup.list` (query) | deferred-with-owner | `crates/executor/src/commands/project.rs:416`; A:71 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.backup.open` (action) | deferred-with-owner | `crates/executor/src/commands/project.rs:448`; A:72 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.close` (action) | deferred-with-owner | `crates/executor/src/commands/project.rs:136`; A:73 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.closed` (event) | deferred-with-owner | `crates/executor/src/commands/project.rs:474`; A:74 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.create` (action) | deferred-with-owner | `crates/executor/src/commands/project.rs:74`; A:75 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.dirty.changed` (event) | implemented | `crates/executor/src/commands/project.rs:497`; A:76 | Typed; 1 | C1.2; no additional per-command defect found |
| `project.info.get` (query) | deferred-with-owner | `crates/executor/src/commands/project.rs:204`; A:77 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.open` (job) | deferred-with-owner | `crates/executor/src/commands/project.rs:108`; A:78 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.opened` (event) | implemented | `crates/executor/src/commands/project.rs:521`; A:79 | Typed; 1 | C1.2; no additional per-command defect found |
| `project.presets.list` (query) | deferred-with-owner | `crates/executor/src/commands/project.rs:280`; A:80 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.properties.get` (query) | deferred-with-owner | `crates/executor/src/commands/project.rs:230`; A:81 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.properties.set` (action) | deferred-with-owner | `crates/executor/src/commands/project.rs:265`; A:82 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.recent.list` (query) | deferred-with-owner | `crates/executor/src/commands/project.rs:293`; A:83 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.recent.remove` (ui) | deferred-with-owner | `crates/executor/src/commands/project.rs:325`; A:84 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.recovery.delete` (ui) | deferred-with-owner | `crates/executor/src/commands/project.rs:394`; A:85 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.recovery.list` (query) | deferred-with-owner | `crates/executor/src/commands/project.rs:338`; A:86 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.recovery.open` (job) | deferred-with-owner | `crates/executor/src/commands/project.rs:362`; A:87 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.save` (job) | deferred-with-owner | `crates/executor/src/commands/project.rs:161`; A:88 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.saveAs` (job) | deferred-with-owner | `crates/executor/src/commands/project.rs:191`; A:89 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `project.saved` (event) | deferred-with-owner | `crates/executor/src/commands/project.rs:533`; A:90 | Typed; 1 | M1 C1.3; no additional per-command defect found |
| `render.frame.run` (query) | divergent | `crates/registry/src/lib.rs:440`; A:903 | Placeholder; 0 | M3 C3.3; C12-R1-12 |
| `render.limits.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:904 | Placeholder; 0 | M3 C3.3; C12-R1-12 |
| `render.range.run` (job) | divergent | `crates/registry/src/lib.rs:440`; A:905 | Placeholder; 0 | M3 C3.3; C12-R1-12 |
| `sound.asset.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:693 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.asset.import` (action) | divergent | `crates/registry/src/lib.rs:440`; A:694 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.asset.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:695 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.asset.missing` (event) | divergent | `crates/registry/src/lib.rs:440`; A:696 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.asset.probe` (query) | divergent | `crates/registry/src/lib.rs:440`; A:697 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.asset.relink` (action) | divergent | `crates/registry/src/lib.rs:440`; A:698 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.editor.close` (action) | divergent | `crates/registry/src/lib.rs:440`; A:699 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.editor.open` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:700 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.editor.state.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:701 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.editor.state.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:702 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.element.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:703 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.element.streamed.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:704 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.envelope.clear` (action) | divergent | `crates/registry/src/lib.rs:440`; A:705 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.envelope.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:706 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.envelope.point.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:707 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.envelope.point.move` (action) | divergent | `crates/registry/src/lib.rs:440`; A:708 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.envelope.point.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:709 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.envelope.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:710 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.fade.apply` (action) | divergent | `crates/registry/src/lib.rs:440`; A:711 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.import` (action) | divergent | `crates/registry/src/lib.rs:440`; A:712 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.placement.add` (action) | divergent | `crates/registry/src/lib.rs:440`; A:713 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.placement.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:714 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.placement.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:715 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.placement.move` (action) | divergent | `crates/registry/src/lib.rs:440`; A:716 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.placement.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:717 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.placement.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:718 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.placement.split` (action) | divergent | `crates/registry/src/lib.rs:440`; A:719 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.placement.trim` (action) | divergent | `crates/registry/src/lib.rs:440`; A:720 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.play` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:721 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.playback.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:722 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.range.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:723 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.range.select` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:724 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.recent.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:725 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.scrubbing.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:726 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.settings.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:727 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.silence.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:728 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.stop` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:729 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.waveform.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:730 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.waveform.ready` (event) | divergent | `crates/registry/src/lib.rs:440`; A:731 | Placeholder; 0 | M8; C12-R1-12 |
| `sound.waveforms.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:732 | Placeholder; 0 | M8; C12-R1-12 |
| `storyline.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:841 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.currentScene.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:842 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.currentScene.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:843 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.currentScene.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:844 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.frame.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:845 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.frame.render` (query) | divergent | `crates/registry/src/lib.rs:440`; A:846 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:847 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.play.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:848 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.play.start` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:849 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.play.stop` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:850 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.playhead.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:851 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.playhead.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:852 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.scene.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:853 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.scene.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:854 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.scene.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:855 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.scene.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:856 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.scene.move` (action) | divergent | `crates/registry/src/lib.rs:440`; A:857 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.scene.rename` (action) | divergent | `crates/registry/src/lib.rs:440`; A:858 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.scene.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:859 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.scene.visible.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:860 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.selection.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:861 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.selection.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:862 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.selection.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:863 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.transition.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:864 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.transition.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:865 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.transition.remove` (action) | divergent | `crates/registry/src/lib.rs:440`; A:866 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.transition.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:867 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.transition.types.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:868 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.view.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:869 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.view.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:870 | Placeholder; 0 | M9; C12-R1-12 |
| `storyline.warning.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:871 | Placeholder; 0 | M9; C12-R1-12 |
| `timeline.backgroundColour.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:475 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.currentFrame.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:476 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.currentFrame.move` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:477 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.currentFrame.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:478 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.effectParameterRows.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:479 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.frameLabel.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:480 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.playbackRange.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:481 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.playbackRange.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:482 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.playbackRange.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:483 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.rows.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:484 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.singleCellSelection.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:485 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.soundWaveforms.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:486 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.timeDisplay.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:487 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.transferOptions.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:488 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.transferOptions.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:489 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `timeline.view.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:490 | Placeholder; 0 | M5 C5.1; C12-R1-12 |
| `tool.active.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:285 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.active.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:286 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.active.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:287 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.brush.options.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:288 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.drawTopLayer.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:289 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.eraser.options.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:290 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.group.select` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:291 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:292 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.options.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:293 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.options.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:294 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.options.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:295 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.pen.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:296 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.pen.create` (action) | divergent | `crates/registry/src/lib.rs:440`; A:297 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.pen.delete` (action) | divergent | `crates/registry/src/lib.rs:440`; A:298 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.pen.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:299 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.pen.rename` (action) | divergent | `crates/registry/src/lib.rs:440`; A:300 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.pen.select` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:301 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.pen.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:302 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.pencil.options.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:303 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.placeBehind.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:304 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.polyline.options.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:305 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.select.options.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:306 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.settings.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:307 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.snapToContour.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:308 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.symmetry.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:309 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.tablet.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:310 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `tool.text.options.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:311 | Placeholder; 0 | M4 C4.2; C12-R1-12 |
| `ui.contextHelpMode.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:208 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `ui.help.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:209 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `ui.help.open` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:210 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `ui.message.shown` (event) | divergent | `crates/registry/src/lib.rs:440`; A:211 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `ui.preferences.open` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:212 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `ui.startScreen.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:213 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `ui.statusBar.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:214 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `ui.statusBar.set` (toggle) | divergent | `crates/registry/src/lib.rs:440`; A:215 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `ui.theme.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:216 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `ui.tips.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:217 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `ui.tips.reset` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:218 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `ui.tips.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:219 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `view.active.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:125 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `view.coordinates.convert` (query) | divergent | `crates/registry/src/lib.rs:440`; A:126 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `view.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:127 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `view.pan.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:128 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `view.rotation.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:129 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `view.transform.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:130 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `view.transform.reset` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:131 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `view.type.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:132 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `view.zoom.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:133 | Placeholder; 0 | M4 C4.1; C12-R1-12 |
| `window.floatingPanels.set` (toggle) | implemented | `crates/executor/src/commands/window.rs:171`; A:141 | Typed; 1 | C1.2; no additional per-command defect found |
| `window.fullScreen.set` (toggle) | divergent | `crates/executor/src/commands/window.rs:22`; A:142 | Typed; 2 | C1.2; C12-R1-15 |
| `window.geometry.get` (query) | deferred-with-owner | `crates/executor/src/commands/window.rs:434`; A:143 | Typed; 1 | M4 C4.1; no additional per-command defect found |
| `window.geometry.set` (ui) | deferred-with-owner | `crates/executor/src/commands/window.rs:471`; A:144 | Typed; 1 | M4 C4.1; no additional per-command defect found |
| `window.minimized.set` (toggle) | deferred-with-owner | `crates/executor/src/commands/window.rs:492`; A:145 | Typed; 1 | M4 C4.1; no additional per-command defect found |
| `window.panel.list` (query) | implemented | `crates/executor/src/commands/window.rs:35`; A:146 | Typed; 1 | C1.2; no additional per-command defect found |
| `window.panel.move` (ui) | implemented | `crates/executor/src/commands/window.rs:137`; A:147 | Typed; 1 | C1.2; no additional per-command defect found |
| `window.panel.set` (toggle) | implemented | `crates/executor/src/commands/window.rs:71`; A:148 | Typed; 2 | C1.2; no additional per-command defect found |
| `window.raiseAll.run` (ui) | deferred-with-owner | `crates/executor/src/commands/window.rs:505`; A:149 | Typed; 1 | M4 C4.1; no additional per-command defect found |
| `window.sidePanelsExtended.set` (toggle) | implemented | `crates/executor/src/commands/window.rs:184`; A:150 | Typed; 1 | C1.2; no additional per-command defect found |
| `window.toolbar.items.reset` (ui) | implemented | `crates/executor/src/commands/window.rs:385`; A:151 | Typed; 1 | C1.2; no additional per-command defect found |
| `window.toolbar.items.set` (ui) | implemented | `crates/executor/src/commands/window.rs:349`; A:152 | Typed; 1 | C1.2; no additional per-command defect found |
| `window.toolbar.list` (query) | implemented | `crates/executor/src/commands/window.rs:248`; A:153 | Typed; 1 | C1.2; no additional per-command defect found |
| `window.toolbar.set` (toggle) | implemented | `crates/executor/src/commands/window.rs:307`; A:154 | Typed; 1 | C1.2; no additional per-command defect found |
| `workspace.changed` (event) | implemented | `crates/executor/src/commands/workspace.rs:319`; A:162 | Typed; 1 | C1.2; no additional per-command defect found |
| `workspace.create` (ui) | implemented | `crates/executor/src/commands/workspace.rs:117`; A:163 | Typed; 1 | C1.2; no additional per-command defect found |
| `workspace.delete` (ui) | implemented | `crates/executor/src/commands/workspace.rs:168`; A:164 | Typed; 1 | C1.2; no additional per-command defect found |
| `workspace.export` (ui) | deferred-with-owner | `crates/executor/src/commands/workspace.rs:259`; A:165 | Typed; 1 | M4 C4.1; no additional per-command defect found |
| `workspace.get` (query) | implemented | `crates/executor/src/commands/workspace.rs:35`; A:166 | Typed; 2 | C1.2; no additional per-command defect found |
| `workspace.import` (ui) | deferred-with-owner | `crates/executor/src/commands/workspace.rs:297`; A:167 | Typed; 1 | M4 C4.1; no additional per-command defect found |
| `workspace.layout.set` (ui) | implemented | `crates/executor/src/commands/workspace.rs:223`; A:168 | Typed; 1 | C1.2; no additional per-command defect found |
| `workspace.list` (query) | implemented | `crates/executor/src/commands/workspace.rs:14`; A:169 | Typed; 1 | C1.2; no additional per-command defect found |
| `workspace.rename` (ui) | divergent | `crates/executor/src/commands/workspace.rs:140`; A:170 | Typed; 1 | C1.2; C12-R1-16 |
| `workspace.reset` (ui) | divergent | `crates/executor/src/commands/workspace.rs:203`; A:171 | Typed; 1 | C1.2; C12-R1-16 |
| `workspace.save` (ui) | implemented | `crates/executor/src/commands/workspace.rs:80`; A:172 | Typed; 1 | C1.2; no additional per-command defect found |
| `workspace.select` (ui) | divergent | `crates/executor/src/commands/workspace.rs:67`; A:173 | Typed; 1 | C1.2; C12-R1-16 |
| `xsheet.columnWidth.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:498 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `xsheet.currentCell.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:499 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `xsheet.display.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:500 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `xsheet.exposures.get` (query) | divergent | `crates/registry/src/lib.rs:440`; A:501 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `xsheet.exposures.set` (action) | divergent | `crates/registry/src/lib.rs:440`; A:502 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `xsheet.lightTable.add` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:503 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `xsheet.lightTable.changed` (event) | divergent | `crates/registry/src/lib.rs:440`; A:504 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `xsheet.lightTable.list` (query) | divergent | `crates/registry/src/lib.rs:440`; A:505 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `xsheet.lightTable.move` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:506 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `xsheet.lightTable.remove` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:507 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `xsheet.lightTable.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:508 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
| `xsheet.panel.set` (ui) | divergent | `crates/registry/src/lib.rs:440`; A:509 | Placeholder; 0 | M3 C3.1; C12-R1-12 |
