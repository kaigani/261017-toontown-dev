"""Independently reconcile R3 row identities with current runtime metadata, spec A, plan §4 and closure evidence.

No builder register or declaration-generator function is used for classification.
"""
from collections import Counter
import copy
import json
from pathlib import Path
import re
import subprocess

from jsonschema import Draft202012Validator as Validator

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
def load(p): return json.loads(p.read_text(encoding='utf-8'))
def save(name,x): (HERE/name).write_text(json.dumps(x,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
prior_commands=load(HERE.parent/'M1-C1.2-r3/command-register.json')
prior_requirements=load(HERE.parent/'M1-C1.2-r3/requirements.json')
runtime=load(HERE/'registry.json')
generated={r['id'] for r in load(ROOT/'crates/registry/src/declared.json')}
appendix={}
for n,line in enumerate((ROOT/'spec/A_command_registry.md').read_text(encoding='utf-8').splitlines(),1):
    if re.match(r'\| `[^`]+` \| (action|toggle|query|job|ui|event) \|',line):
        cells=[s.strip().replace(r'\|','|') for s in re.split(r'(?<!\\)\|',line)[1:-1]]
        appendix[cells[0].strip('`')]={'kind':cells[1],'params':cells[3],'result':cells[4],'undo':cells[5],'line':n}
assert len(runtime)==len(appendix)==796 and {r['id'] for r in runtime}==set(appendix)
assert {r['id'] for r in prior_commands}==set(appendix)
owners={}
for n,line in enumerate((ROOT/'plan/IMPLEMENTATION_PLAN.md').read_text(encoding='utf-8').splitlines(),1):
    m=re.match(r'\| (M\d+) \|',line)
    if m:
        for c in re.findall(r'\bC\d+\.\d+\b',line):
            owners[f'{m[1]} {c}']={'source':f'plan/IMPLEMENTATION_PLAN.md:{n}','text':line}
sources={}
for p in [*(ROOT/'crates/executor/src/commands').glob('*.rs'),ROOT/'crates/registry/src/declared.rs']:
    for n,line in enumerate(p.read_text(encoding='utf-8').splitlines(),1):
        m=re.search(r'\bid(?: =|:) "([^"]+)"',line)
        if m: sources[m[1]]=f'{p.relative_to(ROOT).as_posix()}:{n}'

def nodes(s,path=''):
    if not isinstance(s,dict): return
    yield path,s
    for key in ['properties','$defs','patternProperties']:
        for k,x in s.get(key,{}).items(): yield from nodes(x,path+'/'+key+'/'+k)
    for key in ['items','additionalProperties','x-jobResult','not','if','then','else']:
        if isinstance(s.get(key),dict): yield from nodes(s[key],path+'/'+key)
    for key in ['anyOf','oneOf','allOf','prefixItems']:
        for n,x in enumerate(s.get(key,[])): yield from nodes(x,path+'/'+key+'/'+str(n))

audit=[]; owner_rows=[]; register=[]
new_findings={'doc.diff':['C12-R4-01'],'export.publish.service.list':['C12-R4-02'],'storyline.play.changed':['C12-R4-02']}
for row in runtime:
    cid=row['id']; a=appendix[cid]
    assert a['kind']==row['kind'] and row['spec']=={k:a[k] for k in ['params','result','undo']}
    holes=[]; refs=[]; opens=[]
    for part in ['params','result']:
        schema=row[part+'Schema']; Validator.check_schema(schema)
        for path,s in nodes(schema):
            # R2's established hole criterion deliberately excludes conditional constraints and not schemas.
            hole_path=not re.search(r'/(?:if|then|else|not|additionalProperties|patternProperties)(?:/|$)',path)
            if hole_path and (not any(k in s for k in ['type','$ref','enum','const','anyOf','oneOf','allOf','not','required']) or 'x-specType' in s):
                holes.append(part+path)
            if s.get('type')=='object' and 'additionalProperties' not in s:
                opens.append({'path':part+path,'explicitOpenBinding':s.get('x-open',False)})
            if '$ref' in s:
                ref=s['$ref']; target=schema
                try:
                    assert ref.startswith('#/')
                    for token in ref[2:].split('/'): target=target[token.replace('~1','/').replace('~0','~')]
                except (AssertionError,KeyError): refs.append(part+path+': '+ref)
    examples=[Validator(row['paramsSchema']).is_valid(e['params']) for e in row['examples']]
    assert all(examples) and not refs, cid
    if cid in generated: assert not holes, (cid,holes)
    audit.append({'id':cid,'generated':cid in generated,'holes':holes,'unresolvedRefs':refs,'openObjects':opens,'validExamples':len(examples)})
    status='implemented' if row['status']=='implemented' else 'deferred-with-owner'
    owner=row.get('milestone') or 'M1 C1.2'
    if row['status']=='notImplemented':
        assert owner in owners,(cid,owner)
        owner_rows.append({'id':cid,'owner':owner,'plan':owners[owner]['source']})
    finding=new_findings.get(cid,[])
    if finding: status='divergent'
    register.append({'id':cid,'kind':row['kind'],'status':status,'declaredStatus':row['status'],'owner':owner,
                     'spec':f"spec/A_command_registry.md:{a['line']}",'source':sources[cid],'findings':finding,
                     'evidence':'schema-closure.json; logs/closure-probes.log; logs/new-regressions.log; rust/gates/test.log; ci-audit.json'})

old_by_id={r['id']:r for r in prior_commands}
closed_commands=[]
for r in register:
    prior=old_by_id[r['id']]
    if prior['status']=='divergent':
        closed_commands.append({'id':r['id'],'before':'divergent','after':r['status'],'owner':r['owner'],
                                'priorFindings':prior['findings'],'currentFindings':r['findings'],
                                'evidence':'original14/fresh14 in schema-closure.json' if 'C12-R3-01' in prior['findings'] else
                                           'c12_r3_contracts; closure-probe.rs; c12_r4_closure.rs'})
assert len(closed_commands)==17
requirements=[]; closed_requirements=[]
divergent={'R02':['C12-R4-02'],'R12':['C12-R4-01'],'R37':['C12-R4-01']}
for oldrow in prior_requirements:
    r=copy.deepcopy(oldrow); rid=r['id']; r['priorStatus']=oldrow['status']; r['findings']=divergent.get(rid,[])
    if oldrow['status']=='deferred-with-owner':
        # Existing accepted deferrals are preserved, including compound specialty ownership.
        assert r.get('owner')
        checkpoints=re.findall(r'C\d+\.\d+',r['owner'])
        assert checkpoints and all(any(o.endswith(' '+c) for o in owners) for c in checkpoints)
    else:
        r['status']='divergent' if r['findings'] else 'implemented'
        r['note']='Original R3 counterexample passes; scoped new regression remains.' if r['findings'] else 'Retained suite and scoped closure evidence pass; accepted rulings unchanged.'
        r['tests']='rust/gates/test.log; ci-audit.json; logs/closure-probes.log; schema-closure.json; logs/new-regressions.log'
    # Resolve old source references by the original anchored source label, independently of builder registers.
    r['implementationAtR3']=r['implementation']
    path,line=r['implementation'].rsplit(':',1)
    oldlines=subprocess.check_output(['git','show','be5ef32:'+path],cwd=ROOT).decode('utf-8').splitlines()
    newlines=(ROOT/path).read_text(encoding='utf-8').splitlines()
    needle=oldlines[int(line)-1].strip()
    matches=[n for n,l in enumerate(newlines,1) if l.strip()==needle]
    if matches: r['implementation']=f'{path}:{min(matches,key=lambda n:abs(n-int(line)))}'
    else: r['implementation']=path
    requirements.append(r)
    if oldrow['status']=='divergent':
        closed_requirements.append({'id':rid,'requirement':r['requirement'],'before':'divergent','after':r['status'],'findings':r['findings'],'evidence':r['tests']})
assert len(closed_requirements)==8 and len(requirements)==72
def counts(rs):
    c=Counter(r['status'] for r in rs)
    return {k:c[k] for k in ['implemented','divergent','missing','deferred-with-owner']}
summary={'requirements':counts(requirements),'commands':counts(register),'totalRequirements':72,'totalCommands':796,
         'declaredRuntime':dict(Counter(r['status'] for r in runtime)),
         'previousDivergentCommands':{'closed':sum(r['after']!='divergent' for r in closed_commands),'stillDivergent':sum(r['after']=='divergent' for r in closed_commands)},
         'previousDivergentRequirements':{'closed':sum(r['after']!='divergent' for r in closed_requirements),'stillDivergent':sum(r['after']=='divergent' for r in closed_requirements)},
         'newDivergentCommandRows':[r['id'] for r in register if r['findings'] and old_by_id[r['id']]['status']!='divergent'],
         'schemas':{'generated':len(generated),'generatedHoleRows':sum(r['generated'] and bool(r['holes']) for r in audit),
                    'unresolvedRefs':sum(len(r['unresolvedRefs']) for r in audit),'examples':sum(r['validExamples'] for r in audit)},
         'qualification':'Scoped reconciliation of accepted R3 register and fix-induced regressions, not a fourth exhaustive review or a proof of all possible values.'}
save('command-register.json',register); save('requirements.json',requirements)
save('register-closures.json',{'commands':closed_commands,'requirements':closed_requirements})
save('register-counts.json',summary); save('schema-structure.json',audit)
save('owner-audit.json',{'rows':owner_rows,'checked':len(owner_rows),'ownerCounts':dict(Counter(r['owner'] for r in owner_rows)),'planOwners':owners})
print(json.dumps(summary,indent=2))
