"""Preserve two early Windows ADS logs as portable files, without altering raw bytes."""
from pathlib import Path
import json
import hashlib

HERE=Path(__file__).resolve().parent
for name in ["format", "licenses"]:
    source=HERE/"logs"/f"baseline-gate-{name}:check.log"
    receipt=HERE/"receipts"/f"baseline-gate-{name}:check.json"
    dest=HERE/"logs"/f"baseline-gate-{name}-check.log"
    rd=HERE/"receipts"/f"baseline-gate-{name}-check.json"
    if not source.exists(): continue
    data=source.read_bytes()
    r=json.loads(receipt.read_text(encoding="utf-8"))
    assert hashlib.sha256(data).hexdigest()==r['sha256']
    dest.write_bytes(data)
    r['original_windows_stream']=r['log']
    r['log']=str(dest.relative_to(HERE))
    rd.write_text(json.dumps(r,indent=2)+'\n',encoding="utf-8")
    # Remove only the verified empty carrier files (including their streams),
    # after both raw bytes and portable receipt have been written and checked.
    for folder in ['logs','receipts']:
        base=(HERE/folder/f"baseline-gate-{name}").resolve()
        assert base.is_relative_to(HERE.resolve()) and base.stat().st_size==0
        base.unlink()
    print(name, r['exit_code'], r['sha256'])
