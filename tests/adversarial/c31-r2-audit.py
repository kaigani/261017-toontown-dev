"""Round-two history-pinned engineering and performance audit; default writes no receipts.

--guards OUTPUT runs every inherited Python suite and engineering tools into a new directory.
"""
import collections
import copy
import importlib.util
import json
import pathlib
import subprocess
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
BASE = "e7ac679"
HEAD = "ffffc7387f03e2be125a634dd29f698598532d0d"
EVIDENCE = ROOT / "reviews/evidence/M3-C3.1-r2"


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT).decode("utf-8")


def module(name, file):
    spec = importlib.util.spec_from_file_location(name, ROOT / file)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def observations():
    r1 = module("r1audit", "tests/adversarial/c31-r1-audit.py")
    rows = []
    for os, artifact in [("windows", "Windows"), ("macos", "macOS")]:
        new = r1.load(EVIDENCE / f"ci-artifacts/measurements-{artifact}/c31-measurements/timing-budget.json")
        old = r1.load(ROOT / f"reviews/evidence/M3-C3.1-r1/ci-artifacts/measurements-{artifact}/c31-measurements/timing-budget.json")
        r1.timing(new, os)
        r1.timing(old, os)
        for a, b in zip(new["cases"], old["cases"]):
            rows.append(dict(os=os, case=a["case"], seconds=a["seconds"], previousSeconds=b["seconds"],
                             ratio=a["seconds"] / b["seconds"], outcome=a["outcome"],
                             revisionUnchanged=a["revBefore"] == a["revAfter"]))
    return rows


def register():
    old = json.loads(git("show", f"{BASE}:reviews/M3-C3.1-r1-register.json"))
    current = copy.deepcopy(old)
    current["candidate"] = HEAD
    current["previousRegisterHead"] = BASE
    current["deltaBasis"] = "All unchanged R1 proving suites plus independent R2 variants; findings outside the 64 C3.1 rows are recorded as adjacentCommands."
    units = {
        "01": ["C31-X01"],
        "02": ["C31-X01", "C31-U312", "C31-U328", "C31-U491", "C31-U494"],
        "03": ["C31-U300", "C31-U327"],
        "04": ["C31-U168", "C31-U295", "C31-U299", "C31-U599"],
        "05": ["C31-U088", "C31-U110", "C31-U410", "C31-U299"],
        "06": ["C31-U296", "C31-U297", "C31-U298", "C31-U335", "C31-U460", "C31-X07"],
    }
    commands = {"element.create":["05"], "cell.clipboard.apply":["06"],
                "element.state.changed":["02"], "cell.selection.changed":["02"]}
    repro = {
        "01":"cargo test -p tt-executor --locked -j 4 --test c31_r2_variants r03_rfc_test",
        "02":"cargo test -p tt-executor --locked -j 4 --test c31_r2_variants r03_rfc_copy",
        "03":"cargo test -p tt-executor --locked -j 4 --test c31_r2_variants r04_generic_bridge",
        "04":"cargo test -p tt-executor --locked -j 4 --test c31_r2_variants r05_generic_cut",
        "05":"cargo test -p tt-executor --locked -j 4 --test c31_r2_variants r08_",
        "06":"cargo test -p tt-executor --locked -j 4 --test c31_r2_persistence r04_binary_entry_collision",
    }
    deltas = []
    for group in ["requirements", "commandsAndEvents"]:
        for previous, row in zip(old[group], current[group]):
            refs = [n for n, ids in units.items() if row["id"] in ids] if group=="requirements" else commands.get(row["id"], [])
            row["round1Findings"] = previous.get("findings", [])
            row["findings"] = ["C31-R2-"+n for n in refs]
            row["reproduction"] = [repro[n] for n in refs]
            if refs:
                row["status"] = "divergent"
                row["basis"] = "R2 retained failing reproductions; unaffected aspects retain R1 proving coverage."
            elif row["status"] == "divergent":
                row["status"] = "implemented"
                row["basis"] = "Unchanged R1 proving tests pass at response; R2 closure variants and pinned CI support closure."
            if previous["status"] != row["status"] or previous.get("findings") != row["findings"]:
                deltas.append(dict(group=group,id=row["id"],before=previous["status"],after=row["status"],
                                   previousFindings=previous.get("findings",[]),findings=row["findings"]))
    current["adjacentCommands"] = [dict(id=id,status="divergent",owner="M3 C3.1 bridge / inherited M1 command",findings=["C31-R2-"+n for n in refs])
        for id,refs in {"doc.object.patch":["01","02"],"edit.paste":["03","06"],"edit.pasteSpecial":["05"],"edit.cut":["04"],"edit.cutSpecial":["04"]}.items()]
    states=["implemented","divergent","missing","deferred-with-owner"]
    current["counts"]={g:{s:sum(r["status"]==s for r in current[g]) for s in states} for g in ["requirements","commandsAndEvents"]}
    current["deltas"] = deltas
    current["clarifications"] = {"C31-X14":"R1-12 ruling: observations with a 30-second criterion; the previous under-1-second and <=4-ms claims are superseded, not new gates."}
    return current


