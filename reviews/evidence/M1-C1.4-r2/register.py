"""Re-use the round-one unit parser, re-anchor sources, and adjudicate every delta explicitly."""
from pathlib import Path

HERE = Path(__file__).resolve().parent
original = (HERE.parent / 'M1-C1.4-r1/register.py').read_text(encoding='utf-8')
delta = r'''
# Builder's round-one fixes, with the specific proving suites. Main status is M1 status;
# deferred domain parts retain the owner in the note.
fixed = {
23: 'tests/adversarial/c14-r2-mcp.py::test_long_unicode_and_nested_globs_keep_the_server_usable',
26: 'tests/adversarial/c14-r1-mcp.py::test_elicitation_is_one_audit_entry_per_tool_call',
66: 'crates/executor/tests/c14_obligations.rs::an_activity_export_is_written_only_when_its_call_commits',
81: 'crates/mcp/tests/c14_r1_limits.rs::configured_batch_limit_applies_to_batch_script',
189: 'crates/mcp/tests/c14_r1_limits.rs::saved_observe_policy_and_limits_survive_restart; crates/executor/tests/c14_r2_contracts.rs',
207: 'tests/adversarial/c14-r1-mcp.py::test_malformed_elicitation_does_not_overwrite',
219: 'tests/adversarial/c14-r2-mcp.py::test_decline_cancel_and_invalid_accept_settle_request_id_and_audit_once',
249: 'crates/mcp/tests/c14_r1_limits.rs; crates/mcp/tests/protocol.rs::session_limits_cap_batches_and_jobs',
263: 'tests/adversarial/c14-r2-mcp.py::test_all_state_include_combinations_are_command_projections',
271: 'tests/adversarial/c14-r2-mcp.py::test_all_state_include_combinations_are_command_projections',
272: 'tests/adversarial/c14-r2-mcp.py::test_long_unicode_and_nested_globs_keep_the_server_usable',
280: 'tests/adversarial/c14-r2-mcp.py::test_nested_wrapper_validation_never_edits',
297: 'tests/adversarial/c14-r1-mcp.py::test_activity_batch_export_does_not_turn_dry_runs_into_edits',
300: 'tests/adversarial/c14-r1-mcp.py::test_accepted_elicitation_preserves_request_id_result',
309: 'tests/adversarial/c14-r1-mcp.py::test_activity_batch_export_keeps_committed_batch_operations',
311: 'crates/executor/tests/c14_obligations.rs::an_activity_export_is_written_only_when_its_call_commits',
386: 'tests/adversarial/c14-r2-mcp.py::test_all_state_include_combinations_are_command_projections',
407: 'tests/adversarial/c14-r2-mcp.py::test_nested_wrapper_validation_never_edits',
458: 'tests/adversarial/c14-r1-mcp.py::test_undo_and_redo_emit_the_declared_events',
505: 'tests/adversarial/c14-r1-mcp.py::test_elicitation_is_one_audit_entry_per_tool_call',
571: 'tests/adversarial/c14-r2-mcp.py::test_long_unicode_and_nested_globs_keep_the_server_usable',
573: 'tests/adversarial/c14-r1-mcp.py; scripts/mcp-conformance.py',
588: 'tests/adversarial/c14-r1-mcp.py::test_malformed_elicitation_does_not_overwrite',
}
for r in rows:
    n = r['line']
    if n in fixed:
        r.update(status='implemented', test=fixed[n], note='Round-one fix verified on 74fe3a9; proving tests listed. Re-adjudication below overrides this if a round-two variant fails.')
    if n == 96:
        r.update(note='SI-049 remains open: 2025-06-18 and 2025-11-25 work; current-stable target not implemented. Also C14-R2-04 live elicitation cancellation.', test='tests/adversarial/c14-r2-mcp.py::test_cancel_live_elicitation_releases_the_call_worker; scripts/mcp-conformance.py')
    if n == 417:
        r.update(status='deferred-with-owner', test='crates/mcp/tests/c14_tickets.rs::every_result_lists_its_effective_params', note='C14-R1-16 ticket: builder M4 C4.x (plan section 4); ignored failing test, body unchanged.')
    if n == 263:
        r['note'] += ' Render annotations remain builder M3.'
    if n == 571:
        r['note'] += ' Document-thread restart remains builder M4; a never-returning handler still occupies its worker.'
    if n == 588:
        r['note'] += ' Network half remains builder at the first network command.'
    new = {
        26: ('C14-R2-01/02/03: activity/replay attribution is inaccurate.', 'tests/adversarial/c14-r2-mcp.py; crates/mcp/tests/c14_r2_replay.rs'),
        311: ('C14-R2-05: token, JSON bundle and media exports publish outside the atomic commit gate.', 'crates/executor/tests/c14_r2_effects.rs'),
        505: ('C14-R2-01/02/03: failed file publication recorded as success; replay omits partial commits and duplicates cached retries.', 'tests/adversarial/c14-r2-mcp.py; crates/mcp/tests/c14_r2_replay.rs'),
        573: ('C14-R2-04: cancellation of an active elicitation does not release the call worker.', 'tests/adversarial/c14-r2-mcp.py::test_cancel_live_elicitation_releases_the_call_worker'),
    }
    if n in new:
        note, test = new[n]
        r.update(status='divergent', note=note, test=test)
for d in declarations:
    if d['id'] == 'agent.policy.defaults.set':
        d.update(status='implemented', note='Defaults rehydrated; downgrade/restart and corrupt-file variants checked.', test='crates/mcp/tests/c14_r1_limits.rs; crates/executor/tests/c14_r2_contracts.rs')
    if d['id'] == 'agent.activity.export':
        d.update(note='C14-R2-01/02/03: staged rollback fixed, but publication failures and replay still diverge.', test='tests/adversarial/c14-r2-mcp.py; crates/mcp/tests/c14_r2_replay.rs')
'''
original = original.replace('counts = dict(collections.Counter', delta + '\ncounts = dict(collections.Counter', 1)
original = original.replace('round 1 conformance register', 'round 2 conformance register')
original = original.replace('Baseline: `3906001`', 'Product: `74fe3a9`')
original = original.replace('Remaining event producer gap', 'Event producer closure')
original = original.replace('`doc.undone` / `doc.redone` are specified but have no producer; undo/redo emit `doc.changed`. This is a carried kernel event-contract gap (P2 ticket C14-R1-16), distinct from the now-working subscribed doc.changed path. State/playback/UI event names are tracked under their implementing domain milestones.', '`doc.undone` / `doc.redone` now carry undoStepId and label; round-one protocol test passes. Other domain events retain their declared milestone owners. Ticketed effectiveParams and worked examples remain explicit deferrals.')
exec(compile(original, str(HERE / 'register.py'), 'exec'), {'__file__': str(HERE / 'register.py')})
