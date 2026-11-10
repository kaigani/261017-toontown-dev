from pathlib import Path
import json
p=Path('reviews/evidence/M3-C3.3-r1/register.json')
r=json.loads(p.read_text())
rows=r['rows']
for name,finding,evidence in [
 ('cancellation for an empty render list','C33-R1-07','c33_r1_cancel::empty_frame_observes_requested_cancellation'),
 ('process-death staging cleanup','C33-R1-08','c33_r1_crash::killed_sequence_leaves_no_staged_files')]:
 rows.append(dict(id=finding,requirement=name,status='divergent',clause='ADR-0020 decision 8; charter items 6-7',evidence=evidence,finding=finding))
for command,status,finding in [
 ('render.frame.run','divergent','C33-R1-04'),('render.limits.get','implemented',''),
 ('render.range.run','divergent','C33-R1-02/C33-R1-03/C33-R1-08'),
 ('inspect.frame.describe','divergent','C33-R1-01'),('inspect.hitTest','divergent','C33-R1-01'),
 ('inspect.coordinates.convert','implemented',''),('inspect.contactSheet.render','divergent','C33-R1-04')]:
 rows.append(dict(id='REG-'+command,requirement='registry row '+command,status=status,clause='A registry; 09 11; 11 15.2',evidence='named command suites and reviewer probes',finding=finding))
rows.append(dict(id='READINESS-FRACTIONAL',requirement='fractional-frame coordinate conversion acceptance probe',status='missing',clause='C3.2 closure readiness item 8; round-one charter item 4',evidence='c33_r1_contract::fractional_inspection_frame_is_a_typed_registry_refusal',note='The registry specifies integer frames. This is a readiness-evidence mismatch, not a claim that the integer schema violates its table; no extension or deferral is silently ruled.'))
rows.append(dict(id='READINESS-EVALUATION-COUNT',requirement='instrumented count of evaluations in an actual render-list command',status='missing',clause='round-one charter item 3',evidence='render_list.rs and Evaluator memoization inspected; no public per-query evaluator counter',note='Memoization is source-reviewed; a measured render-list evaluation count is not claimed.'))
r['counts']={s:sum(row['status']==s for row in rows) for s in ['implemented','divergent','missing','deferred-with-owner','pending']}
p.write_text(json.dumps(r,indent=2)+'\n')
p.with_suffix('.md').write_text('# C3.3 round-one requirement register\n\n'+json.dumps(r['counts'])+'\n\nImplemented entries use the named source review and executed suites; missing entries explicitly record acceptance evidence not established.\n\n| ID | Requirement | Disposition | Evidence |\n|---|---|---|---|\n'+'\n'.join(f"| {row['id']} | {row['requirement']} | {row['status']} {row.get('owner','')} | {row['evidence']} |" for row in rows)+'\n')
print(r['counts'])
