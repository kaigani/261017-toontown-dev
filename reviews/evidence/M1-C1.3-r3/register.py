"""Retain all inherited row identities, update whole-row findings and executable proof."""
from pathlib import Path
import collections
import json

OUT=Path(__file__).resolve().parent
OLD=OUT.parent/'M1-C1.3-r2'
def read(name): return json.loads((OLD/name).read_text(encoding='utf-8-sig'))
requirements=read('requirements.json')
commands=read('command-register.json')
supplement=read('supplemental-requirements.json')
proof={
 'FMT-01':'c13_r2_format::overlong_component_is_not_a_portable_folder_entry; c13_r3_format::two_hundred_byte_unicode_components_work_in_windows_folder_form; round_trip',
 'MED-04':'c13_r2_persistence::save_as_package_retains_removed_asset_bytes_for_undo; c13_r2_persistence::save_as_folder_retains_removed_asset_bytes_for_undo; c13_r3_persistence::media_survives_cross_form_save_as_and_recovery_deletion_with_two_undos; c13_r1_persistence::unreferenced_embedded_media_is_removed_from_the_saved_package',
 'SAVE-04':'c13_projects::an_atomic_batch_that_saves_commits_one_step_and_writes_that_revision; c13_r2_persistence::successful_atomic_close_with_save_commits_and_emits_the_saved_revision; c13_r3_persistence::failed_and_dry_atomic_save_variants_preserve_disk_revision_and_events',
 'ExecutorEffects':'c13_r2_persistence::clean_project_atomic_close_saves_staged_edits; c13_r2_persistence::clean_project_atomic_quit_saves_staged_edits; c13_r3_persistence::failed_and_dry_atomic_save_variants_preserve_disk_revision_and_events',
 'Quit':'c13_r2_persistence::clean_project_atomic_quit_saves_staged_edits; c13_r2_persistence::failed_atomic_quit_save_leaves_disk_and_revision_unchanged',
 'RO-02':'c13_r2_persistence::folder_hidden_preserved_entry_change_blocks_save; c13_r3_persistence::folder_fingerprint_includes_nested_hidden_names_but_excludes_root_controls_and_session; c13_r2_persistence::folder_visible_mutations_are_detected_without_waiting_for_maintenance_and_own_save_is_clean',
 'Interchange':'c13_r2_persistence::asset_name_conflict_replace_keeps_one_asset_and_undo_restores_original; c13_doc::replacing_a_used_asset_repoints_its_placements_and_undo_restores_them; c13_r3_persistence::asset_replace_preserves_unrelated_strings_equal_to_the_old_id (FAIL)',
 'LOCK-01':'c13_r1_persistence::concurrent_stale_lock_takeover_has_only_one_owner; c13_r3_format::heartbeat_refuses_an_unreadable_replacement_lock (FAIL)',
 'LOCK-02':'c13_r2_persistence::thirty_two_concurrent_lock_acquisitions_keep_exactly_one_owner; c13_r3_format::a_live_writer_paused_beyond_two_seconds_is_not_taken_over (FAIL)',
}
divergent={'REP-01':'SI-044 awaits the orchestrator/spec-owner ruling; no authority inferred from the builder decision.',
 'Interchange':'C13-R3-02: replacement changes unrelated display strings equal to the old ID.',
 'LOCK-01':'C13-R3-01: old heartbeat publishes over an unreadable replacement.',
 'LOCK-02':'C13-R3-01: a live writer paused beyond two seconds is taken over.'}
for row in requirements:
 if row['id'] in proof:
  row['tests']=proof[row['id']]
  row['note']='Round-2 finding closed by the cited tests; scope and limitations in round-3 report.'
 if row['status']=='divergent' and row['id']!='REP-01': row['status']='implemented'
 if row['id'] in divergent:
  row['status']='divergent';row['note']=divergent[row['id']]
 if row['id']=='MED-04': row['note']='Undo bytes retained across form changes and AUTO-04 deletion. Separate P2 export-cache ticket C13-R3-03.'
 if row['id']=='RO-02': row['note']='Kernel implementation; SI-045 confirmation result remains C1.4 obligation under standing ruling.'
 if row['id']=='LoadPhase-7': row['note']='M2 C2.2 zones; M3 C3.3 renderer-derived thumbnails/caches; M8 C8.1 waveform caches. Existing eager scene lengths remain implemented.'
