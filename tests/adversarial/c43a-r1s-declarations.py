"""Read-only declaration movement audit; run from repository root."""
import json
import re
import subprocess
from pathlib import Path
old = json.loads(subprocess.check_output(["git", "show", "e89dc7c4:crates/registry/src/declared.json"]))
new = json.loads(Path("crates/registry/src/declared.json").read_text(encoding="utf-8"))
a = {x["id"]: x for x in old}
b = {x["id"]: x for x in new}
for key in sorted(a.keys() | b.keys()):
    if a.get(key) != b.get(key):
        print(key, "removed placeholder" if key not in b else "changed", json.dumps(a.get(key), ensure_ascii=True))
        if key not in b:
            sources = "\n".join(p.read_text(encoding="utf-8") for p in Path("crates/executor/src/commands").glob("*.rs"))
            assert 'id = "' + key + '"' in sources, key
rows = Path("spec/A_command_registry.md").read_text(encoding="utf-8").splitlines()
for row in rows:
    if any('`' + key + '`' in row for key in ("anim.tool.set", "anim.tool.get", "anim.view.coords.get", "anim.transform.set", "edit.selection.set")):
        print("SPEC", row)
print("Retained JSON differences:", [k for k in a.keys() & b.keys() if a[k] != b[k]])
