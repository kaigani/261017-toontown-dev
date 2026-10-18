"""Verify the retained receipts and review scope without rerunning expensive gates."""
import gzip
import hashlib
import json
import pathlib
import re
import subprocess

OUT=pathlib.Path(__file__).resolve().parent
ROOT=OUT.parents[2]
def cmd(args):
    return subprocess.check_output(args,cwd=ROOT).decode('utf-8',errors='replace').strip()
def read(name):
    return json.loads((OUT/name).read_text(encoding='utf-8'))

receipts={p.stem:json.loads(p.read_text()) for p in OUT.glob('*.receipt.json')}
assert receipts and all(r['exit']==0 for r in receipts.values())
selftests=[read(f'headless-selftest-{i}.log') for i in (1,2)]
pairs=[[(c['id'],c['ok']) for c in r['checks']] for r in selftests]
assert pairs[0]==pairs[1] and len(pairs[0])==5 and all(ok for _,ok in pairs[0])
raster=read('raster.json')
golden=json.loads((ROOT/'crates/render/examples/c03_raster/hashes.json').read_text())
assert raster['hashes']==golden and not raster['checkMismatches'] and not raster['repeatFailures']
assert sum(map(len,golden['renderers'].values()))==234
ci=read('ci-audit.json')
local_files=set(re.findall(r'src/[^\s\x1b]+\.test\.[jt]sx?',gzip.decompress((OUT/'ci-39134580275.log.gz').read_bytes()).decode('utf-8')))
for run,data in ci.items():
    assert len(data['jobs'])==8 and all(j['conclusion']=='success' for j in data['jobs'].values())
    for name,job in data['jobs'].items():
        if name.startswith('check'):
            assert len(job['frontend_test_files'])==31
            assert set(job['frontend_test_files'])==local_files
protected=cmd(['git','diff','47cee1e','--','spec','plan','docs','desktop','crates','bins','Cargo.lock','pnpm-lock.yaml','.github','tests'])
assert not protected
for mode in ('default','diagnostics'):
    probe=read(f'independent-gating-{mode}.json')
    assert bool(probe['binary_counts']['spike_canvas_scene'])==(mode=='diagnostics')
    assert bool(probe['binary_counts']['bench_report'])==(mode=='diagnostics')
assert read('ipc-bench.json')['frontend']['ok']
report=(ROOT/'reviews/M0-C0.4-r1.md').read_text(encoding='utf-8')
findings=re.findall(r'^### (C04-R1-\d+) — (P\d):',report,re.M)
assert len(findings)==8 and sum(p=='P1' for _,p in findings)==5
tools={name:cmd(args) for name,args in {
    'rustc':['rustc','--version'],'cargo-deny':['cargo','deny','--version'],
    'node':['node','--version'],'pnpm':['cmd','/d','/c','pnpm --version'],
    'python':['python','--version']}.items()}
diff=subprocess.check_output(['git','diff','8bfe6cf..47cee1e'],cwd=ROOT)
(OUT/'reviewed.diff.gz').write_bytes(gzip.compress(diff,mtime=0))
result={'reviewed':'47cee1e392bda2aa4356bf60a6293317ecd6da1b',
    'branch':cmd(['git','branch','--show-current']), 'tools':tools,
    'gate_receipts':{n:r['exit'] for n,r in receipts.items()},
    'selftest_pairs':pairs[0], 'raster_entries':234,'raster_equal_committed':True,
    'ci_jobs_per_run':{r:len(v['jobs']) for r,v in ci.items()},
    'ci_frontend_files_same_31':True,'protected_paths_changed':False,
    'finding_counts':{'P0':0,'P1':5,'P2':3},'reviewed_diff_sha256':hashlib.sha256(diff).hexdigest()}
(OUT/'final-audit.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
print(json.dumps(result,indent=2))
