"""Recompute review hygiene, gate totals and CI receipts; run from any directory."""
from pathlib import Path
import collections, csv, difflib, hashlib, json, re, subprocess
ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
def git(*args):
    return subprocess.check_output(['git',*args],cwd=ROOT).decode('utf8')
def read(p):
    b=Path(p).read_bytes()
    return b.decode('utf-16' if b.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig')
def totals(text):
    matches=re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;',text)
    return list(map(sum,zip(*(map(int,m) for m in matches)))) if matches else []
paths=git('ls-tree','-r','--name-only','9729db1').splitlines()
reviewer=[p for p in paths if p.startswith('tests/adversarial/') or re.search(r'/tests/(?:c\d+_r\d|c21_r1_)',p)]
changed=[p for p in reviewer if git('diff','9729db1','3d6dfa3','--',p)]
assert changed==['crates/executor/tests/c21_r1_contracts.rs'],changed
reviewer_diff=git('diff','9729db1','3d6dfa3','--',*reviewer)
(HERE/'reviewer-paths.diff').write_text(reviewer_diff,encoding='utf8')
products=[]; prohibited=[]
for p in git('diff','--name-only','9729db1','3d6dfa3').splitlines():
    if not p.endswith('.rs') or '/src/' not in p:continue
    products.append(p)
    a=git('show','9729db1:'+p).split('#[cfg(test)]')[0]
    b=git('show','3d6dfa3:'+p).split('#[cfg(test)]')[0]
    for l in difflib.unified_diff(a.splitlines(),b.splitlines(),n=0):
        if l.startswith('+') and not l.startswith('+++') and re.search(r'\b(?:unwrap|expect)\s*\(|\bpanic!\s*\(|\bunsafe\s*(?:\{|fn|impl)|#\s*\[\s*allow',l):prohibited.append([p,l])
assert not prohibited,prohibited
ci=[]
for run,head in [(41172384155,'07ae968'),(41175287616,'3d6dfa3'),(41163880282,'57e0b70')]:
    d=json.loads(read(HERE/f'ci-{run}.json'))
    assert d['headSha'].startswith(head)
    jobs=[{'name':j['name'],'conclusion':j['conclusion'],'url':j['url']} for j in d['jobs']]
    if run!=41163880282: assert d['conclusion']=='success' and all(j['conclusion']=='success' for j in jobs)
    ci.append({'run':run,'head':d['headSha'],'conclusion':d['conclusion'],'jobs':jobs})
gate=ROOT/'reviews/evidence/M2-C2.1/workspace-tests.log'
assert totals(read(gate))==[802,0,9]
ci_log_summary={}
for name in ['ci-fix.log','ci-receipt.log']:
    lines=read(HERE/name).splitlines()
    keep=[l for l in lines if any(x in l for x in ['c21_determinism','c21_r1_', 'c21_worst_case','c21_history','input_needing_more_work','golden digest','SDK conformance']) or ('test result:' in l and any(x in l for x in ['headless release','headless without']))]
    (HERE/name.replace('.log','-excerpts.txt')).write_text('\n'.join(keep)+'\n',encoding='utf8')
    ci_log_summary[name]={'sha256':hashlib.sha256((HERE/name).read_bytes()).hexdigest(),'excerpts':len(keep)}
out={'builderHead':git('rev-parse','3d6dfa3').strip(),'reviewerPaths':len(reviewer),'changedReviewerPaths':changed,'reviewerDiffRuled':True,'productRustFiles':products,'prohibitedAdditions':prohibited,'denyTomlUnchanged':not git('diff','9729db1','3d6dfa3','--','deny.toml'),'builderWorkspaceTotals':totals(read(gate)),'localWorkspaceTotals':totals(read(HERE/'workspace-tests.log')),'ci':ci,'ciLogSummary':ci_log_summary}
trace=list(csv.reader((ROOT/'plan/traceability.csv').read_text(encoding='utf8').splitlines()))
review=(ROOT/'reviews/M2-C2.1-r1.md').read_text(encoding='utf8')
for ticket in ['C21-R1-11','C21-R1-12']:
    wording=re.search(r'\*\*'+ticket+r'[^\n]+?\*\* (.+)',review).group(1)
    row=next(r for r in trace if r[0].startswith(ticket))
    assert row[1]==wording,(ticket,row[1],wording)
out['ticketWordingExact']=True
out['ticketOwnersInPlan']='plan/IMPLEMENTATION_PLAN.md:169 (M2 C2.3/C2.4); M4 C4.2 and M11 C11.1 qualifications retained'
gold='crates/executor/tests/golden/c21-determinism.json'
old=json.loads(git('show','9729db1:'+gold));new=json.loads((ROOT/gold).read_text(encoding='utf8'))
def walk(v):
    if isinstance(v,dict):
        if v.get('type')=='stroke':yield v
        for x in v.values():yield from walk(x)
    elif isinstance(v,list):
        for x in v:yield from walk(x)
a={v['id']:v for v in walk(old['drawing1'])};b={v['id']:v for v in walk(new['drawing1'])}
out['golden']={'oldSha256':old['sha256'],'newSha256':new['sha256'],'oldCanonicalBytes':old['bytes'],'newCanonicalBytes':new['bytes'],'sameDrawingStrokeIds':set(a)==set(b),'changedStrokes':[
 {'id':k,'kind':b[k]['kind'],'oldOutlineAnchors':len(a[k]['points']),'newOutlineAnchors':len(b[k]['points']),'changedFields':[f for f in b[k] if a[k].get(f)!=b[k][f]]} for k in sorted(a.keys() & b.keys()) if a[k]!=b[k]]}
(HERE/'audit.json').write_text(json.dumps(out,indent=2)+'\n',encoding='utf8')
print(json.dumps({k:v for k,v in out.items() if k not in ['ci','productRustFiles']},indent=2))
