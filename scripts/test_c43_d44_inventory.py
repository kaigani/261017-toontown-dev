"""C43B-R3S-03 (ruling D-48): every dialog and question the window can show has its D-44 rows or a stated reason; a new
dialog without either fails. Pure (reads the front end's source): `python -I scripts/test_c43_d44_inventory.py`."""

import importlib.util
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("c43_d44_coverage", HERE / "c43_d44_coverage.py")
cov = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cov)


class Inventory(unittest.TestCase):
    def test_every_dialog_has_rows_or_a_reason(self):
        self.assertEqual(cov.problems(), [])

    def test_the_known_forms_are_in_the_inventory(self):
        items = cov.inventory()
        for i in ("dialog:Project Properties", "dialog:Check Project", "dialog:New Project", "dialog:Open Backup",
                  "message:project.unsavedChanges", "message:keymap.importMode", "popover:Jobs", "class:error box"):
            self.assertIn(i, items)

    def test_a_new_dialog_without_a_row_fails(self):
        items = cov.inventory() + ["dialog:Brand New Dialog"]
        self.assertEqual(cov.problems(items), ["dialog:Brand New Dialog: a dialog or question without a D-44 row or a reason"])

    def test_a_removed_dialog_leaves_a_stale_entry(self):
        items = [i for i in cov.inventory() if i != "dialog:Help"]
        self.assertIn("dialog:Help: listed in the D-44 coverage but no longer in the window's source", cov.problems(items))

    def test_dismiss_only_boxes_are_a_class_with_the_reason(self):
        dismiss = [k for k, v in cov.COVERAGE.items() if v.get("class") == cov.NO_COMMAND]
        for k in ("dialog:About Toontown", "dialog:Project Info", "dialog:Help", "class:error box",
                  "class:native picker", "class:information box", "class:tip with suppression box"):
            self.assertIn(k, dismiss)


if __name__ == "__main__":
    sys.exit(0 if unittest.main(exit=False).result.wasSuccessful() else 1)
