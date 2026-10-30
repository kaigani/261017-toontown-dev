"""Compare every pre-existing Rust test by test name and executable stem across completed sweep logs."""
from pathlib import Path
import collections
import json
import re

HERE = Path(__file__).resolve().parent
def read(p):
    raw = p.read_bytes()
    return raw.decode('utf-16' if raw.startswith(b'\xff\xfe') else 'utf-8', errors='replace')
def cases(text):
    current = None
    out = {}
    for line in text.splitlines():
        m = re.search(r'Running .*[/\\]([^/\\ ()]+)-[0-9a-f]+\.exe\)', line)
        if m:
            current = m[1]
        if line.lstrip().startswith('Doc-tests '):
            current = 'doc:' + line.split('Doc-tests ', 1)[1]
        m = re.match(r'test (.+?) \.\.\. (ok|FAILED|ignored)', line)
        if m and current and not m[1].startswith(('tests\\', 'tests/')):
            out[current + '::' + m[1]] = m[2]
    return out

baseline = cases(read(HERE.parent / 'M1-C1.4/gates/test.log'))
logs = ['workspace-resumed.log', 'workspace-rest.log', 'executor-remaining.log',
        'replay-final.log', 'effects-contracts-final.log']
actual = {}
for name in logs:
    actual.update(cases(read(HERE / name)))
missing = sorted(set(baseline) - set(actual))
regressions = {k: {'baseline': v, 'now': actual.get(k)} for k, v in baseline.items()
               if k in actual and actual[k] != v}
new = {k: v for k, v in actual.items() if k not in baseline}
report = {'logs': logs, 'baselineUniqueCases': len(baseline), 'actualUniqueCases': len(actual),
          'baselineCounts': dict(collections.Counter(baseline.values())),
          'actualCounts': dict(collections.Counter(actual.values())),
          'missingBaselineCases': missing, 'changedBaselineCases': regressions,
          'newCases': new,
          'note': 'Counts are unique case lines, not sums of Cargo summaries or repeat runs. Ignored tickets are intentionally not executed here.'}
(HERE / 'sweep-summary.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
print(json.dumps(report, indent=2))
