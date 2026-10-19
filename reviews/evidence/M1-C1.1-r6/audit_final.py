"""Reconcile completed evidence, command receipts and allowed review-only changes."""
from pathlib import Path
import hashlib
import json
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
def read_log(name):
    return (HERE/'logs'/f'{name}.log').read_text(encoding='utf-8')
def counts(name):
    values = re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored', read_log(name))
    return [sum(int(v[i]) for v in values) for i in range(3)]
receipts = []
expected_failures = {'closure-probe-initial', 'closure-probe-second', 'new-regressions', 'new-regressions-final', 'final-workspace'}
for file in sorted((HERE/'receipts').glob('*.json')):
    receipt = json.loads(file.read_text(encoding='utf-8'))
    assert receipt['sha256'] == hashlib.sha256((HERE/receipt['log']).read_bytes()).hexdigest(), file
    assert bool(receipt['exit_code']) == (file.stem in expected_failures), file
    receipts.append(dict(name=file.stem, exit_code=receipt['exit_code'], seconds=receipt['elapsed_seconds']))
assert counts('workspace') == [288, 0, 1]
assert counts('final-workspace') == [288, 2, 1]
assert counts('new-regressions-final') == [0, 2, 0]
assert counts('test-diagnostics') == [49, 0, 0]
assert counts('closure-probe') == [6, 0, 0]
assert counts('staged-property-10000') == [1, 0, 0]
assert '326 passed' in read_log('pnpm-test') and '# pass 9' in read_log('pnpm-test')
assert '11/11 reviewer Python suites passed' in read_log('reviewer-python')
for name in ['random-recorded', 'random-r5-recorded', 'random-fresh']:
    assert 'successful cases=10000' in read_log(name)
hashes = json.loads((HERE/'rollback-hashes.json').read_text(encoding='utf-8'))
assert len(hashes) == 24 and all(r['identical'] and r['before_sha256'] == r['after_sha256'] for r in hashes)
a,b = [json.loads(read_log(f'selftest-{n}')) for n in [1,2]]
a,b = [[(c['id'], c['ok']) for c in value['checks']] for value in [a,b]]
assert a == b and len(a) == 5 and all(ok for _,ok in a)
status = subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],cwd=ROOT,text=True)
allowed = ('crates/model/tests/c11_r6_staged_inverse.rs', 'reviews/M1-C1.1-r6-closure.md', 'reviews/evidence/M1-C1.1-r6/')
for line in status.splitlines():
    assert line[3:].startswith(allowed), line
report = (ROOT/'reviews/M1-C1.1-r6-closure.md').read_text(encoding='utf-8')
assert 'PENDING' not in report
result = dict(reviewed_commit=subprocess.check_output(['git','rev-parse','fb31cfe'],cwd=ROOT,text=True).strip(),
    verdict='block', open_p0=0, open_p1=1, new_p2=0, closed=['C11-R5-01'], new_findings=['C11-R6-01'],
    baseline_rust=counts('workspace'), final_rust=counts('final-workspace'), rollback_hash_pairs=len(hashes),
    staged_property=json.loads((HERE/'staged-property.json').read_text(encoding='utf-8')),
    gates=receipts, changed_files=status.splitlines())
(HERE/'audit.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
print(json.dumps({k: v for k,v in result.items() if k not in ['gates','changed_files']}, indent=2))
