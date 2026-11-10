import json,collections
from pathlib import Path
p=Path('reviews/evidence/M3-C3.3-r2');r=json.loads(Path('reviews/evidence/M3-C3.3-r1/register.json').read_text());r['reviewedHead']='669ddc3d0dc4efc148c9adf78b53c21b5d1b9fcf'
for row in r['rows']:
 if row['status'] in ['divergent','missing']:
  row['priorStatus']=row['status'];row['status']='implemented';row['evidence']='Round-two source adjudication; inherited finding probes; c33_round1; round-two independent probes. See report for scope and execution outcomes.'
 if row['id']=='READINESS-FRACTIONAL':row['status']='closed';row['evidence']='Integer registry ruling SI-109; unchanged typed-refusal probe'
 if row['id']=='SPEC-WIDTH-DEFAULT':row['evidence']='SI-110 ruling; generic and command use camera width'
 if row['id'] in ['C33-R1-08','REG-render.range.run']:
  row['status']='divergent';row['finding']='C33-R2-01/C33-R2-02';row['evidence']='c33_r2_recovery::recovery_preserves_non_product_marker_files; unrelated file is deleted; c33_r2_wait cancellation receipt omits sweep warning'
r['rows'].append(dict(id='DEF-THUMBNAILS',requirement='project and drawing thumbnails',status='deferred-with-owner',owner='M5 C5.2',clause='ruling; derived.rs',evidence='explicit owner in derived.rs and traceability'))
evidence={
'C33-R1-01':'c33_r2_depth::seeded_anchor_groups_4096; six controlled members, document-tree Timeline oracle, hit-order and perturbation samples',
'C33-R1-02':'unchanged c33_r1_contract manifest probe; c33_round1 timeout/addressability; c33_r2_wait responsiveness and timeout/cancel assertions pass before the separate warning failure',
'C33-R1-03':'unchanged failed-publication probe; c33_r2_stress::last_target_readonly_preserves_originals; builder original restoration',
'C33-R1-04':'c33_r2_wait::dimension_preflight_precedes_slow_work (frame/sheet/range); ::encoded_byte_limit_is_checked_after_encoding; unchanged c33_r1_limits',
'C33-R1-05':'c33_r2_visibility command parity, unknown refs, hidden ancestor and precedence; unchanged c33_r1_tools generic override probe',
'C33-R1-06':'unchanged c33_r1_tools timing probe; c33_r2_visibility renderMs; recorded example normalization source adjudicated',
'C33-R1-07':'c33_r2_cancel::cancellation_at_every_empty_and_nonempty_boundary; unchanged c33_r1_cancel and c23_r3_cancel',
'READINESS-EVALUATION-COUNT':'c33_round1 measured describe/render/hit per-node counts; c33_r2_recovery contact-sheet N-frame count (serialized recorder)',
'REG-render.frame.run':'c33_render, c33_r1_pixels/bitmap/limits; c33_r2_visibility, c33_r2_wait limit/byte probes; golden equality',
'REG-inspect.frame.describe':'c33_r2_depth anchor oracle; c33_r2_visibility parity; unchanged inspection geometry suites',
'REG-inspect.hitTest':'c33_r2_depth reverse-order samples; unchanged c33_r1_pixels and c33_inspect geometry',
'REG-inspect.contactSheet.render':'c33_r2_visibility cell-byte parity; c33_r2_recovery evaluation counts; work-delay dimension preflight',
}
for row in r['rows']:
 if row['id'] in evidence: row['evidence']=evidence[row['id']]
r['counts']=dict(collections.Counter(x['status'] for x in r['rows']));r['counts']['pending']=0
r['verificationNote']='Statuses are adjudications, not a claim of exhaustive adversarial coverage. The report distinguishes executed tests, source review, CI evidence and residual coverage limitations.'
(p/'register.json').write_text(json.dumps(r,indent=2)+'\n')
(p/'register.md').write_text('# C3.3 round-two register\n\n'+str(r['counts'])+'\n\n| ID | Requirement | Disposition | Evidence |\n|---|---|---|---|\n'+'\n'.join('| '+ ' | '.join(str(x.get(k,'')) for k in ['id','requirement','status','evidence'])+' |' for x in r['rows'])+'\n')
print(r['counts'])
