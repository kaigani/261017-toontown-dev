"""Update the immutable R1 register from independently tested R2 decisions."""
from collections import Counter
from pathlib import Path
import json
import re
import sys

sys.stdout.reconfigure(encoding="utf-8")
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
old=json.loads((HERE.parent/"M1-C1.1-r1/conformance.json").read_text(encoding="utf-8"))
rows=old["rows"]
M="crates/model/tests/c11_r2_adversarial.rs"
W="crates/engine/tests/c11_r2_wire.rs"
def test(name, file=M):
    lines=(ROOT/file).read_text(encoding="utf-8").splitlines()
    line=next(i for i,l in enumerate(lines,1) if re.search(r"\bfn "+re.escape(name)+r"\(",l))
    return {"file":file,"line":line,"name":name,
            "command":f"cargo test -p {'tt-engine' if file==W else 'tt-model'} --test {Path(file).stem} {name} --locked -- --exact --nocapture"}

# Entries are in exactly the order of the builder's 37-row response table.
# row, source file:line, fresh test, disposition. No response row was disputed.
decisions=[
(2,"crates/model/src/input.rs:148; crates/model/src/input.rs:166","nfc_is_applied_to_declared_non_name_strings_without_touching_opaque_values","open C11-R2-10; direct declared strings NFC, but patternProperties maps (cellNotes) are skipped as opaque"),
(3,"crates/model/src/schema.rs:98; crates/model/src/validate.rs:809","cell_note_frame_map_keys_obey_signed_int_bounds","open C11-R2-08; direct int fields fixed, frame-map keys still exceed signed int"),
(4,"crates/model/src/canonical.rs:209","nonfinite_nested_structs_maps_and_model_constructors_are_rejected","fixed; nested f32/f64 non-finite values rejected"),
(11,"crates/model/src/validate.rs:674; crates/model/src/validate.rs:809","mouth_label_frame_map_keys_are_positive_and_within_int","open C11-R2-08; direct frames/manual ranges fixed, label frame zero and out-of-int keys accepted"),
(12,"crates/model/src/schema.rs:98","signed_int_boundaries_and_seed_exception","fixed; signed duration bounds, explicit seed exception retained"),
(52,"crates/model/src/input.rs:278","swatch_alias_equal_conflicting_and_nested_forms","fixed; alias accepted before schema, conflicting values rejected"),
(53,"crates/model/src/schema.rs:297","every_envelope_type_is_readonly","fixed; every object envelope type read-only"),
(56,"crates/model/src/input.rs:209; crates/model/src/change.rs:454","opaque_model_lookalikes_deep_arrays_and_patch_inverse_are_preserved","fixed; opaque model-like keys, arrays, nesting and diff/inverse preserved"),
(57,"crates/model/src/input.rs:209; crates/model/src/json_text.rs:241","opaque_model_lookalikes_deep_arrays_and_patch_inverse_are_preserved","fixed; opaque values unchanged; integers lossless or typed error per ruling"),
(74,"crates/model/src/document/mod.rs:572","new_entity_semantic_boundary_controls","fixed; project library must be local"),
(85,"crates/model/src/input.rs:58","randomized_insertion_and_post_patch_revalidation_write_identical_bytes","fixed; content determines map key order"),
(94,"crates/model/src/schema.rs:98","signed_int_boundaries_and_seed_exception","fixed; loops positive signed int"),
(105,"crates/model/src/input.rs:209","nfc_is_applied_to_declared_non_name_strings_without_touching_opaque_values","fixed; note NFC"),
(110,"crates/model/src/document/mod.rs:886","new_entity_semantic_boundary_controls","fixed; image bank drawings contain exactly one bitmap; opaque importOptions separately tested"),
(153,"crates/model/src/validate.rs:406","fixed_text_width_and_texture_paths_reject_new_invalid_cases","fixed; fixed-width text requires width"),
(156,"crates/model/src/validate.rs:329","new_entity_semantic_boundary_controls","fixed; brush closed, spec 10 section 4.9 governs; SI-013 tracks conflicting example"),
(176,"crates/model/src/validate.rs:684","new_entity_semantic_boundary_controls","fixed; manual ranges positive, ordered, within placement"),
(185,"crates/model/src/validate.rs:242","new_entity_semantic_boundary_controls","fixed; locators required/forbidden by link kind"),
(309,"crates/model/src/document/mod.rs:198; crates/model/src/validate.rs:473","duplicate_split_entries_all_five_maps_are_rejected","fixed; all split entry maps, cross-type/global/local duplicate controls pass; allocation transactions still C1.2"),
(310,"crates/model/src/id.rs:232","cloned_generator_eight_threads_100k_each_are_unique","fixed; 800000 IDs each for system and seeded shared generators, no collisions"),
(336,"crates/model/src/input.rs:58","randomized_insertion_and_post_patch_revalidation_write_identical_bytes","fixed; schema order and recursively sorted map keys; SI-014 correctly notes unspecified map order"),
(337,"crates/model/src/canonical.rs:35; crates/model/src/json_text.rs:241","nonfinite_nested_structs_maps_and_model_constructors_are_rejected","fixed; finite canonical numbers; exact large integer transport separately verified"),
(339,"crates/model/src/object.rs:35; crates/model/src/input.rs:58","randomized_insertion_and_post_patch_revalidation_write_identical_bytes","fixed for validated model objects; input depth100 protects writer128; generic wire mismatch is row412"),
(378,"crates/model/src/path.rs:27","deeper_library_paths_near_model_limit_resolve","fixed model addressing; special references remain named C1.2 deferral"),
(381,"crates/model/src/input.rs:58","randomized_insertion_and_post_patch_revalidation_write_identical_bytes","fixed; deterministic canonical content after patch revalidation"),
(386,"crates/model/src/schema.rs:320","every_model_stub_and_full_result_validates_at_depth_zero_one_two_all","fixed; all22 types validate as stubs/full at depths0,1,2,all; R1 slot-type negative passes"),
(387,"crates/model/src/path.rs:27","deeper_library_paths_near_model_limit_resolve","fixed; accepted deep library leaf paths resolve"),
(388,"crates/model/src/change.rs:509; crates/model/src/change.rs:623","array_append_patch_inverse_restores_input","open C11-R2-02/03/04; original overwrite/elision cases fixed, array inverse and malformed-operation gaps remain"),
(389,"crates/model/src/change.rs:667","elided_and_incomplete_operations_fail_without_mutating_input","fixed; elided operations rejected, missing values and absent replace paths rejected"),
(392,"crates/model/src/schema.rs:352; crates/model/src/schema.rs:557","every_schema_property_has_since_and_envelope_type_is_readonly","open C11-R2-11; MouthTrack.stale is added after annotation pass and lacks x-since; envelope type readOnly fixed"),
(394,"crates/model/src/schema.rs:606; crates/model/src/schema.rs:627","property_lookup_follows_effect_condition_without_discarding_it","open C11-R2-06; conditional paths absent, ref siblings dropped; original tuple/child paths fixed"),
(395,"crates/model/src/document/mod.rs:198; crates/model/src/document/mod.rs:886","duplicate_placement_sequence_block_and_channel_ids_are_rejected","fixed; duplicate containment and image invariant checks; existing cycle/orphan tests pass"),
(400,"crates/model/src/document/mod.rs:886","new_entity_semantic_boundary_controls","fixed; image drawing bitmap count"),
(403,"crates/model/src/entities/channel.rs:257; crates/model/src/schema.rs:98","spline_clamps_both_sides_for_irregular_key_spacing","fixed; clamped according to05 section10.3, exact frame limits"),
(406,"crates/model/src/validate.rs:156","fixed_text_width_and_texture_paths_reject_new_invalid_cases","fixed; safe texture package paths"),
(407,"crates/model/src/document/mod.rs:558","new_entity_semantic_boundary_controls","fixed; at least one visible palette"),
(412,"crates/model/src/object.rs:35; crates/model/src/canonical.rs:209; crates/model/src/json_text.rs:95","writer_never_emits_text_its_reader_rejects_at_depth_boundary","open C11-R2-07; model100 protects objects, generic wire writer permits scalar at level129 which reader rejects"),
]
assert len(decisions)==37
assert {x[0] for x in decisions}=={i for i,r in enumerate(rows,1) if r['status'] in ('divergent','missing')}
closure=[]
for number,(row,source,name,note) in enumerate(decisions,1):
    r=rows[row-1]
    r.update(r1_status=r["status"],r1_note=r["note"],implementation=source,
             status="divergent" if note.startswith("open") else "implemented",note=note,
             test=test(name,W if row==412 else M),response_table_row=number)
    closure.append({"response_row":number,"register_row":row,**r})
