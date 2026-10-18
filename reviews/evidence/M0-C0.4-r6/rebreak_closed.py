"""Scoped R5 probes. Synthetic fixtures; retained CLI inputs, output and observations.

This observational runner is copied from R5; only the clock counterexample expectations below are updated
to assert the now-required invariant. The unchanged historical runner is retained.
"""
import copy
import gzip
import json
import re
import unittest
import closure_probes as c

reg = c.load('r5_regression', 'tests/adversarial/c04-r5-decisions.py')
observations = []


def paths(log, shift=0):
    c.native_copy(log)
    for p in log['native']['packets']:
        p['hostEpochMs'] += shift
    for a in log['actions']:
        a['startEpochMs'] += shift
        a['endEpochMs'] += shift
    return log


def stray(inside, tail):
    log = c.f.manual_log(False)
    end = log['actions'][1]['endPerf']
    q = dict(log['web']['samples'][0], handlerT=end, coalescedCount=1)
    # All outside samples stay in the inter-action gap, even for a 4000-sample tail.
    extra = [dict(q, t=end-.9+i*.8/inside, kind='down' if i == 0 else 'move')
             for i in range(inside)]
    extra += [dict(q, t=end+i*.1, kind='move', coalescedCount=8, pressure=.8)
              for i in range(tail)]
    extra.append(dict(q, t=end+tail*.1, kind='up', pressure=0))
    log['web']['samples'] = sorted(log['web']['samples']+extra, key=lambda s: s['t'])
    return paths(log, 1_000_000)


