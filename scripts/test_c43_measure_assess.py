"""Fixture tests of scripts/c43_measure_assess.py (M4 C4.3b, C43B-R1S-08): a complete measurement within its budgets
passes; each way a measurement can be incomplete or wrong fails, on its own fixture.

Ruling D-52: every summary in a fixture is its population's (`summary`, ranked as desktop/src/canvas/metrics.ts ranks);
a fixture that changes a measurement changes the population and recomputes the summary (`measured`), and the fixtures
that report a summary its population does not give are refused.

Usage: python scripts/test_c43_measure_assess.py
"""
import copy
import importlib.util
import math
import pathlib
import unittest

HERE = pathlib.Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("assess", HERE / "c43_measure_assess.py")
assess = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(assess)

BOUNDS = {"frameMs": 17.5, "dragFrameMs": 17.5, "feedbackMs": 16.7, "commitMs": 50.0, "minSamples": 30}
DRAGS = 30
D52 = "every summary is computed from its population"


def summary(values):
    """The canvas's summary of a population, computed independently of the assessor (metrics.ts `stat`: the value at
    rank ceil(q * n))."""
    s = sorted(values)
    if not s:
        return {"n": 0, "p50": None, "p95": None, "max": None}
    at = lambda q: s[min(len(s) - 1, max(0, math.ceil(q * len(s)) - 1))]  # noqa: E731
    return {"n": len(s), "p50": at(0.5), "p95": at(0.95), "max": s[-1]}


def measured(values):
    """A summary that carries its population (`samples`), as the measuring script records work, feedback and commits."""
    return {**summary(values), "samples": list(values)}


def set_frames(sc, which, intervals):
    """A frame-time population (the scene's interval list) and its summary, changed together."""
    key, summ = ("idleIntervals", "idle") if which == "idle" else ("dragIntervals", "dragFrames")
    sc[key] = list(intervals)
    sc[summ] = summary(intervals)


#: The commits of a complete scene: thirty, p95 30 ms and max 31 ms (the reviewer's figures).
COMMITS = [30.0] * (DRAGS - 1) + [31.0]


def scene(input_="os"):
    sc = {"idleWork": measured([2.0] * 300), "dragWork": measured([3.0] * 300), "feedback": measured([12.0] * 300),
          "commits": measured(COMMITS), "input": input_, "drags": DRAGS}
    set_frames(sc, "idle", [16.7] * 300)
    set_frames(sc, "drag", [16.7] * 300)
    return sc


def complete(median=16.7):
    control = summary([median] * 300)
    return {
        "requestedBackend": "webgpu", "backend": "webgpu", "fallback": None,
        "control": {"frameMs": control, "startScreen": True},
        "sample": scene(), "groups1k": {"resident": 1002, **scene()},
    }


def failed(record, drags=DRAGS):
    return [name for ok, name, _ in assess.assess(record, BOUNDS, drags) if not ok]


