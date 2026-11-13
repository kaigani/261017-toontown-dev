"""ADR decision 8 must qualify its accepted sequential-batch blocking exception."""
from pathlib import Path
import unittest

class BatchWaitWording(unittest.TestCase):
    def test_never_document_thread_claim_has_batch_exception(self):
        root=Path(__file__).resolve().parents[2]
        text=(root/'docs/decisions/ADR-0020-compositor-render-list.md').read_text(encoding='utf-8')
        decision=text.split('8. **Never block.**',1)[1].split('10. **Publication',1)[0]
        self.assertFalse('(never the document thread)' in decision and not any(word in decision.lower() for word in ['exception', 'except']),
            'C34-R2-03: unqualified never-document-thread claim contradicts accepted sequential batch waits; qualify the batch exception')

if __name__=='__main__':unittest.main()
