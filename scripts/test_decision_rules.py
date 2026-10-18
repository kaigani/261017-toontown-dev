"""Regression tests for the executable C0.4 decision rule sets (review round 2: C04-R2-01, -02, -03).

Run: python -I scripts/test_decision_rules.py (also run by desktop/src/test/decisionRules.test.ts).

* The tablet analyzer's bands and mitigations equal the normative text of ADR-0009, every rate boundary falls into
  exactly one band, and the two worked examples of ADR-0009 / the go/no-go report give their stated outcomes.
* Action markers map the procedure's actions to contact episodes for both barrel-rotation branches, and a log
  without markers is not evidence.
* The canvas analyzer decides the reviewer's two round-2 counterexamples (an accepted partial matrix and complete
  null-cadence runs) as insufficient evidence, never selects an unscored back end, and applies the selection rule.
Standard library only.
"""

import copy
import importlib.util
import json
import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


tablet = load("tablet_analyzer", "scripts/analyse-tablet-log.py")
canvas = load("canvas_analyzer", "scripts/analyse-canvas-log.py")
ADR9 = (ROOT / "docs/decisions/ADR-0009-tablet-input.md").read_text(encoding="utf-8")


def table_rows(text, after):
    """Cells of the table rows following the heading text `after`, up to the next blank line after the table."""
    part = text[text.index(after):]
    rows, started = [], False
    for line in part.splitlines():
        if line.startswith("|"):
            started = True
            rows.append([c.strip() for c in line.strip("|").split("|")])
        elif started:
            break
    return rows[2:]  # drop header and separator


def half_open(cell):
    m = re.match(r"\[(\d+), (\d+|∞)\)", cell)
    assert m, f"not a half-open band: {cell}"
    return int(m[1]), (tablet.INF if m[2] == "∞" else int(m[2]))


class TabletRules(unittest.TestCase):
    def test_bands_equal_the_adr_text(self):
        rows = {int(r[0]): r for r in table_rows(ADR9, "**Step 1:")}
        for row, bands in tablet.BANDS.items():
            published = tuple(half_open(rows[row][i]) + (res,) for i, res in zip((3, 4, 5), ("pass", "partial", "fail")))
            self.assertEqual(published, bands, f"row {row}")

    def test_mitigations_equal_the_adr_text(self):
        rows = {int(r[0]): (r[1], r[2]) for r in table_rows(ADR9, "| # | Mitigation when partial |") if r[0].isdigit()}
        self.assertEqual(rows, tablet.MITIGATIONS)

    def test_every_rate_falls_in_exactly_one_band(self):
        for row in (7, 8):
            for hz in (0, 59.9, 60, 119, 119.1, 119.9, 120, 199, 199.1, 199.9, 200, 1e6):
                hits = [res for low, high, res in tablet.BANDS[row] if low <= hz < high]
                self.assertEqual(len(hits), 1, f"row {row} at {hz} Hz")
        self.assertEqual([tablet.band(7, v) for v in (59.9, 60, 119.9, 120)], ["fail", "partial", "partial", "pass"])
        self.assertEqual([tablet.band(8, v) for v in (119.9, 120, 199.9, 200)], ["fail", "partial", "partial", "pass"])

    def test_worked_example_1_windows(self):
        ok = {r: "pass" for r in range(1, 10)}
        results = {
            "W": {**ok, 2: "partial", 3: "fail", 4: "fail"},
            "R": {**ok, 4: "fail", 6: "n/a"},
            "H": {**ok, 2: "fail", 3: "fail", 6: "n/a"},
            "D": {**ok, 2: "fail", 3: "fail", 4: "fail", 6: "n/a"},
        }
        d = tablet.aggregate("windows", results)
        self.assertEqual(d["eligible"], ["W", "R", "H", "D"])  # nothing is excluded by a failed row (C04-R3-04)
        self.assertEqual(d["shares"], {"W": "2/3", "R": "7/8", "H": "3/4", "D": "5/8"})  # option (a)
        self.assertEqual(d["outcome"], "go-with-conditions: R")
        self.assertEqual([(c["row"], c["result"]) for c in d["conditions"]], [(4, "fail")])
        self.assertIn("tilt unsupported", d["conditions"][0]["mitigation"])

    def test_worked_example_2_macos(self):
        ok = {r: "pass" for r in range(1, 10)}
        d = tablet.aggregate("macos", {"W": {**ok, 7: "fail", 8: "fail"},
                                       "A": {**ok, 6: "n/a", 7: "partial", 8: "fail"}})
        self.assertEqual(d["shares"], {"W": "7/9", "A": "3/4"})
        # W is selected (7/9 > 3/4); its row 7 fail makes the platform a no-go (option (c)); A is not re-selected.
        self.assertEqual(d["selected"], "W")
        self.assertTrue(d["outcome"].startswith("no-go on this platform: escalate (plan §6 decision 7)"))
        self.assertEqual([(c["row"], c["result"]) for c in d["conditions"]], [(8, "fail")])

    def test_reviewer_r3_04_case_under_share_ranking_and_no_go(self):
        # W 7/9 beats R 6/8; W's pressure fail then makes the platform a no-go (options (a) + (c)).
        ok = {r: "pass" for r in range(1, 10)}
        d = tablet.aggregate("windows", {"W": {**ok, 2: "fail", 3: "fail"}, "R": {**ok, 4: "fail", 5: "fail", 6: "n/a"}})
        self.assertEqual((d["scores"], d["shares"]), ({"W": 7, "R": 6}, {"W": "7/9", "R": "3/4"}))
        self.assertEqual((d["selected"], d["blocking"]), ("W", [2]))
        self.assertTrue(d["outcome"].startswith("no-go on this platform"))

    def test_share_excludes_na_rows(self):
        # A native path is not penalised for row 6: A 8/8 beats W 8/9.
        ok = {r: "pass" for r in range(1, 10)}
        d = tablet.aggregate("macos", {"W": {**ok, 8: "partial"}, "A": {**ok, 6: "n/a"}})
        self.assertEqual((d["selected"], d["outcome"]), ("A", "go: A"))
        # A candidate with no applicable measured row is excluded.
        d = tablet.aggregate("macos", {"W": {**ok, 8: "partial"}, "A": {r: "n/a" for r in range(1, 10)}})
        self.assertEqual(d["eligible"], ["W"])

    def test_ties_follow_priority_and_insufficient_evidence_stops_the_evaluation(self):
        ok = {r: "pass" for r in range(1, 10)}
        d = tablet.aggregate("windows", {"W": {**ok, 5: "partial"}, "H": {**ok, 5: "pass", 6: "partial"}})
        self.assertEqual(d["outcome"], "go-with-conditions: W")  # 8 vs 8 passes: W has priority
        d = tablet.aggregate("windows", {"W": {**ok, 7: "insufficient"}})
        self.assertTrue(d["outcome"].startswith("insufficient evidence"))
        d = tablet.aggregate("macos", {"W": {r: "n/a" for r in range(1, 10)}})
        self.assertTrue(d["outcome"].startswith("not evidence"))


