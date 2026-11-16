"""Independent C4.3b review probes. Usage: probe APP case1-vite."""
import sys
if len(sys.argv) < 2:
    print("skipped: no application")
    sys.exit(0)
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "reviews/evidence/M4-C4.3b-r1"
def case1_vite():
    results = []
    with tempfile.TemporaryDirectory(prefix="c43b-review-") as td:
        tmp = Path(td)
        tracked = subprocess.check_output(["git", "ls-files", "desktop", "crates/executor/src", "pnpm-workspace.yaml", "package.json", "pnpm-lock.yaml"], cwd=ROOT, text=True).splitlines()
        for name in tracked:
            src, dst = ROOT/name, tmp/name
            if src.is_file():
                dst.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(src,dst)
        for rel in ["node_modules", "desktop/node_modules"]:
            if os.name == "nt":
                subprocess.run(["cmd", "/c", "mklink", "/J", str(tmp/rel), str(ROOT/rel)], check=True, capture_output=True)
            else:
                (tmp/rel).symlink_to(ROOT/rel, target_is_directory=True)
        manifest = tmp/"desktop/src/ui-bindings/parity.json"
        original = manifest.read_text(encoding="utf-8")
        for fixture in ["baseline", "missing-rendered", "stale-element"]:
            data = json.loads(original)
            if fixture == "missing-rendered":
                data["dom"].pop()
            if fixture == "stale-element":
                data["dom"].append({"id":"review.absent", "surface":"review", "commands":["edit.undo"]})
            manifest.write_text(json.dumps(data,indent=2)+"\n" if fixture != "baseline" else original, encoding="utf-8", newline="\n")
            command = ["pnpm.cmd" if os.name == "nt" else "pnpm", "--dir", "desktop", "test", "src/ui-bindings/parity.test.tsx"]
            run = subprocess.run(command,cwd=tmp,capture_output=True,text=True,encoding="utf-8",timeout=180)
            (OUT/("case1-vite-"+fixture+".txt")).write_text(run.stdout+run.stderr,encoding="utf-8",newline="\n")
            results.append({"fixture":fixture,"exit":run.returncode})
        # Remove junctions before TemporaryDirectory walks the fixture.
        for rel in ["desktop/node_modules", "node_modules"]:
            if os.name == "nt": os.rmdir(tmp/rel)
            else: (tmp/rel).unlink()
    (OUT/"case1-vite.json").write_text(json.dumps(results,indent=2)+"\n",encoding="utf-8",newline="\n")
    assert results[0]["exit"] == 0, results
    assert all(r["exit"] != 0 for r in results[1:]), results
    print(results)
if __name__ == "__main__":
    OUT.mkdir(parents=True,exist_ok=True)
    {"case1-vite":case1_vite}[sys.argv[2]]()
