"""Re-extract the same 365 frozen-source inventory units and adjudicate round 2.

The extractor is retained in round 1. Execute it with this directory as HERE,
then replace dispositions with the independently checked round-2 evidence.
"""
from pathlib import Path
from collections import Counter
import json,re
ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
source=(HERE.parent/'M2-C2.1-r1/build-register.py').read_text(encoding='utf8')
namespace={'__file__':str(HERE/'build-register.py'),'__name__':'inventory_extractor'}
exec(compile(source,'round1-inventory-extractor','exec'),namespace)
rows=json.loads((HERE/'conformance.json').read_text(encoding='utf8'))['rows']
assert len(rows)==365
closures={
 '3.2':'c21_r2_contracts::mixed_patch_variants_and_failed_batch_have_no_effect; c21_obligations::a_patch_of_a_brush_spine_sweeps_the_outline_and_a_patch_of_the_outline_drops_the_spine; topology C2.2',
 '3.3':'c21_r2_geometry::dense_bidirectional_fitting_corpus / hundred_thousand_collinear_samples_respect_polyline_tolerance; c21_r1_geometry; c21_worst_case; pressure exception below',
 '3.4':'c21_r2_geometry::swept_tip_containment_matrix_at_and_between_knots; c21_r1_geometry::looping_brush_preserves_hole_and_clean_swept_area; pressure/work exceptions below',
 '3.7':'c21_r2_contracts::paragraph_patch_variants_are_atomic / malformed_paragraphs_in_package_fail_the_load_gate / unicode_ranges_round_trip_through_content_and_package; formatting exception below; glyph rendering C2.3',
 '4.9':'c21_r1_contracts; c21_r2_contracts; c21_r2_geometry::open_and_closed_anchor_limit_boundaries; c21_draw content round trip; SI-053/054/055 qualifications retained',
 '13.1':'c21_r2_contracts::setter_addressing_revision_and_result_matrix; c21_example_results; c21_obligations; c21_r2_events',
 '13.4':'c21_r2_geometry::open_and_closed_anchor_limit_boundaries; c21_draw::content_round_trips_through_read_back_with_new_ids; full nested read-back',
 '13.6':'c21_r2_contracts::mixed_patch_variants_and_failed_batch_have_no_effect / paragraph_patch_variants_are_atomic / events_exclude_dry_run_and_failures_and_sequential_batch_has_two; c21_r2_events; render/hit queries C2.3',
}
commands={
 'draw.stroke.add':('divergent','C21-R2-01 pressure loss; C21-R2-03 brush work; c21_r2_geometry'),
 'draw.stroke.set':('divergent','C21-R2-03 unbounded brush re-sweep; c21_r2_geometry::many_pressure_knots_brush_respects_watchdog; addressing/results proven by c21_r2_contracts'),
 'draw.text.set':('divergent','C21-R2-02; c21_r2_contracts::unrelated_format_survives_nfc_in_command_path'),
 'draw.content.add':('divergent','C21-R2-02 rich-text NFC uses Content::from_parts; C21-R2-03 spine-only brush sweep; c21_r2_geometry; nested stable Unicode round trip passes c21_r2_contracts'),
 'draw.drawing.changed':('implemented','crates/executor/src/run.rs::drawing_events from commit; c21_r1_contracts::content_changes_emit_drawing_event_including_undo_redo; c21_draw commit-path test; c21_r2_contracts dry/failure/sequential; c21_r2_events MCP subscription'),
}
for row in rows:
    sec=req=None
    sec=row['section'];req=row['requirement']
    if row['status']=='divergent' and sec in closures:
        row.update(status='implemented',evidence=closures[sec])
    cmd=re.match(r'^\| `(draw\.[^`]+)`',req)
    if cmd:
        name=cmd.group(1)
        if name in commands:row['status'],row['evidence']=commands[name]
        elif name in namespace['IMPLEMENTED']:
            f,token=namespace['IMPLEMENTED'][name]
            row.update(status='implemented',evidence=f'crates/executor/src/commands/{f}::{token}; c21_draw; c21_example_results; frontend_schemas; c21_r1_contracts; c21_r2_contracts (common target/revision/result variants)')
    if sec=='3.2' and req.startswith(('- Every stroke has two','- Within a layer')):
        row.update(status='deferred-with-owner',evidence='M2 C2.2 planar map; SI-057; plan section 4; unchanged deferral made explicit')
    if (sec=='3.2' and req.startswith(('- A **brush stroke**','- A **pressure map**'))) or (sec=='3.3' and req.startswith('5.')) or (sec=='3.4' and req.startswith(('- With a pressure-sensitive','- The brush outline'))):
        row.update(status='divergent',evidence='C21-R2-01; crates/vector/src/fit.rs::collapse averages pressure independently of smoothness; c21_r2_geometry::spatial_dedupe_preserves_pressure_extrema_for_the_brush_sweep; C21-R2-03 for brush sweep work')
    if (sec=='3.7' and req.startswith('- Created with')) or (sec=='4.9' and req.startswith('| `text`')):
        row.update(status='divergent',evidence='C21-R2-02; text.rs::compose fallback loses unrelated per-character formats; c21_r2_geometry::nfc_composition_preserves_unaffected_character_formats; c21_r2_contracts::unrelated_format_survives_nfc_in_command_path')
    if sec=='4.9' and req.startswith('| `spine`'):
        row.update(status='divergent',evidence='C21-R2-01 retained pressure map changes unsmoothed pressure; c21_r2_geometry::spatial_dedupe_preserves_pressure_extrema_for_the_brush_sweep; outline authority/mixed patch otherwise proven')
    if row['status']=='implemented' and sec=='3.7':
        row['evidence']='c21_r2_contracts::unicode_ranges_round_trip_through_content_and_package; c21_draw missingFonts/font name; glyph rendering C2.3; break apart C2.3'
    assert 'C21-R1-0' not in row['evidence'],row
counts=dict(Counter(r['status'] for r in rows))
(HERE/'conformance.json').write_text(json.dumps({'counts':counts,'rows':rows},ensure_ascii=False,indent=2)+'\n',encoding='utf8')
md=['# C2.1 round 2 conformance register','',
 'Inventory: the same 365 frozen-spec clauses/source rows as round 1, re-extracted by its retained generator. Repeated command rows remain separate. Counts are requirement dispositions, not defect counts. Implemented certifies the engine subset; future rendering, UI and topology owners remain as stated. Two topology-only rows formerly swept into a section-wide divergence are now explicitly deferred to their existing C2.2 owner.','',f'Counts: {counts}; total {len(rows)}.','',
 '| # | Requirement / source | Disposition | Test, implementation, finding or owner |','|---|---|---|---|']
for n,r in enumerate(rows,1):
    md.append(f"| {n} | {r['source']} section {r['section']}: {r['requirement'].replace('|',' / ')} | {r['status']} | {r['evidence']} |")
(HERE/'conformance.md').write_text('\n'.join(md)+'\n',encoding='utf8')
print(counts,len(rows))
