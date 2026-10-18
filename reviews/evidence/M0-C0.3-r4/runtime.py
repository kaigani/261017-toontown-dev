"""Real release IPC probe, with no security-dialog or input interaction."""
import json
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
command = [str(ROOT / 'target/release/toontown.exe'), '--ipc-bench', str(OUT / 'ipc-bench.json'),
           '--ipc-bench-count', '300']
start = time.time()
with (OUT / 'ipc-bench.log').open('wb') as f:
    p = subprocess.run(command, cwd=ROOT, stdout=f, stderr=subprocess.STDOUT, timeout=180)
report = json.loads((OUT / 'ipc-bench.json').read_bytes())
receipt = dict(command=command, exitCode=p.returncode, seconds=time.time()-start,
               frontendOk=report.get('frontend', {}).get('ok'))
(OUT / 'ipc-bench.receipt.json').write_text(json.dumps(receipt, indent=2)+'\n')
print(receipt)
assert p.returncode == 0 and receipt['frontendOk'] is True
