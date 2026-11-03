#!/usr/bin/env python3
"""Builder tests of scripts/c22-t2-gate.py (C24-R1-03, C24-R2-03): each malformation of otherwise valid evidence exits 2,
writes nothing and names its own reason; valid evidence gives a finite verdict derived from the individual records with
the analyser's p50 rule.

Usage: python scripts/test_c22_t2_gate.py   (CI `check`; standard library only)
"""
import copy
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

GATE = pathlib.Path(__file__).resolve().parent / "c22-t2-gate.py"


def valid():
    strokes = {
        "environment": {"os": "windows", "arch": "x86_64", "profile": "release", "startedAt": "2026-11-02T10:00:00Z",
                        "runId": "41486947308-1-windows"},
        "series": {"strokeAdd.wallMs": {"unit": "ms", "samples": [{"tMs": i, "value": 1000.0 + i} for i in range(6)]}},
    }
    ok = {"seconds": 0.9, "status": "Ok", "error": None, "committed": True, "unchangedOnRefusal": True}
    refused = {"seconds": 0.5, "status": "Error", "error": {"code": "limit.exceeded"}, "committed": False,
               "unchangedOnRefusal": True}
    flatten = {"os": "windows", "arch": "x86_64", "profile": "release", "runId": "41486947308-1-windows",
               "startedAt": "2026-11-02T10:20:00Z", "unbudgeted": [dict(ok, sampleId=i) for i in range(5)],
               "budgeted": [dict(refused, sampleId=i) for i in range(5)], "unbudgetedAllCompleted": True}
    return strokes, flatten


def run(strokes, flatten, env=None):
    return run_full(strokes, flatten, env)[:2]


