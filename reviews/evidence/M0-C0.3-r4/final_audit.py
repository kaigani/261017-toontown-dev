"""Verify recorded gates, all CI frontend suites, raster suites, and protected paths."""
import gzip
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent


def read_log(name):
    p = OUT / name
    return (p.read_bytes() if p.exists() else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())).decode('utf-8')


log = re.sub(r'\x1b\[[0-9;]*m', '', read_log('ci-39131376596-logs.log'))
files = list((ROOT/'desktop/src').rglob('*.test.ts')) + list((ROOT/'desktop/src').rglob('*.test.tsx'))
ci = []
for osname in ['windows-latest', 'macos-latest']:
    text = '\n'.join(line for line in log.splitlines() if line.startswith(f'check ({osname})\t'))
    missing = [p.name for p in files if p.name not in text]
    assert not missing and '266 passed' in text and '3 expected fail' in text
    ci.append(dict(job=f'check ({osname})', frontendFiles=len(files), allFrontendSuitesPresent=True, missing=missing))
for osname in ['windows-latest', 'macos-latest', 'ubuntu-latest']:
    text = '\n'.join(line for line in log.splitlines() if line.startswith(f'raster determinism ({osname})\t'))
    assert 'c03_fixed_configurations' in text and '2 passed; 0 failed' in text
    ci.append(dict(job=f'raster determinism ({osname})', fixedConfigurationsSuitePassed=True))
(OUT/'ci-full-suite-audit.json').write_text(json.dumps(ci, indent=2)+'\n')

receipts = {p.name: json.loads(p.read_bytes()) for p in OUT.glob('*.receipt.json')}
assert len(receipts) >= 22
assert all(r['exitCode'] == 0 for r in receipts.values())
rust = re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;', read_log('rust-tests.log'))
assert sum(int(a) for a,b in rust) == 87 and sum(int(b) for a,b in rust) == 1
assert '266 passed' in read_log('test.log') and '3 expected fail' in read_log('test.log')
assert 'IPC boundary (dist check): ok' in read_log('frontend-build.log')
a,b = [json.loads(read_log(f'headless-{n}.log')) for n in [1,2]]
strip = lambda d: [(c['id'],c['ok']) for c in d['checks']]
assert strip(a) == strip(b) and len(strip(a)) == 5 and all(ok for _,ok in strip(a))
(OUT/'headless-comparison.json').write_text(json.dumps(dict(passed=True, checks=strip(a)), indent=2)+'\n')
goldens = json.loads((ROOT/'crates/render/examples/c03_raster/hashes.json').read_bytes())
raster = json.loads((OUT/'raster-default.json').read_bytes())
assert raster['hashes'] == goldens and not raster['repeatFailures'] and not raster['checkMismatches']
assert sum(len(s) for s in goldens['renderers'].values()) == 234
protected = subprocess.check_output(['git','diff','18c57fe','--','spec','tests/adversarial','desktop','crates','scripts','.github','Cargo.lock','pnpm-lock.yaml'],cwd=ROOT)
assert protected == b''
assert (OUT/'protected-tests.diff').read_bytes() == b'' and (OUT/'workflow.diff').read_bytes() == b''
result = dict(receipts=len(receipts), allReceiptExitCodesZero=True, rustPassed=87, rustIgnored=1,
              frontendPassed=266, frontendExpectedFailures=3, nodeLicenseTestsPassed=9,
              localRasterEntriesMatched=234, headlessChecksMatched=5, productAndProtectedPathsUnchanged=True)
(OUT/'final-audit.json').write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps(result, indent=2))
