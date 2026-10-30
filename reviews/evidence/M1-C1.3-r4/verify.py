"""Recompute scoped review evidence; run from repository root."""
from pathlib import Path
import hashlib
import json
import re
import subprocess

ROOT = Path('reviews/evidence/M1-C1.3-r4')
def read(path):
    data = path.read_bytes()
    return data.decode('utf-16') if data.startswith(b'\xff\xfe') else data.decode('utf-8-sig')

expected = dict(c13_r1_persistence=18, c13_r1_format=8,
                c13_r2_persistence=14, c13_r2_format=7,
                c13_r3_persistence=10, c13_r3_format=3,
                c13_r3_history_r1=1, c13_r3_history_r3=1)
totals, seen, current = {}, {}, {}
for line in read(ROOT/'ci.log').splitlines():
    line = re.sub(r'(?:\x1b|\^\[)\[[0-9;]*m', '', line)
    parts = line.split('\t', 2)
    if len(parts) < 3 or parts[1] != 'Run cargo test --workspace --locked':
        continue
    job = parts[0]
    match = re.search(r'Running tests[\\/](c13_r[^ .]+)\.rs', line)
    if match:
        current[job] = match[1]
    match = re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', line)
    if match:
        nums = list(map(int, match.groups()))
        totals[job] = [a+b for a,b in zip(totals.get(job,[0,0,0]), nums)]
        if job in current:
            seen.setdefault(job,{})[current.pop(job)] = nums
assert len(totals) == 2
assert len(seen) == 2, seen
assert all(all(v.get(k) == [n,0,0] for k,n in expected.items()) for v in seen.values()), seen
receipt = json.loads(Path('reviews/evidence/M1-C1.3/receipt.json').read_text())
bad = [k for k,v in receipt['sha256'].items()
       if hashlib.sha256((Path('reviews/evidence/M1-C1.3')/k).read_bytes()).hexdigest() != v]
assert not bad, bad
changed = subprocess.check_output(['git','diff','--name-only','3d76c40'],text=True).splitlines()
protected = [p for p in changed if
    re.search(r'c13_r[123]_',p) or
    re.search(r'reviews/(?:evidence/)?M[01]-C(?:0\.|1\.[12])',p) or
    re.search(r'reviews/(?:evidence/)?M1-C1\.3-r[123](?:/|\.md$)',p) or
    p in ('Cargo.lock','deny.toml')]
# Ruling 4 expressly authorizes this inherited test's message/comment change.
assert not protected, protected
changed_tests = [p for p in changed if '/tests/' in p and 'c13_r4_' not in p]
assert set(changed_tests) == {
    'crates/executor/tests/c12_kernel_state.rs',
    'crates/executor/tests/c13_doc.rs',
    'crates/executor/tests/c13_projects.rs'}, changed_tests
ci = json.loads(read(ROOT/'ci.json'))
assert ci['headSha'] == '205cd97d7b88fc588848c89d77a8cf64ed688edf'
assert ci['conclusion'] == 'success'
gate_summaries = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',
                            read(Path('reviews/evidence/M1-C1.3/gates/test.log')))
gate_totals = [sum(int(n[i]) for n in gate_summaries) for i in range(3)]
assert gate_totals == [628,0,2], gate_totals
result = dict(ciTotals=totals, reviewerSuites=seen,
              receiptHashes=len(receipt['sha256']), receiptAllMatch=True,
              receiptStatuses=receipt['status'], builderWorkspaceTotals=gate_totals,
              protectedDiff=protected, changedExistingTests=changed_tests)
(ROOT/'verification.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
