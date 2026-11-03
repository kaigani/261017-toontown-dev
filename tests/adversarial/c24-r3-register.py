"""Round-3 scoped conformance register; explicit output path only, no historical writes."""
from collections import Counter
import copy
import json
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
HEAD = '871e8bd6299011a62e38d3da073755a54faec7f5'


def build():
    prior = json.loads((ROOT / 'reviews/evidence/M2-C2.4-r2/register.json').read_text(encoding='utf-8'))
    current = copy.deepcopy(prior)
    current['reviewedHead'] = HEAD
    updates = {
        'U042': ('divergent', 'C24-R3-02/04: changed-route domains improve coverage, but Vectorize accepts a wrong mask boundary and the Envelope oracle uses a finer point probe; scoped closure required'),
        'U043': ('implemented', 'Ruling (a) confirmed by independent Fraction intersections/shoelace/snapping and actual-delta perimeter bounds; authorized preservation substitutions retained'),
        'U049': ('divergent', 'C24-R3-01: original metadata/run/status/overflow malformations fixed; identity-free numeric samples and incompatible declared units still accepted'),
        'U050': ('implemented', 'Fresh handoff check: current round-3 review state, all declared ownership edits, accepted C22-T2 deferral and explicit SI-077 scope decision obligation'),
        'U067': ('implemented', 'Rulings (a)/(b)/(c) confirmed: only three permitted files changed, all thirteen route test functions byte-identical, helper mirroring independently verified'),
        'U068': ('implemented', 'Four structural receipts reproduce; exact symmetry corpus hashes match historical goldens; corrected displacement/cause declared; new malformed-input analyzer defect separated as U071'),
        'U069': ('implemented', 'Zero-length retained contours remain snap candidates and stay out of face construction; unchanged positive control and fresh-seed point-move route'),
        'U070': ('implemented', 'Closing-edge parameter fix and unchanged R2 controls; new 4096 repeated-interior/consecutive/terminal-anchor crossing variants preserve source multiplicity'),
    }
    for row in current['requirements']:
        if row['id'] in updates:
            row['state'], row['evidenceOrOwner'] = updates[row['id']]
    current['requirements'].append({'id': 'U071', 'clause': 'C24-R2-05; whole-checkpoint evidence hygiene',
        'requirement': 'golden analyzer rejects malformed duplicate identities before ID alignment', 'state': 'divergent',
        'evidenceOrOwner': 'C24-R3-03: duplicate IDs silently overwrite an earlier changed stroke; CLI writes a zero-change success receipt'})
    current['requirements'].append({'id': 'U072', 'clause': 'C24-R2-01; SI-070 point-location model',
        'requirement': 'projective-envelope expected fill uses the established point-query representation', 'state': 'divergent',
        'evidenceOrOwner': 'C24-R3-04: finer dyadic probe crosses a source boundary differently from the 1/256-du product query; original expectation and fixed failure retained, independent rounded-query control passes'})
    for row in current['readiness']:
        if row['id'] == '2':
            row.update(state='divergent', evidenceOrOwner='C24-R3-02: independent changed-mask boundary proof remains incomplete; other added domains and original proving controls retained')
    for row in current['commands']:
        if row['id'] == 'draw.flatten.apply':
            row.update(state='implemented', evidenceOrOwner='Closed-fold traversal/parameter fix; partial-cover regression and mixed-kind domain controls; no new product defect demonstrated')
    for group in ('requirements', 'readiness', 'commands'):
        counts = Counter(row['state'] for row in current[group])
        current['counts'][group] = {'total': len(current[group]), **{
            k: counts[k] for k in ('implemented', 'divergent', 'missing', 'deferred-with-owner')}}
    deltas = []
    for group in ('requirements', 'readiness', 'commands'):
        before = {r['id']: r for r in prior[group]}
        for row in current[group]:
            if before.get(row['id']) != row:
                deltas.append({'register': group, 'id': row['id'], 'before': before.get(row['id']), 'after': row})
    return current, deltas


if __name__ == '__main__':
    if len(sys.argv) == 3 and sys.argv[1] == '--out':
        p = Path(sys.argv[2])
        q = p.with_name(p.stem + '-deltas.json')
        if p.exists() or q.exists():
            raise RuntimeError('refusing to overwrite a register')
        doc, delta = build()
        p.write_text(json.dumps(doc, indent=2) + '\n', encoding='utf-8')
        q.write_text(json.dumps(delta, indent=2) + '\n', encoding='utf-8')
        print(json.dumps(doc['counts']))
    else:
        class Register(unittest.TestCase):
            def test_all_units_retained_and_deferred_owners_preserved(self):
                doc, _ = build()
                self.assertEqual(len({r['id'] for r in doc['requirements']}), 72)
                for row in doc['requirements']:
                    if row['state'] == 'deferred-with-owner':
                        self.assertIn('Owner', row['evidenceOrOwner'])
                self.assertEqual(doc['counts']['requirements']['divergent'], 4)
        unittest.main()
