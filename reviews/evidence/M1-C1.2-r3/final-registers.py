"""Independent R3 adjudication. Prior reviewer row identities are retained, not builder statuses.

This does not import/run build-register.py, the declaration generator, or R2 audit code.
Classification uses the fresh exported registry, independent structural/semantic checks,
fresh regressions, retained passing tests, and the accepted seven deferrals.
"""
from collections import Counter
import json
import re
from pathlib import Path

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
def read(p): return p.read_text(encoding='utf-8')
def load(p): return json.loads(read(p))
def write(name,x): (HERE/name).write_text(json.dumps(x,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
rows=load(HERE/'registry.json')
checks={r['id']:r for r in load(HERE/'schema-audit.json')}
semantics=load(HERE/'semantic-schema-cases.json')
sample=load(HERE/'samples.json')
assert len(rows)==796 and len(checks)==796
assert all(not r['unresolvedRefs'] for r in checks.values())
assert all(not any(r['holes'].values()) for r in checks.values() if r['generated'])

plan=read(ROOT/'plan/IMPLEMENTATION_PLAN.md')
owners={}
for n,l in enumerate(plan.splitlines(),1):
    m=re.match(r'\| (M\d+) \|',l)
    if m:
        for c in re.findall(r'\bC\d+\.\d+\b',l):
            owners[f'{m[1]} {c}']={'source':f'plan/IMPLEMENTATION_PLAN.md:{n}','row':l}
owner_audit=[]
for r in rows:
    if r['status']=='notImplemented':
        owner=r['milestone']
        assert owner in owners, (r['id'], owner)
        owner_audit.append({'id':r['id'],'owner':owner,'planSource':owners[owner]['source']})
write('owner-audit.json',{'checked':len(owner_audit),'ownerCounts':dict(Counter(x['owner'] for x in owner_audit)),
    'planOwners':owners,'rows':owner_audit,
    'review':'All 727 deferred handlers map to an actual checkpoint in section 4. Group review agrees with retained ownership split: transforms/keys C3.2, paths/function UI C6.2, bones C6.3, movie import C8.3, global library C9.1, starter C9.3, file/PDF/print C10.2; drawing/model and desktop UI readiness split retained.'})

appendix={}
for n,l in enumerate(read(ROOT/'spec/A_command_registry.md').splitlines(),1):
    m=re.match(r'\| `([^`]+)` \| (action|toggle|query|job|ui|event) \|',l)
    if m: appendix[m[1]]=n
assert set(appendix)=={r['id'] for r in rows}
source={}
for p in sorted((ROOT/'crates/executor/src/commands').glob('*.rs')):
    for n,l in enumerate(read(p).splitlines(),1):
        m=re.search(r'id = "([^"]+)"',l)
        if m: source[m[1]]=f'{p.relative_to(ROOT).as_posix()}:{n}'
for n,l in enumerate(read(ROOT/'crates/registry/src/declared.rs').splitlines(),1):
    m=re.search(r'id: "([^"]+)"',l)
    if m: source[m[1]]=f'crates/registry/src/declared.rs:{n}'
fail={r['id']:['C12-R3-01'] for r in semantics}
fail.update({'checkpoint.create':['C12-R3-02'],'job.wait':['C12-R3-03'],'doc.diff':['C12-R3-04','C12-R3-02']})
register=[]
for r in rows:
    cid=r['id']; finding=fail.get(cid,[])
    status='divergent' if finding else 'implemented' if r['status']=='implemented' else 'deferred-with-owner'
    assert cid in source, cid
    register.append({'id':cid,'kind':r['kind'],'status':status,'declaredStatus':r['status'],
      'owner':r.get('milestone','M1 C1.2'),'spec':f'spec/A_command_registry.md:{appendix[cid]}','source':source[cid],
      'findings':finding,'schemaAudit':'schema-audit.json#'+cid,
      'sample80':cid in sample['random80'],'spot40':cid in sample['implemented40'],
      'examplesChecked':checks[cid]['validExamples'],'scopes':r['scopes'],'needsProject':r['needsProject'],
      'enablement':r['enablement'],'guiLocations':r['guiLocations'],'agentOnly':r['agentOnly'],
      'evidence':'c12_r3_stress; c12_r2_registry; c12_registry; baseline/final logs',
      'qualification':'See reproducible counterexample' if finding else 'No further defect found by structural/metadata audit, retained tests and selected semantic inspection; a validating generated sample alone does not certify all values.'})
write('command-register.json',register)

old=load(HERE.parent/'M1-C1.2-r2/requirements.json')
assert len(old)==72
# Read only the prior reviewer's source-location labels, not its executable audit or judgments.
locations=re.search(r"locations='''(.*?)'''",read(HERE.parent/'M1-C1.2-r2/build-registers.py'),re.S).group(1)
current={}
for l in locations.strip().splitlines():
    rid,p,needle=l.split('|',2)
    if rid=='R15': p,needle='crates/executor/src/cx.rs','pub fn new_id'
    matches=[n for n,l in enumerate(read(ROOT/p).splitlines(),1) if needle in l]
    assert matches, (rid,p,needle)
    current[rid]=f'{p}:{matches[0]}'
open_rows={
 'R02':(['01'],'c12_r3_registry; semantic-schema-cases.json','Zero structural holes; fourteen spec-valid contracts rejected.'),
 'R12':(['04'],'staged_drawing_only_diff_keeps_content_invalidation','Filtered query loses a prior staged stroke edit.'),
 'R19':(['03'],'overlapping_identical_waits_retain_one_original_envelope','Concurrent identical wait replies differ.'),
 'R23':(['02'],'checkpoint_after_stroke_edit_has_no_diff_at_same_committed_revision','Checkpoint predicted revision and contained stamps disagree.'),
 'R28':(['03'],'overlapping_identical_waits_retain_one_original_envelope','Pending identical retries execute independent waits and overwrite cached reply.'),
 'R34':(['04'],'staged_drawing_only_diff_keeps_content_invalidation','Drawing-only staged diff omits a changed drawing revision.'),
 'R37':(['02','04'],'c12_r3_contracts','Retained historical regressions pass; checkpoint and staged filtered diff counterexamples remain.'),
 'R63':(['03'],'overlapping_identical_waits_retain_one_original_envelope','Real waiting and ownership fixed, duplicate request contract still broken.'),
}
deferred={
 'R30':'M1 C1.4','R35':'M1 C1.3','R42':'M2 C2.3; M3 C3.2/C3.3; M7 C7.2; M8 C8.1',
 'R43':'M2 C2.3; M3 C3.1/C3.2; owning specialty checkpoints',
 'R55':'M4 C4.1','R62':'M1 C1.4','R72':'M1 C1.3',
}
req=[]
for r in old:
    rid=r['id']
    new={k:r[k] for k in ('id','requirement')}
    new.update(implementation=current[rid],priorStatus=r['status'],status='implemented',findings=[],
        tests=r['tests']+'; baseline/final full workspace logs',
        note='Source rechecked at current commit; retained suites pass, with fresh closure/stress checks where applicable.')
    if rid in deferred:
        new.update(status='deferred-with-owner',owner=deferred[rid],note=r['note'],tests=r['tests'])
    if rid in open_rows:
        ff,tt,note=open_rows[rid]
        new.update(status='divergent',findings=['C12-R3-'+f for f in ff],tests=tt,note=note)
    req.append(new)
write('requirements.json',req)

spot=[]
for cid in sample['implemented40']:
    r=next(r for r in register if r['id']==cid)
    spot.append({'id':cid,'source':r['source'],'status':r['status'],'findings':r['findings'],
      'checked':'spec A/01 or owning section; source declaration/handler; enablement/no-project/scopes; example/result schema; malformed execution',
      'runtimeEvidence':'c12_r3_stress::every_executable_typed_example_also_validates_its_result_schema; retained c12_policy/c12_workspace/c12_jobs/c12_transactions/c12_registry suites',
      'note':'Event is registered metadata and rejects invocation; event tests verify delivery.' if r['kind']=='event' else 'Command checked as callable handler.'})
write('implemented40-adjudication.json',spot)
sample_review=[]
for cid in sample['random80']:
    r=next(r for r in register if r['id']==cid)
    sample_review.append({'id':cid,'spec':r['spec'],'source':r['source'],'status':r['status'],
       'findings':r['findings'],'schemaEvidence':'sample80-review.json; sample-generated-mutations.json',
       'metadataReview':'Owning-section/01 project context, scopes, enablement predicates and GUI/agent declaration inspected.',
       'semanticReview':'Known counterexample recorded in semantic-schema-cases.json' if 'C12-R3-01' in r['findings'] else 'No additional established semantic defect; not a proof of every possible value.'})
write('sample80-adjudication.json',sample_review)
counts={'requirements':dict(Counter(r['status'] for r in req)),'commands':dict(Counter(r['status'] for r in register)),
 'declaredRuntime':dict(Counter(r['status'] for r in rows)), 'totalRequirements':72,'totalCommands':796,
 'implementedEventRows':sum(r['status']=='implemented' and r['kind']=='event' for r in rows),
 'schemaDivergentRows':len(semantics),'semanticSampleFailures':sum(r['id'] in sample['random80'] for r in semantics),
 'missing':0,'ownerRowsVerified':len(owner_audit)}
write('register-counts.json',counts)
for name,data,label in [('requirements',req,'requirement'),('command-register',register,'id')]:
    lines=[f'# Round 3 {name}', '', '| Row | Status | Owner | Findings |', '|---|---|---|---|']
    for r in data:
        lines.append(f"| {r['id']}"+(f" — {r[label]}" if label!='id' else '')+f" | {r['status']} | {r.get('owner','M1 C1.2')} | {', '.join(r['findings']) or '—'} |")
    (HERE/(name+'.md')).write_text('\n'.join(lines)+'\n',encoding='utf-8')
print(json.dumps(counts,indent=2))