class Assess(unittest.TestCase):
    def test_a_complete_measurement_within_budgets_passes(self):
        self.assertEqual(COMMITS and summary(COMMITS)["p95"], 30.0)
        self.assertEqual(failed(complete()), [])

    def test_a_complete_capped_measurement_passes_on_work_and_long_share(self):
        r = complete(median=33.3)
        for s in ("sample", "groups1k"):
            set_frames(r[s], "idle", [33.3] * 299 + [33.4])
            set_frames(r[s], "drag", [33.3] * 299 + [33.4])
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
        r["sample"]["commits"] = measured(COMMITS[:-1])
        self.assertTrue(any(n.startswith("sample: every one of the 30 drags") for n in failed(r)))
        r = complete()
        r["groups1k"]["drags"] = DRAGS - 5
        self.assertTrue(any(n.startswith("groups1k: every one of the 30 drags") for n in failed(r)))

    def test_short_rings_fail(self):
        for which, what in (("idle", "idle frame intervals"), ("drag", "frame intervals during the drags")):
            r = complete()
            set_frames(r["sample"], which, [16.7] * 120)
            self.assertIn(f"sample: 300 {what}", failed(r), which)
        for key, what in (("idleWork", "idle frames' work"), ("dragWork", "work of the frames drawn during the drags"),
                          ("feedback", "drag feedback samples")):
            r = complete()
            r["groups1k"][key] = measured([1.0] * 40)
            self.assertIn(f"groups1k: 300 {what}", failed(r), key)

    def test_missing_drag_work_fails_rather_than_passing_on_nothing(self):
        r = complete(median=33.3)
        for s in ("sample", "groups1k"):
            set_frames(r[s], "idle", [33.3] * 300)
            set_frames(r[s], "drag", [33.3] * 300)
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
        # Each rule alone, on a population and the summary computed from it: a non-finite work figure, a negative
        # feedback sample, a zero interval.
        for mutate in (lambda s: s.__setitem__("idleWork", measured([2.0] * 299 + [float("inf")])),
                       lambda s: s.__setitem__("feedback", measured([12.0] * 299 + [-1.0])),
                       lambda s: set_frames(s, "drag", [16.7] * 299 + [0.0])):
            r = complete()
            mutate(r["sample"])
            self.assertTrue(any("every measurement finite" in n for n in failed(r)))
        # An unordered reported summary is refused too.
        r = complete()
        r["sample"]["feedback"].update(p50=5.0, p95=4.0)
        self.assertTrue(any("every measurement finite" in n for n in failed(r)))
        # A valid record keeps passing the validation.
        self.assertFalse(any("every measurement finite" in n for n in failed(complete())))

    def test_a_summary_its_population_does_not_give_is_refused(self):
        # C43B-R3S-05, ruling D-52. The reviewer's exact input: the complete fixture with every commit sample a million
        # ms, `n` 30, `p95` 30.0 and `max` 31.0 left as they were.
        r = complete()
        r["sample"]["commits"]["samples"] = [1000000.0] * 30
        self.assertEqual((r["sample"]["commits"]["n"], r["sample"]["commits"]["p95"], r["sample"]["commits"]["max"]),
                         (30, 30.0, 31.0))
        names = failed(r)
        self.assertIn(f"sample: {D52} (D-52: n exactly, p50, p95 and max within 0.01 ms)", names)
        # No budget passes on the reported figures: none is assessed on a refused record.
        self.assertFalse(any("release-to-committed p95" in n for n in names if not n.startswith("sample: every")), names)
        detail = next(d for ok, n, d in assess.assess(r, BOUNDS, DRAGS) if n.startswith(f"sample: {D52}"))
        self.assertTrue(any("commits: reports p95 = 30.0, its population gives 1000000.0" in p for p in detail), detail)
        # The same input in the old fixture's literal shape (`commits` with n, p95, max and samples; no p50).
        r = complete()
        r["sample"]["commits"] = {"n": DRAGS, "p95": 30.0, "max": 31.0, "samples": [1000000.0] * 30}
        self.assertIn(f"sample: {D52} (D-52: n exactly, p50, p95 and max within 0.01 ms)", failed(r))
        # A count mismatch: thirty commits reported, twenty-nine measured.
        r = complete()
        r["groups1k"]["commits"]["samples"] = COMMITS[:-1]
        detail = next(d for ok, n, d in assess.assess(r, BOUNDS, DRAGS) if n.startswith(f"groups1k: {D52}"))
        self.assertIn("commits: reports n = 30, its population has 29", detail)
        self.assertIn(f"groups1k: {D52} (D-52: n exactly, p50, p95 and max within 0.01 ms)", failed(r))
        # A frame-time summary that is not its intervals' (p95 reported under the measured one), and a ring count.
        r = complete()
        r["groups1k"]["dragIntervals"] = [16.7] * 280 + [40.0] * 20
        self.assertTrue(any(n.startswith(f"groups1k: {D52}") for n in failed(r)))
        r = complete()
        r["sample"]["idleWork"]["n"] = 301
        self.assertTrue(any(n.startswith(f"sample: {D52}") for n in failed(r)))
        # Within the rounding tolerance a summary agrees; beyond it, it does not.
        r = complete()
        r["sample"]["commits"]["p95"] = 30.0 + assess.TOLERANCE_MS / 2
        self.assertEqual(failed(r), [])
        r["sample"]["commits"]["p95"] = 30.0 + 3 * assess.TOLERANCE_MS
        self.assertTrue(any(n.startswith(f"sample: {D52}") for n in failed(r)))

    def test_a_summary_without_its_population_is_refused(self):
        # Ruling D-52: a budgeted or counted quantity reported as a summary alone.
        for key in ("commits", "idleWork", "dragWork", "feedback"):
            r = complete()
            r["sample"][key].pop("samples")
            detail = next(d for ok, n, d in assess.assess(r, BOUNDS, DRAGS) if n.startswith(f"sample: {D52}"))
            self.assertTrue(any(p.startswith(f"{key}: a summary with no population") for p in detail), (key, detail))
        r = complete()
        del r["groups1k"]["idleIntervals"]
        self.assertTrue(any(n.startswith(f"groups1k: {D52}") for n in failed(r)))

    def test_budgets_use_the_population(self):
        # The budgets read the summary computed from the population: a consistent record over its bounds fails them.
        r = complete()
        r["sample"]["commits"] = measured([80.0] * DRAGS)
        self.assertTrue(any("release-to-committed" in n for n in failed(r)))
        r = complete()
        set_frames(r["groups1k"], "drag", [25.0] * 300)
        self.assertTrue(any(n.startswith("groups1k: frame time p95, during scene drags") for n in failed(r)))

    def test_a_missing_scene_fails(self):
        r = complete()
        del r["sample"]
        self.assertIn("sample: measured", failed(r))

    def test_one_drag_fails_the_workstation_bounds_even_when_it_committed(self):
        # C43B-R1-05: `--drags 1` with its one drag committed is still fewer than thirty samples per scene.
        r = complete()
        for s in ("sample", "groups1k"):
            r[s]["drags"] = 1
            r[s]["commits"] = measured([30.0])
        names = failed(r, drags=1)
        self.assertIn("sample: at least 30 actual release-to-committed samples", names)
        self.assertIn("groups1k: at least 30 actual release-to-committed samples", names)

    def test_agent_input_needs_no_feedback_samples(self):
        r = complete()
        for s in ("sample", "groups1k"):
            r[s]["input"] = "agent"
            r[s]["feedback"] = {"n": 0, "p50": None, "p95": None, "max": None}
        self.assertEqual(failed(copy.deepcopy(r)), [])

    def test_the_measuring_scripts_record_shape_is_accepted(self):
        # scripts/c43-camera-measure.py: `idle` and `dragFrames` are the canvas's summaries of `idleIntervals` and
        # `dragIntervals`; `idleWork`, `dragWork` and `feedback` are its summaries with their samples; `commits` is
        # summarised from its samples with the assessor's own p95 (no p50). Every value rounded to 0.01 ms as the
        # canvas rounds.
        def canvas(values):
            raw = [round(v, 2) for v in values]
            return raw, summary(raw)
        r = complete()
        for s in ("sample", "groups1k"):
            sc = r[s]
            for which, base in (("idle", 16.6), ("drag", 16.8)):
                raw, summ = canvas([base + (i % 7) * 0.013 for i in range(300)])
                key = "idleIntervals" if which == "idle" else "dragIntervals"
                sc[key], sc["idle" if which == "idle" else "dragFrames"] = raw, summ
            for key, base in (("idleWork", 1.9), ("dragWork", 2.7), ("feedback", 11.0)):
                raw, summ = canvas([base + (i % 11) * 0.031 for i in range(300)])
                sc[key] = {**summ, "samples": raw}
            mine = [round(29.0 + (i % 5) * 0.37, 2) for i in range(DRAGS)]
            sc["commits"] = {"n": len(mine), "p95": assess.p95(mine), "max": max(mine), "samples": mine}
        self.assertEqual(failed(r), [])


if __name__ == "__main__":
    unittest.main()
