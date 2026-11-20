"""Advisory static checks; standard library only; never fail CI or run product code."""
from pathlib import Path
import json
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def check(name, fn):
    try:
        print(name + ': ' + str(fn()))
    except Exception as exc:
        print(name + ': inspection error ' + type(exc).__name__)


def read(path):
    return (ROOT / path).read_text(encoding='utf-8')


def evidence():
    files = [p for p in (ROOT / 'reviews/evidence/M4-C4.4a-r3').rglob('*') if p.is_file()]
    suspicious = 0
    for p in files:
        # Receipts' explicitly labelled SHA-256 values are provenance, not bearer tokens.
        text = re.sub(r'"sha256"\s*:\s*"[a-f0-9]{64}"', '', p.read_text(encoding='utf-8'))
        suspicious += len(re.findall(r'[A-Za-z0-9+/]{80,}={0,2}|\b[a-fA-F0-9]{40,}\b|data:image', text))
    return f'{len(files)} files; max={max(p.stat().st_size for p in files)}; total={sum(p.stat().st_size for p in files)}; unclassified long strings={suspicious}'


check('S1 navigation forget', lambda: 'FAIL: Started clears deliveries' if 'PageLoadEvent::Started' in read('desktop/src-tauri/src/lib.rs') and 'self.reveals.forget(label)' in read('desktop/src-tauri/src/host.rs') else 'inspect changed source')
check('S2 cleanup paint fence', lambda: 'FAIL: immediate tokenHidden on unmount' if 'if (delivery !== null) void engine.tokenHidden(delivery)' in read('desktop/src/prefs/AgentAccessPage.tsx') else 'inspect changed source')
check('S3 held wait call sites', lambda: str([(i, line.strip()) for i, line in enumerate(read('crates/mcp/src/server.rs').splitlines(), 1) if 'held_until_answered(' in line]))
check('S4 dead-key model', lambda: 'FAIL: key=Dead, code=KeyE, Option -> Alt+E; typed=False; editing-set match=False; accelerator=True')
check('S5 scroll protection', lambda: all(x in read('desktop/src/shell/ContextMenu.tsx') for x in ['const scrollTop =', 'onPointerLeave: stopHold', 'useEffect(() => stopHold', 'boxSizing:']))
check('S6 coverage guard', lambda: 'full-table gate present' if 'if not only and own:' in read('scripts/c43-d39-bindings.py') else 'inspect changed source')
check('S7 evidence', evidence)
check('S7 allows', lambda: sum(sum(int(v) for v in values.values()) for values in json.loads(read('scripts/product-allows.json')).values()))
check('tracked scope', lambda: subprocess.check_output(['git', 'diff', '--name-only', 'ce7bc57a'], cwd=ROOT, text=True).strip().replace('\n', ', '))
