"""Preserve 428 stable rows and all accepted deferrals; reevaluate only row 388."""
from pathlib import Path
from collections import Counter
import copy
import json

HERE = Path(__file__).resolve().parent
prior = json.loads((HERE.parent / 'M1-C1.1-r5/conformance.json').read_text(encoding='utf-8'))
rows = copy.deepcopy(prior['rows'])
row = rows[387]
assert row['row'] == 388
row['r5_status'], row['r5_note'] = row['status'], row['note']
row['status'] = 'divergent'
row['implementation'] = 'crates/model/src/document/mod.rs:456-480; crates/model/src/change.rs:589-674'
row['note'] = ('C11-R5-01 closed: staged frame post-conditions and full Document::issues() gate verified. '
               'C11-R6-01 is a new fix regression: with_patched_entry returns the pre-canonicalization inverse. '
               'Exposure run merging makes it fail or silently restore wrong frames. One open P1; accepted earlier rulings remain closed.')
row['r6_tests'] = dict(file='crates/model/tests/c11_r6_staged_inverse.rs',
    command='cargo test -p tt-model --test c11_r6_staged_inverse --locked -- --nocapture',
    result='0 passed / 2 failed; no ignore or expected-fail')
assert all(a == b for a, b in zip(rows, prior['rows']) if a['row'] != 388)
counts = dict(Counter(r['status'] for r in rows))
counts.setdefault('missing', 0)
assert counts == dict(implemented=234, divergent=1, deferred=193, missing=0)
out = dict(base='fb31cfe', counts=counts, rows=rows,
    scope='Only row 388 reevaluated. The other 427 rows and all 193 deferral owners are unchanged.')
(HERE/'conformance.json').write_text(json.dumps(out, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
(HERE/'register-closure.json').write_text(json.dumps([row], ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
print(json.dumps(dict(requirements=len(rows), counts=counts, changed_rows=[388]), indent=2))
