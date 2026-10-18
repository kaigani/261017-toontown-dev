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
import sys
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
        self.assertEqual(d["eligible"], ["W", "R", "H", "D"])
        self.assertEqual(d["shares"], {"W": "2/3", "R": "7/8", "H": "3/4", "D": "5/8"})
        self.assertEqual(d["disqualified"], {"H": [2], "D": [2]})
        self.assertEqual(d["outcome"], "go-with-conditions: R")
        self.assertEqual([(c["row"], c["result"]) for c in d["conditions"]], [(4, "fail")])
        self.assertIn("tilt unsupported", d["conditions"][0]["mitigation"])

    def test_worked_example_2_macos(self):
        ok = {r: "pass" for r in range(1, 10)}
        d = tablet.aggregate("macos", {"W": {**ok, 7: "fail", 8: "fail"},
                                       "A": {**ok, 6: "n/a", 7: "partial", 8: "fail"}})
        self.assertEqual(d["shares"], {"W": "7/9", "A": "3/4"})
        # W is disqualified (row 7 at fail) and listed only; A (row 7 partial) is selected.
        self.assertEqual(d["disqualified"], {"W": [7]})
        self.assertEqual(d["outcome"], "go-with-conditions: A")
        self.assertEqual([(c["row"], c["result"]) for c in d["conditions"]], [(7, "partial"), (8, "fail")])

    def test_worked_example_3_all_disqualified(self):
        ok = {r: "pass" for r in range(1, 10)}
        d = tablet.aggregate("macos", {"W": {**ok, 2: "fail"}, "A": {**ok, 6: "n/a", 7: "fail"}})
        self.assertIsNone(d["selected"])
        self.assertEqual(d["bestEvidence"], "W")  # W 8/9 > A 7/8; named only as evidence
        self.assertTrue(d["outcome"].startswith("no-go on this platform: escalate (plan §6 decision 7)"))

    def test_reviewer_r3_04_case_selects_r(self):
        # W fails pressure, so it is disqualified although its share 7/9 is higher than R's 3/4.
        ok = {r: "pass" for r in range(1, 10)}
        d = tablet.aggregate("windows", {"W": {**ok, 2: "fail", 3: "fail"}, "R": {**ok, 4: "fail", 5: "fail", 6: "n/a"}})
        self.assertEqual((d["scores"], d["shares"]), ({"W": 7, "R": 6}, {"W": "7/9", "R": "3/4"}))
        self.assertEqual((d["selected"], d["disqualified"]), ("R", {"W": [2]}))
        self.assertEqual(d["outcome"], "go-with-conditions: R")

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
        # Spans [start - 1, start + 4999): the last sample is at start + 4998. Actions are 1000 ms apart, so a skip in
        # between keeps the page's 500 ms minimum between marker transitions (C04-R4-02).
        records.append(dict(id=aid, state="done", startPerf=start - 1, endPerf=start + 4999,
                            startEpochMs=1e12 + start - 1, endEpochMs=1e12 + start + 4999))
    if not rotation:
        records.insert(3, dict(id="A4", state="skipped", startPerf=17499, endPerf=17499,
                               startEpochMs=1e12 + 17499, endEpochMs=1e12 + 17499))
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