def run_full(strokes, flatten, env=None):
    with tempfile.TemporaryDirectory() as tmp:
        d = pathlib.Path(tmp)
        (d / "s.json").write_text(json.dumps(strokes))
        (d / "f.json").write_text(json.dumps(flatten))
        environ = {k: v for k, v in os.environ.items() if k != "TT_EXPECT_RUN_ID"}
        environ.update(env or {})
        r = subprocess.run([sys.executable, str(GATE), str(d / "s.json"), str(d / "f.json"), str(d / "o.json")],
                           capture_output=True, text=True, env=environ)
        out = json.loads((d / "o.json").read_text()) if (d / "o.json").exists() else None
        return r.returncode, out, r.stdout


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

    def test_round_2_malformations_exit_2_for_their_own_reason(self):
        # C24-R2-03: each case is otherwise valid evidence, so the message shows which check refused it.
        def dup(f, kind):
            f[kind] = [copy.deepcopy(f[kind][0]) for _ in range(5)]

        def samples(s):
            return s["series"]["strokeAdd.wallMs"]["samples"]

        cases = {
            "OS missing on both": (lambda s, f: (s["environment"].pop("os"), f.pop("os")), "environment.os is missing"),
            "arch missing on both": (lambda s, f: (s["environment"].pop("arch"), f.pop("arch")),
                                     "environment.arch is missing"),
            "profile not a string": (lambda s, f: (s["environment"].__setitem__("profile", 1), f.__setitem__("profile", 1)),
                                     "environment.profile is missing or not a string"),
            "run id missing": (lambda s, f: s["environment"].pop("runId"), "environment.runId is missing"),
            "run id missing on Flatten": (lambda s, f: f.pop("runId"), "Flatten runId is missing"),
            "run ids differ": (lambda s, f: f.__setitem__("runId", "41425059602-1-windows"), "different runs"),
            "Flatten before stroke-add": (lambda s, f: f.__setitem__("startedAt", "2026-11-01T10:00:00Z"),
                                          "contradictory start times"),
            "Flatten a day later": (lambda s, f: f.__setitem__("startedAt", "2026-11-03T10:00:00Z"),
                                    "contradictory start times"),
            "Flatten start missing": (lambda s, f: f.pop("startedAt"), "Flatten startedAt is missing"),
            "start not a timestamp": (lambda s, f: f.__setitem__("startedAt", "yesterday"), "not an ISO 8601 timestamp"),
            "duplicated unbudgeted records": (lambda s, f: dup(f, "unbudgeted"), "duplicated unbudgeted records"),
            "duplicated budgeted records": (lambda s, f: dup(f, "budgeted"), "duplicated budgeted records"),
            "record without identity": (lambda s, f: f["unbudgeted"][1].pop("sampleId"), "sampleId is not"),
            "duplicated stroke-add sample": (lambda s, f: samples(s).__setitem__(1, dict(samples(s)[0])),
                                             "duplicated stroke-add sample"),
            "refusal with status Ok": (lambda s, f: f["budgeted"][0].__setitem__("status", "Ok"),
                                       "neither a completion nor an atomic refusal"),
            "completion with an error": (lambda s, f: f["unbudgeted"][0].__setitem__("error", {"code": "limit.exceeded"}),
                                         "neither a completion nor an atomic refusal"),
            "unbudgeted refusal not atomic": (
                lambda s, f: f["unbudgeted"][0].update(status="Error", error={"code": "limit.exceeded"}, committed=False,
                                                       unchangedOnRefusal=False),
                "neither a completion nor an atomic refusal"),
            "huge integer duration": (lambda s, f: f["unbudgeted"][0].__setitem__("seconds", 10 ** 400), "too large"),
            "huge integer sample": (lambda s, f: samples(s)[0].__setitem__("value", 10 ** 400), "too large"),
            # C24-R3-01: observations carry identity in every accepted shape; the unit is checked first.
            "bare numeric samples": (lambda s, f: s["series"]["strokeAdd.wallMs"].__setitem__("samples", [1000] * 6),
                                     "has no identity"),
            "series as a bare list": (lambda s, f: s["series"].__setitem__("strokeAdd.wallMs", [1000.0 + i for i in range(6)]),
                                      "not an object with a unit"),
            "sample without a time": (lambda s, f: samples(s)[2].pop("tMs"), "has no identity"),
            "repeated time": (lambda s, f: samples(s)[2].__setitem__("tMs", samples(s)[1]["tMs"]),
                              "duplicated stroke-add sample"),
            "unit in seconds": (lambda s, f: s["series"]["strokeAdd.wallMs"].__setitem__("unit", "s"), "not 'ms'"),
            "unit missing": (lambda s, f: s["series"]["strokeAdd.wallMs"].pop("unit"), "not 'ms'"),
        }
        for name, (mutate, reason) in cases.items():
            s, f = copy.deepcopy(valid())
            mutate(s, f)
            code, out, stdout = run_full(s, f)
            self.assertEqual((name, code, out), (name, 2, None))
            self.assertIn(reason, stdout, name)

    def test_the_expected_run_id_binds_the_evidence_to_this_run(self):
        code, out = run(*valid(), env={"TT_EXPECT_RUN_ID": "41486947308-1-windows"})
        self.assertEqual((code, out["runId"]), (0, "41486947308-1-windows"))
        code, out, stdout = run_full(*valid(), env={"TT_EXPECT_RUN_ID": "41499999999-1-windows"})
        self.assertEqual((code, out), (2, None))
        self.assertIn("not this run", stdout)

    def test_equal_durations_with_distinct_identities_are_legitimate(self):
        s, f = valid()
        for r in f["unbudgeted"]:
            r["seconds"] = 0.9
        for x in s["series"]["strokeAdd.wallMs"]["samples"]:
            x["value"] = 1000.0
        code, out = run(s, f)
        self.assertEqual((code, out["met"]), (0, True))

    def test_completion_is_derived_from_the_records_not_the_flag(self):
        s, f = valid()
        f["unbudgeted"][0].update({"status": "Error", "error": {"code": "limit.exceeded"}, "committed": False})
        f.pop("unbudgetedAllCompleted")
        code, out = run(s, f)
        self.assertEqual((code, out["met"], out["flattenUnbudgetedAllCompleted"]), (0, False, False))


if __name__ == "__main__":
    unittest.main()
