"""Read-only static witnesses; diagnostic only, always exits zero in CI."""
from pathlib import Path
import json
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]

def read(p):
    return (ROOT / p).read_text(encoding="utf-8")

def check(name, fn):
    try:
        print(name + ": " + str(fn()))
    except Exception as e:
        print(name + ": unavailable " + type(e).__name__)

def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT).decode("utf-8")

def pending():
    s = read("crates/executor/src/text_edit.rs").split("pub(crate) fn take_pending", 1)[1].split("/// The session for", 1)[0]
    return "FAIL witness: no run termination" if "end_run" not in s else "changed; inspect"

def caret():
    s = read("crates/executor/src/text_edit.rs").split("pub(crate) fn settle", 1)[1].split("/// Ends the session", 1)[0]
    return "FAIL witness: no index reconciliation" if "caret =" not in s else "changed; inspect"

def colour():
    s = read("crates/executor/src/text_edit.rs").split("fn type_text(", 1)[1].split("fn key(", 1)[0]
    return "FAIL witness: insertion never resolves current colour" if "current_colour" not in s else "changed; inspect"

def schema():
    s = read("crates/executor/src/commands/draw_transform.rs")
    return "FAIL witness: reset schema says handles removed" if "Both handles removed (a corner)." in s else "changed; inspect"

def owners():
    return json.dumps(json.loads(read("desktop/src/ui-bindings/deferred.json")))[:0] or "inspect deferred owners in source"

def large():
    paths = git("diff", "--name-only", "--diff-filter=A", "87f63b5f", "e6f71489").splitlines()
    return [(p, (ROOT / p).stat().st_size) for p in paths if (ROOT / p).is_file() and (ROOT / p).stat().st_size > 200000]

def reviewer():
    paths = git("diff", "--name-only", "7dad9a7e", "57bad5ee").splitlines()
    return [p for p in paths if p.startswith("tests/adversarial/") or re.search(r"/c\d+_r\d+_.*\.rs$", p)]

for name, fn in [("01 pending format", pending), ("02 stale caret", caret),
                 ("03 current colour", colour), ("05 schema", schema),
                 ("S5 reviewer tests changed", reviewer), ("S7 large additions", large)]:
    check(name, fn)
