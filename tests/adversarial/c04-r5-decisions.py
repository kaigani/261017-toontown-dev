"""C04-R5-01: excluded native packets must not choose an action's rate clock.

Synthetic regression, not physical-input evidence. Run from the repository root:
    python tests/adversarial/c04-r5-decisions.py
"""
import copy
import importlib.util
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('rules', ROOT / 'scripts/test_decision_rules.py')
rules = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rules)
analyzer = rules.tablet


def clock_pair():
    log = rules.manual_log(False)
    log['environment']['userAgent'] = 'Macintosh'
    lo, hi = log['actions'][1]['startPerf'], log['actions'][1]['endPerf']
    a2 = [s for s in log['web']['samples'] if lo <= s['t'] < hi]
    others = [s for s in log['web']['samples'] if not lo <= s['t'] < hi]
    # Both paths' A2 samples arrive at 50 Hz; native device timestamps report 200 Hz.
    a2 = [dict(s, t=6000+i*20, handlerT=6001+i*20) for i, s in enumerate(a2[:240])]
    a2.append(dict(a2[-1], t=10800, kind='up', pressure=0))
    log['web']['samples'] = sorted(others+a2, key=lambda s: s['t'])
    log['nativeInfo'] = {'backend': 'test-native'}
    log['native']['packets'] = [dict(
        kind={'move': 'pose'}.get(s['kind'], s['kind']), tool='pen', pressure=s['pressure'],
        hostEpochMs=1e12+s['t'], deviceMs=s['t']/4,
        tiltX=s['tiltX'], tiltY=s['tiltY'], twist=s['twist']) for s in log['web']['samples']]
    extra = copy.deepcopy(log)
    end = log['actions'][-1]['endEpochMs']
    extra['native']['packets'] += [
        dict(kind='down', tool='pen', pressure=.5, hostEpochMs=end+100),
        dict(kind='up', tool='pen', pressure=0, hostEpochMs=end+101),
    ]
    return log, extra


def analyze(log):
    web, native = analyzer.web_analysis(log), analyzer.native_analysis(log)
    actions = analyzer.action_analysis(log, web, native)
    return actions, analyzer.tablet_decision(log, web, native, actions)


class OutsideNativeClock(unittest.TestCase):
    def test_outside_packets_cannot_change_action_rate_or_platform_decision(self):
        base, extra = clock_pair()
        self.assertEqual(base['native']['packets'], extra['native']['packets'][:-2])
        a, da = analyze(base)
        b, db = analyze(extra)
        self.assertEqual(a['excludedSamples']['native'], 0)
        self.assertEqual(b['excludedSamples']['native'], 2)
        self.assertEqual(da['selected'], 'A')
        with self.subTest(invariant='action rate'):
            self.assertEqual(a['actions'][1]['native']['stats']['rateHz'],
                             b['actions'][1]['native']['stats']['rateHz'])
        with self.subTest(invariant='capability rows'):
            self.assertEqual(da['rows'], db['rows'])
        with self.subTest(invariant='platform outcome'):
            self.assertEqual(da['outcome'], db['outcome'])


if __name__ == '__main__':
    unittest.main(verbosity=2)
