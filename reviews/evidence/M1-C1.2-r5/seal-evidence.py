"""Check report links, preserved carryover, allowed file scope and evidence hashes before committing."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
report = ROOT / 'reviews/M1-C1.2-r5-closure.md'
content = report.read_text(encoding='utf-8')
links = re.findall(r'\]\(([^)]+)\)', content)
for link in links:
    if not link.startswith('http') and (report.parent / link) != HERE / 'sha256-manifest.json':
        assert (report.parent / link).exists(), link
carryover = (HERE / 'c13-carryover.txt').read_text(encoding='utf-8')
assert len([s for s in carryover.splitlines() if s.startswith('- **')]) == 9
assert content.endswith(carryover)
subprocess.run(['python', str(HERE / 'summarize.py')], cwd=ROOT, check=True)
changes = subprocess.check_output(['git', 'diff', '--name-only', 'ec4329c'], cwd=ROOT, text=True).splitlines()
assert all(p == 'reviews/M1-C1.2-r5-closure.md' or p.startswith('reviews/evidence/M1-C1.2-r5/') for p in changes)
files = {p.relative_to(HERE).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
         for p in sorted(HERE.rglob('*'))
         if p.is_file() and p.name != 'sha256-manifest.json' and '__pycache__' not in p.parts and p.suffix != '.pyc'}
manifest = {'algorithm': 'sha256', 'reviewedHead': 'ec4329cb1abb47821ada701175730a3e691d41d7',
            'report': {'path': report.relative_to(ROOT).as_posix(), 'sha256': hashlib.sha256(report.read_bytes()).hexdigest()},
            'fileCount': len(files), 'files': files}
(HERE / 'sha256-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
assert all(link.startswith('http') or (report.parent / link).exists() for link in links)
print(f'Sealed {len(files)} evidence files; report links and nine carryover items verified.')
