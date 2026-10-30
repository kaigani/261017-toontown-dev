"""Malformed measurement summaries must fail the evidence gate, even when Python compares bool == int."""
import importlib.util
import json
import pathlib
import re
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("measurements", ROOT / "scripts/analyse-measurements.py")
analyzer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(analyzer)


class Measurements(unittest.TestCase):
    def test_no_new_product_allow_attributes_since_round_two(self):
        diff = subprocess.check_output(["git", "diff", "--unified=0", "0e7d02e", "--",
                                       "crates/*/src/**", "bins/*/src/**", "desktop/src/**"], cwd=ROOT, text=True)
        added = [line for line in diff.splitlines() if line.startswith("+") and not line.startswith("+++")]
        self.assertEqual([line for line in added if re.search(r"#\[allow", line)], [],
                         "standing review rule forbids new product allow attributes")

    def test_boolean_summary_fields_are_not_valid_numeric_evidence(self):
        for field in ["n", "p50", "p90", "p99", "max"]:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                summary = {"n": 1, "p50": 1, "p90": 1, "p99": 1, "max": 1}
                summary[field] = True
                doc = {"environment": {"os": "windows", "arch": "x86_64", "cpus": 1,
                       "profile": "test", "startedAt": "2026-10-30T00:00:00Z", "timeOrigin": "start"},
                       "series": {"latency": {"unit": "ms", "samples": [{"tMs": 0, "value": 1}], "summary": summary}}}
                path = pathlib.Path(tmp) / "latency-windows.json"
                path.write_text(json.dumps(doc), encoding="utf-8")
                self.assertFalse(analyzer.check_file(path)["ok"], f"accepted boolean summary.{field}")

    def test_malformed_shapes_are_rejected(self):
        for doc in [{"series": []}, {"series": {}}, {"series": {"x": None}},
                    {"series": {"x": {"samples": [None]}}},
                    {"series": {"x": {"samples": [{"tMs": False, "value": 0}]}}},
                    {"series": {"x": {"samples": [{"tMs": 0, "value": float("inf")}]}}}]:
            with self.subTest(doc=doc), tempfile.TemporaryDirectory() as tmp:
                path = pathlib.Path(tmp) / "latency-windows.json"
                path.write_text(json.dumps(doc), encoding="utf-8")
                self.assertFalse(analyzer.check_file(path)["ok"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
