#!/usr/bin/env python3
"""Builder tests of scripts/c22-t2-gate.py (C24-R1-03): each malformation of otherwise valid evidence exits 2 and writes
nothing; valid evidence gives a finite verdict derived from the individual records with the analyser's p50 rule.

Usage: python scripts/test_c22_t2_gate.py   (CI `check`; standard library only)
"""
import copy
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

GATE = pathlib.Path(__file__).resolve().parent / "c22-t2-gate.py"


def valid():
    strokes = {
        "environment": {"os": "windows", "arch": "x86_64", "profile": "release", "startedAt": "2026-11-02T10:00:00Z"},
        "series": {"strokeAdd.wallMs": {"unit": "ms", "samples": [{"tMs": i, "value": 1000.0 + i} for i in range(6)]}},
    }
    ok = {"seconds": 0.9, "status": "Ok", "error": None, "committed": True, "unchangedOnRefusal": True}
    refused = {"seconds": 0.5, "status": "Error", "error": {"code": "limit.exceeded"}, "committed": False,
               "unchangedOnRefusal": True}
    flatten = {"os": "windows", "arch": "x86_64", "profile": "release", "unbudgeted": [dict(ok) for _ in range(5)],
               "budgeted": [dict(refused) for _ in range(5)], "unbudgetedAllCompleted": True}
    return strokes, flatten


def run(strokes, flatten):
    with tempfile.TemporaryDirectory() as tmp:
        d = pathlib.Path(tmp)
        (d / "s.json").write_text(json.dumps(strokes))
        (d / "f.json").write_text(json.dumps(flatten))
        r = subprocess.run([sys.executable, str(GATE), str(d / "s.json"), str(d / "f.json"), str(d / "o.json")],
                           capture_output=True, text=True)
        out = json.loads((d / "o.json").read_text()) if (d / "o.json").exists() else None
        return r.returncode, out


class Gate(unittest.TestCase):
    def test_valid_evidence_meets_and_uses_the_upper_nearest_rank(self):
        code, out = run(*valid())
        self.assertEqual(code, 0)
        # Six samples 1000..1005 ms: the upper nearest rank is sorted[round(2.5)] = sorted[3] = 1003 ms.
        self.assertEqual(out["strokeAddP50Seconds"], 1.003)
        self.assertTrue(out["met"])
        self.assertTrue(out["flattenUnbudgetedAllCompleted"])

    def test_a_valid_missed_gate_exits_0_with_met_false(self):
        s, f = valid()
        for r in f["unbudgeted"]:
            r["seconds"] = 40.0
        code, out = run(s, f)
        self.assertEqual((code, out["met"]), (0, False))

    def test_each_malformation_exits_2_and_writes_nothing(self):
        cases = {
            "negative": lambda s, f: f["unbudgeted"][2].__setitem__("seconds", -1),
            "boolean": lambda s, f: f["unbudgeted"][2].__setitem__("seconds", False),
            "infinite": lambda s, f: f["unbudgeted"][2].__setitem__("seconds", float("inf")),
            "string": lambda s, f: f["budgeted"][0].__setitem__("seconds", "1.0"),
            "boolean sample": lambda s, f: s["series"]["strokeAdd.wallMs"]["samples"][0].__setitem__("value", True),
            "negative sample": lambda s, f: s["series"]["strokeAdd.wallMs"]["samples"][0].__setitem__("value", -5),
            "too few": lambda s, f: f.__setitem__("unbudgeted", f["unbudgeted"][:4]),
            "missing series": lambda s, f: s.__setitem__("series", {}),
            "missing budgeted": lambda s, f: f.pop("budgeted"),
            "not a list": lambda s, f: f.__setitem__("unbudgeted", {"seconds": 1}),
            "record not an object": lambda s, f: f["unbudgeted"].__setitem__(0, 3.0),
            "other OS": lambda s, f: f.__setitem__("os", "macos"),
            "debug profile": lambda s, f: (s["environment"].__setitem__("profile", "debug"), f.__setitem__("profile", "debug")),
            "no run environment": lambda s, f: s.pop("environment"),
            "flag disagrees": lambda s, f: f["unbudgeted"][0].__setitem__("committed", False),
            "refusal not atomic": lambda s, f: f["budgeted"][1].__setitem__("unchangedOnRefusal", False),
            "error without code": lambda s, f: f["budgeted"][1].__setitem__("error", {}),
        }
        for name, mutate in cases.items():
            s, f = copy.deepcopy(valid())
            mutate(s, f)
            code, out = run(s, f)
            self.assertEqual((name, code, out), (name, 2, None))

    def test_completion_is_derived_from_the_records_not_the_flag(self):
        s, f = valid()
        f["unbudgeted"][0].update({"status": "Error", "error": {"code": "limit.exceeded"}, "committed": False})
        f.pop("unbudgetedAllCompleted")
        code, out = run(s, f)
        self.assertEqual((code, out["met"], out["flattenUnbudgetedAllCompleted"]), (0, False, False))


if __name__ == "__main__":
    unittest.main()
