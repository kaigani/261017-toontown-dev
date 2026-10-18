"""Compact, reproducible review checks and CI suite receipts."""
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).resolve().parent
os.chdir(ROOT)

def command(name,args,env=None):
    start=time.time()
    r=subprocess.run(args,capture_output=True,env=env)
    (OUT/(name+'.log')).write_bytes(r.stdout+r.stderr)
    (OUT/(name+'.receipt.json')).write_text(json.dumps(dict(command=args,exitCode=r.returncode,seconds=time.time()-start),indent=2)+'\n')
    return r

command('reviewer-tests-diff',['git','diff','3ee0207..5e8c7b7','--','tests/adversarial','desktop/src/test','crates/render/tests'])
command('response-diff',['git','diff','3ee0207..5e8c7b7','--','.',':!reviews',':!crates/render/examples/c03_raster/goldens',':!crates/render/examples/c03_raster/hashes.json'])
command('r1-tooling',['python','-I','tests/adversarial/c03-r1-tooling.py'])
command('analyzer-attacks',['python','-I','tests/adversarial/c03-r2-tooling.py'])
env={**os.environ,'CARGO_MANIFEST_DIR':str(ROOT/'desktop/src-tauri')}
command('encoder-compile',['rustc','--edition=2024','--test','tests/adversarial/c03-r2-encoder.rs','-o','target/review-r2/encoder-tests.exe'],env)
command('encoder-attacks',[str(ROOT/'target/review-r2/encoder-tests.exe')])
command('final-fmt',['cargo','fmt','--all','--check'])
paths=['reviews/evidence/M0-C0.3/raster-r1-report-local-windows.json']+[f'reviews/evidence/M0-C0.3/raster-r1-report-ci-{system}-39126106552.json' for system in ['Linux','Windows','macOS']]
comparison=command('raster-comparison',['python','scripts/compare-raster-reports.py','crates/render/examples/c03_raster/hashes.json',*paths],{**os.environ,'PYTHONIOENCODING':'utf-8'})
assert comparison.stdout.decode().replace('\r\n','\n')==(ROOT/'reviews/evidence/M0-C0.3/raster-r1-comparison.md').read_text(encoding='utf-8')
old=json.loads(subprocess.check_output(['git','show','3ee0207:crates/render/examples/c03_raster/hashes.json']))['renderers']
new=json.loads((ROOT/'crates/render/examples/c03_raster/hashes.json').read_text())['renderers']
assert all(new[r][s]==h for r,ss in old.items() for s,h in ss.items())
ci=[]
for run in [39126106552,39126796883]:
    path=OUT/f'ci-{run}.json'; raw=path.read_bytes()
    data=json.loads(raw.decode('utf-16' if raw[:2]==b'\xff\xfe' else 'utf-8-sig'))
    path.write_text(json.dumps(data,indent=2)+'\n')
    logpath=OUT/f'ci-{run}-logs.log'
    log=(logpath.read_bytes() if logpath.exists() else gzip.decompress(logpath.with_suffix('.log.gz').read_bytes())).decode('utf-8')
    # Exact named tests in check jobs; Linux uses -q, its successful two-test suite is recorded below.
    selected=[]
    for line in log.splitlines():
        if any(x in line for x in ['fixed_scalar_configuration_hashes_identically_across_oses ...','c03_r1_04_simd_dispatch','c03_r1_05_source_over','u8_pipeline_loses_opacity','test result: ok. 2 passed','cargo test --release --locked -q -p tt-render --test c03_fixed_configurations','234/234']):
            selected.append(re.sub(r'\x1b\[[0-9;]*m','',line))
    (OUT/f'ci-{run}-test-excerpts.txt').write_text('\n'.join(selected)+'\n',encoding='utf-8')
    ci.append(dict(run=run,sha=data['headSha'],conclusion=data['conclusion'],jobs=[dict(name=j['name'],conclusion=j['conclusion'],steps=[dict(name=s['name'],conclusion=s['conclusion']) for s in j['steps']]) for j in data['jobs']]))
heads=[json.loads((OUT/f'headless-{n}.log').read_text()) for n in [1,2]]
stable=lambda v:[(x['id'],x['ok']) for x in v['checks']]
assert stable(heads[0])==stable(heads[1]) and all(ok for _,ok in stable(heads[0]))
summary=dict(originalHashEntriesUnchanged=sum(map(len,old.values())),comparisonReproduces=True,headless=stable(heads[0]),ci=ci)
(OUT/'final-audit.json').write_text(json.dumps(summary,indent=2)+'\n')
print('audit complete: original 92 hashes unchanged; comparison exact; headless twice identical; CI steps extracted')
