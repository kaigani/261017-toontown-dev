"""Retain every predecessor source unit and explicitly enumerate round-two deltas."""
import collections
import json
import pathlib
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
# Receipts: a temporary directory on routine runs; durable evidence only with an explicit `--out PATH` (C22-R3-04).
OUT = (pathlib.Path(sys.argv[sys.argv.index('--out') + 1]) if '--out' in sys.argv
       else pathlib.Path(tempfile.mkdtemp(prefix='c22-r2-register-')))
src = (ROOT / 'reviews/evidence/M2-C2.2-r1/conformance.md').read_text(encoding='utf-8')
rows = []
counts = collections.Counter()
deltas = []
for line in src.splitlines():
    if not line.startswith('| '):
        continue
    c = line[2:-2].split(' | ')
    if len(c) != 4 or c[2] not in ['implemented', 'divergent', 'missing', 'deferred-with-owner']:
        continue
    req, txt, prior, evidence = c
    now = prior
    note = 'Retained; predecessor source audit plus current regressions/CI. Scope and named owner unchanged.'
    if prior == 'divergent':
        now = 'implemented'
        note = 'R1 fix reproduced; geometry/commands/surface closure logs and round-two variants.'
        if ('§3.11' in req or ('§1.6' in req and 'recovery' in txt.lower()) or
                ('§11.2' in req and 'recreated from' in txt) or
                ('§4.11' in req and ('full swatch' in txt or 'Recovery itself' in txt or txt == 'swatch'))):
            now = 'divergent'
            note = 'C22-R2-03: recovery drops swatch ext, including protection; known field/value recovery works.'
        if '§3.12' in req and txt.startswith('1.'):
            now = 'divergent'
            note = 'C22-R2-01/04: vertex crossing lost by f64 cancellation; rounded overlay ranks unequal overlaps as ties.'
        if '§4.9' in req and 'disagreement' in txt:
            now = 'divergent'
            note = 'C22-R2-02: normal/backup/recovery load paths repair; JSON import bypasses required side repair.'
        if '§7.2' in req and 'Inconsistent zone fills' in txt:
            now = 'divergent'
            note = 'C22-R2-02: open reports fill repair; import does not apply/report it.'
        if '§7.2' in req and 'Painted with a single click' in txt:
            now = 'divergent'
            note = 'C22-R2-05 (P2): native tiled texture scale absent; default gradient box variant passes.'
        if '§7.2' in req and 'Zone split or merged' in txt:
            now = 'divergent'
            note = 'C22-R2-04: exact overlap scoring loses fractional area and can inherit the wrong old fill.'
        if '§7.2' in req and ('Edited with the edit texture tool' in txt or 'Object transformed' in txt):
            now = 'deferred-with-owner'
            note = 'M2 C2.3 mapping/transform commands; M4 C4.2 gestures. R1 pencil matrix storage is fixed.'
    if now != prior:
        deltas.append({'unit': len(rows) + 1, 'requirement': req, 'from': prior, 'to': now})
    counts[now] += 1
    rows.append([f'U{len(rows)+1:03}', req, txt, prior, now, note, evidence])
assert len(rows) == 345, len(rows)
summary = {'counts': {k: counts[k] for k in ['implemented', 'divergent', 'missing', 'deferred-with-owner']},
           'previous': {'implemented': 153, 'divergent': 36, 'missing': 0, 'deferred-with-owner': 156},
           'deltas': deltas}
head = ('# M2 C2.2 round 2: 345-unit conformance register\n\n'
        'Target `7ccea58`. Units retain round-one order and wording. This is a source-unit register, '
        'not 345 independent tests. Inherited entries use the predecessor audit plus current regressions/CI; '
        'they are not represented as freshly proved individually. Mixed clauses remain divergent if a '
        'current-checkpoint part fails. Later UI/rendering obligations retain named owners.\n\n'
        + str(summary['counts']) + '; total 345.\n\n'
        '| Unit | Requirement | Text / field | Round 1 | Round 2 | Delta / evidence / owner | Prior audit reference |\n'
        '|---|---|---|---|---|---|---|\n')
(OUT / 'conformance.md').write_text(head + '\n'.join('| ' + ' | '.join(r) + ' |' for r in rows) + '\n', encoding='utf-8')
(OUT / 'register-deltas.json').write_text(json.dumps(summary, indent=2) + '\n', encoding='utf-8')
print(summary['counts'], 'disposition changes', len(deltas))
