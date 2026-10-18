"""Recompute review evidence; never trust a reported pass/count without raw input."""
from collections import Counter
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
def read(p): return Path(p).read_text(encoding="utf-8-sig")
def js(p): return json.loads(read(p))
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def rust(text):
    values=re.findall(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;",text)
    return dict(zip(['passed','failed','ignored'],[sum(int(v[i]) for v in values) for i in range(3)]))
receipts=[]
for p in sorted((HERE/'receipts').glob('*.json')):
    r=js(p)
    log=HERE/r['log'].replace('\\','/')
    assert sha(log)==r['sha256'], str(p)
    receipts.append({"name":p.stem,"exit_code":r['exit_code'],"command":r['command'],"sha256":r['sha256']})
tests={name:rust(read(HERE/'logs'/f'{name}.log')) for name in ['r2-initial-rust','final-workspace-verified','r1-model-closure','r1-wire-closure']}
tests['round1TypeScript']=re.findall(r"Tests\s+([^\n]+)",read(HERE/'logs/r1-ts-closure.log'))[-1].strip()
tests['typescriptBaseline']=re.findall(r"Tests\s+([^\n]+)",read(HERE/'logs/r2-initial-pnpm.log'))[-1].strip()
tests['typescriptFinal']=re.findall(r"Tests\s+([^\n]+)",read(HERE/'logs/final-pnpm-test.log'))[-1].strip()
ci=js(HERE/'logs/ci-summary.log')
assert ci['headSha']=='7c5405d6d4cb9016e80a214da6534cb236d01cf6'
assert len(ci['jobs'])==8 and all(j['conclusion']=='success' for j in ci['jobs'])
lines=read(HERE/'ci/full.log').splitlines()
ci_result=[]
for job in ci['jobs']:
    text='\n'.join(l for l in lines if l.startswith(job['name']+'\t'))
    item={"job":job['name'],"url":job['url'],"conclusion":job['conclusion'],
          "skippedSteps":[s['name'] for s in job['steps'] if s['conclusion']=='skipped']}
    if job['name'].startswith('check '):
        commands=['pnpm install --frozen-lockfile','pnpm lint','pnpm typecheck','pnpm test','pnpm format:check',
                  'pnpm licenses:check','pnpm --dir desktop build:diagnostics','pnpm --dir desktop build',
                  'cargo fmt --all --check','cargo clippy --workspace --all-targets --locked -- -D warnings',
                  'cargo test --workspace --locked','cargo deny --locked check','check-headless-deps.py',
                  'cargo test -p tt-desktop --locked --features m0-diagnostics']
        assert all(c in text for c in commands), job['name']
        workspace='\n'.join(l for l in lines if l.startswith(job['name']+'\tRun cargo test --workspace --locked\t'))
        item['workspace']=rust(workspace)
        assert item['workspace']=={'passed':209 if 'windows' in job['name'] else 203,'failed':0,'ignored':1}
        item['vitest']=re.findall(r"Tests\s+([^\n]+)",text)[-1].strip()
        assert '313 passed' in item['vitest']
        assert re.search(r"# pass 9",text)
        item['nodePassed']=9
        item['commandsVerified']=commands
    ci_result.append(item)
inventory=js(HERE/'ci/inventory.json')
preserved=0
for a in inventory:
    archive=HERE/'ci/archives'/f"{a['name']}.zip"
    if not archive.exists():
        cached=Path(a['archive'])
        assert sha(cached)==a['sha256']
        archive.parent.mkdir(exist_ok=True)
        archive.write_bytes(cached.read_bytes())
    assert sha(archive)==a['sha256']
    for e in a['entries']:
        if 'preserved' in e:
            assert sha(HERE/e['preserved'].replace('\\','/'))==e['sha256']
            preserved+=1
recomputed=HERE/'ci-recomputed'
recomputed.mkdir(exist_ok=True)
analyses=[]
for platform in ['Windows','macOS']:
    folder=HERE/'ci'/f'desktop-{platform}'
    tasks=[('canvas',f'canvas-spike-{platform}')]
    if platform=='Windows': tasks.append(('tablet','tablet-spike-windows'))
    for kind,stem in tasks:
        out=recomputed/f'{stem}.analysis.json'
        p=subprocess.run([sys.executable,str(folder/f'analyse-{kind}-log.py'),str(folder/f'{stem}.json'),'--json',str(out)],
                         cwd=ROOT,env=dict(os.environ,PYTHONIOENCODING='utf-8'),capture_output=True)
        (recomputed/f'{stem}.log').write_bytes(p.stdout+p.stderr)
        assert p.returncode==0, (stem,p.stderr)
        assert js(out)==js(folder/f'{stem}.analysis.json'), stem
        analyses.append(stem)
head=[js(HERE/'logs'/f'final-headless-{i}.log') for i in [1,2]]
checks=lambda r:[(c['id'],c['ok']) for c in r['checks']]
assert checks(head[0])==checks(head[1]) and all(ok for _,ok in checks(head[0]))
expected=js(ROOT/'crates/render/examples/c03_raster/hashes.json')['renderers']
raster=[]
for p in [HERE/'raster.json',*(HERE/'ci').glob('c03-raster-*/report-run*.json')]:
    r=js(p)
    assert not r['repeatFailures'] and not r['checkMismatches']
    # Recompute comparison from individual raw hashes rather than the summary.
    for renderer,scenes in expected.items():
        assert r['hashes']['renderers'][renderer]==scenes,(str(p),renderer)
    raster.append(str(p.relative_to(HERE)))
perf=[]
for p in [*(HERE.parent/'M1-C1.1-r1-response').glob('model-scale*.json'),*(HERE/'logs').glob('perf-scale-*.log')]:
    text=read(p)
    data=json.loads(text[text.index('{'):])
    perf.append({"source":str(p.relative_to(HERE.parent)),"sha256":sha(p),"measurements":data['measurements']})
reg=js(HERE/'conformance.json')
counts=dict(Counter(r['status'] for r in reg['rows']))
counts.setdefault('missing',0)
assert counts==reg['counts'] and len(reg['rows'])==428
findings=js(HERE/'findings.json')
priorities=dict(Counter(f['priority'] for f in findings))
priorities.setdefault('P0',0)
assert priorities=={'P0':0,'P1':3,'P2':8}
assert len(js(HERE/'tickets.json'))==8
for finding in findings:
    for name in finding['regressions']:
        assert any(name in read(ROOT/p) for p in [
            'crates/model/tests/c11_r2_adversarial.rs', 'crates/engine/tests/c11_r2_wire.rs',
            'desktop/src/ipc/c11-r2-adversarial.test.ts','tests/adversarial/c11-r2-review-policy.py']), name
assert (HERE/'rust-to-ts.json').read_bytes()==(HERE/'ts-to-rust.json').read_bytes()
out={"receiptCount":len(receipts),"receipts":receipts,"tests":tests,"ci":ci_result,
     "artifacts":{"archives":len(inventory),"bytes":sum(a['bytes'] for a in inventory),"rawExtractedFiles":preserved,"recomputedAnalyses":analyses},
     "headlessChecks":checks(head[0]),"rasterReportsComparedToCommittedHashes":raster,"performance":perf,"conformance":counts,"findings":priorities}
(HERE/'analysis.json').write_text(json.dumps(out,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in out.items() if k not in ['receipts','performance']},ensure_ascii=False,indent=2))
