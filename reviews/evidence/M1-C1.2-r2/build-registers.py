"""Independent R2 adjudications; no imports from the product's declaration generator.

The register records positive evidence and remaining counterexamples separately. A typed
declaration or a schema-valid example alone is not proof that the schema matches the spec.
"""
from collections import Counter
import json,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
def read(p):return (ROOT/p).read_text(encoding='utf-8')
def source(p,needle):
    return f"{p}:{next(i for i,l in enumerate(read(p).splitlines(),1) if needle in l)}"
def write(name,value):(HERE/name).write_text(json.dumps(value,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
spec={};namespace=''
for n,line in enumerate(read('spec/A_command_registry.md').splitlines(),1):
    m=re.match(r'## Namespace `([^`]+)`',line)
    if m:namespace=m[1]
    if re.match(r'\| `[^`]+` \| (action|toggle|query|job|ui|event) \|',line):
        columns=[s.strip().replace(r'\|','|') for s in re.split(r'(?<!\\)\|',line)[1:-1]]
        assert len(columns)==6
        cid=columns[0].strip('`')
        spec[cid]=dict(zip(['id','kind','summary','params','result','undo'],columns))|{'line':n,'namespace':namespace}
rows=json.loads((HERE/'registry.json').read_text(encoding='utf-8'))
assert len(rows)==len(spec)==796 and set(spec)=={r['id'] for r in rows}
holes={r['id']:r for r in json.loads((HERE/'schema-holes.json').read_text(encoding='utf-8'))}
prior={r['id']:r for r in json.loads((HERE.parent/'M1-C1.2-r1/command-register.json').read_text(encoding='utf-8'))}
declarations={}
for p in (ROOT/'crates/executor/src/commands').glob('*.rs'):
    for i,line in enumerate(p.read_text(encoding='utf-8').splitlines(),1):
        m=re.search(r'id = "([^"]+)"',line)
        if m:declarations[m[1]]=f'{p.relative_to(ROOT).as_posix()}:{i}'
generated={}
for i,line in enumerate(read('crates/registry/src/declared.rs').splitlines(),1):
    m=re.search(r'id: "([^"]+)"',line)
    if m:generated[m[1]]=f'crates/registry/src/declared.rs:{i}'
assert len(generated)==662
examples=json.loads((HERE/'examples.json').read_text(encoding='utf-8'))
example_counts=Counter(x['id'] for x in examples)
defects={
 'checkpoint.create':['03'], 'checkpoint.list':['03'], 'checkpoint.delete':['03'],
 'doc.diff':['12'],'doc.object.patch':['11'],'job.wait':['06','07'],'edit.clipboard.set':['10'],
 'edit.copy':['05','08','13'],'edit.cut':['05','08','13'],'edit.delete':['05','08'],
 'edit.cutSpecial':['05','08'], 'edit.paste':['04','05','08','09','13','14'],
 'edit.pasteSpecial':['04','05','08','09','13','14'],'edit.pasteAsNewElement':['04','08'],
}
bare_results={'draw.drawing.get','element.get','lipsync.settings.set','sound.editor.state.set','timeline.rows.list','xsheet.exposures.get'}
global_writes={'library.item.delete','library.item.duplicate','library.item.move'}
no_project={'library.open','library.close','library.create','library.list','library.tree.get'}
register=[]
for r in rows:
    cid=r['id'];s=spec[cid]
    assert r['kind']==s['kind'] and cid.split('.')[0]==s['namespace']
    assert r['spec']=={k:s[k] for k in ['params','result','undo']}
    issues=[]
    if cid in generated:
        for area in ['params','result']:
            for h in holes[cid][area]:
                issues.append({'field':area+h['pointer'],'reason':'unconstrained field/prose-only schema or unresolved named type','schema':h['schema']})
    if cid in bare_results:issues.append({'field':'resultSchema','reason':'whole named result is only type:object; owning section declares fields'})
    if cid in global_writes and 'library.global' not in r['scopes']:
        issues.append({'field':'scopes','reason':'global-library writes lack library.global (11 section 6.1)'})
    if cid in no_project and not r['needsProject']:
        issues.append({'field':'needsProject','reason':'01 section 1.3 only exempts saving the global library'})
    if cid=='xsheet.columnWidth.set':
        issues.append({'field':'resultSchema/properties/widths','reason':'column-width map has no value constraints (04 column widths 20..1000)'})
    findings=['C12-R2-'+x for x in defects.get(cid,[])]
    if issues:findings.insert(0,'C12-R2-02')
    status='divergent' if findings else 'implemented' if r['status']=='implemented' else 'deferred-with-owner'
    own=[]
    for p in sorted((ROOT/'spec').glob(r['section'][:2]+'_*.md')):
        own.extend(f'{p.relative_to(ROOT).as_posix()}:{n}' for n,l in enumerate(p.read_text(encoding='utf-8').splitlines(),1) if f'`{cid}`' in l)
    register.append({
        'id':cid,'kind':r['kind'],'status':status,'priorStatus':prior[cid]['status'],
        'builderClaim':'typed/fixed; handler '+r['status'],'declaredStatus':r['status'],
        'owner':r.get('milestone','M1 C1.2'),'spec':f"spec/A_command_registry.md:{s['line']}",
        'owningSectionSources':own,'source':declarations.get(cid,generated.get(cid)),
        'paramsSchema':r['paramsSchema'],'resultSchema':r['resultSchema'],'schemaIssues':issues,
        'undoable':r['undoable'],'undoLabel':r['undoLabel'],'specUndo':s['undo'],
        'enablement':r['enablement'],'needsProject':r['needsProject'],'scopes':r['scopes'],
        'examples':r['examples'],'examplesExecuted':example_counts[cid],
        'guiLocations':r['guiLocations'],'agentOnly':r['agentOnly'],
        'tests':['c12_r1_stress::every_executable_typed_example_also_validates_its_result_schema',
                 'c12_r2_registry::independently_validate_every_declared_parameter_example',
                 'c12_registry::ui_binding_manifest_parity'],
        'findings':findings,
        'qualification':'No additional per-entry defect found' if not findings else 'See findings and schemaIssues; future handler ownership does not defer current declaration requirements',
    })
write('command-register.json',register)

requirements=json.loads((HERE.parent/'M1-C1.2-r1/requirements.json').read_text(encoding='utf-8'))
# Every location is resolved against the reviewed source, not copied from stale R1 line numbers.
locations='''
R01|crates/registry/src/inventory.rs|pub static INVENTORY
R02|scripts/gen-command-declarations.py|def parse_fields
R03|crates/registry/tests/declared.rs|fn schemas_compile_reject_unknown_parameters_and_accept_the_examples
R04|crates/registry/src/owners.rs|pub fn owner_of
R05|crates/registry/src/lib.rs|pub const REGISTRY_VERSION
R06|crates/registry-macros/src/lib.rs|example
R07|crates/executor/src/run.rs|pub(crate) fn load
R08|crates/executor/src/lib.rs|fn document_thread
R09|crates/model/src/document/mod.rs|next.rebuild_derived();
R10|crates/executor/src/run.rs|struct Rollback
R11|crates/executor/src/run.rs|fn batch_atomic
R12|crates/executor/src/commands/checkpoint.rs|fn checkpoint_list
R13|crates/executor/src/run.rs|fn resolve_refs
R14|crates/executor/src/run.rs|Kind::Job
R15|crates/executor/src/commands/edit_ops.rs|fn new_id
R16|crates/executor/src/run.rs|fn apply_effects
R17|crates/executor/src/run.rs|preview = param_dry_run
R18|crates/executor/src/run.rs|fn simulate
R19|crates/executor/src/run.rs|fn wait_begin
R20|crates/executor/src/cx.rs|fn restore_snapshot
R21|crates/executor/src/run.rs|fn commit
R22|crates/executor/src/commands/checkpoint.rs|fn checkpoint_create
R23|crates/executor/src/run.rs|fn apply_effects
R24|crates/model/src/document/mod.rs|pub struct Document
R25|crates/executor/src/run.rs|fn check_expect_rev
R26|crates/executor/src/commands/doc.rs|expect_rev
R27|crates/executor/src/run.rs|"currentRev": current
R28|crates/executor/src/run.rs|fn wait_begin
R29|crates/executor/src/run.rs|fn fingerprint
R30|crates/executor/src/run.rs|struct RequestCache
R31|crates/executor/src/cx.rs|fn next_rev
R32|crates/executor/src/cx.rs|current.checked_add(1)
R33|crates/model/src/document/stamp.rs|pub fn with_revisions
R34|crates/executor/src/commands/doc.rs|for (c, o) in change::diff
R35|crates/executor/src/commands/project.rs|id = "project.save"
R36|crates/model/src/change.rs|fn materialize_omitted
R37|crates/executor/src/commands/doc.rs|fn doc_at
R38|crates/executor/src/session.rs|pub fn log_commit
R39|crates/executor/src/run.rs|fn cap
R40|crates/executor/src/cx.rs|pub fn resolve
R41|crates/executor/src/commands/derived.rs|pub(crate) fn
R42|crates/executor/src/commands/derived.rs|doc.derivedUnavailable
R43|crates/executor/src/commands/doc.rs|x-command
R44|scripts/gen-command-declarations.py|NO_PROJECT =
R45|crates/executor/src/commands/app.rs|fn app_quit
R46|crates/executor/src/commands/window.rs|fn window_full_screen_set
R47|crates/executor/src/commands/app.rs|fn app_mode_set
R48|crates/executor/src/ui.rs|pub fn select
R49|crates/executor/src/ui.rs|pub fn panels
R50|crates/executor/src/commands/edit_ops.rs|fn edit_copy
R51|crates/executor/src/commands/edit.rs|fn validate_clipboard
R52|crates/executor/src/commands/edit.rs|fn edit_clipboard_get
R53|crates/executor/tests/c12_registry.rs|fn ui_binding_manifest_parity
R54|crates/executor/tests/c12_registry.rs|fn ui_binding_manifest_parity
R55|desktop/src-tauri/src/menu.rs|pub fn
R56|crates/executor/src/lib.rs|fn call
R57|crates/executor/src/lib.rs|fn call
R58|crates/executor/tests/c12_never_block.rs|fn a_handler_that_does_not_return
R59|crates/executor/src/run.rs|fn missing_scopes
R60|crates/executor/src/commands/job.rs|fn visible
R61|scripts/gen-command-declarations.py|def scopes_of
R62|crates/executor/src/commands/agent.rs|id = "agent.server.set"
R63|crates/executor/src/commands/job.rs|fn job_wait
R64|Cargo.toml|unwrap_used
R65|desktop/eslint.config.js|ipc
R66|desktop/src/ipc/commands.test.ts|describe(
R67|crates/executor/src/cx.rs|fn stage
R68|crates/model/src/document/paths.rs|struct PathMemo
R69|Cargo.lock|version =
R70|.github/workflows/ci.yml|cargo test --workspace
R71|crates/executor/tests/c12_r2_contracts.rs|fn eight_fresh_clients
R72|crates/model/src/document/mod.rs|pub fn from_parts
'''
impl={}
for line in locations.strip().splitlines():
    ident,p,needle=line.split('|',2)
    try:impl[ident]=source(p,needle)
    except StopIteration:raise ValueError((ident,p,needle))
divergent={
 'R02':('02','c12_r2_registry','597 generated entries retain schema holes; named-result shells, scopes and project rules also incomplete.'),
 'R07':('01','safe_revision_reconstruction_cannot_install_future_object_revisions','Load capability and dirty-project policy fixed, but public revision reconstruction + load installs rev>documentRev.'),
 'R12':('03','checkpoint_is_visible_to_later_atomic_query','Later steps cannot read pending checkpoint creation.'),
 'R15':('04','paste_dry_run_predicts_created_ids_exactly','Ordinary metadata previews fixed; ID-creating edits predict different IDs.'),
 'R18':('03','sequential_dry_run_simulates_checkpoints','Simulation suppresses checkpoint creation, so later query result differs.'),
 'R19':('06,07','pending_job_wait_reserves_request_id_against_other_commands; sequential_batch_job_wait_honours_wait_timeout_contract','Two-phase wait lacks in-flight reservation; batched wait uses polling-only handler.'),
 'R22':('03','checkpoint_create_then_delete_in_atomic_batch_succeeds','Standalone lifecycle works; create/delete composition fails.'),
 'R23':('03','checkpoint_between_edits_captures_its_staged_content','Checkpoint stores final batch content, not its staged point; before-edit prediction has wrong revision.'),
 'R28':('06','pending_job_wait_reserves_request_id_against_other_commands','Eviction and session isolation fixed; pending wait has no request-cache entry.'),
 'R29':('06','pending_job_wait_reserves_request_id_against_other_commands','Completed collisions conform to R1-22; overlapping different command still executes.'),
 'R34':('12','hundred_thousand_stroke_change_reports_drawing_in_envelope_event_and_diff','Envelope and event now include drawing rev; doc.diff still drops drawing invalidation.'),
 'R36':('11','json04_materialised_parent_is_removed_by_raw_patch_inverse','JSON-04 synthesised parent is absent from inverse; opaque nested meta/ext containers invented.'),
 'R37':('12','diff_reports_drawing_content_revision_invalidation','Historical reconstruction fixed; semantic diff omits drawing revision-only change.'),
 'R44':('02','library_close_has_no_project_rule_from_01_1_3','All edit.* now gated; generated library declarations still exempt disallowed no-project commands.'),
 'R50':('04,05,08,09,13,14','c12_r2_contracts','Seven handlers exist; hierarchy, locks, opaque metadata, targets and deterministic prediction remain incorrect.'),
 'R51':('10','clipboard_element_properties_validates_values_not_only_names','elementProperties checks allowed field names, but accepts noninteger columnWidth.'),
 'R61':('02','library_global_write_scope_is_declared','File/network examples improved; global-library item mutations still lack library.global.'),
 'R63':('07','sequential_batch_job_wait_honours_wait_timeout_contract','Owned job table and event ordering work; sequential batch wait ignores timeout.'),
}
fresh={
 'R03':'independently_validate_every_declared_parameter_example',
 'R08':'eight_fresh_clients_mix_object_guards_and_atomic_batches',
 'R10':'failed_batch_rolls_back_all_five_effect_classes_together',
 'R16':'failed_batch_rolls_back_all_five_effect_classes_together; c12_r1_contracts',
 'R20':'edit_object_kinds_clipboard_and_history_roundtrip; standalone_m1_element_types_roundtrip_undo_redo',
 'R25':'mixed_expect_rev_guards_cover_paths_compound_ids_and_unmodified_project',
 'R26':'mixed_expect_rev_guards_cover_paths_compound_ids_and_unmodified_project',
 'R27':'mixed_expect_rev_guards_cover_paths_compound_ids_and_unmodified_project',
 'R32':'revision_max_blocks_fresh_paste_and_keeps_queries_alive',
 'R38':'fresh_historical_diff_replays_and_agrees_with_checkpoint_across_batch_steps; diff_retains_the_last_save_even_after_ten_thousand_transactions',
 'R47':'fresh_mode_workspace_and_renamed_builtin_reset_are_coherent',
 'R48':'fresh_mode_workspace_and_renamed_builtin_reset_are_coherent',
 'R58':'c12_never_block; ci-audit.json; baseline/gates/headless.log',
 'R60':'fresh_job_cross_session_list_wait_cancel_are_isolated',
 'R68':'c12_r1_scale; logs/release-scale.log; large-documents.json',
 'R70':'ci-audit.json; static-audit.json; final/gates/test.log',
 'R71':'eight_fresh_clients_mix_object_guards_and_atomic_batches; random-history.json; c11_r3_random',
}
for r in requirements:
    rid=r['id'];was=r['status'];r['priorStatus']=was
    r['builderClaim']='fixed' if was in ['divergent','missing'] else 'unchanged'
    r['implementation']=impl[rid]
    r['status']='deferred-with-owner' if was=='deferred-with-owner' else 'implemented'
    r['note']=('Retained deferral: '+r['note']) if was=='deferred-with-owner' else 'Rechecked source and passing inherited suite in baseline/final logs.'
    r['tests']=fresh.get(rid,r['tests'])
    r['findings']=[]
    if rid in divergent:
        f,t,n=divergent[rid];r.update(status='divergent',tests=t,note=n,findings=['C12-R2-'+x for x in f.split(',')])
    if rid=='R58':r['note']='Citation accepted: 11 P5 / actual section 14.6 prohibit modal/user waits, not forced Rust thread preemption. Test proves bounded caller timeout and recovery after release only. Process restart/real modal interception remain C1.4/M4.'
    if rid=='R54':r['note']='Reverse parity now visits all 796 entries; 662 GUI locations/agent-event justifications are declarative future bindings, not native dispatch proof.'
write('requirements.json',requirements)
sample=json.loads((HERE/'samples.json').read_text())
by_id={r['id']:r for r in register}
sample_rows=[]
for cid in sample['random60']:
    r=by_id[cid]
    sample_rows.append({k:r[k] for k in ['id','status','spec','owningSectionSources','source','needsProject','enablement','scopes','undoable','undoLabel','specUndo','examples','examplesExecuted','guiLocations','agentOnly','schemaIssues','findings','tests']})
write('sample-60-audit.json',{'seed':sample['seed'],'rows':sample_rows})
spot=[{k:by_id[cid][k] for k in ['id','status','source','tests','findings']} for cid in sample['previousImplemented40']]
write('spot-40-audit.json',{'seed':sample['previousImplemented40Seed'],'rows':spot})
counts={'inventory':796,'declared':796,'handwritten':134,'generated':662,'generatedWithSchemaHoles':sum(bool(holes[k]['params'] or holes[k]['result']) for k in generated),
        'commandClassifications':dict(Counter(r['status'] for r in register)),
        'requirementClassifications':dict(Counter(r['status'] for r in requirements)),
        'random60':dict(Counter(r['status'] for r in sample_rows)),
        'previousImplemented40':dict(Counter(r['status'] for r in spot)),
        'examplesExecuted':len(examples),'exampleOutcomes':dict(Counter(x['error']['code'] if x['error'] else x['status'] for x in examples))}
write('register-counts.json',counts)
md=['# Round 2 conformance registers','',json.dumps(counts,indent=2),'',
    'All 796 IDs/kinds and verbatim appendix columns match. All declarations and parameter examples exist. Classification is evidence-bounded: implemented means exercised with no additional per-entry defect found; deferred-with-owner retains its actual future handler owner; divergent means a current declaration or behavior counterexample. Generic JSON Value and explicitly dynamic dictionaries in handwritten contracts are not automatically called placeholders. Generated holes are retained with exact schema pointers. Cross-cutting defects apply beyond per-command counts.','',
    '## Requirement closure (all 72)','',
    '| Row / requirement | R1 → R2 | Current source | Evidence | Adjudication |','|---|---|---|---|---|']
for r in requirements:md.append(f"| {r['id']} {r['requirement']} | {r['priorStatus']} → {r['status']} | `{r['implementation']}` | {r['tests']} | {r['note']} {' '.join(r['findings'])} |")
md+=['','## Inventory (all 796)','','| ID | R1 → R2 | Declaration / appendix | Examples run | Findings / owner |','|---|---|---|---|---|']
for r in register:md.append(f"| `{r['id']}` | {r['priorStatus']} → {r['status']} | `{r['source']}`; `{r['spec']}` | {r['examplesExecuted']} | {', '.join(r['findings']) or r['owner']} |")
(HERE/'conformance.md').write_text('\n'.join(md)+'\n',encoding='utf-8')
print(json.dumps(counts,indent=2))
