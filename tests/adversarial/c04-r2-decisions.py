"""Document-contract regressions. Run explicitly; failures are open C04-R2 findings.

These read the normative tables, not an invented product decision implementation.
The manual fixture exercises the real analyzer; it is synthetic test evidence only.
"""
import importlib.util
import json
import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
ADR = ROOT / 'docs/decisions/ADR-0009-tablet-input.md'
spec = importlib.util.spec_from_file_location('analyzer', ROOT / 'scripts/analyse-tablet-log.py')
analyzer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(analyzer)


def capability_row(number):
    text = ADR.read_text(encoding='utf-8').split('**Step 1:')[1].split('**Step 2:')[0]
    line = next(line for line in text.splitlines() if line.startswith(f'| {number} |'))
    return [cell.strip() for cell in line.strip('|').split('|')]


def numeric_match(cell, hz):
    """Read the published closed ranges and inequalities without rounding the measurement."""
    # Builder, C0.4 round 2: the parser extension this function asks for, for the ruled half-open notation
    # `[low, high)` / `[low, ∞)`. The existing branches and every assertion are unchanged.
    if match := re.match(r'\[(\d+), (\d+|∞)\)', cell):
        return int(match[1]) <= hz and (match[2] == '∞' or hz < int(match[2]))
    if match := re.match(r'≥ (\d+)', cell):
        return hz >= int(match[1])
    if match := re.match(r'< (\d+)', cell):
        return hz < int(match[1])
    if match := re.match(r'(\d+)–(\d+)', cell):
        return int(match[1]) <= hz <= int(match[2])
    raise AssertionError(f'Unrecognized numeric rule; update the parser explicitly: {cell}')


def manual_fixture(rotation):
    """Follow instructions: pressure, speed, tilt, optional roll, hover, stationary pressure."""
    actions = ['pressure', 'speed', 'tilt'] + (['rotation'] if rotation else []) + ['hover', 'stationary']
    samples = []
    for index, action in enumerate(actions):
        start = index * 6000
        for n in range(601):
            # Hover has no down/up, and no pressure, so creates no contact episode.
            kind = 'move' if action == 'hover' else ('down' if n == 0 else 'up' if n == 600 else 'move')
            samples.append(dict(t=start + n * 8.33, handlerT=start + n * 8.33 + 1, kind=kind,
                                pointerType='pen', pointerId=1, x=n if action != 'stationary' else 20,
                                y=20, pressure=0 if action == 'hover' or kind == 'up' else (n % 511 + 1)/512,
                                tiltX=0, tiltY=0, twist=0, dispatch=len(samples), coalescedCount=1))
    return dict(version=3, source='manual', environment=dict(coalescedApi=True),
                web=dict(samples=samples, frames=[]), native=dict(packets=[]), runs=[], simulations=[])


class DecisionContracts(unittest.TestCase):
    def test_C04_R2_03_canvas_has_a_guard_for_analyzer_accepted_missing_scenes(self):
        canvas_spec = importlib.util.spec_from_file_location('canvas_analyzer', ROOT / 'scripts/analyse-canvas-log.py')
        canvas = importlib.util.module_from_spec(canvas_spec)
        canvas_spec.loader.exec_module(canvas)
        log = json.loads((ROOT / 'reviews/evidence/M0-C0.3-r4/page-r3-one-frame.json').read_bytes())
        log['runs'] = log['runs'][:1]  # Interactive Save after paths100k, before groups1k / brush10k.
        canvas.validate(log)  # Exit-2 guard does not catch this valid partial interactive capture.
        self.assertEqual([r['scene'] for r in log['runs']], ['paths100k'])
        text = (ROOT / 'docs/decisions/ADR-0007-interactive-canvas.md').read_text(encoding='utf-8')
        rules = text.split('<!-- canvas-rules:begin -->')[1].split('<!-- canvas-rules:end -->')[0]
        predicates = '\n'.join(line.split('|')[1] for line in rules.splitlines() if line.startswith('|'))
        self.assertRegex(predicates.lower(), r'missing|incomplete',
                         'A valid partial interactive log needs an explicit missing-scene guard before selection')

    def test_C04_R2_01_rate_intervals_cover_fractional_hz(self):
        for row, rates in [(7, [59.9, 60, 119, 119.1, 119.9, 120]),
                           (8, [119.9, 120, 199, 199.1, 199.9, 200])]:
            for hz in rates:
                with self.subTest(row=row, hz=hz):
                    cells = capability_row(row)[3:6]
                    matches = [cell for cell in cells if numeric_match(cell, hz)]
                    self.assertEqual(len(matches), 1, f'{hz} Hz must match exactly one of {cells}')

    def test_C04_R2_02_stationary_instruction_identifies_an_existing_contact_row(self):
        # ADR says its Stroke column denotes the analyzer's per-stroke figures.
        stationary_index = int(capability_row(9)[2])
        for rotation in (False, True):
            with self.subTest(rotation_capable=rotation):
                log = manual_fixture(rotation)
                self.assertTrue(analyzer.is_manual(log, analyzer.log_version(log)))
                strokes = analyzer.web_analysis(log)['strokes']
                self.assertEqual(stationary_index, len(strokes),
                                 'Stationary pressure is the last contact: 4 without roll, 5 with roll; '
                                 'hover creates no row. Publish an explicit action-to-episode mapping.')


if __name__ == '__main__':
    unittest.main(verbosity=2)