def manual_log(rotation, markers=True):
    """The reviewer's six-action fixture (tests/adversarial/c04-r2-decisions.py), with the page's action markers."""
    actions = ["A1", "A2", "A3"] + (["A4"] if rotation else []) + ["A5", "A6"]
    samples, records = [], []
    for index, aid in enumerate(actions):
        start = index * 6000
        for n in range(601):
            kind = "move" if aid == "A5" else ("down" if n == 0 else "up" if n == 600 else "move")
            samples.append(dict(t=start + n * 8.33, handlerT=start + n * 8.33 + 1, kind=kind, pointerType="pen",
                                pointerId=1, x=n if aid != "A6" else 20, y=20,
                                pressure=0 if aid == "A5" or kind == "up" else (n % 511 + 1) / 512,
                                tiltX=0, tiltY=0, twist=0, dispatch=len(samples), coalescedCount=1))
        records.append(dict(id=aid, state="done", startPerf=start - 100, endPerf=start + 5100,
                            startEpochMs=1e12 + start - 100, endEpochMs=1e12 + start + 5100))
    if not rotation:
        records.insert(3, dict(id="A4", state="skipped", startPerf=17500, endPerf=17500,
                               startEpochMs=1e12 + 17500, endEpochMs=1e12 + 17500))
    log = dict(version=3, source="manual", environment=dict(coalescedApi=True, userAgent="test"),
               metadata=dict(penTilt="yes", penRotation="yes" if rotation else "no"),
               web=dict(samples=samples, frames=[]), native=dict(packets=[]), runs=[], simulations=[])
    if markers:
        log["actions"] = records
    return log


