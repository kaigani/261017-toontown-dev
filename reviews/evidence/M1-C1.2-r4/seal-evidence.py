"""Validate closure evidence and seal committed review artifacts with SHA-256 hashes."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
def load(p): return json.loads(p.read_text(encoding='utf-8'))
def totals(path):
    text=path.read_text(encoding='utf-8')
    rows=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored',text)
    return {k:sum(int(r[i]) for r in rows) for k,i in [('passed',1),('failed',2),('ignored',3)]}
for p in (HERE/'receipts').glob('*.json'):
    r=load(p)
    assert hashlib.sha256((HERE/r['log']).read_bytes()).hexdigest()==r['sha256'],p
for section in ['frontend','rust','final']:
    receipt=load(HERE/section/'receipt.json')
    assert all(code==0 for code in receipt['status'].values()),section
    for name,sha in receipt['sha256'].items():
        assert hashlib.sha256((HERE/section/name).read_bytes()).hexdigest()==sha,(section,name)
base=totals(HERE/'rust/gates/test.log')
diag=totals(HERE/'rust/gates/test-diag.log')
new=totals(HERE/'logs/new-regressions.log')
assert base=={'passed':496,'failed':0,'ignored':1},base
assert diag=={'passed':49,'failed':0,'ignored':0},diag
assert new=={'passed':0,'failed':4,'ignored':0},new
assert load(HERE/'diff-filter-matrix.json')['cases']==400
w=load(HERE/'wait16.json')
assert len(w['followers'])==15 and len(w['cached'])==32
assert all(x==w['owner'] for x in w['followers']+w['cached'])
schemas=load(HERE/'schema-closure.json')
assert all(c['acceptedNow'] for c in schemas['cases'] if c['group'] in ['original14','fresh14'])
assert {c['id'] for c in schemas['cases'] if not c['acceptedNow']}=={'export.publish.service.list','storyline.play.changed'}
assert all(c['acceptedBefore'] for c in schemas['cases'] if not c['acceptedNow'])
assert all(x['valid'] for x in schemas['specExamples']) and len(schemas['specExamples'])==85
assert len(schemas['placeholders'])==86 and all(x['documented'] for x in schemas['placeholders'])
counts=load(HERE/'register-counts.json')
assert counts['requirements']=={'implemented':62,'divergent':3,'missing':0,'deferred-with-owner':7}
assert counts['commands']=={'implemented':68,'divergent':3,'missing':0,'deferred-with-owner':725}
ci=load(HERE/'ci-audit.json')
assert ci['head']=='f2100570e636292028e70bc6a32ab444c37547ae'
assert len(ci['jobs'])==8 and all(x['conclusion']=='success' for x in ci['jobs'])
report=ROOT/'reviews/M1-C1.2-r4-closure.md'
text=report.read_text(encoding='utf-8')
assert 'Verdict: BLOCK' in text and not re.search(r'Pending|TODO|TBD',text)
carry=text.split('Paste the following owner-labelled list verbatim',1)[1]
assert len(re.findall(r'^- ',carry,re.M))<=12
# Every local report link resolves; anchors are permitted, external URLs are skipped.
for target in re.findall(r'\]\(([^)]+)\)',text):
    if not target.startswith('https://'):
        assert (report.parent/target.split('#')[0]).exists(),target
tracked=subprocess.check_output(['git','diff','--name-only'],cwd=ROOT,text=True).splitlines()
assert not tracked,tracked
summary={'reviewedCommit':ci['head'],'verdict':'BLOCK','newFindings':{'P0':0,'P1':2,'P2':0},
         'baselineWorkspace':base,'diagnostics':diag,'newRegressionTests':new,
         'r3Tests':32,'filterCases':400,'identicalWaiters':16,'cacheReplays':32,
         'registers':counts,'scope':'Original product and inherited tests unchanged; four new genuine P1 regression assertions.'}
(HERE/'verification-summary.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
files=[p for p in HERE.rglob('*') if p.is_file() and '__pycache__' not in p.parts and p.name!='sha256-manifest.json']
files += [report,ROOT/'crates/executor/tests/c12_r4_closure.rs']
manifest={p.relative_to(ROOT).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(files)}
(HERE/'sha256-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'sealedFiles':len(manifest),'verdict':'BLOCK','newP1':2,'workspace':base,'newTests':new,'registers':counts},indent=2))
