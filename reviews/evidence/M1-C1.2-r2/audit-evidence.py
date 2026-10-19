"""Verify exact receipts, coverage and finding/test mapping; seal reproducible review artifacts.

Use --check after checkout to verify the sealed bytes without updating the manifest.
"""
import gzip,hashlib,json,re,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def read(p):return json.loads(p.read_text(encoding='utf-8'))
manifest=HERE/'SHA256SUMS.json'
if '--check' in sys.argv:
    data=read(manifest)
    for path,digest in data['files'].items():assert sha(ROOT/path)==digest,path
    print('Verified',len(data['files']),'sealed files.')
    sys.exit(0)
for p in (HERE/'receipts').glob('*.json'):
    r=read(p);assert sha(HERE/r['log'])==r['sha256'],p
for folder in ['baseline','final','final-complete','final-verification']:
    receipt=read(HERE/folder/'receipt.json')
    for name,digest in receipt['sha256'].items():assert sha(HERE/folder/name)==digest,(folder,name)
for r in read(HERE/'ci-summary.json'):
    assert hashlib.sha256(gzip.decompress((HERE/f"ci-{r['run']}.log.gz").read_bytes())).hexdigest()==r['rawLogSha256']
registry=read(HERE/'command-register.json');requirements=read(HERE/'requirements.json')
assert len(registry)==len({r['id'] for r in registry})==796
assert len(requirements)==len({r['id'] for r in requirements})==72
assert all(r['source'] and r['tests'] and r['examples'] for r in registry)
assert all(r['implementation'] and r['tests'] for r in requirements)
assert len(read(HERE/'sample-60-audit.json')['rows'])==60
assert len(read(HERE/'spot-40-audit.json')['rows'])==40
assert all(r['valid'] for r in read(HERE/'example-validation.json'))
assert len(read(HERE/'example-validation.json'))==809
tests={
 '01':['safe_revision_reconstruction_cannot_install_future_object_revisions'],
 '02':['generated_result_rejects_wrong_enabled_type','generated_nested_parameters_reject_wrong_handle_type','generated_named_result_is_not_an_unconstrained_object','library_global_write_scope_is_declared','library_close_has_no_project_rule_from_01_1_3','result_schema_catalogue_does_not_accept_null_for_device_record'],
 '03':['checkpoint_is_visible_to_later_atomic_query','checkpoint_create_then_delete_in_atomic_batch_succeeds','checkpoint_before_edit_predicts_same_result_in_dry_run','checkpoint_between_edits_captures_its_staged_content','sequential_dry_run_simulates_checkpoints'],
 '04':['paste_dry_run_predicts_created_ids_exactly','identical_document_and_paste_sequence_produce_identical_ids'],
 '05':['element_delete_preserves_opaque_metadata_and_extension_strings','drawing_copy_preserves_object_shaped_opaque_metadata'],
 '06':['pending_job_wait_reserves_request_id_against_other_commands'],
 '07':['sequential_batch_job_wait_honours_wait_timeout_contract'],
 '08':['edit_envelopes_report_explicit_or_selection_targets'],
 '09':['malformed_paste_destination_is_rejected_without_fallback'],
 '10':['clipboard_element_properties_validates_values_not_only_names'],
 '11':['json04_materialised_parent_is_removed_by_raw_patch_inverse','json04_materialisation_does_not_invent_opaque_nested_containers'],
 '12':['diff_reports_drawing_content_revision_invalidation','hundred_thousand_stroke_change_reports_drawing_in_envelope_event_and_diff'],
 '13':['copying_a_peg_includes_descendants_per_scene_edit_rules','cutting_a_peg_removes_descendants_per_scene_edit_rules','copying_complete_scene_forest_preserves_effect_parentage_on_paste'],
 '14':['cross_bank_cell_paste_cannot_overwrite_a_locked_drawing'],
}
logs='\n'.join(p.read_text(encoding='utf-8') for p in [HERE/'logs/final-contracts.log',HERE/'logs/final-review-targets.log',HERE/'logs/scale-r2.log'])
assert sum(map(len,tests.values()))==29
for names in tests.values():
    for name in names:assert f'test {name} ... FAILED' in logs,name
(HERE/'finding-test-map.json').write_text(json.dumps({'C12-R2-'+k:v for k,v in tests.items()},indent=2)+'\n',encoding='utf-8')
paths=[p for p in HERE.rglob('*') if p.is_file() and p!=manifest]
paths += [ROOT/'reviews/M1-C1.2-r2.md',*sorted((ROOT/'crates/executor/tests').glob('c12_r2_*.rs'))]
data={'algorithm':'sha256','builderCommit':'237c8f34286921d4db5cb8d06c2aeb6392368418',
      'files':{p.relative_to(ROOT).as_posix():sha(p) for p in sorted(paths)}}
manifest.write_text(json.dumps(data,indent=2)+'\n',encoding='utf-8')
print('Verified receipts and all 29 failing regression mappings; sealed',len(paths),'files.')
