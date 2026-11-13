"""History-pinned final-round audit. Explicit --collect writes new receipts; never rewrites evidence."""
import copy
import datetime
import importlib.util
import json
import math
import pathlib
import re
import statistics
import subprocess
import sys
import unittest

ROOT=pathlib.Path(__file__).resolve().parents[2]
BASE="6fadf00"
HEAD="9d42034ab37cc2c6e904d0e4bceba3fe975a7d92"
EVIDENCE=ROOT/"reviews/evidence/M3-C3.1-r3"
def module():
    spec=importlib.util.spec_from_file_location("r1audit",ROOT/"tests/adversarial/c31-r1-audit.py")
    m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def git(*args):return subprocess.check_output(["git",*args],cwd=ROOT).decode("utf-8")
def write(name,data):
    p=EVIDENCE/name
    if p.exists():raise FileExistsError("preserve receipt: "+str(p))
    p.write_text(json.dumps(data,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")

def observations():
    audit=module();timings=[];baseline={};planar={};stroke_series={};planar_comparison={}
    for label,os in [("Windows","windows"),("macOS","macos")]:
        def artifact(round,area,file):return ROOT/f"reviews/evidence/{round}/ci-artifacts/{area}-{label}/{file}"
        new=audit.load(artifact("M3-C3.1-r3","measurements","c31-measurements/timing-budget.json"));audit.timing(new,os)
        for old_name in ["M3-C3.1-r1","M3-C3.1-r2"]:
            old=audit.load(artifact(old_name,"measurements","c31-measurements/timing-budget.json"));audit.timing(old,os)
            for a,b in zip(new["cases"],old["cases"]):timings.append(dict(os=os,case=a["case"],seconds=a["seconds"],baseline=old_name,previousSeconds=b["seconds"],ratio=a["seconds"]/b["seconds"],outcome=a["outcome"],revisionUnchanged=a["revBefore"]==a["revAfter"]))
        old=audit.load(artifact("M2-C2.4-r3","measurements",f"c24-measurements/budgets-{os}.json"));new=audit.load(artifact("M3-C3.1-r3","measurements",f"c24-measurements/budgets-{os}.json"))
        baseline[os]={}
        for key in old["series"][0]:
            a=statistics.median(audit.number(r[key]) for r in old["series"]);b=statistics.median(audit.number(r[key]) for r in new["series"])
            baseline[os][key]=dict(m2Median=a,currentMedian=b,ratio=b/a if a else None)
        for group in ["queuedRead","maintenance"]:
            baseline[os][group]={}
            for key in old[group][0]:
                if not key.endswith("Seconds") or old[group][0][key] is None or new[group][0][key] is None:continue
                a=statistics.median(audit.number(r[key]) for r in old[group]);b=statistics.median(audit.number(r[key]) for r in new[group])
                baseline[os][group][key]=dict(m2Median=a,currentMedian=b,ratio=b/a if a else None)
        old_path=artifact("M2-C2.4-r3","headless",f"c22-planar-worst-case/planar-worst-case-{os}.json")
        new_path=artifact("M3-C3.1-r3","headless",f"c22-planar-worst-case/planar-worst-case-{os}.json")
        planar[os]=dict(m2=audit.load(old_path),current=audit.load(new_path))
        planar_comparison[os]=[]
        for a,b in zip(planar[os]["m2"]["cases"],planar[os]["current"]["cases"]):
            if a["name"]!=b["name"]:raise ValueError("planar identity mismatch")
            old_median=statistics.median(audit.number(r["wallMs"]) for r in a["runs"])
            new_median=statistics.median(audit.number(r["wallMs"]) for r in b["runs"])
            planar_comparison[os].append(dict(case=a["name"],m2MedianMs=old_median,currentMedianMs=new_median,ratio=new_median/old_median,outcomes=[r["outcome"] for r in b["runs"]]))
        a=audit.load(artifact("M2-C2.4-r3","headless",f"c21-measurements/strokes-100k-{os}.json"))
        b=audit.load(artifact("M3-C3.1-r3","headless",f"c21-measurements/strokes-100k-{os}.json"))
        stroke_series[os]={}
        if set(a["series"])!=set(b["series"]):raise ValueError("100k series identity mismatch")
        for key in a["series"]:
            def sample(value):
                if type(value) not in (int,float) or not math.isfinite(value):raise ValueError("nonfinite/non-numeric sample")
                if not key.endswith(".retainedBytes") and value<0:raise ValueError("negative non-delta sample")
                return value
            old_samples=[sample(r["value"]) for r in a["series"][key]["samples"]]
            new_samples=[sample(r["value"]) for r in b["series"][key]["samples"]]
            if not old_samples or not new_samples:raise ValueError("missing samples")
            old_median=statistics.median(old_samples);new_median=statistics.median(new_samples)
            stroke_series[os][key]=dict(m2Median=old_median,currentMedian=new_median,ratio=new_median/old_median if old_median else None)
    run=module().load(EVIDENCE/"ci-run.json")
    if run["headSha"]!=HEAD or len(run["jobs"])!=22 or any(j["conclusion"]!="success" for j in run["jobs"]):raise ValueError("wrong/incomplete CI")
    jobs=[]
    for j in run["jobs"]:
        seconds=(datetime.datetime.fromisoformat(j["completedAt"])-datetime.datetime.fromisoformat(j["startedAt"])).total_seconds()
        jobs.append(dict(name=j["name"],seconds=seconds,conclusion=j["conclusion"]))
    return dict(head=HEAD,run=41689238994,jobs=jobs,c31=timings,m2Baseline=baseline,planar=planar,planarComparison=planar_comparison,stroke100k=stroke_series)

def register():
    old=json.loads(git("show",f"{BASE}:reviews/evidence/M3-C3.1-r2/register-complete.json"))
    current=copy.deepcopy(old);current["candidate"]=HEAD;current["previousRegisterHead"]=BASE
    current["deltaBasis"]="Unchanged R1/R2 proving suites, final-round independent variants, SDK release sweep and fixed-head CI; finite coverage, not exhaustive proof."
    units={"C31-X01":["01"],"C31-U296":["03"],"C31-U297":["02","04"],"C31-U298":["04"],"C31-U335":["04"],"C31-U459":["03"],"C31-U460":["02","04"],"C31-U609":["04"],"C31-X07":["02","03","04"]}
    commands={"cell.clipboard.set":["03"],"cell.clipboard.apply":["02","04"]}
    reproductions={"01":"cargo test -p tt-executor --locked -j 4 --test c31_r3_patch", "02":"cargo test -p tt-executor --locked -j 4 --test c31_r3_binary same_ids", "03":"cargo test -p tt-executor --locked -j 4 --test c31_r3_binary paste_then_copy", "04":"cargo test -p tt-executor --locked -j 4 --test c31_r3_binary texture_recovery"}
    deltas=[]
    for group,mapping in [("requirements",units),("commandsAndEvents",commands)]:
        for prior,row in zip(old[group],current[group]):
            findings=mapping.get(row["id"],[])
            row["round2Findings"]=prior.get("findings",[])
            row["findings"]=["C31-R3-"+n for n in findings]
            row["reproduction"]=[reproductions[n] for n in findings]
            if findings:
                row["status"]="divergent";row["basis"]="Retained final-round failing assertions; raw evidence M3-C3.1-r3."
            elif row["status"]=="divergent":
                row["status"]="implemented";row["basis"]="Original proving assertion and independent final-round controls pass; no replacement requirement."
            if row["status"]!=prior["status"] or row["findings"]!=prior.get("findings",[]):
                deltas.append(dict(group=group,id=row["id"],before=prior["status"],after=row["status"],previousFindings=prior.get("findings",[]),findings=row["findings"]))
    adjacent={"doc.object.patch":["01"],"edit.paste":["02","04"],"edit.pasteSpecial":["02","04"],"edit.copy":["03"],"edit.cut":[],"edit.cutSpecial":[],"project.save":["04"],"project.saveAs":["04"]}
    current["adjacentCommands"]=[dict(id=k,status="divergent" if v else "implemented",owner="C3.1 transfer/delegation/persistence interaction",findings=["C31-R3-"+n for n in v]) for k,v in adjacent.items()]
    current["counts"]={g:{s:sum(r["status"]==s for r in current[g]) for s in ["implemented","divergent","missing","deferred-with-owner"]} for g in ["requirements","commandsAndEvents"]}
    current["deltas"]=deltas
    return current

class Audit(unittest.TestCase):
    def test_protected_diff_and_m13_receipts_empty(self):
        paths=["tests/adversarial","crates/*/tests/c0*_r*","crates/*/tests/c1*_r*","crates/*/tests/c2*_r*","crates/*/tests/c31_r1_*","crates/*/tests/c31_r2_*","desktop/src/test","reviews/evidence/M1-C1.3*"]
        self.assertEqual(git("diff","--stat",f"{BASE}..{HEAD}","--",*paths),"")
    def test_no_new_forbidden_product_lines(self):
        diff=git("diff","--unified=0",f"{BASE}..{HEAD}","--","crates/*/src","bins/*/src","desktop/src-tauri/src")
        added="\n".join(l[1:] for l in diff.splitlines() if l.startswith("+") and not l.startswith("+++"))
        self.assertIsNone(re.search(r"\.(?:unwrap|expect)\s*\(|\b(?:panic|todo|unimplemented)!|\bunsafe\s*\{|#!?\[allow",added))
    def test_selection_reads_are_borrowed_at_fixed_head(self):
        timing=git("show",f"{HEAD}:crates/executor/src/commands/timing.rs")
        self.assertIn('cell_span(cx.app().selection_list("xsheet"))',timing)
        self.assertIn('cell_span(app.selection_list("xsheet"))',timing)
        app=git("show",f"{HEAD}:crates/executor/src/app.rs")
        self.assertIn("Arc::ptr_eq(w, ids)",app)
        self.assertIn("cell_ref_parts(k)",app)
    def test_current_raw_timing_rejects_malformed(self):
        m=module();raw=m.load(EVIDENCE/"ci-artifacts/measurements-Windows/c31-measurements/timing-budget.json")
        m.timing(raw,"windows")
        for bad in [-1,True,"0.1",float("nan"),float("inf"),10**1000]:
            altered=copy.deepcopy(raw);altered["cases"][0]["seconds"]=bad
            with self.assertRaises(ValueError):m.timing(altered,"windows")
    def test_builder_tests_added_without_existing_test_changes(self):
        changed=git("diff","--name-status",f"{BASE}..{HEAD}","--","crates/*/tests/*.rs").splitlines()
        self.assertEqual(changed,["A\tcrates/executor/tests/c31_round2.rs"])
    def test_register_keeps_every_unit_and_deferred_owner(self):
        r=register();self.assertEqual(len(r["requirements"]),694);self.assertEqual(len(r["commandsAndEvents"]),64)
        for row in r["requirements"]+r["commandsAndEvents"]:
            if row["status"]=="divergent":self.assertTrue(row["reproduction"])
            if row["status"]=="deferred-with-owner":self.assertTrue(row["owner"])

if __name__=="__main__":
    if len(sys.argv)==3 and sys.argv[1]=="--collect":
        write(sys.argv[2],observations())
    elif len(sys.argv)==3 and sys.argv[1]=="--register":
        data=register();write(sys.argv[2],data);print(json.dumps(data["counts"]))
    else:unittest.main()
