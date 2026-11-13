"""Parse the single completed workspace invocation; preserve each binary/doc-test result."""
from pathlib import Path
import collections
import hashlib
import json
import re

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
raw = (OUT / 'workspace.txt').read_bytes()
text = raw.decode('utf-16' if raw[:2] == b'\xff\xfe' else 'utf-8-sig')
code = int((OUT / 'workspace.exit.txt').read_text())
records = []
raw_records = []
current = None
def finish():
    global current
    if current is not None:
        results = current.pop('harness_summaries')
        assert results, current
        records.append({**current, **results[-1], 'nested_harness_summaries': results[:-1]})
        current = None

for line in text.splitlines():
    match = re.search(r'Running\s+(.+?) \((.+)\)', line)
    if match:
        finish()
        current = {'kind': 'binary', 'name': match[1], 'executable': match[2], 'harness_summaries': []}
    match = re.search(r'Doc-tests\s+(\S+)', line)
    if match:
        finish()
        current = {'kind': 'doctest', 'name': match[1], 'harness_summaries': []}
    match = re.search(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out; finished in ([0-9.]+)s', line)
    if match:
        assert current is not None, line
        result = {'status': match[1], 'passed': int(match[2]), 'failed': int(match[3]), 'ignored': int(match[4]), 'measured': int(match[5]), 'filtered': int(match[6]), 'seconds': float(match[7])}
        current['harness_summaries'].append(result)
        raw_records.append(result)
finish()
summary = {'native_exit': code, 'result_summaries': len(records), 'raw_harness_summaries': len(raw_records), 'passed': sum(r['passed'] for r in records), 'failed': sum(r['failed'] for r in records), 'ignored': sum(r['ignored'] for r in records), 'filtered': sum(r['filtered'] for r in records), 'recorded_test_seconds': sum(r['seconds'] for r in records)}
summary['raw_harness_totals'] = {key: sum(r[key] for r in raw_records) for key in ['passed', 'failed', 'ignored', 'filtered']}
meta = json.loads((OUT / 'workspace-metadata.json').read_text())
summary['metadata'] = meta
summary['all_inherited_c32_targets_present'] = True
expected = collections.Counter()
for crate in ['executor', 'anim', 'model', 'mcp']:
    for path in (ROOT / 'crates' / crate / 'tests').glob('c32*.rs'):
        if re.match(r'c32_r[123]_|c32_round[123]|c32_passage$|c32_contract$', path.stem):
            expected[path.stem] += 1
observed = collections.Counter(Path(r['name'].replace('\\', '/')).stem for r in records if r['kind'] == 'binary')
summary['inherited_targets'] = dict(expected)
assert not (expected - observed), expected - observed
for path, digest in meta['test_sha256'].items():
    assert hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == digest
(OUT / 'workspace-binary-results.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')
(OUT / 'workspace-results.json').write_text(json.dumps(summary, indent=2) + '\n', encoding='utf-8')
assert code == 0 and summary['failed'] == 0 and summary['filtered'] == 0 and summary['raw_harness_totals']['failed'] == 0, summary
print(json.dumps({k: v for k, v in summary.items() if k not in ['metadata', 'inherited_targets']}, indent=2))
