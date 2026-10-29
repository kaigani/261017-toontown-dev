"""Validate the delivered report/register/test counts and hash the finished evidence bundle."""
import collections
import datetime
import hashlib
import json
import pathlib
import re
import subprocess

OUT=pathlib.Path(__file__).resolve().parent
ROOT=OUT.parents[2]
findings=json.loads((OUT/"findings.json").read_text(encoding="utf-8"))
priorities=collections.Counter(f["priority"] for f in findings)
assert priorities=={"P1":14,"P2":4},priorities
test_files=[ROOT/"crates/executor/tests/c13_r1_persistence.rs",ROOT/"crates/persist/tests/c13_r1_format.rs"]
test_names=[]
for p in test_files:
    test_names.extend(re.findall(r"#\[test\]\s*fn (\w+)",p.read_text(encoding="utf-8")))
assert len(test_names)==26,len(test_names)
regressions=[t for f in findings for t in f["tests"]]
assert len(regressions)==20 and len(set(regressions))==20
assert set(regressions)<=set(test_names)
log=(OUT/"final-checks/regressions.log").read_text(encoding="utf-8")
assert "test result: FAILED. 5 passed; 13 failed;" in log
assert "test result: FAILED. 1 passed; 7 failed;" in log
assert "MED-01: identical SHA-256 media was stored twice" in log
assert "MED-04: package retained media with zero references" in log
checks=json.loads((OUT/"final-checks.json").read_text(encoding="utf-8"))
assert all(c["exitCode"]==(101 if c["name"]=="regressions" else 0) for c in checks["checks"])
counts=json.loads((OUT/"register-counts.json").read_text(encoding="utf-8"))
assert counts["requirements"]=={"implemented":88,"divergent":17,"deferred-with-owner":14}
assert counts["commands"]=={"implemented":35,"divergent":13}
ci=json.loads((OUT/"ci-analysis.json").read_text(encoding="utf-8"))
assert ci["rerun"]["conclusion"]=="success"
assert all(j["conclusion"]=="success" for j in ci["rerun"]["jobs"])
for p in [ROOT/"reviews/M1-C1.3-r1.md",*test_files]:
    assert p.is_file()
manifest={"recordedAt":datetime.datetime.now(datetime.timezone.utc).isoformat(),"reviewedProductCommit":"f119948487cfa11e6e62bef2c2d5179a265e16cc","findings":dict(priorities),"testCount":26,"passingControls":6,"failingRegressions":20,"registerCounts":counts,"sha256":{p.relative_to(OUT).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(OUT.rglob("*")) if p.is_file() and p.name!="sha256-manifest.json"},"deliverablesSha256":{p.relative_to(ROOT).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in [ROOT/"reviews/M1-C1.3-r1.md",*test_files]}}
(OUT/"sha256-manifest.json").write_text(json.dumps(manifest,indent=2),encoding="utf-8")
print(json.dumps({"findings":dict(priorities),"tests":len(test_names),"evidenceFiles":len(manifest["sha256"]),"checks":"pass"}))
