"""C3.1 reviewer history-pinned audit and strict raw timing analysis.

Default: unittest (no output files). --collect OUTPUT_DIRECTORY writes new derived receipts there only.
Inputs stay unchanged. Timing claims refer to run 41621608304, never the current working tree's product.
"""
import copy
import hashlib
import json
import math
import pathlib
import statistics
import subprocess
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
BASE = "0b8fd0b"
HEAD = "8f44969"


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT).decode("utf-8")


def load(path):
    b = path.read_bytes()
    return json.loads(b.decode("utf-16" if b.startswith((b'\xff\xfe', b'\xfe\xff')) else "utf-8-sig"))


def number(v):
    try:
        valid = type(v) in (int, float) and math.isfinite(v) and v >= 0
    except OverflowError:
        valid = False
    if not valid:
        raise ValueError("duration must be a finite nonnegative number")
    return v


CASES = ["read one 100k column", "cycle 999 x 100 frames", "cycle past the scene limit",
         "set exposure on a tenth of the columns", "read the whole sheet", "element list",
         "unused drawings of the scene", "remove exposure on every column", "insert a cell in all columns",
         "set every exposure to 9999 frames on every column"]
COMMANDS = ['xsheet.exposures.get','cell.cycle.create','cell.cycle.create','cell.exposure.set',
            'xsheet.exposures.get','element.list','element.unusedDrawing.list','cell.exposure.remove',
            'cell.range.add','cell.exposure.set']


def timing(raw, os_name):
    if not isinstance(raw, dict) or raw.get("os") != os_name:
        raise ValueError("missing/wrong OS")
    for key, value in [("columns", 1001), ("framesPerColumn", 100000), ("budget", 300000000)]:
        if type(raw.get(key)) is not int or raw[key] != value:
            raise ValueError(f"invalid {key}")
    rows = raw.get("cases")
    if not isinstance(rows, list) or len(rows) != len(CASES):
        raise ValueError("missing cases")
    if [r.get("case") for r in rows] != CASES:
        raise ValueError("missing/duplicate/reordered identity")
    for i,r in enumerate(rows):
        number(r["seconds"])
        if r.get("command") != COMMANDS[i]:
            raise ValueError("wrong command for case")
        if any(type(r.get(k)) is not int or r[k] < 0 for k in ("revBefore", "revAfter")):
            raise ValueError("invalid revision")
        outcome = r.get("outcome")
        if outcome not in ("ok", "doc.limitExceeded", "limit.exceeded"):
            raise ValueError("invalid outcome")
        if outcome != "ok" and r["revBefore"] != r["revAfter"]:
            raise ValueError("refusal committed")
        if r["revAfter"] - r["revBefore"] not in (0, 1):
            raise ValueError("not a single revision")
        expected_outcome = 'doc.limitExceeded' if i==2 else 'limit.exceeded' if i==9 else 'ok'
        if outcome != expected_outcome:
            raise ValueError('wrong case outcome')
        expected_delta = int(i in (1,3,7,8))
        if r['revAfter'] - r['revBefore'] != expected_delta:
            raise ValueError('wrong case revision behavior')
    return {
        "os": os_name, "cases": rows,
        "wholeSheetUnderOneSecond": all(rows[i]["seconds"] < 1 for i in (4, 6, 7, 8)),
        "refusalAtMostFourMs": rows[9]["seconds"] <= .004,
        "allUnderWatchdog": all(r["seconds"] < 30 for r in rows),
        "samplesPerCase": 1,
    }


