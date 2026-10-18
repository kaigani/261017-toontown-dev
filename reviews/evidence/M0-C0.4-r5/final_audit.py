"""Audit gate receipts and preserved review evidence without hiding the P1 failure."""
import gzip
import hashlib
import json
import pathlib
import subprocess

OUT = pathlib.Path(__file__).resolve().parent
ROOT = OUT.parents[2]


def content(name):
    p = OUT/name
    return p.read_bytes() if p.exists() else gzip.decompress((OUT/(name+'.gz')).read_bytes())


required = ['lint', 'typecheck', 'test', 'format-check', 'licenses-check',
            'frontend-build-diagnostics', 'frontend-build-default', 'fmt', 'clippy', 'rust-tests',
            'clippy-diagnostics', 'rust-tests-diagnostics', 'deny', 'headless-build',
            'headless-selftest-1', 'headless-selftest-2', 'headless-guard', 'raster',
            'tauri-build-default', 'tauri-build-diagnostics', 'binary-gating-default',
            'binary-gating-diagnostics', 'independent-gating-default', 'independent-gating-diagnostics',
            'test_decision_rules', 'c02-r1-tooling', 'c02-r2-tooling', 'c02-r3-tooling',
            'c03-r1-tooling', 'c03-r2-tooling', 'c04-r2-decisions', 'c04-r3-decisions', 'c04-r3-page',
            'closure-probes', 'page-probe', 'copy-probe', 'rebreak', 'c04-r5-decisions']
receipts = []
for name in required:
    receipt = json.loads((OUT/(name+'.receipt.json')).read_bytes())
    assert receipt['exit'] == (1 if name == 'c04-r5-decisions' else 0), (name, receipt)
    content(name+'.log')  # Some successful gates (for example rustfmt) are silent.
    receipts.append(dict(name=name, **receipt))
headless = [json.loads(content(f'headless-selftest-{i}.log')) for i in (1, 2)]
pairs = [[(c['id'], c['ok']) for c in x['checks']] for x in headless]
assert pairs[0] == pairs[1] and all(ok for _, ok in pairs[0])
raster = json.loads(content('raster.json'))
assert raster['repeat'] == 3 and not raster['checkMismatches'] and not raster['repeatFailures']
goldens = json.loads((ROOT/'crates/render/examples/c03_raster/hashes.json').read_bytes())
assert raster['hashes'] == goldens
golden_records = sum(len(v) for v in goldens['renderers'].values())
assert golden_records == 234
default = json.loads((OUT/'independent-gating-default.json').read_bytes())
diag = json.loads((OUT/'independent-gating-diagnostics.json').read_bytes())
assert default['sha256'] != diag['sha256']
assert len(default['launches']) == 6 and all(x['exit'] == 1 for x in default['launches'])
assert diag['launches'][0]['exit'] == 0
ci = json.loads((OUT/'ci-audit.json').read_bytes())
assert ci['head'] == '99d1a6bdf9008c81477cc0e441a52bda612e47d1'
assert len(ci['jobs']) == 8
for j in ci['jobs']:
    if j['name'].startswith('check '):
        assert len(j['frontendSuites']) == 34 and len(j['rustTestTargets']) == 24
changes = subprocess.check_output(['git', 'diff', '99d1a6b', '--name-only'], cwd=ROOT, text=True).splitlines()
allowed = lambda p: (p == 'reviews/M0-C0.4-r5-closure.md' or p == 'tests/adversarial/c04-r5-decisions.py'
                     or p.startswith('reviews/evidence/M0-C0.4-r5/'))
assert all(allowed(p) for p in changes), changes
assert not subprocess.check_output(['git', 'diff', '906e8e3..99d1a6b', '--',
                                   'tests/adversarial', 'desktop/src/test'], cwd=ROOT)
report = (ROOT/'reviews/M0-C0.4-r5-closure.md').read_text(encoding='utf-8')
bullets = [x for x in report.split('## Final M1 readiness list')[1].splitlines() if x.startswith('- ')]
old = (ROOT/'reviews/M0-C0.4-r4-closure.md').read_text(encoding='utf-8')
old_bullets = [x for x in old.split('## Final M1 readiness list')[1].splitlines() if x.startswith('- ')]
assert len(bullets) == 15 and bullets[1:] == old_bullets[1:]
assert report.startswith('**Verdict: block')
result = dict(reviewedHead=ci['head'], receipts=receipts, headlessPairs=pairs[0],
              rasterRepeat=3, rasterMismatches=0, rasterRepeatFailures=0, goldenRecords=golden_records,
              failingRegression='c04-r5-decisions: required invariant fails in three subtests; exit 1',
              openCounts=dict(P0=0, P1=1, P2=1), changedPaths=changes,
              builderReviewerTestDiffEmpty=True, readinessBullets=len(bullets))
(OUT/'final-audit.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
print(f'{len(receipts)} receipts audited; ordinary gates pass; P1 regression remains a real exit-1 failure.')