class Rebreak(unittest.TestCase):
    def test_clipped_episode_lengths_and_separate_clocks(self):
        for inside, expected in [(3, 2), (650, 3)]:
            pair = []
            for tail in (0, 4000):
                code, a = c.cli(f'clip-{inside}-inside-{tail}-outside', stray(inside, tail))
                self.assertEqual(code, 0)
                pair.append(a)
                for path in ('web', 'native'):
                    action = a['actions']['actions'][1][path]
                    self.assertEqual(action['outcome'], 'split: 2 episodes')
                    self.assertEqual(action['episodes'], [2, 3])
                    self.assertEqual(action['measured'], expected)
                    self.assertEqual(action['stats']['samples'], max(600, inside))
                self.assertEqual(a['actions']['excludedSamples'], {'web': tail+1, 'native': tail+1})
                text = gzip.decompress((c.OUT/f'clip-{inside}-inside-{tail}-outside.log.gz').read_bytes()).decode()
                self.assertIn('excluded', text)
            self.assertEqual(pair[0]['decision']['rows'], pair[1]['decision']['rows'])
            for path in ('web', 'native'):
                self.assertEqual(pair[0]['actions']['actions'][1][path]['stats'],
                                 pair[1]['actions']['actions'][1][path]['stats'])

    def test_one_episode_crossing_end_uses_clipped_length_and_original_number(self):
        log = c.f.manual_log(False)
        end = log['actions'][1]['endPerf']
        samples = log['web']['samples']
        up = next(s for s in samples if s['kind'] == 'up' and end-2 < s['t'] < end)
        samples.remove(up)
        q = next(s for s in samples if s['kind'] == 'down' and 6000 <= s['t'] < end)
        samples += [dict(q, t=end+i*.1, kind='move', coalescedCount=8) for i in range(4000)]
        samples.append(dict(up, t=end+400))
        samples.sort(key=lambda s: s['t'])
        code, a = c.cli('one-episode-crossing-end', paths(log, 1_000_000))
        self.assertEqual(code, 0)
        for path in ('web', 'native'):
            action = a['actions']['actions'][1][path]
            self.assertEqual(action['episodes'], [2])
            self.assertEqual(action['measured'], 2)
            self.assertEqual(action['stats']['samples'], 600)
            self.assertAlmostEqual(action['stats']['rateHz'], 1000/8.33, places=5)
        self.assertFalse(a['actions']['actions'][1]['web']['stats']['coalescedObserved'])

    def test_mapping_with_native_prefix(self):
        for rotation in (False, True):
            log = paths(c.f.manual_log(rotation))
            k = 2
            prefix = []
            for i in range(k):
                prefix += [dict(kind='down', tool='pen', pressure=.5, hostEpochMs=1e12-1000+i*10),
                           dict(kind='up', tool='pen', pressure=0, hostEpochMs=1e12-999+i*10)]
            log['native']['packets'] = prefix+log['native']['packets']
            code, a = c.cli(f'mapping-rotation-{rotation}-native-prefix-{k}', log)
            self.assertEqual(code, 0)
            a6 = a['actions']['actions'][-1]
            self.assertEqual(a6['id'], 'A6')
            self.assertEqual(a6['web']['measured'], 5 if rotation else 4)
            self.assertEqual(a6['native']['measured'], k+(5 if rotation else 4))

    def test_native_clock_counterexample_and_noncontact_control(self):
        base, extra = reg.clock_pair()
        a, b = [c.cli(name, log)[1] for name, log in [('clock-baseline', base), ('clock-outside', extra)]]
        self.assertEqual(a['actions']['actions'][1]['native']['stats']['rateHz'], 200)
        self.assertEqual(b['actions']['actions'][1]['native']['stats']['rateHz'], 200)
        self.assertEqual(a['actions']['actions'], b['actions']['actions'])
        self.assertEqual(a['decision'], b['decision'])
        self.assertEqual(a['decision']['selected'], 'A')
        self.assertEqual(b['decision']['selected'], 'A')
        self.assertTrue(b['decision']['outcome'].startswith('go-with-conditions'))
        # Missing device time on an outside non-contact packet does not select host time.
        control = copy.deepcopy(base)
        control['native']['packets'].append(extra['native']['packets'][-1])
        code, d = c.cli('clock-noncontact-control', control)
        self.assertEqual(code, 0)
        self.assertEqual(a['decision'], d['decision'])

    def test_key_sequence_observations(self):
        base = c.f.OutsideSpanAndKeys.keyed(c.f.manual_log(False))
        base['environment']['perfNowAtSave'] = base['actions'][-1]['endPerf']+100
        cases = {}
        x = copy.deepcopy(base); x['keyEvents'][1]['key'] = 'k'; cases['different-keyup'] = x
        x = copy.deepcopy(base); x['keyEvents'][0]['repeat'] = True; cases['repeat-first'] = x
        x = copy.deepcopy(base)
        x['keyEvents'][-1]['t'] = x['keyEvents'][-2]['t'] = x['actions'][-1]['startPerf']+499
        x['actions'][-1]['endPerf'] = x['actions'][-1]['startPerf']+499
        x['actions'][-1]['endEpochMs'] = x['actions'][-1]['startEpochMs']+499
        cases['499ms-matching-record'] = x
        x = copy.deepcopy(base); x['keyEvents'][1]['outcome'] = 'transition: A6 skipped'; cases['label-mismatch'] = x
        x = copy.deepcopy(base); x['keyEvents'].pop(); cases['missing-final-keyup'] = x
        for label in ('ignored: all actions recorded', 'transition: A6 done'):
            x = copy.deepcopy(base)
            x['keyEvents'] += [dict(type='keydown', key=' ', repeat=False,
                                    t=x['environment']['perfNowAtSave']+500, outcome='pressed'),
                               dict(type='keyup', key=' ', repeat=False,
                                    t=x['environment']['perfNowAtSave']+501, outcome=label)]
            cases['ignored-events-after-save' if label.startswith('ignored') else 'transition-after-save'] = x
        self.assertEqual(c.cli('rebreak-keys-valid', base)[0], 0)
        for name, log in cases.items():
            code, _ = c.cli('rebreak-'+name, log)
            output = gzip.decompress((c.OUT/('rebreak-'+name+'.log.gz')).read_bytes()).decode('utf-8')
            expected = 0 if name == 'ignored-events-after-save' else 2
            self.assertEqual(code, expected)
            observations.append(dict(case=name, exit=code, eventIndexPresent=bool(re.search(r'key event \d+', output)),
                                     diagnostic=output.strip() if code else 'accepted'))


if __name__ == '__main__':
    try:
        unittest.main(verbosity=2)
    finally:
        (c.OUT/'rebreak-closed-results.json').write_text(json.dumps(dict(cli=c.results, keyObservations=observations),
                                                          indent=2)+'\n', encoding='utf-8')