class Audit(unittest.TestCase):
    def test_register_preserves_every_source_unit_and_command(self):
        r=register()
        self.assertEqual(len(r["requirements"]),694)
        self.assertEqual(len(r["commandsAndEvents"]),64)
        self.assertEqual(len({x["id"] for x in r["requirements"]}),694)
        for row in r["requirements"]+r["commandsAndEvents"]:
            if row["status"]=="divergent":
                self.assertTrue(row["reproduction"])
            if row["status"]=="deferred-with-owner":
                self.assertTrue(row["owner"])
    def test_reviewer_owned_files_unchanged_at_response(self):
        paths = ["tests/adversarial", "crates/*/tests/c0*_r*", "crates/*/tests/c1*_r*",
                 "crates/*/tests/c2*_r*", "crates/*/tests/c31_r1_*", "desktop/src/test"]
        self.assertEqual(git("diff", "--stat", f"{BASE}..{HEAD}", "--", *paths), "")

    def test_whole_timeline_in_both_acceptance_snapshots(self):
        for file in ["crates/executor/src/run.rs", "crates/executor/src/cx.rs"]:
            text = git("show", f"{HEAD}:{file}")
            self.assertIn("timeline: crate::commands::timing::TimelineState", text)
            self.assertIn("timeline.clone()", text)

    def test_new_product_lines_no_forbidden_constructs(self):
        import re
        diff = git("diff", "--unified=0", f"{BASE}..{HEAD}", "--", "crates/*/src", "bins/*/src", "desktop/src")
        code = "\n".join(l[1:] for l in diff.splitlines() if l.startswith("+") and not l.startswith("+++"))
        self.assertIsNone(re.search(r"\.(?:unwrap|expect)\s*\(|\b(?:panic|unimplemented|todo)!|\bunsafe\s*\{|#!?\[allow", code))

    def test_performance_receipt_recomputes_twenty_cases(self):
        rows = observations()
        self.assertEqual(len(rows), 20)
        self.assertTrue(all(r["seconds"] < 30 for r in rows))
        self.assertTrue(all(r["revisionUnchanged"] for r in rows if r["outcome"] != "ok"))
        self.assertEqual(json.loads((EVIDENCE / "performance.json").read_text())["observations"], rows)

    def test_new_raw_timing_negative_and_infinite_variants_refuse(self):
        r1 = module("r1audit_bad", "tests/adversarial/c31-r1-audit.py")
        raw = r1.load(EVIDENCE / "ci-artifacts/measurements-Windows/c31-measurements/timing-budget.json")
        for invalid in [-7, True, "0.75", float("nan"), float("inf")]:
            modified = copy.deepcopy(raw)
            modified["cases"][8]["seconds"] = invalid
            with self.assertRaises(ValueError):
                r1.timing(modified, "windows")


def guards(dest):
    dest.mkdir(parents=True, exist_ok=False)
    records = []
    # Use the repository's existing historical-receipt dispositions; do not treat collectors as live suites.
    for cmd in [[sys.executable, "scripts/run-reviewer-python-suites.py"],
                [sys.executable, "scripts/check-reviewer-ownership.py"],
                [sys.executable, "scripts/check-product-allows.py"],
                [sys.executable, "scripts/gen-command-declarations.py", "--check"],
                ["cargo", "deny", "check"], ["pnpm.cmd", "--dir", "desktop", "test", "--run"]]:
        with (dest / f"engineering-{len(records)}.txt").open("wb") as output:
            p = subprocess.run(cmd, cwd=ROOT, stdout=output, stderr=subprocess.STDOUT)
        records.append(dict(command=cmd, exit=p.returncode))
    (dest / "summary.json").write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(dict(suites=len(records), failures=[r for r in records if r["exit"]])))
    return int(any(r["exit"] for r in records))


if __name__ == "__main__":
    if len(sys.argv)==3 and sys.argv[1]=="--register":
        output=pathlib.Path(sys.argv[2]).resolve()
        if output.exists():
            raise FileExistsError("preserve previous register receipt")
        result=register()
        output.write_text(json.dumps(result,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
        print(json.dumps(result["counts"]))
        raise SystemExit(0)
    if len(sys.argv) == 3 and sys.argv[1] == "--guards":
        raise SystemExit(guards(pathlib.Path(sys.argv[2]).resolve()))
    unittest.main()
