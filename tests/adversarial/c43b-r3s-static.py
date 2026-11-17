"""Read-only static regression probes; exit 1 while a finding reproduces."""
from pathlib import Path
import json
ROOT = Path(__file__).resolve().parents[2]
failures = []
def check(name, ok):
    print(("PASS " if ok else "FAIL ") + name)
    if not ok: failures.append(name)
def read(path): return (ROOT / path).read_text(encoding="utf-8")
r = json.loads(read("desktop/src/ui-bindings/reads.json"))["surfaces"]
expected = {"About Toontown": "app.info.get", "Project Info": "project.info.get",
            "Check Project": "doc.validate", "Open Backup": "project.backup.list"}
check("S1 actual dialog read ownership", all(set(r["dialog:" + s]["reads"]) == {q} for s, q in expected.items()))
raise SystemExit(bool(failures))
