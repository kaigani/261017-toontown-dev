"""Permanent round-2 analyzer attacks. No product code or round-1 tests modified."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("tablet_analysis_r2", ROOT / "scripts/analyse-tablet-log.py")
analysis = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(analysis)


def fixture():
    origin = 1_700_000_000_000
    samples = [dict(kind="down" if i == 0 else "move", pointerType="pen", pointerId=7,
                    pressure=(i + 1) / 1024, t=100 + i * 4, handlerT=102 + i * 4,
                    tiltX=0, tiltY=0, twist=0, x=10, y=20, dispatch=i,
                    coalescedCount=1) for i in range(1024)]
    samples.append(dict(samples[-1], kind="up", pressure=0, t=4196, handlerT=4198))
    report = dict(method="InjectSyntheticPointerInput", frames=1024, hz=250, achievedHz=250,
                  injectEpochMs=[origin + 99 + i * 4 for i in range(1024)],
                  injectReturnEpochMs=[origin + 99.5 + i * 4 for i in range(1024)],
                  pressure=list(range(1, 1025)), tiltX=[0]*1024, tiltY=[0]*1024,
                  rotation=[0]*1024, screenX=[10]*1024, screenY=[20]*1024,
                  screenRect=[0, 0, 100, 100], pattern="stationary", target="webview")
    return dict(version=2, spike="tablet", environment=dict(timeOrigin=origin, coalescedApi=True),
                web=dict(samples=samples, frames=[], truncated=0), native=dict(packets=[]),
                runs=[dict(label="stationary fixture", target="webview", report=report,
                           request=dict(frames=1024, hz=250, left=0, top=0, width=100, height=100,
                                        pattern="stationary"),
                           webRange=[0, len(samples)], nativeRange=[0, 0], error=None, probe=None)])


def invoke(log):
    with tempfile.TemporaryDirectory(prefix="toontown-r2-") as tmp:
        path = Path(tmp) / "log.json"
        path.write_text(json.dumps(log), encoding="utf-8")
        return subprocess.run([sys.executable, "-I", str(ROOT / "scripts/analyse-tablet-log.py"), str(path)],
                              capture_output=True, text=True, timeout=30)


class AnalyzerAttacks(unittest.TestCase):
    def test_macos_extended_accessor_contracts(self):
        source = (ROOT / "desktop/src-tauri/src/spikes/tablet/native_macos.rs").read_text(encoding="utf-8")
        prelude = (ROOT / "tests/adversarial/c02-r2-macos-accessors.rs").read_text(encoding="utf-8")
        with tempfile.TemporaryDirectory(prefix="toontown-r2-mac-") as tmp:
            path = Path(tmp) / "mac.rs"
            path.write_text(prelude + "\n" + source[source.index("fn is_mouse_type("):], encoding="utf-8")
            exe = Path(tmp) / ("mac.exe" if sys.platform == "win32" else "mac")
            built = subprocess.run(["rustc", "--edition=2024", "--test", str(path), "-o", str(exe)],
                                   capture_output=True, text=True, timeout=60)
            self.assertEqual(built.returncode, 0, built.stderr)
            result = subprocess.run([str(exe), "--test-threads=1", "--nocapture"], capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_C02_R2_01_explicit_v1_is_rejected(self):
        log = fixture()
        log["version"] = 1
        result = invoke(log)
        self.assertNotEqual(result.returncode, 0, "Binding ruling requires explicit v1 rejection")
        self.assertIn("version", result.stderr.lower())

    def test_C02_R2_01_successful_v2_run_without_raw_report_fails_loudly(self):
        for target in ("webview", "nativeWindow"):
            with self.subTest(target=target):
                log = fixture()
                run = log["runs"][0]
                run.update(target=target, report=None)
                if target == "nativeWindow":
                    run["probe"] = dict(report=None, error=None, listeners={})
                result = invoke(log)
                self.assertNotEqual(result.returncode, 0, "Missing successful-run evidence must not look like n/a")
                self.assertIn("report", result.stderr.lower())

    def test_C02_R2_01_empty_report_does_not_fabricate_zero_frame_success(self):
        log = fixture()
        log["runs"][0]["report"] = {"pattern": "stationary"}
        result = invoke(log)
        self.assertNotEqual(result.returncode, 0, "Incomplete raw report currently prints received 0 / missing 0")

    def test_C02_R2_02_achieved_rate_is_recomputed_or_rejected(self):
        log = fixture()
        log["runs"][0]["report"]["achievedHz"] = 123456789
        try:
            result = analysis.run_analysis(log)
        except analysis.LogError:
            return  # Explicit inconsistency rejection is also a sound fix.
        self.assertAlmostEqual(result[0]["achievedHz"], 250, places=6,
                               msg="4 ms raw injection intervals cannot establish 123456789 Hz")

    def test_lossless_stationary_including_down_has_zero_missing(self):
        log = fixture()
        result = analysis.run_analysis(log)[0]["web"]
        self.assertEqual((result["received"], result["missing"]), (1024, 0))
        self.assertEqual(result["missingFrames"], [])
        self.assertEqual(result["repeatedPixelFrames"], 1023)

    def test_explicit_failure_without_report_remains_analyzable(self):
        log = fixture()
        log["runs"][0].update(report=None, error="spike.notForeground: covered")
        result = invoke(log)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("spike.notForeground", result.stdout)

    def test_C02_R2_04_offline_stroke_ranges_support_manual_tilt_threshold(self):
        log = fixture()
        for i, s in enumerate(log["web"]["samples"][:-1]):
            s.update(tiltX=-30 if i % 2 else 30, tiltY=-45 if i % 2 else 45)
        log["native"]["packets"] = [
            dict(kind="pose", pressure=s["pressure"], deviceMs=s["t"], hostEpochMs=s["t"],
                 tool="pen", tiltX=s["tiltX"], tiltY=s["tiltY"], twist=0)
            for s in log["web"]["samples"]
        ]
        for path, result in (("web", analysis.web_analysis(log)), ("native", analysis.native_analysis(log))):
            with self.subTest(path=path):
                stroke = result["strokes"][0]
                self.assertEqual(stroke.get("tiltRangeX"), [-30, 30],
                                 "tiltPresent alone cannot assess the manual's +/-30 degree target")
                self.assertEqual(stroke.get("tiltRangeY"), [-45, 45])

    def test_pause_segmentation_and_pose_only_native_reference_unknown(self):
        log = fixture()
        first = log["web"]["samples"]
        log["web"]["samples"] = first + [dict(s, t=s["t"] + 100000, handlerT=s["handlerT"] + 100000,
                                              dispatch=s["dispatch"] + 2000) for s in first]
        web = analysis.web_analysis(log)
        self.assertEqual([s["rateHz"] for s in web["strokes"]], [250, 250])
        packets = []
        for start in (0, 100000):
            for i, p in enumerate((0, .1, .5, 1, 0)):
                packets.append(dict(kind="pose", pressure=p, deviceMs=start+i*4,
                                    hostEpochMs=1700000000000+start+i*4, tool="pen", tiltX=0, tiltY=0, twist=0))
        log["native"]["packets"] = packets
        native = analysis.native_analysis(log)
        self.assertEqual([s["rateHz"] for s in native["strokes"]], [250, 250])
        self.assertEqual(native["loss"], "unknown, gap heuristic only")


if __name__ == "__main__":
    unittest.main(verbosity=2)