for row in supplement:
 row['status']='implemented'
 if row['id']=='SAVE-02':
  row['proof']='c13_r2_format::actual_folder_writer_keeps_manifest_after_removal_renames; journal::a_save_interrupted_after_any_rename_recovers_to_one_whole_version; c13_r1_format::partial_folder_rollback_restores_every_already_replaced_entry'
  row['note']='Changing manifest, removal, and post-final-rename journal boundary pass.'
 if row['id']=='MED-01': row['note']='Shared content and undo retained; separate resource-growth P2 C13-R3-03.'
for row in commands:
 row['tests']=row['tests'].replace('(FAIL)','(original reproducer now passes)')
 row['status']='divergent' if row['id']=='doc.import.json' else 'implemented'
 row['findings']='C13-R3-02' if row['id']=='doc.import.json' else ''
 if row['id']=='doc.import.json': row['tests']+='; c13_r3_persistence::asset_replace_preserves_unrelated_strings_equal_to_the_old_id (FAIL)'
 if row['id']=='doc.package.export': row['tests']+='; P2 C13-R3-03 repeated-export cache growth; serialization passes'
counts={'requirements':dict(collections.Counter(r['status'] for r in requirements)),
        'supplement':dict(collections.Counter(r['status'] for r in supplement)),
        'combined':dict(collections.Counter(r['status'] for r in requirements+supplement)),
        'declarations':dict(collections.Counter(r['status'] for r in commands))}
assert len(requirements)==119 and len(supplement)==4 and len(commands)==48
assert all(r['tests'].strip() for r in requirements if r['status']=='implemented')
assert all(r['tests'].strip() for r in commands)
for name,data in [('requirements.json',requirements),('supplemental-requirements.json',supplement),('command-register.json',commands),('register-counts.json',counts)]:
 (OUT/name).write_text(json.dumps(data,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
lines=['# C1.3 final register — independent round 3','',
 'Reviewed product commit `4333ff1`. Whole-row status; 119 inherited identities, four parser-supplement identities, and 48 verbatim declaration rows retained. All implemented rows cite executable tests. Inherited source coordinates are navigation hints from round 2; the report supplies current defect locations. Status is regression-based conformance, not certification of every possible input.','',
 '`'+json.dumps(counts)+'`','',
 'Order: implemented / divergent / missing / deferred-with-owner. **119: 101 / 4 / 0 / 14; 123: 105 / 4 / 0 / 14; declarations: 47 / 1 / 0 / 0.** Builder expectation is corrected for LOCK-01, LOCK-02 and Interchange; REP-01 still awaits SI-044. No deferred row was promoted. P2 cache growth is recorded separately from serialization conformance. SAVE-01 retains the standing Windows write-through deviation; RO-02 retains the standing C1.4 confirmation handoff.','',
 '## Requirement rows','','| ID / requirement | Status | Implementation | Test / ruling |','|---|---|---|---|']
for r in requirements:
 lines.append('| '+r['id']+' — '+r['requirement'].replace('|','\\|')+' | '+r['status']+' | '+r['implementation']+' | '+(r['tests']+'; '+r['note']).replace('|','\\|')+' |')
lines+=['','## Supplemental rows','','| ID | Status | Test / ruling |','|---|---|---|']
for r in supplement:lines.append('| '+r['id']+' | '+r['status']+' | '+r['proof']+'; '+r['note']+' |')
lines+=['','## Declarations','','| ID | Status | Tests / findings |','|---|---|---|']
for r in commands:lines.append('| `'+r['id']+'` | '+r['status']+' | '+(r['tests']+'; '+r['findings']).replace('|','\\|')+' |')
lines+=['','## Deferred ownership validation','',
 'Every deferred row names an owner present in plan §4: LOCK-03 / library lock repair → M9 C9.1; zone/colour repairs and load derivation → M2 C2.2; font repair/lint → M2 C2.1; derived caches/thumbnails → M3 C3.3 and M8 C8.1; shell settings → M4 C4.1, startup content → M9 C9.3, host wiring → M1 C1.4; lip-sync lint → M8 C8.2; transition lint → M9 C9.2; template lint → M9 C9.1. These owners are listed, not newly deferred by this review.','']
(OUT/'conformance.md').write_text('\n'.join(lines),encoding='utf-8')
print(counts)
