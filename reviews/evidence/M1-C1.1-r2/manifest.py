"""Hash all delivered review files; --check verifies rather than regenerates."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
DEST=HERE/'manifest.json'
paths=[p for p in HERE.rglob('*') if p.is_file() and p!=DEST and '__pycache__' not in p.parts]
paths += [ROOT/p for p in [
    'reviews/M1-C1.1-r2.md', 'crates/model/tests/c11_r2_adversarial.rs',
    'crates/engine/tests/c11_r2_wire.rs','desktop/src/ipc/c11-r2-adversarial.test.ts',
    'tests/adversarial/c11-r2-review-policy.py']]
entries=[{'path':p.relative_to(ROOT).as_posix(),'bytes':p.stat().st_size,
          'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in sorted(paths)]
if '--check' in sys.argv or '--staged' in sys.argv:
    assert json.loads(DEST.read_text(encoding='utf-8'))==entries
    print('Verified',len(entries),'review files')
    if '--staged' in sys.argv:
        staged=set(subprocess.check_output(['git','diff','--cached','--name-only'],cwd=ROOT,text=True).splitlines())
        assert staged=={e['path'] for e in entries}|{DEST.relative_to(ROOT).as_posix()}
        for e in entries:
            content=subprocess.check_output(['git','show',':'+e['path']],cwd=ROOT)
            assert hashlib.sha256(content).hexdigest()==e['sha256'],e['path']
        print('All staged blobs match the manifest; only authorized review paths are staged.')
else:
    DEST.write_text(json.dumps(entries,indent=2)+'\n',encoding='utf-8')
    print('Recorded',len(entries),'review files')
