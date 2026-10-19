"""Close only row 388; preserve every other row and accepted deferral verbatim."""
from pathlib import Path
from collections import Counter
import copy
import json

HERE = Path(__file__).resolve().parent
prior = json.loads((HERE.parent/'M1-C1.1-r6/conformance.json').read_text(encoding='utf-8'))
rows = copy.deepcopy(prior['rows'])
row = rows[387]
assert row['row'] == 388 and row['status'] == 'divergent'
row['r6_status'], row['r6_note'] = row['status'], row['note']
row['status'] = 'implemented'
row['implementation'] = ('crates/model/src/document/mod.rs:456-508; '
                         'crates/model/src/change.rs:341-345,568-576')
row['note'] = ('C11-R6-01 closed: undo is derived from the committed canonical entry after the full '
               'staged integrity gate. Both unchanged R6 regressions, fresh merge/split/append/nested '
               'and mouth-length probes, and two fresh 10000-case builder properties pass. '
               'Canonical undo/redo snapshots are byte-identical. The self-check falls back to a '
               'whole-entry replace without panicking. No open P0/P1; earlier rulings retained.')
row['r7_tests'] = dict(
    regressions='cargo test -p tt-model --test c11_r6_staged_inverse --locked -- --nocapture',
    probes='python reviews/evidence/M1-C1.1-r7/run_probes.py probe',
    properties='python reviews/evidence/M1-C1.1-r7/run_probes.py properties',
    result='2/2 R6 regressions; 4/4 evidence probes; 10000 cases in each builder property')
assert all(a == b for a, b in zip(rows, prior['rows']) if a['row'] != 388)
counts = dict(Counter(r['status'] for r in rows))
for status in ['missing', 'divergent']:
    counts.setdefault(status, 0)
assert counts == dict(implemented=235, divergent=0, missing=0, deferred=193)
out = dict(base='0159179', counts=counts, rows=rows,
           scope='Only row 388 reevaluated; other 427 rows and all 193 deferral owners unchanged.')
for name, data in [('conformance.json', out), ('register-closure.json', [row])]:
    (HERE/name).write_text(json.dumps(data, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
print(json.dumps(dict(requirements=len(rows), counts=counts, changed_rows=[388]), indent=2))
