"""Independent, bounded documentation checks for the adopted cancellation ruling."""
from pathlib import Path
import re
import unittest
ROOT = Path(__file__).resolve().parents[2]

class CancellationDocumentation(unittest.TestCase):
    def test_adr_distinguishes_poll_interval_and_bounded_batch_exception(self):
        text = (ROOT / 'docs/decisions/ADR-0020-compositor-render-list.md').read_text(encoding='utf-8')
        decision = text.split('8. **Never block.**', 1)[1].split('10. **Publication', 1)[0]
        self.assertIn('not an end-to-end bound', decision)
        self.assertIn('document thread', decision)
        self.assertIn('remaining watchdog budget minus its margin', decision)
        self.assertIn('M11 C11.1', decision)
        self.assertIn('nothing is remembered', decision)
        self.assertIn('A commit that got there first stands', decision)
        self.assertIn('Request cancellation never cancels a job', decision)

    def test_si111_preserves_the_adopted_interim_shape(self):
        text = (ROOT / 'plan/spec-issues.md').read_text(encoding='utf-8')
        item = re.search(r'^## SI-111\b.*?(?=^## SI-|\Z)', text, re.M | re.S)
        self.assertIsNotNone(item)
        self.assertIn('interim, awaiting spec owner', item.group())
        self.assertIn('command.timeout', item.group())
        self.assertIn('details.reason: "cancelled"', item.group())
        self.assertIn('details.jobId', item.group())
        self.assertIn('no answer', item.group())

if __name__ == '__main__':
    unittest.main()
