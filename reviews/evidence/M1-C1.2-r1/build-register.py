"""Independent spec-A set comparison and evidence-backed conformance register.

Reads the frozen spec and executor-captured registry separately. Does not import the builder's generator.
The classifications below are reviewer adjudications, not inferred claims of semantic correctness.
"""
from collections import Counter
import csv
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def source(path, needle):
    lines = (ROOT / path).read_text(encoding="utf-8").splitlines()
    line = next(i for i, text in enumerate(lines, 1) if needle in text)
    return f"{path}:{line}"


def main():
    spec = {}
    namespace = ""
    for n, line in enumerate((ROOT / "spec/A_command_registry.md").read_text(encoding="utf-8").splitlines(), 1):
        m = re.match(r"## Namespace `([^`]+)`", line)
        if m:
            namespace = m[1]
        if re.match(r"\| `[^`]+` \| (action|toggle|query|job|ui|event) \|", line):
            cols = [c.strip().replace(r"\|", "|") for c in re.split(r"(?<!\\)\|", line)[1:-1]]
            assert len(cols) == 6, (n, cols)
            ident = cols[0].strip("`")
            assert ident not in spec
            spec[ident] = dict(zip(["id", "kind", "summary", "params", "result", "undo"], cols)) | {"line": n, "namespace": namespace}
    registry = json.loads((HERE / "registry.json").read_text(encoding="utf-8"))
    by_id = {e["id"]: e for e in registry}
    menu=json.loads((ROOT/"desktop/src/ui-bindings/menu.json").read_text(encoding="utf-8"))
    def menu_ids(v):
        if isinstance(v,dict):
            yield from ([v["command"]] if v.get("kind")=="command" else [])
            for child in v.values():yield from menu_ids(child)
        elif isinstance(v,list):
            for child in v:yield from menu_ids(child)
    bound=set(menu_ids(menu))
    assert len(by_id) == len(registry) == len(spec) == 796
    assert set(by_id) == set(spec), "missing or extra inventory ID"
    declarations = {}
    for path in (ROOT / "crates/executor/src/commands").glob("*.rs"):
        for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            m = re.search(r'\bid = "([^"]+)"', line)
            if m:
                declarations[m[1]] = f"{path.relative_to(ROOT).as_posix()}:{n}"
    defects = {
        "app.mode.set": "C12-R1-15", "app.state.get": "C12-R1-15", "window.fullScreen.set": "C12-R1-15",
        "doc.object.patch": "C12-R1-03,07,11,19", "doc.diff": "C12-R1-08,09", "doc.object.find": "C12-R1-23",
        "checkpoint.create": "C12-R1-10", "checkpoint.list": "C12-R1-10",
        "job.wait": "C12-R1-04,05,13", "job.list": "C12-R1-05", "job.cancel": "C12-R1-02,05",
        "edit.clipboard.get": "C12-R1-18,20", "edit.clipboard.set": "C12-R1-02,17,20",
        "edit.clipboard.changed": "C12-R1-02", "workspace.select": "C12-R1-16",
        "workspace.rename": "C12-R1-16", "workspace.reset": "C12-R1-16",
    }
    for ident in ["edit.cut", "edit.copy", "edit.paste", "edit.delete", "edit.cutSpecial", "edit.pasteSpecial", "edit.pasteAsNewElement"]:
        defects[ident] = "C12-R1-14"
    examples = json.loads((HERE / "examples.json").read_text(encoding="utf-8"))
    example_counts = Counter(e["id"] for e in examples)
    rows = []
    for ident, e in by_id.items():
        s = spec[ident]
        assert e["kind"] == s["kind"]
        assert ident.split(".")[0] == s["namespace"]
        assert e["specParameters"] == s["params"]
        assert e["specResult"] == s["result"]
        assert e["specUndo"] == s["undo"]
        finding = defects.get(ident, "")
        if not e["typed"]:
            status, finding = "divergent", "C12-R1-12"
        elif finding:
            status = "divergent"
        elif e["status"] == "notImplemented":
            status = "deferred-with-owner"
        else:
            status = "implemented"
        if ident == "import.movie.run":
            finding += ",21"
        # Typed schema success only establishes structural contracts; known semantic failures remain explicit.
        row = {"id": ident, "kind": e["kind"], "namespace": s["namespace"], "status": status,
               "declaredStatus": e["status"], "owner": e.get("milestone", "C1.2"),
               "spec": f"spec/A_command_registry.md:{s['line']}",
               "source": declarations.get(ident, "crates/registry/src/lib.rs:440"),
               "paramsSchema": "typed" if e["typed"] else "divergent: unconstrained object",
               "resultSchema": ("divergent: timedOut required but omitted" if ident == "job.wait" else "typed") if e["typed"] else "divergent: accepts every value",
               "examplesExecuted": example_counts[ident], "examples": len(e["examples"]),
               "enablement": e["enablement"], "needsProject": e["needsProject"], "scopes": e["scopes"],
               "undoLabel": e["undoLabel"], "specUndo": s["undo"],
               "parity": "manifest" if ident in bound else "declared location/justification" if e["typed"] else "missing reverse coverage",
               "findings": finding,
               "tests": "c12_r1_stress::every_executable_typed_example_also_validates_its_result_schema" if e["typed"] else "c12_r1_contracts::registry_stubs_have_actual_parameter_constraints_and_examples",
               "limits": "All typed examples are exercised; events must reject invocation; stubs must return notImplemented. Implemented means no per-command divergence found in this sweep, not immunity to cross-cutting executor defects."}
        params=s["params"].strip("`")
        names=[]
        for m in re.finditer(r"(?:^|;\s*)([A-Za-z][A-Za-z0-9]*)\??\s*:",params):
            before=params[:m.start()]
            if before.count("{")==before.count("}"):
                names.append(m[1])
        row["specifiedTopLevelParameterNames"]=names
        row["typedMissingTopLevelParameters"]=[name for name in names if name not in e["paramsSchema"].get("properties",{})] if e["typed"] and e["kind"]!="event" else []
        rows.append(row)
    (HERE / "command-register.json").write_text(json.dumps(rows, indent=2)+"\n", encoding="utf-8")

    # Requirement | status | implementation location | test/evidence | adjudication.
    raw = r"""
01 Inventory IDs, namespace, kind and verbatim table columns (A)|implemented|crates/registry/src/inventory.rs:8|c12_registry; registry.json; this independent set comparison|796 exact; no added/missing ID
02 Every inventory entry has constrained parameter/result schemas (plan 3.2; 11 14.2)|divergent|crates/registry/src/lib.rs:464|registry_stubs_have_actual_parameter_constraints_and_examples|C12-R1-12: 662 untyped placeholders
03 Every command example executes and matches its result schema (11 14.2)|divergent|crates/executor/src/commands/job.rs:63|every_executable_typed_example_also_validates_its_result_schema|147 typed examples; one result-schema violation; 662 entries lack examples; C12-R1-12,13
04 Every stub names a correct owner (plan 4)|divergent|crates/registry/src/owners.rs:14|registry_owners_follow_the_milestone_plan|C12-R1-21; typed deferred owners in command-register.json
05 Registry version and generated frontend schema freshness|implemented|crates/registry/src/lib.rs:52|frontend_schemas; c12_registry|0.1.0; generated snapshots remain current
06 Attribute macro rejects missing example/duplicate ID/wrong kind|implemented|crates/registry-macros/src/lib.rs:176|c12_r1_macro|Missing example compile error; duplicates/wrong kind assembly errors
07 Exclusive mutation capability (plan 3.2; 11 P2)|divergent|crates/executor/src/lib.rs:180|external_safe_code_cannot_install_a_mutated_stale_snapshot; compile_fail|Private Cx fields enforced; safe snapshot/edit/open replacement bypass remains; C12-R1-01
08 One serialized document queue (REV-06)|implemented|crates/executor/src/lib.rs:249|eight_clients_cas_commands_and_batches_have_no_lost_updates|8 clients, 200 commits; every rev 2..201 once; no lost updates
09 Staged full integrity gate and rollback of document edits|implemented|crates/executor/src/cx.rs:326|c12_transactions; staged_commit; c11_r4/r5/r6|Multi-entry writes use checked staging; inherited structural bounds retained
10 Atomic rollback covers every observable side effect (11 7.3)|divergent|crates/executor/src/run.rs:48|failed_atomic_batch_cannot_cancel_a_running_job; failed_batch_clipboard_emits_no_change_event|C12-R1-02: shared jobs and pending events escape rollback
11 Atomic batch one revision, undo step and label|implemented|crates/executor/src/run.rs:512|c12_transactions|One commit, batch label, rollback on model failure
12 Query steps see prior staged changes (11 7.3)|divergent|crates/executor/src/commands/doc.rs:1215|doc_diff_sees_prior_uncommitted_atomic_steps|Most queries do; doc.diff reads live project instead; C12-R1-09
13 Numeric/named intermediate references|implemented|crates/executor/src/run.rs:622|c12_transactions|1-based SI-017 interpretation; missing/forward references rejected
14 Long-running job-kind steps rejected in atomic batches|implemented|crates/executor/src/run.rs:138|c12_transactions|Kind=job rejected; job.cancel is kind=ui and still must roll back
15 Ordinary edit dryRun changes/history/revisions|implemented|crates/executor/src/run.rs:370|ordinary_dry_run_preserves_exact_history_and_matches_real_result|Exact snapshot/history unchanged; result and changes match real call
16 Dry run covers clipboard events and cancellation|divergent|crates/executor/src/run.rs:48|dry_run_cannot_cancel_a_running_job; dry_run_clipboard_emits_no_change_event|C12-R1-02
17 Command-parameter dryRun in atomic batch|divergent|crates/executor/src/run.rs:512|parameter_dry_run_is_honoured_inside_atomic_batch|C12-R1-03
18 Sequential dry-run batch simulates preceding edits|divergent|crates/executor/src/run.rs:471|sequential_batch_dry_run_queries_see_previous_simulated_edits|C12-R1-03
19 Common run_command envelope including job.wait|divergent|crates/executor/src/lib.rs:218|job_wait_obeys_request_id_and_retains_document_revision|job.wait bypasses queue/options/rev/cache; C12-R1-04
20 Undo/redo canonical content and revision stamps|implemented|crates/executor/src/cx.rs:450|ten_thousand_history_operations_match_independent_content_oracle_and_replay; c11_r6|Fresh seed; 10k operations + fresh 10k replay byte-identical
21 Failed batch history, redo invalidation, grouped labels|implemented|crates/executor/src/run.rs:231|c12_transactions; c12_r1_stress|No failed-document batch step retained; new edits invalidate redo
22 Checkpoint create/list/delete/restore and history independence|implemented|crates/executor/src/commands/checkpoint.rs:1|c12_transactions; c12_r1_stress|Normal checkpoints restore undoably; copy-on-write, no full serialized copy
23 Checkpoint created from staged content has coherent rev|divergent|crates/executor/src/commands/checkpoint.rs:60|checkpoint_in_atomic_batch_has_the_committed_content_revision|C12-R1-10
24 Checkpoint cost at 100k strokes/10k drawings|implemented|crates/model/src/document/mod.rs:159|c12_r1_scale; large-documents.json|20 creates measured; per-entry Arc sharing, index sharing, map clone cost remains
25 expectRev scalar and envelope guard|implemented|crates/executor/src/run.rs:173|c12_transactions; eight_clients_cas_commands_and_batches_have_no_lost_updates|Normal CAS conflicts prevent mutations
26 expectRev map checks every listed object (REV-05)|divergent|crates/executor/src/commands/doc.rs:874|expect_rev_map_checks_every_listed_object_not_only_patch_targets|C12-R1-07
27 Conflict details.currentRev and current small JSON (REV-05)|divergent|crates/executor/src/run.rs:189|expect_rev_error_reports_current_rev_and_small_current_object|C12-R1-07
28 requestId retry same ID returns original without execution|divergent|crates/executor/src/run.rs:22|session_idempotency_survives_more_than_ten_thousand_other_requests|Eviction permits destructive retry in same session; job.wait bypass; C12-R1-04,06
29 requestId different parameters same ID|implemented|crates/executor/src/run.rs:356|request_id_collision_returns_original_result_per_literal_spec|Literal 11 7.2 met; stricter review-charter rule conflicts/needs ruling; C12-R1-22 P2
30 requestId persistence across process restart|deferred-with-owner|crates/executor/src/run.rs:22|source audit|Not required by 11 7.2 (within-session); durable session protocol is C1.4 decision, not a persistence defect
31 documentRev exactly +1 per edit/undo/redo (REV-01/03)|implemented|crates/executor/src/cx.rs:388|c12_transactions; c12_r1_stress|Normal revisions checked on every random operation
32 Revision overflow is controlled error, not thread panic|divergent|crates/executor/src/cx.rs:388|revision_exhaustion_is_an_error_and_does_not_kill_the_document_thread|C12-R1-19
33 Drawing rev follows content edits (REV-02)|implemented|crates/model/src/document/stamp.rs:1|c12_transactions; changes_report_drawing_revision_when_a_stroke_changes|Stamp correct; notification incomplete in next row
34 Every changed revision returned to caller (11 P6)|divergent|crates/executor/src/cx.rs:617|changes_report_drawing_revision_when_a_stroke_changes|C12-R1-11: changed drawing omitted
35 Persist revs through save/reopen (REV-04)|deferred-with-owner|crates/executor/src/commands/project.rs:1|typed declarations; handoff|C1.3 persistence; cannot prove with C1.2 stubs
36 Change sets oldValue/concrete paths/numeric opaque keys|implemented|crates/model/src/document/mod.rs:535|staged_commit; staged_inverse; c11_r6; c12_r1_stress|Checked canonical before/after diffs; no stateless inversion resurrected
37 Historical doc.diff within retained 10k window|divergent|crates/executor/src/commands/doc.rs:1166|diff_older_than_snapshot_window_replays_repeated_entry_batch; historical_diff_reports_the_revision_of_its_requested_end_state|C12-R1-08: repeated-entry reconstruction fails and old revs drift
38 doc.diff retains last save even beyond 10k (AG-09)|divergent|crates/executor/src/session.rs:104|diff_retains_the_last_save_even_after_ten_thousand_transactions|C12-R1-08
39 500-change envelope cap and oversized event elision|implemented|crates/executor/src/run.rs:184|c12_transactions; c12_policy|ChangesSince/truncated and >64 KiB elision; query for full values
40 IDs/compound paths/special references/projections|implemented|crates/executor/src/cx.rs:1|c12_policy; c11_r1/r2/r3|SI-018 current-element path choice qualified; known active-view defect C12-R1-15
41 Derived fields already computable from model|implemented|crates/executor/src/commands/derived.rs:1|c12_policy|Exposures/frame counts/drawing content projections covered
42 Geometry/render/media-derived fields not yet computable|deferred-with-owner|crates/executor/src/commands/derived.rs:1|c12_policy; doc.derivedUnavailable|Explicit M2/M3/M7/M8 owners and warnings; no pretend complete renderer
43 x-command / structural mutation routes and drawing aliases|deferred-with-owner|crates/executor/src/commands/doc.rs:837|c12_registry::drawing_command_delegation_ruling; c12_policy|Aliases mapped; specialized handlers intentionally M2/M3 etc; no hidden generic mutation
44 No-project enablement (01 1.3)|divergent|crates/executor/src/commands/edit.rs:635|edit_clipboard_is_disabled_without_a_project|Clipboard get/set omit project requirement; C12-R1-20
45 Dirty-project quit fail/discard/save policy|implemented|crates/executor/src/commands/app.rs:481|c12_kernel_state|Fail and discard implemented; save returns owned C1.3 stub; native quit routing later
46 Fullscreen explicit enabled parameter|implemented|crates/executor/src/commands/window.rs:31|c12_kernel_state; c12_r1_contracts|Explicit setting/repeat idempotent; active context divergence next row
47 Mode/current view/fullscreen readback agreement|divergent|crates/executor/src/commands/app.rs:381|setting_mode_activates_a_view_of_the_requested_type; fullscreen_state_and_active_view_agree_with_visible_central_panel|C12-R1-15
48 Workspace autoSave/default/save/reset rules|divergent|crates/executor/src/ui.rs:917|workspace_switch_defaults_to_autosaving_current_arrangement; explicitly_saving_and_reselecting_current_workspace_keeps_layout; renamed_builtin_workspace_can_still_be_restored|C12-R1-16
49 Queryable panels/layout/toolbars|implemented|crates/executor/src/ui.rs:1|c12_kernel_state|Kernel state only; no native rendering assertion
50 All edit.* functionality due C1.2 (plan 4)|missing|crates/executor/src/commands/edit.rs:1|edit_commands_promised_for_c12_are_implemented|Seven commands return M4 stubs without a plan exception; C12-R1-14
51 Clipboard structured kind validation|divergent|crates/executor/src/commands/edit.rs:608|clipboard_set_validates_the_content_for_its_kind|C12-R1-17
52 Clipboard return shape|divergent|crates/executor/src/commands/edit.rs:630|clipboard_get_returns_the_specified_clipboard_directly|C12-R1-18
53 Manifest forward parity; declared typed reverse coverage|implemented|crates/executor/tests/c12_registry.rs:138|c12_registry::ui_binding_manifest_parity|Existing menu IDs present; strings describe intended GUI locations
54 Full-inventory reverse parity build gate (11 14.1)|divergent|crates/executor/tests/c12_registry.rs:166|every_registry_entry_has_a_gui_binding_or_agent_only_justification|662 skipped; C12-R1-12
55 Real native command dispatch/state parity|deferred-with-owner|desktop/src-tauri/src/menu.rs:1|handoff; source audit|M4 C4.1 per accepted readiness split; native fullscreen still toggles directly
56 Every registered command malformed inputs under watchdog|implemented|crates/executor/src/lib.rs:149|c12_r1_stress; c12_r1_scale|2388 small +1592 >1MiB calls; no internal/panic on these fixtures
57 Watchdog bounds caller vs actual execution|implemented|crates/executor/src/lib.rs:149|c12_kernel_state::the_watchdog_answers_within_its_budget; job_wait_timeout_policy_is_explicitly_measured|Caller timeout only; no cancellation/loop recovery claim; job.wait uses own timeout
58 Perpetually looping registered handler and modal interception proof|missing|crates/executor/src/lib.rs:249|source audit; no injectable Core/registry hook|Cannot inject a handler through public API without product changes; no catch/unwind/interrupt; headless examples alone do not prove recovery
59 Explicit scopes on implemented calls|implemented|crates/executor/src/run.rs:114|c12_policy|Agent read/edit/ui checks; confirmation engine still C1.4
60 Job session isolation|divergent|crates/executor/src/commands/job.rs:37|agent_cannot_read_another_sessions_jobs_with_all_true; agent_cannot_cancel_another_sessions_job|C12-R1-05
61 File/prefs/network scopes on inventory stubs|divergent|crates/registry/src/lib.rs:450|registry_file_and_network_commands_declare_required_scopes|C12-R1-12
62 Protocol initialize/tools/resources/subscriptions/prompts/events/audit/confirmation|deferred-with-owner|crates/executor/src/commands/agent.rs:1|agent.* typed declarations; bins/headless flags|C1.4; no MCP inspector run possible at this checkpoint
63 Job wait/status/cancel/progress table|implemented|crates/executor/src/jobs.rs:1|c12_kernel_state|Background progress and cooperative cancellation exist; isolation/transaction exceptions above
64 No new unsafe/product unwrap/expect/panic allowances|implemented|crates/executor/src/lib.rs:1|source audit; clippy; CI|New unsafe absent; test/tool allowances scoped; overflow panic is separate finding
65 Existing three-layer IPC gate / worker ban / diagnostics gate|implemented|desktop/eslint.config.js:1|baseline build; Vitest boundary 69+46; CI default/diagnostics builds|No worker exception introduced; accepted runtime-string limitation unchanged
66 Composite command-result boundary controls|implemented|desktop/src/ipc/commands.test.ts:1|7 command schema tests; frontend_schemas|Wide ints, finite doubles, own keys, snapshots, depth, accessors/proxies retained
67 Touched-object diff and 100k full-commit costs|implemented|crates/executor/src/cx.rs:617|commit-scale.log; large-documents.json|Touched path implemented; whole-entry parse/diff/inverse integrity cost remains; M2 optimization
68 10k-drawing query performance|divergent|crates/executor/src/commands/doc.rs:485|c12_r1_scale; large-documents.json|6.372 s release find100 on empty drawings; P2 C12-R1-23
69 Dependency/licence and lockfile changes|implemented|Cargo.lock:1|cargo deny; CI; git diff|Only local executor/registry plus trybuild support additions; no new unapproved licence found
70 Reviewer test preservation and CI coverage accuracy|implemented|.github/workflows/ci.yml:1|ci-audit.json; both raw run logs; git diff|Existing reviewer tests untouched; Windows/macOS check runs include suites
71 Fresh-seed randomized coverage and shrinking|implemented|crates/executor/tests/c12_r1_stress.rs:1|random-history.json; existing proptest suites|Fresh deterministic sequence recorded; new oracle reports operation but does not shrink; existing properties shrink
72 Persistence unchecked assembly confinement|deferred-with-owner|crates/model/src/document/mod.rs:1|C1.1 closure carryover|C1.3 load/repair; open capability must first close C12-R1-01
"""
    requirements=[]
    for line in raw.strip().splitlines():
        desc,status,implementation,test,note=line.split("|",4)
        ident,desc=desc.split(" ",1)
        requirements.append(dict(id="R"+ident,requirement=desc,status=status,implementation=implementation,tests=test,note=note))
    (HERE / "requirements.json").write_text(json.dumps(requirements,indent=2)+"\n",encoding="utf-8")
    counts={"inventory":len(rows),"typed":sum(e["typed"] for e in registry),"typedImplemented":sum(e["typed"] and e["status"]=="implemented" for e in registry),"typedDeferred":sum(e["typed"] and e["status"]!="implemented" for e in registry),"untyped":sum(not e["typed"] for e in registry),"events":sum(e["kind"]=="event" for e in registry),"implementedEvents":sum(e["kind"]=="event" and e["status"]=="implemented" for e in registry),"examples":len(examples),"commandClassifications":dict(Counter(r["status"] for r in rows)),"requirementClassifications":dict(Counter(r["status"] for r in requirements))}
    counts["untypedBoundInManifest"]=sum(not e["typed"] and e["id"] in bound for e in registry)
    counts["unreachableUnannotatedEntries"]=sum(r["parity"]=="missing reverse coverage" for r in rows)
    counts["typedMissingTopLevelParameters"]={r["id"]:r["typedMissingTopLevelParameters"] for r in rows if r["typedMissingTopLevelParameters"]}
    (HERE / "register-counts.json").write_text(json.dumps(counts,indent=2)+"\n",encoding="utf-8")
    md=["# M1-C1.2 round 1 conformance register", "", "Reviewed 23ca3d5. Rebuild with `python reviews/evidence/M1-C1.2-r1/build-register.py` after recording the Rust sweeps. `registry.json` contains the complete schemas and verbatim spec columns; `command-register.json` contains each field assessment. This file is an index, not a substitute for the failing tests.", "", "## Counts", "", "```json", json.dumps(counts,indent=2), "```", "", "Implemented means exercised with no additional per-row defect found; cross-cutting executor findings still apply. Deferred-with-owner means delivery is later per plan, not that its placeholder can satisfy current registry metadata requirements. In particular all 662 untyped entries are divergent metadata, while their handlers remain future work. Event examples are checked for non-invocability, not counted as successful actions. Native GUI and MCP coverage are explicitly separate.", "", "## Cross-cutting requirements", "", "| Requirement | Ruling | Implementation | Evidence/test | Qualification |", "|---|---|---|---|---|"]
    for r in requirements:
        md.append(f"| {r['id']} {r['requirement']} | {r['status']} | `{r['implementation']}` | {r['tests']} | {r['note']} |")
    md += ["", "## Complete spec A inventory", "", "All 796 IDs, namespace/kind and verbatim parameters/result/undo columns match the frozen appendix; no extra ID exists. `Typed` below means real generated schemas (with the job.wait exception), examples and declaration metadata. `Placeholder` means unconstrained schemas, zero examples, default needsProject=false, undoLabel=null, guessed kind-based scopes and no reverse parity. Scope/undo/enablement values for **every** row are retained in the companion JSON. Owners are claimed owners, with identified plan disagreements flagged. M1 typed fields were compared with the area tables and exercised by the example/contract suites; future handler behavior is not claimed verified.", "", "| ID / kind | Ruling | Declaration / spec A line | Contract / examples executed | Owner / findings |", "|---|---|---|---|---|"]
    for r in rows:
        md.append(f"| `{r['id']}` ({r['kind']}) | {r['status']} | `{r['source']}`; A:{spec[r['id']]['line']} | {'Typed' if by_id[r['id']]['typed'] else 'Placeholder'}; {r['examplesExecuted']} | {r['owner']}; {r['findings'] or 'no additional per-command defect found'} |")
    (HERE / "conformance.md").write_text("\n".join(md)+"\n",encoding="utf-8")
    print(json.dumps(counts,indent=2))


if __name__ == "__main__":
    main()
