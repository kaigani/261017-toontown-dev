"""Download raw Actions log archives byte-for-byte, then derive timings and suite counts."""
import datetime as dt
import collections
import hashlib
import io
import json
import pathlib
import re
import subprocess
import zipfile

OUT = pathlib.Path(__file__).resolve().parent
REPO = "repos/kaigani/261017-toontown-dev"

def api(path):
    p = subprocess.run(["gh", "api", path], capture_output=True, check=True)
    return p.stdout

def stamp(s):
    return dt.datetime.fromisoformat(s.replace("Z", "+00:00"))

summary = {}
for label, run, attempt in [("baseline",39292688834,1),("initial",40423796550,1),("rerun",40423796550,2),("budget",40419016272,1)]:
    base = f"{REPO}/actions/runs/{run}/attempts/{attempt}"
    info = json.loads(api(base))
    jobs = json.loads(api(base + "/jobs?per_page=100"))
    (OUT / f"{label}-run.json").write_text(json.dumps(info,indent=2),encoding="utf-8")
    (OUT / f"{label}-jobs.json").write_text(json.dumps(jobs,indent=2),encoding="utf-8")
    result = {"run":run,"attempt":attempt,"status":info["status"],"conclusion":info["conclusion"],"headSha":info["head_sha"],"jobs":[]}
    for j in jobs["jobs"]:
        result["jobs"].append({"name":j["name"],"status":j["status"],"conclusion":j["conclusion"],"steps":[
            {"name":s["name"],"conclusion":s["conclusion"],"seconds":(stamp(s["completed_at"])-stamp(s["started_at"])).total_seconds() if s.get("completed_at") and s.get("started_at") else None} for s in j["steps"]]})
    if info["status"] == "completed":
        archive = OUT / f"{label}-raw-logs.zip"
        if not archive.exists():
            archive.write_bytes(api(base + "/logs"))
        z = zipfile.ZipFile(io.BytesIO(archive.read_bytes()))
        result["logArchiveSha256"] = hashlib.sha256(archive.read_bytes()).hexdigest()
        logs = {}
        for n in z.namelist():
            if "_check (" in n and n.endswith(".txt"):
                text = re.sub(r"\x1b\[[0-9;]*m", "", z.read(n).decode("utf-8-sig",errors="replace"))
                job = next((j for j in jobs["jobs"] if j["name"] in n),None)
                test_step = next((s for s in job["steps"] if "cargo test --workspace" in s["name"]),None) if job else None
                binaries = {}
                targets = collections.deque()
                for line in text.splitlines():
                    if test_step and test_step.get("started_at") and test_step.get("completed_at"):
                        when = re.match(r"^(\d{4}-\d\d-\d\dT\S+Z)",line)
                        if when and not (stamp(test_step["started_at"]) <= stamp(when[1]) <= stamp(test_step["completed_at"])):
                            continue
                    m = re.search(r"(?:Running|Doc-tests) (.+)",line)
                    if m: targets.append(m[1])
                    m = re.search(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;.*finished in ([\d.]+)s",line)
                    if m:
                        target = targets.popleft() if targets else "unattributed"
                        binaries[target]={"state":m[1],"passed":int(m[2]),"failed":int(m[3]),"ignored":int(m[4]),"seconds":float(m[5])}
                logs[n]={"binaries":binaries,"sumBinarySeconds":sum(b["seconds"] for b in binaries.values()),"passed":sum(b["passed"] for b in binaries.values()),"failed":sum(b["failed"] for b in binaries.values()),"ignored":sum(b["ignored"] for b in binaries.values()),"timeoutLines":[l for l in text.splitlines() if "ETIMEDOUT" in l or "lint coverage follows" in l]}
        result["logs"]=logs
    summary[label]=result
    print(label,result["status"],result["conclusion"],flush=True)
(OUT / "ci-analysis.json").write_text(json.dumps(summary,indent=2),encoding="utf-8")
