"""Reparse every original specification unit, retaining the established unit boundaries and reanchoring sources."""
import ast
from pathlib import Path

HERE = Path(__file__).resolve().parent
old = (HERE.parent/'M1-C1.4-r2/register.py').read_text(encoding='utf-8')
tree = ast.parse(old)
r2_delta = next(ast.literal_eval(n.value) for n in tree.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='delta' for t in n.targets))
source = (HERE.parent/'M1-C1.4-r1/register.py').read_text(encoding='utf-8')
r3_delta = r'''
# Original tests pass. Promote the round-two fixes, then adjudicate the new variants.
for r in rows:
    if r['line'] in (216,311):
        r.update(status='implemented', note='Round-two fix verified on 39ea1b2 by unchanged reviewer suites; staged effects and external-change confirmation are implemented.')
    if r['line'] in (26,505):
        r.update(status='divergent', note='C14-R3-01/02: failed publication drops committed atomic edits from replay; flattening atomic batches changes revisions and history. Owner builder/orchestrator, C1.4 closure.', test='crates/mcp/tests/c14_r3_replay.rs')
    if r['line'] in (219,573):
        r.update(status='divergent', note='C14-R3-03: a cancelled elicitation accepts a late answer and overwrites the file. Owner builder, C1.4 closure.', test='tests/adversarial/c14-r3-mcp.py::test_cancelled_elicitation_does_not_authorize_a_late_accept')
    if r['line']==96:
        r.update(note='SI-049 remains the original spec-owner divergence: implements 2025-06-18/2025-11-25; current-stable target awaits a spec-owner decision. SDK 24/24 does not resolve it.', test='scripts/mcp-conformance.py; reviews/evidence/M1-C1.4-r3/sdk.json')
    if r['status']=='implemented' and '74fe3a9' in r['note']:
        r['note']=r['note'].replace('74fe3a9','39ea1b2').replace(' Re-adjudication below overrides this if a round-two variant fails.','')
for d in declarations:
    if d['id']=='agent.activity.export':
        d.update(status='divergent', note='C14-R3-01/02: export is staged and publication status is truthful, but its replay loses committed edits and atomic history boundaries. Owner builder/orchestrator, C1.4 closure.',test='crates/mcp/tests/c14_r3_replay.rs')
# Require real plan checkpoints for every deferred row; broad milestone ownership is refined here.
checkpoints={2:'C2.3',3:'C3.3',4:'C4.2',5:'C5.2',6:'C6.2',7:'C7.2',8:'C8.2',9:'C9.1',10:'C10.2',11:'C11.1'}
plan=(ROOT/'plan/IMPLEMENTATION_PLAN.md').read_text(encoding='utf-8')
for r in rows+declarations:
    if r['status']!='deferred-with-owner': continue
    owners=[]
    milestones=sorted({int(m) for m in re.findall(r'\bM(\d+)\b',r['note'])})
    if not milestones:
        raise AssertionError(('deferred owner missing',r))
    for m in (x for x in milestones if x > 1):
        cp=checkpoints[m]
        if m==4 and any(x in r['note'].lower() for x in ('http','attached','effectiveparams','restart','indicator','confirmation')): cp='C4.1'
        if m==2 and 'worked example' in r['note'].lower(): cp='C2.1'
        assert cp in plan,(m,cp)
        owners.append(f'M{m} {cp}')
    if not owners and r.get('line')==566:
        owners=['M2 C2.1','M3 C3.1','M3 C3.2','M7 C7.2','M8 C8.1','M9 C9.1']
    assert owners,('deferred row lacks a real checkpoint',r)
    r['ownerCheckpoints']=owners
    r['note']+=' Explicit implementing owner checkpoints: builder '+', '.join(owners)+'.'
'''
source = source.replace('counts = dict(collections.Counter', r2_delta+'\n'+r3_delta+'\ncounts = dict(collections.Counter',1)
source = source.replace('round 1 conformance register','round 3 conformance register').replace('Baseline: `3906001`','Product: `39ea1b2`')
source = source.replace('## Remaining event producer gap','## Event producers and residual domain owners')
source = source.replace('`doc.undone` / `doc.redone` are specified but have no producer; undo/redo emit `doc.changed`. This is a carried kernel event-contract gap (P2 ticket C14-R1-16), distinct from the now-working subscribed doc.changed path. State/playback/UI event names are tracked under their implementing domain milestones.', '`doc.undone` / `doc.redone` carry undoStepId and label; unchanged protocol regressions pass. State/playback/UI event producers retain their implementing domain checkpoints. Effective parameters and worked examples remain ticketed.')
exec(compile(source,str(HERE/'register.py'),'exec'),{'__file__':str(HERE/'register.py')})

