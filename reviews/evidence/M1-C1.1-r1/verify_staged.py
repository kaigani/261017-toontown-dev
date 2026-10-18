"""Fail if the staged review edits product/spec files or changes raw evidence bytes."""
import hashlib
import json
from pathlib import Path
import subprocess

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
def git(*args): return subprocess.check_output(["git",*args],cwd=ROOT)
allowed={"reviews/M1-C1.1-r1.md","crates/model/tests/c11_r1_adversarial.rs",
         "crates/engine/tests/c11_r1_wire.rs","desktop/src/ipc/c11-r1-adversarial.test.ts"}
staged=git("diff","--cached","--name-status").decode().splitlines()
for row in staged:
    status,path=row.split("\t",1)
    assert status=="A",row
    assert path in allowed or path.startswith("reviews/evidence/M1-C1.1-r1/"),row
    if path.endswith((".log",".json",".txt")):
        assert git("show",":"+path)==(ROOT/path).read_bytes(),path
assert not git("diff","--name-only"),"unstaged tracked changes"
print(json.dumps({"stagedFiles":len(staged),"onlyAuthorizedAdditions":True,"rawEvidenceBytesPreserved":True}))
