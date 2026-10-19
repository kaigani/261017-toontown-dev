"""Verify receipts, actual CI counts, old reviewer-test preservation, and evidence paths."""
from collections import defaultdict
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]


def gh_counts(run):
    text=gzip.decompress((HERE/f"ci-{run}.log.gz").read_bytes()).decode("utf-8")
    jobs={}
    for os in ["windows-latest","macos-latest"]:
        lines=[l for l in text.splitlines() if l.startswith(f"check ({os})\t")]
        counts=defaultdict(lambda:[0,0,0])
        suites={}
        target=None
        for line in lines:
            fields=line.split("\t",2)
            step=fields[1]
            clean=re.sub(r"\x1b\[[0-9;]*m","",line)
            match=re.search(r"Running (?:tests[/\\])?([^ ]+) \(",clean)
            if match: target=match[1]
            m=re.search(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored",clean)
            if m:
                for i in range(3): counts[step][i]+=int(m[i+1])
                if target: suites[f"{step}: {target}"]=list(map(int,m.groups()))
        jobs[os]={"rustSteps":dict(counts),"rustTotalPassed":sum(v[0] for v in counts.values()),
                  "rustTotalIgnored":sum(v[2] for v in counts.values()),"rustSuites":suites,
                  "python11of11":any("11/11 reviewer Python suites passed" in l for l in lines),
                  "vitest333":any(re.search(r"Tests.*333 passed",l) for l in lines),
                  "exclusiveMutationPassed":any("exclusive_mutation_capability ... ok" in l for l in lines),
                  "reviewerLines":[re.sub(r"\x1b\[[0-9;]*m","",l) for l in lines if re.search(r"c11[-_]r[1-7]|c04-r3|boundary\.(lint|build)|staged_inverse|staged_commit",l)]}
        assert jobs[os]["python11of11"] and jobs[os]["vitest333"] and jobs[os]["exclusiveMutationPassed"]
    return jobs


def main():
    entries={e["id"]:e for e in json.loads((HERE/"registry.json").read_text(encoding="utf-8"))}
    example_outcomes=defaultdict(int)
    for example in json.loads((HERE/"examples.json").read_text(encoding="utf-8")):
        entry=entries[example["id"]]
        expected="command.disabled" if entry["kind"]=="event" else "command.notImplemented" if entry["status"]=="notImplemented" else "ok"
        actual=example["error"]["code"] if example["error"] else example["status"]
        assert expected==actual,(example,expected)
        example_outcomes[actual]+=1
    (HERE/"example-outcomes.json").write_text(json.dumps(dict(example_outcomes),indent=2)+"\n",encoding="utf-8")
    # Preserve per-profile artifacts even when a subsequent cargo test updates the live artifact.
    for logname in ["scale-v3", "workspace-final"]:
        for line in (HERE/f"logs/{logname}.log").read_text(encoding="utf-8").splitlines():
            if line.startswith('{"profile":'):
                data=json.loads(line)
                (HERE/f"large-documents-{data['profile']}.json").write_text(json.dumps(data,indent=2)+"\n",encoding="utf-8")
    # Passing-test stdout is hidden by cargo without --nocapture; the test's direct artifact is authoritative.
    live=json.loads((HERE/"large-documents.json").read_text(encoding="utf-8"))
    (HERE/f"large-documents-{live['profile']}.json").write_text(json.dumps(live,indent=2)+"\n",encoding="utf-8")
    workspace=(HERE/"logs/workspace-final.log").read_text(encoding="utf-8")
    totals=[0,0,0]
    for match in re.finditer(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored",workspace):
        for i in range(3):totals[i]+=int(match[i+1])
    (HERE/"final-test-counts.json").write_text(json.dumps({"workspacePassedFailedIgnored":totals},indent=2)+"\n")
    receipt_checks=[]
    for receipt in (HERE/"receipts").glob("*.json"):
        r=json.loads(receipt.read_text())
        path=HERE/r["log"]
        good=hashlib.sha256(path.read_bytes()).hexdigest()==r["sha256"]
        receipt_checks.append({"receipt":receipt.name,"matches":good,"exitCode":r["exitCode"]})
        assert good,path
    for folder in [HERE/"baseline",ROOT/"reviews/evidence/M1-C1.2"]:
        for receipt in folder.glob("*receipt.json"):
            r=json.loads(receipt.read_text())
            for name,want in r.get("sha256",{}).items():
                p=folder/name
                good=hashlib.sha256(p.read_bytes()).hexdigest()==want
                receipt_checks.append({"receipt":str(receipt.relative_to(ROOT)),"log":name,"matches":good})
                assert good,p
    changed=subprocess.check_output(["git","diff","--name-only","6b52426","23ca3d5"],cwd=ROOT,text=True).splitlines()
    reviewer=[p for p in changed if p.startswith(("tests/adversarial/","desktop/src/test/")) or re.search(r"/tests/c11_|/ipc/c11-",p) or p in ["crates/model/tests/staged_commit.rs","crates/model/tests/staged_inverse.rs"]]
    assert not reviewer,reviewer
    assert ".github/workflows/ci.yml" not in changed
    reg=json.loads((HERE/"requirements.json").read_text())
    for r in reg:
        file,line=r["implementation"].rsplit(":",1)
        assert (ROOT/file).is_file(),r
        assert int(line)<=len((ROOT/file).read_text(encoding="utf-8").splitlines()),r
    added=subprocess.check_output(["git","diff","f4c9db5","23ca3d5","--","crates/executor/src","crates/registry/src","crates/registry-macros/src"],cwd=ROOT,text=True,encoding="utf-8")
    unsafe=[l for l in added.splitlines() if l.startswith("+") and re.search(r"\bunsafe\s*\{|allow\([^)]*unsafe",l)]
    result={"receipts":receipt_checks,"changedReviewerFiles":reviewer,"ciWorkflowUnchanged":True,"newUnsafeBlocksOrAllowances":unsafe,
            "cargoLockDelta":subprocess.check_output(["git","diff","f4c9db5","23ca3d5","--","Cargo.lock"],cwd=ROOT,text=True),
            "ci":{str(run):gh_counts(run) for run in [39200848231,39198980037]}}
    (HERE/"ci-audit.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
    print(json.dumps({"receiptsChecked":len(receipt_checks),"exampleOutcomes":dict(example_outcomes),"workspacePassedFailedIgnored":totals,"oldReviewerFilesChanged":reviewer,"newUnsafe":unsafe,
                      "ciCounts":{run:{os:{k:v for k,v in details.items() if k not in ["reviewerLines","rustSuites"]} for os,details in jobs.items()} for run,jobs in result["ci"].items()}},indent=2))


if __name__=="__main__":main()
