"""Fill the report's 37-row closure table from the audited register."""
from pathlib import Path
import json

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
labels=['NFC strings','Signed int bounds','Finite numbers','Positive frames/ranges','Duration bounds','Swatch id alias',
        'Immutable type','Opaque meta','Opaque ext','Local project library','paletteStyles ordering','Positive loops',
        'Element note','Image drawings/importOptions','Fixed text width','Closed brush outlines','Manual mouth ranges',
        'Asset links','ID-01 uniqueness','ID-02 generation','JSON-02 key order','JSON-03 numbers','JSON-05 stable saves',
        'AG-03 addressing','AG-06 determinism','Full/stub schemas','Canonical paths','Invertible change sets','Elision/reread',
        'Property annotations','Property schema lookup','Containment tree','Image bitmap count','Handle clamping','Texture paths',
        'Visible palette','Depth/boundary guarantees']
rows=json.loads((HERE/'register-closure.json').read_text(encoding='utf-8'))
table=['| Response # / register row | Requirement / disposition | Source location | Fresh evidence command |','|---|---|---|---|']
for label,r in zip(labels,rows,strict=True):
    status='Fixed' if r['status']=='implemented' else '**Open**'
    t=r['test']; command=('W' if 'engine/' in t['file'] else 'M')+' `'+t['name']+'`'
    table.append(f"| {r['response_row']} / {r['register_row']} | {label}: {status} | {r['implementation']} | {command} |")
report=ROOT/'reviews/M1-C1.1-r2.md'
text=report.read_text(encoding='utf-8')
start='<!-- REGISTER_CLOSURE_TABLE -->'
end='<!-- END_REGISTER_CLOSURE_TABLE -->'
block=start+'\n\n'+'\n'.join(table)+'\n\n'+end
if end in text:
    left=text.index(start); right=text.index(end)+len(end)
    text=text[:left]+block+text[right:]
else:
    assert start in text
    text=text.replace(start,block)
text=text.replace('76–80 (excluding 81)','76–80')
report.write_text(text,encoding='utf-8',newline='\n')
findings=[
('01','P1','Unauthorized third reviewer-test edit','C1.1 retry','test_only_two_authorized_reviewer_test_edits'),
('02','P1','Array patch inverses fail or restore wrong content','C1.1 retry','array_append_patch_inverse_restores_input; nested_array_move_with_overwrite_inverse_restores_input'),
('03','P1','Malformed range replacement deletes text','C1.1 retry','range_patch_rejects_nonstring_mouth_value_without_deleting_text'),
('04','P2','Missing copy source defaults to root','C1.2 before patch consumers','missing_copy_source_is_not_implicitly_the_document_root'),
('05','P2','Composite numeric coercion rejects valid values','C1.2 schema consumers','does not let an inapplicable oneOf double branch corrupt an integer branch; coerces scalar doubles declared through allOf'),
('06','P2','Property discovery loses conditionals and annotations','C1.1 retry','property_lookup_follows_effect_condition_without_discarding_it; property_lookup_preserves_ref_property_annotations'),
('07','P2','Generic wire writer/reader depth mismatch','C1.2 before payload expansion','writer_never_emits_text_its_reader_rejects_at_depth_boundary'),
('08','P2','Frame-map keys escape signed/positive bounds','C1.1 retry','cell_note_frame_map_keys_obey_signed_int_bounds; mouth_label_frame_map_keys_are_positive_and_within_int'),
('09','P2','Historical response raw logs absent','C1.1 retry','test_claimed_response_raw_logs_are_committed'),
('10','P2','Declared pattern-map text skips NFC','C1.1 retry','nfc_is_applied_to_declared_non_name_strings_without_touching_opaque_values'),
('11','P2','Published MouthTrack.stale lacks x-since','C1.1 retry','every_schema_property_has_since_and_envelope_type_is_readonly'),
]
records=[dict(id='C11-R2-'+n,priority=p,title=t,targetCheckpoint=c,regressions=r.split('; '),status='open') for n,p,t,c,r in findings]
(HERE/'findings.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8')
(HERE/'tickets.json').write_text(json.dumps([r for r in records if r['priority']=='P2'],indent=2)+'\n',encoding='utf-8')
print('Report populated: 37 closure rows; 11 findings; 8 P2 tickets.')
