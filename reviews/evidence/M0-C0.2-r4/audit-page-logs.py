"""Feed exact logs saved by the rendered page to the real analyzer CLI."""
import json
import gzip
import os
from pathlib import Path
import subprocess
import sys

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
results = []
for path in sorted(OUT.glob('*-page-log.json.gz')):
    expected = 2 if path.name.startswith(('auto-before-', 'auto-cleared-')) or '-pending-' in path.name else 0
    stem = path.name.removesuffix('.json.gz')
    destination = ROOT / 'target/review-r4' / (stem + '.analysis.json')
    r = subprocess.run([sys.executable, 'scripts/analyse-tablet-log.py', str(path), '--json', str(destination)],
                       cwd=ROOT, capture_output=True, text=True, encoding='utf-8', env=dict(os.environ,PYTHONIOENCODING='utf-8'))
    (OUT / (stem + '.txt')).write_text(r.stdout+r.stderr,encoding='utf-8')
    log=json.loads(gzip.decompress(path.read_bytes()))
    row=dict(file=path.name,source=log.get('source'),auto=log['environment']['mode']['auto'],
             runs=len(log['runs']),simulations=len(log['simulations']),samples=len(log['web']['samples']),
             exit=r.returncode,expected=expected,stdoutBytes=len(r.stdout.encode()),stderr=r.stderr.strip(),
             outcome=[s for s in r.stdout.splitlines() if 'outcome:' in s])
    row['pass']=r.returncode==expected and (expected==0 or not r.stdout)
    results.append(row)
    print(path.name, row['pass'], row['source'], r.returncode, row['stderr'] or row['outcome'])
(OUT / 'page-analysis.json').write_text(json.dumps(results,indent=2),encoding='utf-8')
assert len(results)==12 and all(r['pass'] for r in results)
