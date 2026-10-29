from pathlib import Path
import json,collections,re,subprocess,difflib
out=Path('reviews/evidence/M1-C1.3-r2'); old=Path('reviews/evidence/M1-C1.3-r1')
r=json.loads((old/'requirements.json').read_text(encoding='utf-8-sig'));c=json.loads((old/'command-register.json').read_text(encoding='utf-8-sig'));runtime={x['id']:x for x in json.loads((out/'persistence-registry.json').read_text(encoding='utf-8-sig'))}
proof={
'SAVE-04':'c13_r2_persistence::clean_project_atomic_close_saves_staged_edits (FAIL); ::successful_atomic_close_with_save_commits_and_emits_the_saved_revision (FAIL)',
'SAVE-02':'c13_r1_format::partial_folder_rollback_restores_every_already_replaced_entry; journal (3 pass); c13_r2_format::actual_folder_writer_keeps_manifest_after_removal_renames (FAIL)',
'FMT-01':'c13_r1_format::folder_has_the_same_mimetype_entry_as_package; round_trip; c13_r2_format::overlong_component_is_not_a_portable_folder_entry (FAIL)',
'ZIP-01':'c13_r1_format::zip_names_reject_drive_prefixes_in_every_path_component; c13_r2_format::portable_entry_names_reject_windows_special_forms',
'VER-02':'c13_r1_format::newer_minor_unknown_properties_survive_on_the_same_frozen_entry; c13_r2_format::frozen_entry_nested_unknown_data_survives_other_entry_edit_both_forms_and_reopen',
'MED-04':'c13_r1_persistence::unreferenced_embedded_media_is_removed_from_the_saved_package; c13_r2_persistence::save_as_package_retains_removed_asset_bytes_for_undo (FAIL); ::save_as_folder_retains_removed_asset_bytes_for_undo (FAIL)',
'REP-01':'c13_doc::a_repaired_load_is_one_non_undoable_step_and_reported; c13_r2_format::document_revision_boundaries_with_and_without_repairs',
'REC-01':'c13_r1_persistence::recovering_an_untitled_copy_claims_it_before_a_second_session_can_recover; c13_r2_format::recovery_claim_expires_when_actual_child_owner_exits_and_release_restores_metadata',
'RO-02':'c13_r1_persistence::folder_entry_change_is_detected_even_when_manifest_is_unchanged; c13_r2_persistence::folder_visible_mutations_are_detected_without_waiting_for_maintenance_and_own_save_is_clean; ::folder_hidden_preserved_entry_change_blocks_save (FAIL)',
'LOCK-02':'c13_r1_persistence::concurrent_stale_lock_takeover_has_only_one_owner; ::save_as_current_path_does_not_overwrite_another_sessions_lock; c13_r2_persistence::thirty_two_concurrent_lock_acquisitions_keep_exactly_one_owner; c13_projects::a_session_whose_lock_was_taken_over_cannot_save_over_the_new_owner',
'Interchange':'c13_r1_persistence::import_replace_replaces_a_same_named_scene_instead_of_renaming_it; c13_doc::import_replace_replaces_elements_of_the_target_scene_and_palettes_by_name; c13_r2_persistence::asset_name_conflict_replace_keeps_one_asset_and_undo_restores_original (FAIL)',
'Thumbnail':'c13_r1_format::existing_project_thumbnail_survives_save',
'Repair-19':'c13_r2_format::frozen_entry_nested_unknown_data_survives_other_entry_edit_both_forms_and_reopen',
'ExecutorEffects':'c13_r1_persistence::dry_run_close_with_save_does_not_write_the_project_or_backups; ::failed_atomic_batch_after_save_does_not_leak_disk_or_clean_marker_changes; c13_r2_persistence::dry_run_persistence_paths_do_not_change_disk_state_or_clean_marker; ::clean_project_atomic_close_saves_staged_edits (FAIL); ::successful_atomic_close_with_save_commits_and_emits_the_saved_revision (FAIL)',
'RoundTrip':'c13_r1_format::package_and_folder_round_trip_all_fixture_entries_and_binary_bytes; c13_r2_format::frozen_entry_nested_unknown_data_survives_other_entry_edit_both_forms_and_reopen',
'InspectionBound':'c13_r1_persistence::package_inspection_reads_a_bounded_header; c13_r2_persistence::gigabyte_package_inspection_is_bounded_with_and_without_validation; inspection-memory.json',
'UndoLabels':'c13_r1_persistence::import_and_repair_undo_labels_match_the_command_table',
'Preferences':'c13_r1_persistence::default_preference_values_are_not_persisted_as_overrides',
'Quit':'c13_projects::unsaved_changes_policies_and_quit; c13_r2_persistence::failed_atomic_quit_save_leaves_disk_and_revision_unchanged; ::clean_project_atomic_quit_saves_staged_edits (FAIL)'}
div={'SAVE-04':'C13-R2-01: the atomic-batch save snapshots uncommitted staged work; clean live state bypasses the save.', 'SAVE-02':'C13-R2-06: manifest rename precedes removal renames; recovery whole-version component fixed.', 'FMT-01':'C13-R2-05: package permits a component the folder form cannot represent (P2).','MED-04':'C13-R2-02: omitted base media lost from live undo after Save As.','REP-01':'SI-044: literal undoable-repair requirement still awaits spec-owner ruling; MAX revision component fixed.','RO-02':'C13-R2-04: preserved nested hidden entries are skipped; SI-045 confirmation remains C1.4.','Interchange':'C13-R2-03: asset name replacement still appends.','ExecutorEffects':'C13-R2-01: staged close/quit loses edits or drops sealed commit.','Quit':'C13-R2-01; native quit remains M4/C1.4.'}
cache={}
def current_loc(s):
    m=re.fullmatch(r'(.+):(\d+)',s)
    if not m or not Path(m[1]).is_file():return s
    file,line=m[1],int(m[2])-1
    if file not in cache:
        before=subprocess.check_output(['git','show','60b8ad6:'+file],encoding='utf-8').splitlines();after=Path(file).read_text(encoding='utf-8-sig').splitlines(); cache[file]=difflib.SequenceMatcher(None,before,after).get_opcodes()
    for tag,a,b,x,y in cache[file]:
        if a<=line<b:return f'{file}:{x+min(line-a,max(0,y-x-1))+1}'
    return s
