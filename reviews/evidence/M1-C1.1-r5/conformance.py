"""Carry all 428 stable rows and deferral owners forward; only row 388 is re-evaluated."""
from pathlib import Path
from collections import Counter
import copy
import json

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
prior = json.loads((HERE.parent / 'M1-C1.1-r4/conformance.json').read_text(encoding='utf-8'))
rows = copy.deepcopy(prior['rows'])
row = rows[387]
assert row['row'] == 388
row['r4_status'], row['r4_note'] = row['status'], row['note']
row['status'] = 'divergent'
row['implementation'] = 'crates/model/src/change.rs:767'
row['note'] = ('R4 range totals/windows and whole exposure add/replace checks are fixed; both R4 regressions pass. '
               'C11-R5-01 continues C11-R4-01: whole mouth strings, nested exposure counts, append batches and '
               'copy/move destinations still bypass the 100,000-frame patch-boundary limit. One P1, not double-counted.')
row['r5_test'] = dict(file='crates/model/tests/c11_r5_write_bounds.rs',
                      command='cargo test -p tt-model --test c11_r5_write_bounds --locked -- --nocapture',
                      result='0 passed / 4 failed; no ignore or expected-fail')
assert all(a == b for a, b in zip(rows, prior['rows']) if a['row'] != 388)
counts = dict(Counter(r['status'] for r in rows))
counts.setdefault('missing', 0)
assert len(rows) == 428 and counts == dict(implemented=234, divergent=1, deferred=193, missing=0)
out = dict(base='9c3f5861bebd00f4d68ebee43a35167452cea615', counts=counts, rows=rows,
           scope='Only row 388 re-evaluated. Other 427 rows, including all 193 deferrals and their owners, unchanged.')
(HERE / 'conformance.json').write_text(json.dumps(out, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
(HERE / 'register-closure.json').write_text(json.dumps([row], ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps(dict(requirements=len(rows), counts=counts, changed_rows=[388]), indent=2))