class ActionMapping(unittest.TestCase):
    def analyse(self, log):
        web, native = tablet.web_analysis(log), tablet.native_analysis(log)
        actions = tablet.action_analysis(log, web, native)
        return actions, tablet.tablet_decision(log, web, native, actions)

    def test_stationary_action_maps_to_its_episode_in_both_branches(self):
        for rotation, a6 in ((False, 4), (True, 5)):
            with self.subTest(rotation=rotation):
                actions, _ = self.analyse(manual_log(rotation))
                by = {a["id"]: a for a in actions["actions"]}
                self.assertEqual(by["A6"]["web"]["measured"], a6)
                self.assertEqual(by["A5"]["web"]["outcome"], "performed, no events")
                self.assertEqual(by["A4"]["web"]["outcome"], "episode 4" if rotation else "omitted")

    def test_contacts_outside_every_action_change_no_decision_input(self):
        base = manual_log(False)
        stray = copy.deepcopy(base)
        # A batched contact before A1 (outside every span): it must not reach row 6 or any other row.
        before = [dict(s, t=s["t"] - 900, coalescedCount=4, dispatch=-1) for s in base["web"]["samples"][:50]]
        before[-1] = dict(before[-1], kind="up", pressure=0)
        stray["web"]["samples"] = before + base["web"]["samples"]
        (a1, d1), (a2, d2) = self.analyse(base), self.analyse(stray)
        self.assertEqual(a2["excludedSamples"]["web"], 50)
        self.assertEqual(d1["rows"], d2["rows"])
        self.assertEqual(d1["outcome"], d2["outcome"])

    def test_log_without_markers_is_not_evidence(self):
        _, d = self.analyse(manual_log(False, markers=False))
        self.assertTrue(d["outcome"].startswith("not evidence"))

    def test_fixture_decision_is_total(self):
        # Constant tilt with a tilt-capable pen is a measured fail, not missing evidence; the rate is 120 Hz.
        _, d = self.analyse(manual_log(False))
        self.assertEqual(d["rows"]["W"]["4"]["result"], "fail")
        self.assertEqual(d["rows"]["W"]["5"]["result"], "n/a")
        self.assertEqual(d["outcome"], "go-with-conditions: W")


class CanvasRules(unittest.TestCase):
    def decide(self, path):
        log = json.loads((ROOT / path).read_bytes())
        canvas.validate(log)  # accepted by the real analyzer
        return canvas.canvas_decision(log, [canvas.analyse_run(r) for r in log["runs"]])

    def test_reviewer_partial_matrix_is_insufficient_and_lists_missing_cells(self):
        d = self.decide("reviews/evidence/M0-C0.4-r2/partial-canvas.json")
        self.assertIsNone(d["selected"])
        self.assertTrue(d["outcome"].startswith("insufficient evidence"))
        self.assertTrue(any("groups1k: not measured" in m for m in d["notMeasured"]))

    def test_reviewer_null_cadence_on_both_back_ends_is_insufficient(self):
        d = self.decide("reviews/evidence/M0-C0.4-r2/null-cadence-both.json")
        self.assertIsNone(d["selected"])
        self.assertEqual(d["scores"], {})
        self.assertTrue(d["outcome"].startswith("insufficient evidence"))

    def synthetic(self, fps):
        """A decision over hand-made analyses: fps[backend] = [paths, groups, brush] (None = one sample)."""
        log = {"environment": {"webgpuApi": True}, "runs": []}
        analyses = []
        for backend, values in fps.items():
            for scene, v in zip(canvas.SCENES, values):
                log["runs"].append({"backend": backend, "scene": scene})
                analyses.append({"outcome": "ok", "fps": v})
        return canvas.canvas_decision(log, analyses)

    def test_unscored_back_end_is_never_selected_over_a_scored_one(self):
        d = self.synthetic({"webgpu": [None, 120, 120], "webgl2": [58, 58, 58]})
        self.assertEqual(d["selected"], "webgl2")
        self.assertTrue(d["provisional"])

    def test_selection_by_worst_scene_and_tie_to_webgpu(self):
        self.assertEqual(self.synthetic({"webgpu": [60, 40, 60], "webgl2": [60, 60, 60]})["selected"], "webgl2")
        self.assertEqual(self.synthetic({"webgpu": [45, 60, 40], "webgl2": [58, 60, 40]})["selected"], "webgpu")

    def test_outcome_rows(self):
        self.assertEqual(self.synthetic({"webgpu": [60, 60, 60], "webgl2": [60, 60, 60]})["outcome"], "go as written")
        self.assertTrue(self.synthetic({"webgl2": [60, 60, 40]})["outcome"].startswith("go-with-condition"))
        self.assertTrue(self.synthetic({"webgl2": [40, 60, 60]})["outcome"].startswith("go; M4 adds culling"))
        self.assertTrue(self.synthetic({"webgl2": [60, 40, 60]})["outcome"].startswith("no-go"))


if __name__ == "__main__":
    unittest.main(verbosity=2)