for row in r:
    row['implementation']=current_loc(row['implementation'])
    if row['id'] in proof:
        row['status']='divergent' if row['id'] in div else 'implemented';row['tests']=proof[row['id']];row['note']=div.get(row['id'],'Round-1 divergence closed by the cited passing tests; scope as in round-2 report.')
cmddiv={'project.create':'C13-R2-06','doc.package.export':'C13-R2-06','app.quit':'C13-R2-01','project.close':'C13-R2-01','project.saveAs':'C13-R2-02','project.save':'C13-R2-04','doc.import.json':'C13-R2-03'}
for row in c:
    prev=row['findings']; row['implementation']=current_loc(row['implementation']);row['status']='divergent' if row['id'] in cmddiv else 'implemented';row['findings']=cmddiv.get(row['id'],'')
    rt=runtime[row['id']];row['actualScopes']=rt['scopes'];row['actualEnablement']=rt['enablement'];row['actualUndoLabel']=rt['undoLabel']
    if prev:row['tests']+='; round-2 report closure table for '+prev
    if row['id'] in cmddiv:row['tests']+='; round-2 finding '+cmddiv[row['id']]+' permanent regression'
counts={'requirements':dict(collections.Counter(x['status'] for x in r)),'declarations':dict(collections.Counter(x['status'] for x in c)),'requirementTotal':len(r),'declarationTotal':len(c)}
supplement=[
{'id':'MED-01','status':'implemented','spec':'10 section 6.6','proof':'c13_r1_persistence::identical_embedded_asset_content_is_stored_once; c13_doc::linked_media_relink_embed_blob_export_and_remove','note':'Undo after container switching is separately divergent in MED-04 / C13-R2-02.'},
{'id':'MED-02','status':'implemented','spec':'10 section 6.6','proof':'c13_doc::linked_media_relink_embed_blob_export_and_remove','note':'Linked original resolution and missing media handling; specialty relink UX remains deferred as before.'},
{'id':'SAVE-01','status':'implemented','spec':'10 section 8.1','proof':'c13_r1_persistence::failed_package_export_overwrite_keeps_the_previous_destination; round_trip','note':'Subject to accepted ADR-0014 section 3 Windows write-through deviation; no new durability claim.'},
{'id':'SAVE-02','status':'divergent','spec':'10 section 8.1','proof':proof['SAVE-02'],'note':div['SAVE-02']},
]
counts['supplementalNamedRequirements']={'implemented':3,'divergent':1,'missing':0,'deferred-with-owner':0,'total':4}
(out/'supplemental-requirements.json').write_text(json.dumps(supplement,indent=2)+'\n',encoding='utf-8')
for n,v in [('requirements.json',r),('command-register.json',c),('register-counts.json',counts)]: (out/n).write_text(json.dumps(v,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
lines=['# C1.3 round-2 conformance register','', 'Reviewed commit `7a5f866`. Status covers each whole inherited row. The 119 requirement identities and 48 verbatim specification declaration rows are retained; source positions are mapped to this commit. Runtime schemas/scopes/enablement/examples/undo labels were independently recaptured by the unchanged reviewer registry test. No deferred ownership was promoted.','', 'Counts: `'+json.dumps(counts)+'`','', 'Order: implemented / divergent / missing / deferred-with-owner. Requirements **97 / 8 / 0 / 14**; declarations **41 / 7 / 0 / 0**. SAVE-01 retains the previously accepted Windows durability deviation (ADR-0014 §3); it is not silently claimed as write-through verification. SI-044 remains divergent.','', 'Every formerly divergent requirement below names executable proof, including failing proof when the row stays divergent. Unchanged inherited rows retain their inherited evidence and are covered by the workspace sweep. This is a regression-based conformance register, not exhaustive input-space certification.','', '## Requirements','', '| ID / requirement | Status | Implementation | Tests / ruling |','|---|---|---|---|']
for x in r:lines.append('| '+x['id']+' — '+x['requirement'].replace('|','\\|')+' | '+x['status']+' | `'+x['implementation']+'` | '+(x['tests']+'; '+x['note']).replace('|','\\|')+' |')
lines+=['','## Four inherited parser omissions','','The original parser skipped MED-01, MED-02, SAVE-01 and SAVE-02 because their bold headings include a descriptive suffix. They are now explicit supplemental rows; the original 119 row identities stay comparable. Together: **123 requirements, 100 implemented / 9 divergent / 0 missing / 14 deferred-with-owner**. These are accounting corrections, not new specification requirements.','','| ID | Status | Proof / ruling |','|---|---|---|']
for x in supplement: lines.append('| '+x['id']+' | '+x['status']+' | '+x['proof']+'; '+x['note']+' |')
lines+=['','## Declarations','','| ID / kind | Status | Implementation | Scope / undo | Tests / findings |','|---|---|---|---|---|']
for x in c:lines.append('| `'+x['id']+'` / '+x['kind']+' | '+x['status']+' | `'+x['implementation']+'` | '+', '.join(x['actualScopes'])+' / '+str(x['actualUndoLabel'])+' | '+(x['tests']+'; '+x['findings']).replace('|','\\|')+' |')
(out/'conformance.md').write_text('\n'.join(lines)+'\n',encoding='utf-8');print(counts)
