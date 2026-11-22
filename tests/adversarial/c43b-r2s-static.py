"""Read-only static recount and adversarial lexical probe. No application needed."""
from pathlib import Path
import json
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
def read(name):
    return (ROOT / name).read_text(encoding="utf-8")

def main():
    rows = {}
    for path in (ROOT / "crates").rglob("*.rs"):
        rel = path.relative_to(ROOT).as_posix()
        if "/src/" not in rel or rel.startswith("crates/registry/"):
            continue
        for block in re.findall(r"#\[command\((.*?)\)\]", path.read_text(encoding="utf-8"), re.S):
            ident = re.search(r'\bid\s*=\s*"([^"]+)"', block)
            kind = re.search(r"\bkind\s*=\s*(\w+)", block)
            if ident and kind and kind[1] != "event":
                rows[ident[1]] = "agent_only" in block
    manifest = json.loads(read("desktop/src/ui-bindings/parity.json"))
    reads = json.loads(read("desktop/src/ui-bindings/reads.json"))
    deferred = json.loads(read("desktop/src/ui-bindings/deferred.json"))["deferred"]
    actions = {c for sec in ("dom", "menu", "keymap", "toolbar", "gestures", "windowGestures")
               for row in manifest[sec] for c in row["commands"]}
    queries = {q for surface in reads["surfaces"].values() for q in surface["reads"]}
    counts = dict(total=len(rows), action=len(actions & rows.keys()),
                  read=len((queries-actions) & rows.keys()), agent=sum(rows.values()), deferred=len(deferred))
    print(json.dumps(counts, sort_keys=True))
    assert counts['total'] == counts['action'] + counts['read'] + counts['agent'] + counts['deferred'] and counts['total'] >= 347 and counts['action'] >= 122 and counts['read'] >= 32 and counts['deferred'] <= 132
    # Ruling D-46 item 1: each fixture fed to the product's call-site extractor (scripts/call-sites.mjs, the parser the
    # parity test uses) instead of a copied regex.
    fixtures = ["// engine.run('tool.settings.get', {})", 'const text = "engine.run(\'tool.settings.get\', {})";']
    matched = [json.loads(subprocess.run(["node", str(ROOT / "scripts/call-sites.mjs"), "--as", "desktop/src/x.ts", "-"],
                                         input=text, capture_output=True, text=True, check=True, cwd=ROOT).stdout)
               for text in fixtures]
    print("C43B-R2S-01 excluded lexical contexts counted:", matched)
    return 1 if any(matched) else 0

if __name__ == "__main__":
    sys.exit(main())
