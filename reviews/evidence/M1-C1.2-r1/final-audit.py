"""Check review deliverables and record a final byte-level manifest, excluding the manifest itself."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
REPORT=ROOT/"reviews/M1-C1.2-r1.md"


def main():
    report=REPORT.read_text(encoding="utf-8")
    assert report.startswith("**Verdict: block.**")
    ids=re.findall(r"^### C12-R1-(\d+)",report,re.M)
    assert sorted(map(int,ids))==list(range(1,24))
    assert len(re.findall(r"^### C12-R1-",report.split("## P2 tickets")[0],re.M))==17
    for link in re.findall(r"\]\(([^)]+)\)",report):
        if link.startswith("https:"):continue
        assert (REPORT.parent/link).resolve().exists(),link
    tracked=subprocess.check_output(["git","diff","--name-only"],cwd=ROOT,text=True).splitlines()
    assert not tracked,tracked
    staged=subprocess.check_output(["git","diff","--cached","--name-only"],cwd=ROOT,text=True).splitlines()
    allowed_tests={f"crates/executor/tests/c12_r1_{suffix}.rs" for suffix in ["contracts","stress","scale","macro"]}
    def allowed(p):
        return p in allowed_tests or p=="reviews/M1-C1.2-r1.md" or p.startswith(("reviews/evidence/M1-C1.2-r1/","crates/executor/tests/c12-r1-ui/"))
    assert all(allowed(p) for p in staged),staged
    paths=[p for p in HERE.rglob("*") if p.is_file() and p.name!="SHA256SUMS.json"]
    paths += [REPORT] + [ROOT/p for p in allowed_tests] + list((ROOT/"crates/executor/tests/c12-r1-ui").glob("*"))
    # Hash staged Git bytes, including its declared LF normalization of text. Raw JSON/log/gzip files use -text.
    manifest={}
    for path in sorted(paths):
        rel=path.relative_to(ROOT).as_posix()
        manifest[rel]=hashlib.sha256(subprocess.check_output(["git","show",":"+rel],cwd=ROOT)).hexdigest()
    (HERE/"SHA256SUMS.json").write_text(json.dumps({"reviewedHead":"23ca3d5726ab65d639f1e145045dd30ba8f24351","hashedBytes":"Git index (raw logs preserved; declared text normalization applied)","files":manifest},indent=2)+"\n",encoding="utf-8")
    print(json.dumps({"findings":23,"P1":17,"P2":6,"filesHashed":len(manifest),"stagedPathsChecked":len(staged)},indent=2))


if __name__=="__main__":main()
