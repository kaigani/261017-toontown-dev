import datetime
import json
from pathlib import Path
import subprocess
import time
ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).resolve().parent
output=OUT/'browser-headroom.json'
start=datetime.datetime.now(datetime.timezone.utc).isoformat()
with (OUT/'headroom-runtime.txt').open('w') as f:
    process=subprocess.Popen([str(ROOT/'target/release/toontown.exe'),'--spike','canvas'],cwd=ROOT,stdout=f,stderr=subprocess.STDOUT)
    try:
        deadline=time.monotonic()+180
        while not output.exists() and time.monotonic()<deadline and process.poll() is None: time.sleep(1)
        success=output.exists()
        (OUT/'headroom-runtime.receipt.json').write_text(json.dumps(dict(startUtc=start,pid=process.pid,outputWritten=success,binary='Tauri release host with evidence-only frontendDist; same original renderers'),indent=2))
    finally:
        if process.poll() is None: process.terminate()
        process.wait(timeout=10)
print('saved',success)