class OutsideSpanAndKeys(unittest.TestCase):
    """C04-R4-01 (the reviewer's closure probe, made permanent, with boundaries taken from the fixture) and
    C04-R4-02 (the raw key log as a state machine)."""

    def decide(self, log):
        web, native = tablet.web_analysis(log), tablet.native_analysis(log)
        return tablet.tablet_decision(log, web, native, tablet.action_analysis(log, web, native))

    def stray(self, log, inside, tail):
        """A stray pen contact in A2: `inside` samples between the real A2 stroke's pointerup (end - 1) and A2's end,
        then `tail` samples after the end."""
        end = log["actions"][1]["endPerf"]
        s = copy.deepcopy(log["web"]["samples"][0])
        s.update(handlerT=0, dispatch=90000, coalescedCount=1)
        extra = [dict(s, t=end - (inside - i) * 0.2, kind="down" if i == 0 else "move") for i in range(inside)]
        extra += [dict(s, t=end + i, kind="move", dispatch=90001, coalescedCount=8, pressure=0.8) for i in range(tail)]
        extra.append(dict(s, t=end + tail, kind="up", pressure=0))
        log["web"]["samples"] = sorted(log["web"]["samples"] + extra, key=lambda q: q["t"])
        return log

    def test_outside_tail_cannot_choose_measured_episode(self):
        base = self.stray(manual_log(False), inside=1, tail=0)
        tailed = self.stray(manual_log(False), inside=1, tail=700)
        a, b = self.decide(base), self.decide(tailed)
        self.assertEqual(a["rows"], b["rows"], "an outside-span tail must not select a different A2 episode")
        self.assertEqual(a["outcome"], b["outcome"])
        # Two in-span samples: the tail must not turn a qualified candidate into a platform no-go either.
        slow, slow_tail = self.stray(manual_log(False), 2, 0), self.stray(manual_log(False), 2, 700)
        self.assertEqual(self.decide(slow)["outcome"], self.decide(slow_tail)["outcome"])

    def test_outside_tail_longer_than_the_in_span_episode(self):
        # A2's real episode has 600 samples; the stray contact has 3 inside and 2000 outside (2003 in total). Only
        # its clipped 3 compete, so the real episode stays the measurement.
        log = self.stray(manual_log(False), inside=3, tail=2000)
        web, native = tablet.web_analysis(log), tablet.native_analysis(log)
        actions = tablet.action_analysis(log, web, native)
        a2 = actions["actions"][1]["web"]
        self.assertEqual(a2["outcome"], "split: 2 episodes")
        self.assertEqual(a2["measured"], 2)
        self.assertEqual(a2["stats"]["samples"], web["strokes"][1]["samples"])
        self.assertGreater(web["strokes"][2]["samples"], a2["stats"]["samples"])  # the unclipped stray is longer
        self.assertFalse(a2["stats"]["coalescedObserved"])
        self.assertEqual(self.decide(log)["rows"], self.decide(manual_log(False))["rows"])

    @staticmethod
    def keyed(log):
        keys = []
        for a in log["actions"]:
            steps = [("k", a["startPerf"], "skipped")] if a["state"] == "skipped" else [
                (" ", a["startPerf"], "started"), (" ", a["endPerf"], "done")]
            for key, when, state in steps:
                keys += [dict(type="keydown", key=key, repeat=False, t=when, outcome="pressed"),
                         dict(type="keyup", key=key, repeat=False, t=when, outcome=f"transition: {a['id']} {state}")]
        log["keyEvents"] = keys
        return log

    def test_key_log_is_validated_as_a_state_machine(self):
        base = self.keyed(manual_log(False))
        web, native = tablet.web_analysis(base), tablet.native_analysis(base)
        self.assertEqual(tablet.action_analysis(base, web, native)["keyEvents"]["transitions"], 11)
        mutations = {
            "missing keydown": lambda k: [e for e in k if e["type"] == "keyup"],
            "transitions 499 ms apart": lambda k: [dict(e, t=(i // 2) * 499) for i, e in enumerate(k)],
            "wrong label": lambda k: [dict(e, outcome="transition: A6 skipped") if i == 1 else e for i, e in enumerate(k)],
            "transition on a keydown": lambda k: [dict(e, type="keydown") if i == 1 else e for i, e in enumerate(k)],
            "keydown of another key": lambda k: [dict(e, key="k") if i == 0 else e for i, e in enumerate(k)],
            "repeat keydown only": lambda k: [dict(e, repeat=True, outcome="ignored: auto-repeat") if i == 0 else e
                                              for i, e in enumerate(k)],
        }
        for name, mutate in mutations.items():
            with self.subTest(mutation=name):
                log = copy.deepcopy(base)
                log["keyEvents"] = mutate(log["keyEvents"])
                w, n = tablet.web_analysis(log), tablet.native_analysis(log)
                with self.assertRaisesRegex(tablet.LogError, r"key event \d+|need"):
                    tablet.action_analysis(log, w, n)

    def test_malformed_key_log_exits_2_from_the_cli(self):
        import subprocess
        import tempfile
        log = self.keyed(manual_log(False))
        log["keyEvents"] = [e for e in log["keyEvents"] if e["type"] == "keyup"]
        with tempfile.TemporaryDirectory() as d:
            path = pathlib.Path(d) / "log.json"
            path.write_text(json.dumps(log), encoding="utf-8")
            r = subprocess.run([sys.executable, "-I", str(ROOT / "scripts/analyse-tablet-log.py"), str(path)],
                               capture_output=True, text=True)
        self.assertEqual(r.returncode, 2)
        self.assertEqual(r.stdout, "")
        self.assertIn("key event 0: transition without a non-repeat keydown", r.stderr)


class NativeClockPerAction(unittest.TestCase):
    """C04-R5-01: the native rate clock is chosen per action from its clipped packets only."""

    @staticmethod
    def native_log(device_in_span=True, device_outside=True):
        log = manual_log(False)
        log["nativeInfo"] = {"backend": "test-native"}
        packets = [dict(kind={"move": "pose"}.get(q["kind"], q["kind"]), tool="pen", pressure=q["pressure"],
                        hostEpochMs=1e12 + q["t"], deviceMs=q["t"] / 4, tiltX=0, tiltY=0, twist=0)
                   for q in log["web"]["samples"]]
        if not device_in_span:
            for q in packets:
                del q["deviceMs"]
        end = log["actions"][-1]["endEpochMs"]
        outside = [dict(kind="down", tool="pen", pressure=0.5, hostEpochMs=end + 100),
                   dict(kind="up", tool="pen", pressure=0, hostEpochMs=end + 101)]
        if device_outside:
            for q in outside:
                q["deviceMs"] = 99999
        log["native"]["packets"] = packets + outside
        return log

    def a2_native(self, log):
        web, native = tablet.web_analysis(log), tablet.native_analysis(log)
        return tablet.action_analysis(log, web, native)["actions"][1]["native"]["stats"]

    def test_outside_packets_without_device_time_do_not_change_the_clock(self):
        clean, mixed = self.native_log(), self.native_log(device_outside=False)
        self.assertEqual(tablet.native_analysis(mixed)["clock"], "host")  # the capture-wide table falls back
        self.assertEqual(self.a2_native(mixed)["clock"], "device")  # the action does not
        self.assertEqual(self.a2_native(mixed)["rateHz"], self.a2_native(clean)["rateHz"])

    def test_in_span_packets_without_device_time_use_host_time_even_if_outside_ones_have_it(self):
        log = self.native_log(device_in_span=False, device_outside=True)
        stats = self.a2_native(log)
        self.assertEqual(stats["clock"], "host")
        self.assertAlmostEqual(stats["rateHz"], 1000 / 8.33, places=3)  # host time, not deviceMs / 4

    def test_missing_final_keyup_names_the_pending_keydown(self):
        log = OutsideSpanAndKeys.keyed(manual_log(False))
        keys = log["keyEvents"]
        log["keyEvents"] = keys[:-1]  # the last keyup (A6 done) is lost
        web, native = tablet.web_analysis(log), tablet.native_analysis(log)
        with self.assertRaisesRegex(tablet.LogError, rf"key event {len(keys) - 2}: keydown of ' ' never received"):
            tablet.action_analysis(log, web, native)


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
