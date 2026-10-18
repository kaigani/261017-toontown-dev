"""Audit gate receipts, selftests, raster hashes, CI suites and permitted review changes."""
import gzip
import hashlib
import json
import pathlib
import re
import subprocess

OUT=pathlib.Path(__file__).resolve().parent
ROOT=OUT.parents[2]
receipts={p.name:json.loads(p.read_text()) for p in OUT.glob('*.receipt.json')}
for name,r in receipts.items():
    assert r['exit']==(1 if name in ('r3-decisions.receipt.json','r3-page.receipt.json') else 0),(name,r)
required=['fmt','clippy','rust-tests','clippy-diagnostics','rust-tests-diagnostics','deny','lint','typecheck','test',
          'format-check','licenses-check','frontend-build-default','frontend-build-diagnostics','tauri-build-default',
          'tauri-build-diagnostics','binary-gating-default','binary-gating-diagnostics','headless-build',
          'headless-selftest-1','headless-selftest-2','headless-guard','raster']
assert all(n+'.receipt.json' in receipts for n in required)
def log_bytes(name):
    p=OUT/name
    return p.read_bytes() if p.exists() else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())

heads=[json.loads(log_bytes(f'headless-selftest-{i}.log')) for i in (1,2)]
checks=[[(c['id'],c['ok']) for c in h['checks']] for h in heads]
assert checks[0]==checks[1] and all(ok for _,ok in checks[0])
raster=json.loads((OUT/'raster.json').read_bytes())
assert raster['hashes']==json.loads((ROOT/'crates/render/examples/c03_raster/hashes.json').read_bytes())
assert not raster['checkMismatches'] and not raster['repeatFailures']
for mode in ('default','diagnostics'):
    p=json.loads((OUT/f'independent-gating-{mode}.json').read_bytes())
    assert all(bool(v)==(mode=='diagnostics') for k,v in p['binary_counts'].items() if k.startswith(('spike_','bench_')))
assert json.loads((OUT/'ipc-bench.json').read_bytes())['frontend']['ok']
assert subprocess.check_output(['git','diff','--name-only','47459da','--','spec','plan','docs','desktop','crates','bins',
                               'Cargo.lock','pnpm-lock.yaml'],cwd=ROOT)==b''
subprocess.run(['git','merge-base','--is-ancestor','b9b966b','HEAD'],cwd=ROOT,check=True)
assert subprocess.check_output(['git','show','b9b966b:plan/IMPLEMENTATION_PLAN.md'],cwd=ROOT).replace(b'\r\n',b'\n')==(ROOT/'plan/IMPLEMENTATION_PLAN.md').read_bytes().replace(b'\r\n',b'\n')
ci=gzip.decompress((OUT/'ci-39140679107.log.gz').read_bytes()).decode()
files={p.relative_to(ROOT/'desktop').as_posix() for p in (ROOT/'desktop/src').rglob('*') if re.search(r'\.test\.tsx?$',p.name)}
assert len(files)==34,len(files)
for os in ('windows','macos'):
    lines='\n'.join(x for x in ci.splitlines() if x.startswith('check ('+os+'-latest)\t'))
    assert all(f in lines for f in files), (os,files)
    assert '275 passed' in lines and '3 expected fail' in lines
logs={}
for name in sorted({p.name for p in OUT.glob('*.log')} | {p.name[:-3] for p in OUT.glob('*.log.gz')}):
    p=OUT/name
    data=log_bytes(name)
    if data.startswith(b'\xff\xfe'): data=data.decode('utf-16').encode()
    p.with_suffix('.log.gz').write_bytes(gzip.compress(data,mtime=0))
    logs[p.name]={'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
summary=dict(reviewed='47459da976a34963962c5dc7ca479273dac3273a',receipts=receipts,
             headlessChecks=checks[0],rasterEntries=234,rasterRepeat=raster['repeat'],ciJobs=8,
             frontendFilesPerOS=34,productPathsUnchanged=True,planSyncVerified=True,
             intendedFailures='R3 Python: three assertions; R3 page: two assertions',logs=logs)
(OUT/'final-audit.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
print('Verified',len(receipts),'receipts; product gates pass; R3 regressions remain failing; 34 CI suites per OS')
