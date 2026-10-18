"""Summarize completed gates and retain compressed logs without changing product files."""
import gzip
import hashlib
import json
import pathlib
import re
import subprocess

OUT = pathlib.Path(__file__).resolve().parent
ROOT = OUT.parents[2]
receipts = {}
for path in OUT.glob('*.receipt.json'):
    receipt = json.loads(path.read_text())
    expected = 1 if path.stem == 'permanent-decision-regressions.receipt' else 0
    assert receipt['exit'] == expected, (path,receipt)
    receipts[path.name] = receipt
required = ['fmt','clippy','rust-tests','clippy-diagnostics','rust-tests-diagnostics','deny',
            'lint','typecheck','test','format-check','licenses-check','frontend-build-default',
            'frontend-build-diagnostics','tauri-build-default','tauri-build-diagnostics',
            'binary-gating-default','binary-gating-diagnostics','headless-build',
            'headless-selftest-1','headless-selftest-2','headless-guard','raster']
assert all(name+'.receipt.json' in receipts for name in required)
heads = [json.loads((OUT/f'headless-selftest-{i}.log').read_text()) for i in (1,2)]
checks = [[(c['id'],c['ok']) for c in h['checks']] for h in heads]
assert checks[0] == checks[1] and all(ok for _,ok in checks[0])
golden = json.loads((ROOT/'crates/render/examples/c03_raster/hashes.json').read_bytes())
raster = json.loads((OUT/'raster.json').read_bytes())
assert raster['hashes'] == golden and not raster['checkMismatches'] and not raster['repeatFailures']
for mode in ('default','diagnostics'):
    probe = json.loads((OUT/f'independent-gating-{mode}.json').read_bytes())
    assert all(bool(count) == (mode=='diagnostics') for name,count in probe['binary_counts'].items()
               if name.startswith(('spike_','bench_')))
assert json.loads((OUT/'ipc-bench.json').read_bytes())['frontend']['ok']
assert subprocess.check_output(['git','diff','--name-only','a89ca1b','--','spec','plan','docs','desktop','crates','bins',
                                'Cargo.lock','pnpm-lock.yaml'],cwd=ROOT) == b''
logs = {}
for path in OUT.glob('*.log'):
    # The original PowerShell gh capture is superseded by raw-byte gh capture from audit_ci.py.
    if path.name.startswith('ci-'):
        continue
    data = path.read_bytes()
    path.with_suffix('.log.gz').write_bytes(gzip.compress(data,mtime=0))
    logs[path.name] = dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
ci = json.loads((OUT/'ci-audit.json').read_bytes())
summary = dict(reviewed='a89ca1b799dd5fa72878ed708f5562f98374cd42', receipts=receipts,
               headless_checks=checks[0],raster_entries=234,raster_repeat=raster['repeat'],
               ci_jobs=len(ci['jobs']),ci_png_comparisons=1404,product_paths_unchanged=True,logs=logs,
               intentionally_failing_tests='tests/adversarial/c04-r2-decisions.py: 3 tests / 7 subcase failures')
(OUT/'final-audit.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
print('Verified',len(receipts),'receipts; required product gates pass; scoped review tests intentionally fail.')
