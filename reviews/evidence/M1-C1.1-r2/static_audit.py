"""Reproduce the narrowly scoped source, dependency and policy audit."""
from pathlib import Path
import json
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
def git(*a):
    return subprocess.check_output(["git", *a], cwd=ROOT, text=True, encoding="utf-8")

diff = git("diff", "0460ad0..7c5405d", "--", "crates", "desktop/src", "deny.toml", "Cargo.toml", "Cargo.lock")
path = ""
flags = []
for line in diff.splitlines():
    if line.startswith("+++ b/"): path = line[6:]
    elif line.startswith("+") and not line.startswith("+++"):
        if re.search(r"#\!?\[allow|\bunsafe\b|\.unwrap\(|\.expect\(|\bpanic!", line):
            flags.append({"path":path, "added":line[1:]})
panics = []
for f in (ROOT/"crates/model/src").rglob("*.rs"):
    # These files place their unit-test modules after a cfg(test) marker.
    code = f.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    for i,l in enumerate(code.splitlines(),1):
        if re.search(r"\.unwrap\(|\.expect\(|\bpanic!|\bunsafe\s*\{", l):
            panics.append(f"{f.relative_to(ROOT).as_posix()}:{i}:{l}")
assert not panics, panics
doc = (ROOT/"crates/model/src/document/mod.rs").read_text(encoding="utf-8")
mutable = re.findall(r"pub fn[^\{]+(?:&mut[^\{]*)",doc)
assert not mutable, mutable
metadata = json.loads(subprocess.check_output(["cargo","metadata","--locked","--format-version","1"],cwd=ROOT))
packages = {p["id"]:p for p in metadata["packages"]}
nodes = {n["id"]:n for n in metadata["resolve"]["nodes"]}
start = next(p["id"] for p in packages.values() if p["name"]=="jsonschema")
seen = set()
def visit(i):
    if i in seen: return
    seen.add(i)
    for dep in nodes[i]["dependencies"]: visit(dep)
visit(start)
tree = [{"name":packages[i]["name"],"version":packages[i]["version"],"license":packages[i]["license"],
         "features":nodes[i]["features"]} for i in sorted(seen)]
issues=(ROOT/"plan/spec-issues.md").read_text(encoding="utf-8")
for n in [9,10,12]: assert not re.search(rf"^## SI-{n:03}",issues,re.M)
for n in [3,4,5,6,7,8,11]: assert re.search(rf"^## SI-{n:03}",issues,re.M)
record={"base":"7c5405d", "addedReviewFlags":flags, "productPanicMatches":panics,
        "publicMutableDocumentMethods":mutable,"jsonschemaDependencyTree":tree,
        "specIssuesRemoved":[9,10,12],"specIssuesRetained":[3,4,5,6,7,8,11]}
(HERE/"static-audit.json").write_text(json.dumps(record,indent=2)+"\n",encoding="utf-8")
(HERE/"reviewer-test-diff.patch").write_text(git("diff","0460ad0..7c5405d","--","crates/model/tests","crates/engine/tests","desktop/src/ipc/c11-r1-adversarial.test.ts","desktop/src/test","tests/adversarial"),encoding="utf-8")
print(json.dumps({"addedFlags":len(flags),"productPanics":len(panics),"mutableDocumentMethods":len(mutable),"jsonschemaPackages":len(tree)}))
