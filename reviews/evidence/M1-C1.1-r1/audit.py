"""Recompute receipts, suite totals, exact-SHA CI status and evidence inventory."""
import hashlib
import json
from pathlib import Path
import re

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
def read(n): return (HERE/n).read_text(encoding="utf-8")
def totals(n):
    results=re.findall(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored",read(n))
    return dict(zip(["passed","failed","ignored"],map(sum,zip(*(tuple(map(int,r)) for r in results)))))
receipts=[]
for p in HERE.glob("*.receipt.json"):
    r=json.loads(p.read_text(encoding="utf-8"))
    log=p.with_name(p.name.replace(".receipt.json",".log"))
    assert hashlib.sha256(log.read_bytes()).hexdigest()==r["logSha256"],p
    receipts.append({"receipt":p.name,"exit":r["exit"],"seconds":r["seconds"]})
ci=json.loads(read("ci-metadata.log"))
assert ci["headSha"]=="53c9a289cd74f5bd87125fb618b1f7466a60a99d"
assert ci["conclusion"]=="success" and len(ci["jobs"])==8
jobs=[]
raw=re.sub(r"(?:\x1b|\^\[)\[[0-9;]*m", "", read("ci-full.log"))
for job in ci["jobs"]:
    assert job["conclusion"]=="success"
    skips=[s["name"] for s in job["steps"] if s["conclusion"]=="skipped"]
    if skips:
        assert "macos" in job["name"].lower() and len(skips)==1 and "tablet" in skips[0]
    lines=[l for l in raw.splitlines() if l.startswith(job["name"]+"\t")]
    summary=[l for l in lines if re.search(r"Tests\s+303 passed|# pass 9|test result:.*(?:[1-9]\d*) passed|ignored,",l)]
    jobs.append({"name":job["name"],"conclusion":job["conclusion"],"skippedSteps":skips,"testSummaries":summary})
    if job["name"].startswith("check ("):
        text="\n".join(lines)
        assert re.search(r"Tests\s+303 passed",text),job["name"]
        assert "# pass 9" in text
        assert "revisions_newer_than_the_document_are_integrity_errors ... ok" in text
        assert "Running tests/properties.rs" in text or "Running tests\\properties.rs" in text
inventory=json.loads(read("ci-artifact-inventory.json"))
assert len(inventory)==7
for artifact in inventory:
    archive=Path(artifact["localArchive"])
    if archive.exists():
        assert hashlib.sha256(archive.read_bytes()).hexdigest()==artifact["verifiedSha256"]
    for item in artifact["entries"]:
        if "preserved" in item: assert hashlib.sha256((HERE/item["preserved"]).read_bytes()).hexdigest()==item["sha256"]
builder=json.loads((ROOT/"reviews/evidence/M1-C1.1/model-scale-windows.json").read_text(encoding="utf-8"))
scale=read("model-scale.log")
ours=json.loads(scale[scale.index('{\n'):])
model_comparison={k:{"builder":v,"review":ours["measurements"][k]} for k,v in builder["measurements"].items()}
bench=json.loads((ROOT/"reviews/evidence/M1-C1.1/ipc-bench-windows-webview2.json").read_text(encoding="utf-8"))
final_totals=totals("final-rust-tests-corrected.log")
old_model=re.search(r"test result: FAILED\. (\d+) passed; (\d+) failed; 0 ignored",read("final-rust-tests-corrected.log")[read("final-rust-tests-corrected.log").index("Running tests\\c11_r1_adversarial.rs"):])
new_model=totals("model-regressions-delivery.log")
final_totals["passed"]+=new_model["passed"]-int(old_model[1])
final_totals["failed"]+=new_model["failed"]-int(old_model[2])
report={"baseSha":ci["headSha"],"receiptCount":len(receipts),"receipts":receipts,"ciJobs":jobs,
        "baselineRust":totals("baseline-rust.log"),"finalRust":final_totals,
        "finalRustDerivation":"Corrected --workspace --no-fail-fast log, replacing model regression target totals with model-regressions-delivery.log after the final added tests.",
        "baselineFrontend":"35 files, 303 tests, node 9 (raw logs)","finalFrontend":"36 files, 307 pass/2 fail; node suite not reached because pnpm uses &&",
        "conformance":json.loads(read("conformance.json"))["counts"],"scaleComparison":model_comparison,
        "builderIpcReportedAggregates":bench["frontend"]["bursts"],
        "ipcEvidenceLimit":"Benchmark JSON preserves reported aggregate quantiles, not individual samples; quantiles cannot be independently recomputed from those aggregates.",
        "artifactCount":len(inventory),"preservedOriginalEvidenceFiles":sum("preserved" in x for a in inventory for x in a["entries"])}
(HERE/"audit.json").write_text(json.dumps(report,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
print(json.dumps({k:report[k] for k in ["receiptCount","baselineRust","finalRust","conformance","artifactCount","preservedOriginalEvidenceFiles"]}))
