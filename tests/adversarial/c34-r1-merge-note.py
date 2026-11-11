"""C34-R1-01: the merge receipt must list inherited open issues."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class MergeReceipt(unittest.TestCase):
    def test_open_issue_inventory_covers_register_headings(self):
        issues = (ROOT / 'plan/spec-issues.md').read_text(encoding='utf-8')
        note = (ROOT / 'reviews/M3-merge-note.md').read_text(encoding='utf-8')
        inventory = note.split('## 4. Open spec issues', 1)[1].split('Every other', 1)[0]
        open_ids = {
            match.group(1)
            for match in re.finditer(r'^## (SI-\d+) .*$', issues, re.MULTILINE)
            if 1 <= int(match.group(1).split('-')[1]) <= 110
            and re.search(r'\b(open|unresolved|awaiting spec owner)\b', match.group(0))
        }
        missing = sorted(issue for issue in open_ids if issue not in inventory)
        self.assertFalse(missing, 'C34-R1-01: merge note omits open issues: ' + ', '.join(missing))


if __name__ == '__main__':
    unittest.main()
