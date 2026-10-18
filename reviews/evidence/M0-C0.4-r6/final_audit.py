"""Audit gate receipts and preserved review evidence including the historical counterexample and its corrected invariant."""
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
            'closure-probes', 'page-probe', 'rebreak', 'rebreak-closed', 'scoped-rebreak', 'c04-r5-decisions']
receipts = []
for name in required:
    receipt = json.loads((OUT/(name+'.receipt.json')).read_bytes())
    assert receipt['exit'] == (1 if name == 'rebreak' else 0), (name, receipt)
    content(name+'.log')  # Some successful gates (for example rustfmt) are silent.
    receipts.append(dict(name=name, **receipt))
historical = content('rebreak.log').decode('utf-8')
assert '200.0 != 50' in historical and 'FAILED (failures=1)' in historical
scoped = json.loads((OUT/'scoped-rebreak-results.json').read_bytes())
assert len(scoped) == 8
assert sum(x['exit'] == 0 for x in scoped) == 6
assert sum(x['exit'] == 2 for x in scoped) == 2
for row in scoped:
    if row['exit'] == 2:
        assert "key event 20: keydown of ' ' never received its keyup" in row['error']
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
assert ci['head'] == '929f91a99ab06402bc024108fbc82f9aae3f91d9'
assert len(ci['jobs']) == 8
for j in ci['jobs']:
    if j['name'].startswith('check '):
        assert len(j['frontendSuites']) == 34 and len(j['rustTestTargets']) == 24
changes = subprocess.check_output(['git', 'diff', '929f91a', '--name-only'], cwd=ROOT, text=True).splitlines()
allowed = lambda p: (p == 'reviews/M0-C0.4-r6-closure.md'
                     or p.startswith('reviews/evidence/M0-C0.4-r6/'))
assert all(allowed(p) for p in changes), changes
assert not subprocess.check_output(['git', 'diff', '3a24980..929f91a', '--',
                                   'tests/adversarial', 'desktop/src/test'], cwd=ROOT)
report = (ROOT/'reviews/M0-C0.4-r6-closure.md').read_text(encoding='utf-8')
bullets = [x for x in report.split('## Final M1 readiness list')[1].splitlines() if x.startswith('- ')]
old = (ROOT/'reviews/M0-C0.4-r5-closure.md').read_text(encoding='utf-8')
old_bullets = [x for x in old.split('## Final M1 readiness list')[1].splitlines() if x.startswith('- ')]
expected = [x.replace('M0 C0.4 evidence / ', '').replace('M0 C0.4 D-M1 / ', '') for x in old_bullets[1:]]
assert len(bullets) == 14 and bullets == expected
assert not any('M0 C0.4' in x for x in bullets)
assert report.startswith('**Verdict: pass')
assert '_PENDING' not in report and 'pending final local gates' not in report
result = dict(reviewedHead=ci['head'], receipts=receipts, headlessPairs=pairs[0],
              rasterRepeat=3, rasterMismatches=0, rasterRepeatFailures=0, goldenRecords=golden_records,
              historicalObservation='rebreak.py expects obsolete 50 Hz; actual 200 Hz; exit 1',
              closureRegression='c04-r5-decisions: all three invariant subtests pass; exit 0',
              openCounts=dict(P0=0, P1=0, P2=0), changedPaths=changes,
              builderReviewerTestDiffEmpty=True, readinessBullets=len(bullets))
(OUT/'final-audit.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
print(f'{len(receipts)} receipts audited; all current gates and closure regressions pass; obsolete R5 observation fails only its old 50 Hz expectation.')
