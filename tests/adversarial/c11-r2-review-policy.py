"""C1.1 R2 permanent process regressions; compare a response commit with R1.

Run: python tests/adversarial/c11-r2-review-policy.py
TT_REVIEW_RESPONSE can select the next response; default HEAD. This deliberately
fails on the reviewed response, like the adjacent model defect tests.
"""
import os
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]
RESPONSE = os.environ.get("TT_REVIEW_RESPONSE", "HEAD")


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True, encoding="utf-8")


class ReviewPolicy(unittest.TestCase):
    def test_only_two_authorized_reviewer_test_edits(self):
        # Builder-owned properties/goldens and newly added tests are deliberately
        # excluded: the ruling protects reviewer-owned tests, not all test code.
        changed = set(git("diff", "--name-only", "--diff-filter=MD", "0460ad0", RESPONSE,
                          "--", "crates/model/tests/c11_r1_adversarial.rs",
                          "crates/engine/tests/c11_r1_wire.rs",
                          "desktop/src/ipc/c11-r1-adversarial.test.ts",
                          "desktop/src/test", "tests/adversarial").splitlines())
        allowed = {"crates/model/tests/c11_r1_adversarial.rs",
                   "crates/engine/tests/c11_r1_wire.rs"}
        self.assertFalse(changed - allowed, "C11-R2-01: unauthorized reviewer-test edit: " + str(changed - allowed))

    def test_claimed_response_raw_logs_are_committed(self):
        prefix = "reviews/evidence/M1-C1.1-r1-response/"
        tracked = set(git("ls-tree", "-r", "--name-only", RESPONSE, "--", prefix).splitlines())
        required = {prefix + "rust-10mb-drawing-release.log", prefix + "ts-10mb-drawing-node.log"}
        self.assertFalse(required - tracked, "C11-R2-09: measurement logs cited by receipt are missing: " + str(required - tracked))
        self.assertTrue(any(p.startswith(prefix + "gates/") and p.endswith(".log") for p in tracked),
                        "C11-R2-09: response claims raw gates/ logs, but none is committed")


if __name__ == "__main__":
    unittest.main()
