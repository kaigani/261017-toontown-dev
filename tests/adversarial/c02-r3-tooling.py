"""R3 regression: missing synthetic run records must not become manual/hardware evidence."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("r2_fixture", ROOT / "tests/adversarial/c02-r2-tooling.py")
r2 = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(r2)


class EvidenceClassification(unittest.TestCase):
    def test_C02_R3_01_orphaned_synthetic_reports_are_not_manual_hardware_logs(self):
        for version in (2, 3):
            for absent in (False, True):
                with self.subTest(version=version, run_list_absent=absent):
                    log = r2.fixture()
                    log["version"] = version
                    run = log["runs"][0]
                    if version == 3:
                        run["report"].update(contactStartNs=0, contactEndNs=4_096_000_000,
                                             contactStartEpochMs=1_700_000_000_099,
                                             contactEndEpochMs=1_700_000_004_195, focus=[])
                    # The current page saves both runs[] and simulations[]. Losing the first list
                    # cannot turn the retained raw synthetic report into a real hardware capture.
                    log["simulations"] = [dict(label=run["label"], request=run["request"], report=run["report"])]
                    if absent:
                        del log["runs"]
                    else:
                        log["runs"] = []
                    result = r2.invoke(log)
                    self.assertNotEqual(result.returncode, 0,
                                        "Synthetic evidence without its run records must fail, not print manual/hardware")
                    self.assertEqual(result.stdout, "", "Reject missing evidence before emitting results")


if __name__ == "__main__":
    unittest.main(verbosity=2)
