"""Check the handoff's retained reviewer suite counts against both product OS logs."""
import json
import pathlib
import re
import zipfile
OUT=pathlib.Path(__file__).resolve().parent
d=json.loads((OUT/"ci-analysis.json").read_text(encoding="utf-8"))
expected={"c11_r1_adversarial":39,"c11_r2_adversarial":39,"c11_r3_adversarial":11,"c11_r3_random":1,"c11_r3_register":4,"c11_r4_exposure_bounds":2,"c11_r5_write_bounds":4,"c11_r6_staged_inverse":2,"c11_r1_wire":4,"c11_r2_wire":6,"c11_r3_wire":2,"c01":7,"c03-r2-encoder":11,"c12_r1_contracts":36,"c12_r1_stress":5,"c12_r1_macro":2,"c12_r1_scale":1,"c12_r2_contracts":40,"c12_r2_registry":7,"c12_r2_scale":1,"c12_r3_contracts":15,"c12_r3_registry":12,"c12_r3_stress":5,"c12_r4_closure":4}
front={"c11-r1-adversarial":6,"c11-r2-adversarial":9,"c11-r3-adversarial":4,"adversarial":8,"c01-r2-node":2,"c01-r2-schema":12,"c01-r3-node":1,"c01-r3-schema":14,"c01-r4-node":1,"c02-r1-measurement":5,"c02-r1-tooling":1,"c02-r2-tooling":2,"c02-r3-tooling":1,"c03-r1-binary":9,"c03-r1-tooling":1,"c03-r2-binary":9,"c03-r2-page":3,"c03-r2-tooling":1,"c03-r3-page":2,"boundary.lint":69,"boundary.build":46}
records={}
for platform,label in [("windows","initial"),("macos","rerun")]:
    parsed=next(v for n,v in d[label]["logs"].items() if platform in n)
    rust={}
    for name,count in expected.items():
        matches=[v for n,v in parsed["binaries"].items() if re.search(r"[\\/]"+re.escape(name)+r"\.rs\b",n)]
        rust[name]={"expected":count,"observed":matches,"matches":len(matches)==1 and matches[0]["passed"]==count and matches[0]["failed"]==0}
    z=zipfile.ZipFile(OUT/f"{label}-raw-logs.zip")
    raw=next(z.read(n).decode("utf-8-sig",errors="replace") for n in z.namelist() if "_check (" in n and platform in n)
    raw=re.sub(r"\x1b\[[0-9;]*m","",raw)
    frontend={}
    for name,count in front.items():
        lines=[l for l in raw.splitlines() if re.search(r"[\\/]"+re.escape(name)+r"\.test\.[tj]sx?\b",l) and re.search(r"\("+str(count)+r" tests?\)",l)]
        frontend[name]={"expected":count,"matches":bool(lines),"logLines":lines}
    python_names=["c02-r1-tooling","c02-r2-tooling","c02-r3-tooling","c03-r1-tooling","c03-r2-tooling","c04-r2-decisions","c04-r3-decisions","c04-r3-page","c04-r5-decisions","c11-r2-review-policy","headless_guard"]
    python={n:bool(re.search(re.escape(n)+r"\.py",raw)) for n in python_names}
    records[platform]={"rust":rust,"allRustMatch":all(r["matches"] for r in rust.values()),"frontend":frontend,"allFrontendMatch":all(r["matches"] for r in frontend.values()),"python":python,"allPythonPresent":all(python.values()),"workspacePassed":parsed["passed"],"workspaceIgnored":parsed["ignored"]}
(OUT/"ci-coverage.json").write_text(json.dumps(records,indent=2),encoding="utf-8")
print(json.dumps({p:{k:v for k,v in r.items() if k not in ("rust","frontend","python")} for p,r in records.items()},indent=2))