class Audit(unittest.TestCase):
    def test_protected_files_and_historical_receipts_unchanged_at_candidate(self):
        paths = ["tests/adversarial", "crates/*/tests/c0*_r*", "crates/*/tests/c1*_r*",
                 "crates/*/tests/c2*_r*", "desktop/src/test", "reviews/evidence/M1-C1.3*"]
        # The brief's broad c1*_r* also matches builder-owned c12_registry.rs (explicitly declared).
        literal = git("diff", "--name-only", f"{BASE}..{HEAD}", "--", *paths).splitlines()
        self.assertEqual(literal, ["crates/executor/tests/c12_registry.rs"])
        reviewer = [p.replace("_r*", "_r[0-9]*") for p in paths]
        self.assertEqual(git("diff", "--stat", f"{BASE}..{HEAD}", "--", *reviewer), "")

    def test_registry_count_is_64_not_63(self):
        old = json.loads(git("show", f"{BASE}:crates/registry/src/declared.json"))
        new = json.loads(git("show", f"{HEAD}:crates/registry/src/declared.json"))
        ids = {r["id"] for r in old} - {r["id"] for r in new}
        self.assertEqual(len(ids), 64)
        self.assertEqual(sum(i.endswith(".changed") for i in ids), 5)

    def test_timing_parser_rejects_malformed(self):
        valid = {"os":"windows", "columns":1001,"framesPerColumn":100000,"budget":300000000,
                 "cases":[{"case":c,"command":COMMANDS[i],"seconds":.1,"outcome":'doc.limitExceeded' if i==2 else 'limit.exceeded' if i==9 else 'ok',"revBefore":1,"revAfter":1+int(i in (1,3,7,8))} for i,c in enumerate(CASES)]}
        timing(valid, "windows")
        mutations = [lambda r:r["cases"].pop(), lambda r:r["cases"].append(r["cases"][0]),
                     lambda r:r.update(columns=True), lambda r:r.update(os=""),
                     lambda r:r["cases"][0].update(seconds=True), lambda r:r["cases"][0].update(seconds=-1),
                     lambda r:r["cases"][0].update(seconds=float("nan")), lambda r:r["cases"][0].update(seconds=float("inf")),
                     lambda r:r["cases"][0].update(revBefore=False), lambda r:r["cases"][0].update(outcome="limit.exceeded"),
                     lambda r:r["cases"][0].update(case=CASES[1]), lambda r:r["cases"][0].update(command='wrong.command'),
                     lambda r:r["cases"][0].update(seconds=10**1000), lambda r:r["cases"][0].update(revAfter=2),
                     lambda r:r["cases"][1].update(revAfter=1)]
        for mutate in mutations:
            bad = copy.deepcopy(valid)
            mutate(bad)
            with self.assertRaises((ValueError, KeyError, TypeError)):
                timing(bad, "windows")

    def test_no_new_product_forbidden_constructs(self):
        import re
        added = git("diff", "--unified=0", f"{BASE}..{HEAD}", "--", "crates/*/src", "bins/*/src", "desktop/src-tauri/src")
        prohibited = re.compile(r"\.(?:unwrap|expect)\s*\(|\b(?:panic!|unsafe\s*\{)|#\[allow\b")
        self.assertEqual([s for s in added.splitlines() if s.startswith("+") and not s.startswith("+++") and prohibited.search(s)], [])


def collect(out):
    out.mkdir(parents=True, exist_ok=True)
    artifacts = out / "ci-artifacts"
    run = load(out / "ci-run.json")
    if run['headSha'] != '8f44969741f8662831e25a01a8a024b686daf02e' or len(run['jobs']) != 22:
        raise ValueError('wrong candidate or incomplete job inventory')
    report = {"headSha":run["headSha"],"run":41621608304,"jobs":[],"timing":[],"baseline":{},"e2":[]}
    for j in run["jobs"]:
        import datetime
        start = datetime.datetime.fromisoformat(j["startedAt"].replace("Z", "+00:00"))
        end = datetime.datetime.fromisoformat(j["completedAt"].replace("Z", "+00:00"))
        report["jobs"].append({"name":j["name"],"conclusion":j["conclusion"],"minutes":(end-start).total_seconds()/60})
    for label, os_name in [("Windows","windows"),("macOS","macos")]:
        p = artifacts / f"measurements-{label}/c31-measurements/timing-budget.json"
        report["timing"].append(timing(load(p), os_name))
        old_path = ROOT / f"reviews/evidence/M2-C2.4-r3/ci-artifacts/measurements-{label}/c24-measurements/budgets-{os_name}.json"
        new_path = artifacts / f"measurements-{label}/c24-measurements/budgets-{os_name}.json"
        old, new = load(old_path), load(new_path)
        comparison = {}
        for key in old["series"][0]:
            a = statistics.median(number(r[key]) for r in old["series"])
            b = statistics.median(number(r[key]) for r in new["series"])
            comparison[key] = {"m2r3Median":a,"candidateMedian":b,"ratio":b/a if a else None}
        comparison["memory"] = {"m2r3":old["memory"],"candidate":new["memory"]}
        for key in ("queuedRead", "maintenance"):
            comparison[key] = {"m2r3":old.get(key),"candidate":new.get(key)}
        report["baseline"][os_name] = comparison
    for p in sorted((artifacts / "e2-Windows").glob("*.png")):
        q = artifacts / "e2-macOS" / p.name
        a,b = hashlib.sha256(p.read_bytes()).hexdigest(),hashlib.sha256(q.read_bytes()).hexdigest()
        if a!=b: raise ValueError(f"cross-OS PNG mismatch: {p.name}")
        report["e2"].append({"name":p.name,"sha256":a,"identical":True})
    if len(report["e2"])!=7: raise ValueError("missing E2 PNGs")
    result=out / 'independent-analysis.json'
    if result.exists():
        raise FileExistsError('preserve evidence: choose a new output directory for a new analysis')
    result.write_text(json.dumps(report, indent=2)+"\n",encoding="utf-8")
    print(json.dumps({"jobs":len(report["jobs"]),"e2Identical":len(report["e2"]),"baseline":{k:{m:round(v["ratio"],4) for m,v in values.items() if isinstance(v,dict) and "ratio" in v} for k,values in report["baseline"].items()}}, indent=2))


if __name__ == "__main__":
    if len(sys.argv)==3 and sys.argv[1]=="--collect":
        collect(pathlib.Path(sys.argv[2]).resolve())
    else:
        unittest.main()
