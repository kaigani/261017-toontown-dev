"""Verify closure receipts, test totals, exact-SHA CI and the unchanged review scope."""
from pathlib import Path
import hashlib
import json
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
EXPECTED = '3b6179f57bbdf65ac847787f34ef937f88035178'

def read(name):
    return (HERE / 'logs' / (name + '.log')).read_text(encoding='utf-8')

def counts(name):
    matches = [tuple(map(int, m)) for m in re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored', read(name))]
    assert matches, name
    return [sum(t[i] for t in matches) for i in range(3)]

receipts = []
for file in sorted((HERE/'receipts').glob('*.json')):
    receipt = json.loads(file.read_text(encoding='utf-8'))
    assert receipt['base'] == EXPECTED, file
    assert hashlib.sha256((HERE/receipt['log']).read_bytes()).hexdigest() == receipt['sha256'], file
    code = 101 if file.stem in ['new-regressions', 'final-workspace'] else 0
    assert receipt['exit_code'] == code, file
    receipts.append(dict(name=file.stem, exit_code=receipt['exit_code'], sha256=receipt['sha256']))
required = ['install', 'pnpm-lint', 'pnpm-typecheck', 'pnpm-test', 'pnpm-format-check', 'pnpm-licenses-check',
            'frontend-build-diagnostics', 'frontend-build', 'fmt', 'final-fmt', 'clippy', 'final-clippy',
            'workspace', 'final-workspace', 'new-regressions', 'clippy-diagnostics', 'test-diagnostics', 'deny',
            'tauri-release', 'diagnostics-gate', 'headless-build', 'selftest-1', 'selftest-2', 'headless-guard',
            'raster', 'random-recorded', 'random-fresh', 'rustdoc', 'review-policy', 'old-tooling',
            'closure-probe-final', 'ci-metadata', 'ci-full', 'test-edit-diff', 'reviewed-history', 'reviewed-diff']
assert set(required) <= {r['name'] for r in receipts}
rust = {name: counts(name) for name in ['workspace','final-workspace','new-regressions','test-diagnostics','closure-probe-final']}
assert rust == {'workspace':[278,0,1], 'final-workspace':[278,2,1], 'new-regressions':[0,2,0],
                'test-diagnostics':[49,0,0], 'closure-probe-final':[7,0,0]}, rust
assert 'c11_r4_closure_probe' not in read('final-workspace'), 'Evidence-only probe leaked into the permanent suite'
frontend = re.sub(r'\x1b\[[0-9;]*[a-zA-Z]', '', read('pnpm-test'))
assert re.search(r'Tests\s+326 passed', frontend) and '# pass 9' in frontend
assert 'Ran 2 tests' in read('review-policy') and 'OK' in read('review-policy')
assert 'Ran 12 tests' in read('old-tooling') and 'OK' in read('old-tooling')
seeds = {}
for name in ['recorded','fresh']:
    text = read('random-'+name)
    match = re.search(r'seed=Ok\("(\d+)"\); successful cases=10000; add/remove/replace/move/copy/test=(\[[^\]]+\])', text)
    assert match, name
    seeds[name] = dict(seed=match[1], cases=10000, operation_counts=json.loads(match[2]))
assert seeds['recorded']['seed']=='202610180311'
assert seeds['fresh']['seed'] != seeds['recorded']['seed']
selftests = [json.loads(read('selftest-'+str(n))) for n in [1,2]]
strip = lambda value: [(c['id'],c['ok']) for c in value['checks']]
assert all(v['ok'] for v in selftests)
assert strip(selftests[0]) == strip(selftests[1]) and len(strip(selftests[0]))==5
assert all(ok for _,ok in strip(selftests[0]))
assert 'headless dependency check passed' in read('headless-guard')
ci = json.loads((HERE/'ci-audit.json').read_text(encoding='utf-8'))
assert ci['head']==EXPECTED and len(ci['jobs'])==8 and all(j['conclusion']=='success' for j in ci['jobs'])
assert ci['policy_suite_in_ci'] is False
register = json.loads((HERE/'conformance.json').read_text(encoding='utf-8'))
assert register['counts']==dict(implemented=234,divergent=1,missing=0,deferred=193)
assert not (ROOT/'crates/model/tests/c11_r4_closure_probe.rs').exists()
tracked_changes = subprocess.check_output(['git','diff',EXPECTED,'--name-only'],cwd=ROOT,text=True).splitlines()
allowed = lambda f: f in ['crates/model/tests/c11_r4_exposure_bounds.rs','reviews/M1-C1.1-r4-closure.md'] or f.startswith('reviews/evidence/M1-C1.1-r4/')
assert all(allowed(f) for f in tracked_changes), tracked_changes
files = ['crates/model/tests/c11_r4_exposure_bounds.rs','reviews/M1-C1.1-r4-closure.md']
files += [str(p.relative_to(ROOT)).replace('\\','/') for p in HERE.rglob('*') if p.is_file() and p.name!='audit.json' and '__pycache__' not in str(p)]
hashes = {f:hashlib.sha256((ROOT/f).read_bytes()).hexdigest() for f in sorted(files)}
result = dict(reviewed_commit=EXPECTED, verdict='block', findings=dict(P0=0,P1=1,new_P2=0),
              final_register=register['counts'], receipts=receipts, rust_counts_pass_fail_ignore=rust,
              vitest=326,node=9,policy=2,old_tooling=12,reviewer_suites=dict(R1=49,R2=56,R3_previous_regressions=8),
              random=seeds,headless_checks=strip(selftests[0]),ci_jobs=8,ci_policy_omission='Existing R3 carryover',
              tracked_changes_within_authorized_scope=True,artifact_sha256=hashes)
(HERE/'audit.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in result.items() if k not in ['receipts','artifact_sha256']},indent=2))
print('All', len(receipts), 'command receipts and', len(hashes), 'artifact hashes verified.')