for row,name in [(177,"mouth_label_frame_map_keys_are_positive_and_within_int"),(398,"cell_note_frame_map_keys_obey_signed_int_bounds")]:
    r=rows[row-1]
    r.update(r1_status=r['status'],r1_note=r['note'],status="divergent",implementation="crates/model/src/validate.rs:674; crates/model/src/validate.rs:809",
             note="C11-R2-08: frame map keys accept invalid bounds; previously implemented row downgraded. C11-R2-10 also affects cellNotes NFC.",test=test(name))
spot=[1,5,6,7,8,9,10,13,15,23,24,25,26,27,28,29,30,65,66,67,76,77,78,79,80,82,83,90,95,96,99,101,102,103,104,106,125,126,127,147]
assert len(spot)==40
for row in spot:
    assert rows[row-1]['status']=='implemented'
    rows[row-1]['r2_spotcheck']=test("forty_previously_implemented_register_rows_retain_constraints")
for i,r in enumerate(rows,1): r['row']=i
counts=dict(Counter(r['status'] for r in rows))
counts.setdefault('missing',0)
out={"base":"7c5405d","source":"../M1-C1.1-r1/conformance.json","counts":counts,"rows":rows,"implemented_spotchecks":spot}
(HERE/"conformance.json").write_text(json.dumps(out,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
(HERE/"register-closure.json").write_text(json.dumps(closure,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
def esc(v): return str(v).replace('|','\\|').replace('\n',' ')
md=["# C1.1 round 2 conformance register","",f"{len(rows)} requirements: {counts}. Stable row IDs are 1-based positions in round1. All193 deferred rows retain their original owner/checkpoint.","",
"Every response claim has a source location and a fresh test. Thirty of37 claims close; seven remain divergent. Two additional previously implemented rows are downgraded. Forty other previously implemented rows were checked with fresh valid/invalid controls. No response row was disputed; SI-013/014 explain spec ambiguities without deferring their implemented choices.","",
"The R1 note remains historical evidence for rows without an R2 change; cross-cutting R1 defects are superseded by the closure table, not silently re-opened by historical prose. Present representation does not prove deferred command behavior. The companion inventory enumerates published properties without counting them twice.","",
"| Row | Spec | Requirement | Status | Implementation / owner | Review note / fresh test |","|---|---|---|---|---|---|"]
for r in rows:
    t=r.get('test',r.get('r2_spotcheck'))
    note=r['note']+(f"; {t['file']}:{t['line']} `{t['name']}`" if t else '')
    md.append('| '+' | '.join(esc(x) for x in [r['row'],r['citation'],r['requirement'],r['status'],r['implementation'],note])+' |')
(HERE/"conformance.md").write_text('\n'.join(md)+'\n',encoding="utf-8")
bundle=json.loads((ROOT/"crates/model/schema/model-1.0.schema.json").read_text(encoding="utf-8"))
schema_lines=(ROOT/"crates/model/schema/model-1.0.schema.json").read_text(encoding="utf-8").splitlines()
inventory=[]
for name,s in bundle['$defs'].items():
    line=next((i for i,l in enumerate(schema_lines,1) if l==f'    "{name}": {{'),1)
    for field,p in s.get('properties',{}).items():
        inventory.append({"definition":name,"property":field,"file":"crates/model/schema/model-1.0.schema.json","definitionLine":line,
          "pointer":f"/$defs/{name}/properties/{field}","schema":p,"required":field in s.get('required',[]),
          "requiredInFullObject":field in s.get('then',{}).get('required',[]),"requiredInStub":field in s.get('else',{}).get('required',[])})
(HERE/"property-inventory.json").write_text(json.dumps(inventory,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
print(json.dumps({"requirements":len(rows),"counts":counts,"responseClaimsClosed":sum(r['status']=='implemented' for r in closure),"responseClaimsOpen":sum(r['status']=='divergent' for r in closure),"spotchecks":len(spot),"objectTypes":len(bundle['x-objectTypes']),"properties":len(inventory)}))
