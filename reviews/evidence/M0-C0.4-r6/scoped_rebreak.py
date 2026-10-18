"""R6 closure evidence through the real CLI; synthetic input, no product edits."""
import copy
import gzip
import json
import unittest
import closure_probes as c

reg = c.load('r6_prior_regression', 'tests/adversarial/c04-r5-decisions.py')
observations = []


def checked(name, log, expected=0):
    code, analysis = c.cli(name, log)
    text = gzip.decompress((c.OUT / (name + '.log.gz')).read_bytes()).decode('utf-8')
    assert code == expected, (name, code, text)
    if expected == 0:
        lines = text.split('### Actions', 1)[1].split('\n### ', 1)[0].splitlines()
        assert any('| native clock |' in line for line in lines), name
        rows = [line for line in lines if line.startswith('| A')]
        assert len(rows) == len(log['actions']), (name, rows)
        for row, action in zip(rows, analysis['actions']['actions']):
            cells = [part.strip() for part in row.strip('|').split('|')]
            assert len(cells) == 8, row
            assert cells[0] == action['id']
            assert cells[-1] == (action['native']['stats'] or {}).get('clock', '-'), row
    observations.append(dict(case=name, exit=code,
        actions=analysis['actions']['actions'] if analysis else None,
        decision=analysis['decision'] if analysis else None,
        error=text.strip() if expected else None))
    return analysis, text


class Closure(unittest.TestCase):
    def test_original_clock_pair_is_invariant(self):
        baseline, outside = reg.clock_pair()
        a, _ = checked('r6-clock-baseline', baseline)
        b, _ = checked('r6-clock-outside', outside)
        self.assertEqual(a['actions']['actions'], b['actions']['actions'])
        self.assertEqual(a['decision'], b['decision'])
        for result in (a, b):
            stats = result['actions']['actions'][1]['native']['stats']
            self.assertEqual((stats['clock'], stats['rateHz']), ('device', 200))
        self.assertEqual(a['native']['clock'], 'device')
        self.assertEqual(b['native']['clock'], 'host')

    def test_in_span_missing_or_mixed_device_time_falls_back_per_action(self):
        for mixed in (False, True):
            log = reg.clock_pair()[0]
            action = log['actions'][1]
            points = [p for p in log['native']['packets']
                      if action['startEpochMs'] <= p['hostEpochMs'] < action['endEpochMs']
                      and c.t.is_contact(p)]
            for p in (points[:1] if mixed else points):
                p.pop('deviceMs', None)
            outside = [p for p in log['native']['packets'] if c.t.is_contact(p) and
                       not action['startEpochMs'] <= p['hostEpochMs'] < action['endEpochMs']]
            self.assertTrue(outside)
            self.assertTrue(all('deviceMs' in p for p in outside))
            a, _ = checked('r6-in-span-' + ('mixed' if mixed else 'host-only'), log)
            stats = a['actions']['actions'][1]['native']['stats']
            self.assertEqual((stats['clock'], stats['rateHz']), ('host', 50))
            self.assertEqual(a['actions']['actions'][0]['native']['stats']['clock'], 'device')

    def test_web_only_action_and_empty_native_capture(self):
        for all_native in (False, True):
            log = reg.clock_pair()[0]
            span = log['actions'][1]
            log['native']['packets'] = [] if all_native else [p for p in log['native']['packets']
                if not span['startEpochMs'] <= p['hostEpochMs'] < span['endEpochMs']]
            a, _ = checked('r6-web-only-' + ('capture' if all_native else 'action'), log)
            action = a['actions']['actions'][1]
            self.assertIsNotNone(action['web']['stats'])
            self.assertIsNone(action['native']['stats'])
            self.assertEqual(action['native']['outcome'], 'performed, no events')

    def test_missing_keyup_names_pending_keydown_at_end_and_in_middle(self):
        for middle in (False, True):
            log = c.f.OutsideSpanAndKeys.keyed(c.f.manual_log(False))
            log['keyEvents'].pop()
            pending_index = len(log['keyEvents']) - 1
            if middle:
                when = log['keyEvents'][-1]['t']
                log['keyEvents'] += [
                    dict(type='keydown', key=' ', repeat=True, t=when + 1, outcome='ignored: repeat'),
                    dict(type='keydown', key='k', repeat=False, t=when + 2, outcome='pressed'),
                    dict(type='keyup', key='k', repeat=False, t=when + 3, outcome='ignored: all actions recorded')]
            _, text = checked('r6-missing-keyup-' + ('middle' if middle else 'end'), log, 2)
            self.assertIn(f"key event {pending_index}: keydown of ' ' never received its keyup", text)
            self.assertNotIn('### Decision', text)
            self.assertNotIn('Traceback', text)


if __name__ == '__main__':
    try:
        unittest.main(verbosity=2)
    finally:
        (c.OUT / 'scoped-rebreak-results.json').write_text(
            json.dumps(observations, indent=2) + '\n', encoding='utf-8')
