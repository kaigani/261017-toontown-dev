"""Ruling D-59 item 2 (C44A-R1S-02): every commanded control of the generated control inventory has its D-39 rows or a
stated reason; a new control without either fails. Pure (reads the parity manifest and the table's source):
`python -I scripts/test_c43_d39_inventory.py`."""

import importlib.util
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("c43_d39_coverage", HERE / "c43_d39_coverage.py")
cov = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cov)


class Inventory(unittest.TestCase):
    def test_every_commanded_control_has_rows_or_a_reason(self):
        self.assertEqual(cov.problems(), [])

    def test_the_reviewed_controls_have_rows(self):
        # C44A-R1S-02: each Pen setting at its slider's release and in its numeric field, and each Text number.
        for control, row in (("pen.minSize", "Pen panel › Minimum size (slider)"),
                             ("pen.smoothness", "Pen panel › Smoothness (slider)"),
                             ("pen.tipAngle", "Pen panel › Tip angle (slider)"),
                             ("pen.maxSize.field", "Pen panel › Maximum size (typed)"),
                             ("pen.minSize.field", "Pen panel › Minimum size (typed)"),
                             ("text.kerning", "Text panel › kerning (typed)"),
                             ("text.leftMargin", "Text panel › leftMargin (typed)"),
                             ("text.rightMargin", "Text panel › rightMargin (typed)"),
                             ("text.indent", "Text panel › indent (typed)"),
                             ("text.lineSpacing", "Text panel › lineSpacing (typed)")):
            self.assertIn(control, cov.inventory())
            self.assertEqual(cov.COVERAGE[control], {"rows": [row]})

    def test_a_new_control_without_a_row_fails(self):
        items = cov.inventory() + ["brandNew.control"]
        self.assertIn("brandNew.control: a commanded control without a D-39 row or a reason", cov.problems(items))

    def test_a_removed_control_leaves_a_stale_entry(self):
        items = [i for i in cov.inventory() if i != "inspector.preview"]
        self.assertIn("inspector.preview: listed in the D-39 coverage but matches no control of the generated inventory",
                      cov.problems(items))

    def test_a_row_the_table_does_not_define_fails(self):
        cov.COVERAGE["inspector.preview"] = {"rows": ["Drawing inspector › no such row"]}
        try:
            self.assertIn("inspector.preview: the D-39 script defines no row named 'Drawing inspector › no such row'",
                          cov.problems())
        finally:
            cov.COVERAGE["inspector.preview"] = {"rows": ["Drawing inspector › Show preview"]}

    def test_the_controls_before_d39_are_a_visible_list(self):
        # A control added after C4.3b needs a row: this list grows only by a deliberate edit, seen in review.
        self.assertEqual(cov.before_d39(), sorted([
            "cameraView.focus", "cameraView.type.side", "cameraView.type.top", "history.*", "keymap.capture",
            "keymap.remove.*", "keymap.reset", "mainToolbar.firstFrame", "mainToolbar.lipsync.panel.open",
            "mainToolbar.playback.loop.set", "mainToolbar.workspaceList", "mainToolbar.workspaceList.*",
            "preferences.*.restore", "start.*", "tab.*", "tips.*"]))


if __name__ == "__main__":
    sys.exit(0 if unittest.main(exit=False).result.wasSuccessful() else 1)
