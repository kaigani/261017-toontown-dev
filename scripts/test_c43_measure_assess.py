"""Fixture tests of scripts/c43_measure_assess.py (M4 C4.3b, C43B-R1S-08): a complete measurement within its budgets
passes; each way a measurement can be incomplete or wrong fails, on its own fixture.

Usage: python scripts/test_c43_measure_assess.py
"""
import copy
import importlib.util
import pathlib
import unittest

HERE = pathlib.Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("assess", HERE / "c43_measure_assess.py")
assess = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(assess)

BOUNDS = {"frameMs": 17.5, "dragFrameMs": 17.5, "feedbackMs": 16.7, "commitMs": 50.0, "minSamples": 30}
DRAGS = 30


def stat(n, p95, p50=None):
    return {"n": n, "p50": p50 if p50 is not None else p95, "p95": p95, "max": p95}


def scene(input_="os"):
    return {
        "idle": stat(300, 16.8), "idleWork": stat(300, 2.0), "idleIntervals": [16.7] * 300,
        "dragFrames": stat(300, 16.9), "dragWork": stat(300, 3.0), "dragIntervals": [16.7] * 300,
        "feedback": stat(300, 12.0), "commits": {"n": DRAGS, "p95": 30.0, "max": 31.0, "samples": [30.0] * DRAGS},
        "input": input_, "drags": DRAGS,
    }


def complete(median=16.7):
    return {
        "requestedBackend": "webgpu", "backend": "webgpu", "fallback": None,
        "control": {"frameMs": stat(300, median, median), "startScreen": True},
        "sample": scene(), "groups1k": {"resident": 1002, **scene()},
    }


def failed(record):
    return [name for ok, name, _ in assess.assess(record, BOUNDS, DRAGS) if not ok]


