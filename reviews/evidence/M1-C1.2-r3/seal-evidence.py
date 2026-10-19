"""Verify all command receipts and summarize final gates; seal review artifacts by hash."""
from collections import Counter
import gzip
import hashlib
import json
import re
import subprocess
from pathlib import Path

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
def load(p): return json.loads(p.read_text(encoding='utf-8'))
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def write(name,x): (HERE/name).write_text(json.dumps(x,indent=2)+'\n',encoding='utf-8')
receipts=[]
for p in sorted((HERE/'receipts').glob('*.json')):
    r=load(p)
    assert sha(HERE/r['log'])==r['sha256'], p
    receipts.append({'receipt':str(p.relative_to(HERE)),'exitCode':r['exitCode']})
for area in ['baseline','frontend','final','current']:
    r=load(HERE/area/'receipt.json')
    for name,h in r['sha256'].items(): assert sha(HERE/area/name)==h,(area,name)

def tests(path):
    text=path.read_text(encoding='utf-8')
    items=re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored',text)
    suites=[]; target='unit/doc'
    for line in text.splitlines():
        m=re.search(r'Running (?:tests[\\/])?([^ ]+) \(',line)
        if m: target=m[1]
        m=re.search(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored',line)
        if m: suites.append({'target':target,'passed':int(m[1]),'failed':int(m[2]),'ignored':int(m[3])})
    return {'totals':dict(zip(['passed','failed','ignored'],[sum(int(r[i]) for r in items) for i in range(3)])), 'suites':suites}
summary={a:{'status':load(HERE/a/'receipt.json')['status']} for a in ['baseline','frontend','final','current']}
for a in ['baseline','final','current']: summary[a]['rust']=tests(HERE/a/'gates/test.log')
assert summary['baseline']['rust']['totals']=={'passed':461,'failed':0,'ignored':1}
assert summary['current']['rust']['totals']=={'passed':478,'failed':15,'ignored':1}
assert {s['target'] for s in summary['current']['rust']['suites'] if s['failed']}=={'c12_r3_contracts.rs','c12_r3_registry.rs'}
write('gate-summary.json',summary)
ci=gzip.decompress((HERE/'ci-full.log.gz').read_bytes()).decode('utf-8')
lines=[re.sub(r'\x1b\[[0-9;]*m','',l) for l in ci.splitlines()]
proof=[l for l in lines if re.search(r'333 passed|tests 9\b|69 passed|46 passed|11/11 reviewer|c11-r[123].*test|C01-R',l)]
write('ci-frontend-proof.json',proof)
assert any('333 passed' in l for l in proof)
# Inventory only tracked/unignored deliverables, not Python caches or transient editor files.
all_paths=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','--','crates/executor/tests/c12_r3*','reviews/M1-C1.2-r3.md','reviews/evidence/M1-C1.2-r3'],cwd=ROOT).decode().splitlines()
manifest={}
for relative in sorted(set(all_paths)):
    p=ROOT/relative
    if p.is_file() and p.name!='evidence-sha256.json': manifest[relative]={'bytes':p.stat().st_size,'sha256':sha(p)}
write('evidence-sha256.json',{'reviewedProductCommit':'2e0e5dd5feaf47199f5c15595b187a34db3e5c5e',
    'receiptsVerified':len(receipts),'receiptResults':receipts,'files':manifest})
print(json.dumps({'currentTests':summary['current']['rust']['totals'],'receiptsVerified':len(receipts),
    'sealedFiles':len(manifest),'bytes':sum(x['bytes'] for x in manifest.values())},indent=2))
