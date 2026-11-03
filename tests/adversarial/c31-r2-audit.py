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


class Audit(unittest.TestCase):
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
    if len(sys.argv) == 3 and sys.argv[1] == "--guards":
        raise SystemExit(guards(pathlib.Path(sys.argv[2]).resolve()))
    unittest.main()
