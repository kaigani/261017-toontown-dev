"""Recompute local suite totals and the handoff's CI growth claims from raw logs."""
import collections
import json
import pathlib
import re
import statistics
import zipfile

OUT=pathlib.Path(__file__).resolve().parent
def text(path):
    b=path.read_bytes()
    return b.decode("utf-16") if b.startswith(b"\xff\xfe") else b.decode("utf-8-sig",errors="replace")
def parse(path):
    queue=collections.deque();rows=[]
    for line in re.sub(r"\x1b\[[0-9;]*m","",text(path)).splitlines():
        m=re.search(r"(?:Running|Doc-tests) (.+)",line)
        if m:queue.append(m[1])
        m=re.search(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;.*finished in ([\d.]+)s",line)
        if m:rows.append({"target":queue.popleft() if queue else "nested/unattributed","state":m[1],"passed":int(m[2]),"failed":int(m[3]),"ignored":int(m[4]),"seconds":float(m[5])})
    return rows
local=parse(OUT/"rust-gates/gates/test.log")
original=[r for r in local if "c13_r1" not in r["target"]]
summary={"workspace":{"originalTargets":original,"originalPassed":sum(r["passed"] for r in original),"originalFailed":sum(r["failed"] for r in original),"originalIgnored":sum(r["ignored"] for r in original),"reviewerTargets":[r for r in local if "c13_r1" in r["target"]]},"initialPersistExecutor":parse(OUT/"persist-executor.log")}
ci=json.loads((OUT/"ci-analysis.json").read_text(encoding="utf-8"))
def win(label):return next(v for k,v in ci[label].get("logs",{}).items() if "windows" in k)
comparisons={}
for label in ("baseline","budget","initial"):
    w=win(label)
    targets={}
    for k,v in w["binaries"].items():
        m=re.search(r"tests[\\/](\w+)\.rs",k)
        if m:targets[m[1]]=v
    comparisons[label]={"binarySumSeconds":round(w["sumBinarySeconds"],2),"passed":w["passed"],"targets":{n:targets.get(n) for n in ["c12_r1_stress","compile_fail","c12_kernel_state"]}}
summary["growth"]=comparisons
frontend={}
for label in ("baseline","initial","budget","rerun"):
    archive=OUT/f"{label}-raw-logs.zip"
    if not archive.exists():continue
    z=zipfile.ZipFile(archive)
    platform={}
    for name in z.namelist():
        if "_check (" not in name or not name.endswith(".txt"):continue
        t=re.sub(r"\x1b\[[0-9;]*m","",z.read(name).decode("utf-8-sig",errors="replace"))
        i=t.find("# Subtest: C01-R2-01: lint coverage follows frontend imports outside desktop/src")
        block=t[i:t.find("# Subtest:",i+10)] if i>=0 else ""
        durations=re.findall(r"duration_ms:\s*([\d.]+)",block)
        platform[name]={"outsideLintSubtestDurationMs":durations[0] if durations else None,"timedOut":"ETIMEDOUT" in block,"vitest333":"333" in t and "Test Files" in t,"reviewerPython11":"11 reviewer Python suites" in t or "11/11" in t,"subtestExcerpt":block[:2500]}
    frontend[label]=platform
summary["frontendCI"]=frontend
lint=json.loads((OUT/"lint-comparison.json").read_text(encoding="utf-8"))
summary["localLintComparison"]=[{"ref":r["ref"],"commandsTsBytes":r["commandsTsBytes"],"medianSeconds":statistics.median(x["seconds"] for x in r["runs"]),"allPassed":all(x["exitCode"]==0 for x in r["runs"])} for r in lint]
(OUT/"evidence-analysis.json").write_text(json.dumps(summary,indent=2),encoding="utf-8")
print(json.dumps({"workspace":{k:v for k,v in summary["workspace"].items() if k not in ("originalTargets","reviewerTargets")},"growth":comparisons,"frontend":{k:{n:{x:y for x,y in v.items() if x!="subtestExcerpt"} for n,v in d.items()} for k,d in frontend.items()},"lint":summary["localLintComparison"]},indent=2))
