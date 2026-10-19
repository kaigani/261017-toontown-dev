"""Verify gate receipts, fixed-seed property, regression failures, scope and final counts."""
from pathlib import Path
import datetime
import hashlib
import json
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
BASE = '9c3f5861bebd00f4d68ebee43a35167452cea615'

def read(name):
    return (HERE / 'logs' / (name + '.log')).read_text(encoding='utf-8')

def totals(name):
    matches = re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored', read(name))
    return [sum(int(x[i]) for x in matches) for i in range(3)]

receipts = {}
for path in sorted((HERE / 'receipts').glob('*.json')):
    r = json.loads(path.read_text(encoding='utf-8'))
    assert r['base'] == BASE
    assert hashlib.sha256((HERE / r['log']).read_bytes()).hexdigest() == r['sha256'], path
    receipts[path.stem] = dict(exit_code=r['exit_code'], command=r['command'], sha256=r['sha256'])
failed = {name: r['exit_code'] for name, r in receipts.items() if r['exit_code']}
assert failed == {'final-workspace': 101, 'new-regressions': 101, 'node-guards': 1}, failed
for name in ['install', 'r4-regressions', 'fmt', 'clippy', 'workspace', 'clippy-diagnostics', 'test-diagnostics', 'deny',
             'pnpm-lint', 'pnpm-typecheck', 'pnpm-test', 'pnpm-format-check', 'pnpm-licenses-check',
             'frontend-build-diagnostics', 'frontend-build', 'tauri-release', 'diagnostics-gate', 'headless-build',
             'selftest-1', 'selftest-2', 'headless-guard', 'raster', 'reviewer-python', 'review-policy',
             'node-guards-licensing', 'node-guards-allowlist', 'node-guards-imports', 'random-recorded', 'random-fresh',
             'rustdoc', 'bounds-probe', 'ci-metadata', 'ci-full', 'test-edit-diff', 'static-audit', 'register-audit',
             'final-fmt', 'final-clippy']:
    assert receipts[name]['exit_code'] == 0, name
assert totals('workspace') == [281, 0, 1]
assert totals('final-workspace') == [281, 4, 1]
assert totals('new-regressions') == [0, 4, 0]
assert totals('r4-regressions') == [2, 0, 0]
assert totals('test-diagnostics') == [49, 0, 0]
assert totals('bounds-probe') == [6, 0, 0]
for name in ['random-recorded', 'random-fresh']:
    assert 'successful cases=10000' in read(name) and totals(name) == [1, 0, 0]
assert '202610180311' in read('random-recorded')
assert (HERE / 'fresh-seed.txt').read_text().strip() in read('random-fresh')
assert '326 passed' in read('pnpm-test') and '# pass 9' in read('pnpm-test')
ts_results = json.loads((HERE / 'reviewer-ts-results.json').read_text(encoding='utf-8'))
assert ts_results['numPassedTests'] == 19 and ts_results['numFailedTests'] == 0
assert receipts['reviewer-ts']['exit_code'] == 0
local_suite_counts = {}
for round_number in [1, 2, 3]:
    count = 0
    for suffix, crate in [('adversarial', 'model'), ('wire', 'engine')]:
        file = ROOT / f'crates/{crate}/tests/c11_r{round_number}_{suffix}.rs'
        names = re.findall(r'#\[test\]\s*fn\s+(\w+)', file.read_text(encoding='utf-8'))
        for name in names:
            assert f'test {name} ... ok' in read('final-workspace'), name
        count += len(names)
    file = f'/src/ipc/c11-r{round_number}-adversarial.test.ts'
    result = next(r for r in ts_results['testResults'] if r['name'].replace('\\', '/').endswith(file))
    assert result['status'] == 'passed' and all(a['status'] == 'passed' for a in result['assertionResults'])
    count += len(result['assertionResults'])
    local_suite_counts['R' + str(round_number)] = count
local_suite_counts['R2'] += 2  # Policy suite, separately logged below.
assert re.search(r'Ran 2 tests', read('review-policy')) and '\nOK\n' in read('review-policy')
for suffix in ['register', 'random']:
    source = (ROOT / f'crates/model/tests/c11_r3_{suffix}.rs').read_text(encoding='utf-8')
    names = re.findall(r'#\[test\]\s*fn\s+(\w+)', source)
    for name in names:
        assert f'test {name} ... ok' in read('final-workspace')
    local_suite_counts['R3'] += len(names)
local_suite_counts['R4'] = totals('r4-regressions')[0]
assert local_suite_counts == dict(R1=49, R2=56, R3=22, R4=2)
assert '11/11 reviewer Python suites passed' in read('reviewer-python')
selftests = [json.loads(read('selftest-' + str(n))) for n in [1, 2]]
checks = [[(c['id'], c['ok']) for c in s['checks']] for s in selftests]
assert checks[0] == checks[1] and len(checks[0]) == 5 and all(ok for _, ok in checks[0])
assert all(s['ok'] for s in selftests)
docs = ROOT / 'target/doc/tt_model/change'
assert not (docs / 'fn.invert.html').exists()
assert all((docs / ('fn.' + n + '.html')).exists() for n in ['invert_against', 'apply_with_inverse'])
assert not re.search(r'href="[^"]*fn\.invert\.html', (docs / 'index.html').read_text(encoding='utf-8'))
assert not (ROOT / 'crates/model/tests/c11_r5_bounds_probe.rs').exists()
register = json.loads((HERE / 'conformance.json').read_text(encoding='utf-8'))
assert register['counts'] == dict(implemented=234, divergent=1, deferred=193, missing=0)
assert read('test-edit-diff') == ''
changed = subprocess.check_output(['git', 'diff', '--name-only', BASE], cwd=ROOT, text=True).splitlines()
status = subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=all'], cwd=ROOT, text=True)
allowed = lambda p: p.startswith('reviews/evidence/M1-C1.1-r5/') or p in ['reviews/M1-C1.1-r5-closure.md', 'crates/model/tests/c11_r5_write_bounds.rs']
assert all(allowed(p) for p in changed), changed
assert all(allowed(line[3:]) for line in status.splitlines()), status
audit = dict(base=BASE, audited_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
             verdict='block', open_findings=dict(P0=0, P1=1, P2=0),
             finding='C11-R5-01 continues C11-R4-01; counted once', register_counts=register['counts'],
             workspace_baseline=totals('workspace'), workspace_final=totals('final-workspace'),
             prior_review_suites=local_suite_counts,
             selftests_identical_check_results=checks[0],
             public_stateless_invert_absent_from_rustdoc=True,
             nonzero_receipts=failed, invocation_error='node-guards: corrected case invocations all pass; see launch-note.txt',
             receipts=receipts)
(HERE / 'audit.json').write_text(json.dumps(audit, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps({k: v for k, v in audit.items() if k != 'receipts'}, indent=2))