class Assess(unittest.TestCase):
    def test_a_complete_measurement_within_budgets_passes(self):
        self.assertEqual(failed(complete()), [])

    def test_a_complete_capped_measurement_passes_on_work_and_long_share(self):
        r = complete(median=33.3)
        for s in ("sample", "groups1k"):
            r[s]["idleIntervals"] = [33.3] * 300
            r[s]["dragIntervals"] = [33.3] * 300
            r[s]["idle"] = stat(300, 33.4)
            r[s]["dragFrames"] = stat(300, 33.4)
        self.assertEqual(failed(r), [])
        self.assertIn("D-33", r["assessment"])

    def test_another_back_end_or_a_fallback_fails(self):
        r = complete()
        r["backend"] = "webgl2"
        self.assertTrue(any("requested back end" in n for n in failed(r)))
        r = complete()
        r["fallback"] = "webgpu adapter unavailable"
        self.assertTrue(any("requested back end" in n for n in failed(r)))
        r = complete()
        r["backend"] = None
        self.assertTrue(any("requested back end" in n for n in failed(r)))

    def test_too_few_resident_drawings_fail(self):
        r = complete()
        r["groups1k"]["resident"] = 1001
        self.assertTrue(any("1002 drawings resident" in n for n in failed(r)))

    def test_a_drag_that_did_not_commit_fails(self):
        r = complete()
        r["sample"]["commits"]["n"] = DRAGS - 1
        self.assertTrue(any(n.startswith("sample: every one of the 30 drags") for n in failed(r)))
        r = complete()
        r["groups1k"]["drags"] = DRAGS - 5
        self.assertTrue(any(n.startswith("groups1k: every one of the 30 drags") for n in failed(r)))

    def test_short_rings_fail(self):
        for key, what in (("idleIntervals", "idle frame intervals"), ("dragIntervals", "frame intervals during the drags")):
            r = complete()
            r["sample"][key] = [16.7] * 120
            self.assertIn(f"sample: 300 {what}", failed(r), key)
        for key, what in (("idleWork", "idle frames' work"), ("dragWork", "work of the frames drawn during the drags"),
                          ("feedback", "drag feedback samples")):
            r = complete()
            r["groups1k"][key] = stat(40, 1.0)
            self.assertIn(f"groups1k: 300 {what}", failed(r), key)

    def test_missing_drag_work_fails_rather_than_passing_on_nothing(self):
        r = complete(median=33.3)
        r["sample"]["dragWork"] = None
        names = failed(r)
        self.assertIn("sample: 300 work of the frames drawn during the drags", names)
        self.assertTrue(any("work per frame p95 during scene drags" in n for n in names))

    def test_an_incomplete_control_fails(self):
        r = complete()
        r["control"]["frameMs"]["n"] = 200
        self.assertTrue(any("D-33 control" in n for n in failed(r)))
        r = complete()
        r["control"]["startScreen"] = False
        self.assertTrue(any("D-33 control" in n for n in failed(r)))

    def test_an_implausible_control_cadence_fails_before_either_budget(self):
        # C43B-R2-05: 300 frames with a cadence of zero (the reviewer's `zero-control`), a negative or absent one, an
        # unordered one, one faster than a display or slower than 10 frames a second: each fails, whatever branch.
        for name, frame in (("zero", {"p50": 0, "p95": 0, "max": 0}), ("negative", {"p50": -16.7, "p95": -16.7, "max": -16.7}),
                            ("absent", {"p50": None, "p95": None, "max": None}), ("unordered", {"p50": 20.0, "p95": 16.7, "max": 30.0}),
                            ("too fast", {"p50": 1.0, "p95": 1.0, "max": 1.0}), ("too slow", {"p50": 250.0, "p95": 250.0, "max": 250.0}),
                            ("not finite", {"p50": float("inf"), "p95": float("inf"), "max": float("inf")})):
            r = complete()
            r["control"]["frameMs"].update(frame)
            self.assertTrue(any("plausible cadence" in n for n in failed(r)), name)
        # A 60 Hz control and a 30 Hz capped one pass the check.
        for median in (16.7, 33.3):
            self.assertFalse(any("plausible cadence" in n for n in failed(complete(median=median))), median)

    def test_impossible_scene_measurements_are_refused_before_any_budget(self):
        # C43B-R3S-05 (ruling D-48): the reviewer's record: a plausible control, complete counts, every scene timing -1.
        def neg():
            return {"n": 300, "p50": -1, "p95": -1, "max": -1}

        def bad_scene():
            return {"idle": neg(), "idleWork": neg(), "idleIntervals": [-1] * 300, "dragFrames": neg(),
                    "dragWork": neg(), "dragIntervals": [-1] * 300, "feedback": neg(),
                    "commits": {"n": DRAGS, "p50": -1, "p95": -1, "max": -1}, "input": "os", "drags": DRAGS}
        r = complete()
        r["sample"], r["groups1k"] = bad_scene(), {"resident": 1002, **bad_scene()}
        names = failed(r)
        self.assertTrue(any("every measurement finite" in n for n in names), names)
        # No budget is assessed on them: the failures are the validation's (and nothing passes).
        self.assertFalse(any("p95 <= 8 ms" in n or "frame time" in n for n in names), names)
        # Each rule alone: a non-finite work figure, an unordered summary, a zero interval.
        for mutate in (lambda s: s["idleWork"].update(p95=float("inf")),
                       lambda s: s["feedback"].update(p50=5.0, p95=4.0),
                       lambda s: s.__setitem__("dragIntervals", [16.7] * 299 + [0.0])):
            r = complete()
            mutate(r["sample"])
            self.assertTrue(any("every measurement finite" in n for n in failed(r)))
        # A valid record keeps passing the validation.
        self.assertFalse(any("every measurement finite" in n for n in failed(complete())))

    def test_a_missing_scene_fails(self):
        r = complete()
        del r["sample"]
        self.assertIn("sample: measured", failed(r))

    def test_budgets_still_apply(self):
        r = complete()
        r["sample"]["commits"]["p95"] = 80.0
        self.assertTrue(any("release-to-committed" in n for n in failed(r)))
        r = complete()
        r["groups1k"]["dragFrames"] = stat(300, 25.0)
        self.assertTrue(any(n.startswith("groups1k: frame time p95, during scene drags") for n in failed(r)))

    def test_one_drag_fails_the_workstation_bounds_even_when_it_committed(self):
        # C43B-R1-05: `--drags 1` with its one drag committed is still fewer than thirty samples per scene.
        r = complete()
        for s in ("sample", "groups1k"):
            r[s]["drags"] = 1
            r[s]["commits"] = {"n": 1, "p95": 30.0, "max": 30.0, "samples": [30.0]}
        names = [n for ok, n, _ in assess.assess(r, BOUNDS, 1) if not ok]
        self.assertIn("sample: at least 30 actual release-to-committed samples", names)
        self.assertIn("groups1k: at least 30 actual release-to-committed samples", names)

    def test_agent_input_needs_no_feedback_samples(self):
        r = complete()
        for s in ("sample", "groups1k"):
            r[s]["input"] = "agent"
            r[s]["feedback"] = stat(0, None)
        self.assertEqual(failed(copy.deepcopy(r)), [])


if __name__ == "__main__":
    unittest.main()
