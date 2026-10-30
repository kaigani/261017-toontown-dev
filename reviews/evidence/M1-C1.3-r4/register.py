"""Carry forward the accepted register and apply only the ruled closures."""
from pathlib import Path
from collections import Counter
import json

source = Path('reviews/evidence/M1-C1.3-r3/conformance.md').read_text(encoding='utf-8')
body = source[source.index('## Requirement rows'):]
lines = body.splitlines()
for i, line in enumerate(lines):
    if line.startswith(('| LOCK-01 ', '| LOCK-02 ', '| Interchange ', '| `doc.import.json` ')):
        cells = line.split('|')
        cells[2] = ' implemented '
        cells[-2] = (' C13-R3-01/02 closed as ruled: c13_r3_format (3), '
                     'c13_r3_persistence (10), c13_r4_lock (1), c13_r4_closure (2); '
                     'see M1-C1.3-r4-closure.md. ')
        lines[i] = '|'.join(cells)
    if line.startswith('| MED-04 '):
        lines[i] = line.replace('Separate P2 export-cache ticket C13-R3-03.',
                               'C13-R3-03 closed: repeated exports reuse cached bytes; close clears cache; c13_r4_closure.')
    if line.startswith('| `doc.package.export` '):
        lines[i] = line.replace('P2 C13-R3-03 repeated-export cache growth; serialization passes;',
                               'C13-R3-03 closed: r3 repeated-export test and r4 remove/export/undo/close case pass;')
body = '\n'.join(lines)+'\n'
sections = body.split('## ')
section_counts = {}
for section in sections:
    name = section.splitlines()[0] if section.splitlines() else ''
    section_counts[name] = Counter(line.split('|')[2].strip() for line in section.splitlines()
        if line.startswith('|') and len(line.split('|')) > 3
        and line.split('|')[2].strip() in ('implemented','divergent','missing','deferred-with-owner'))
assert section_counts['Requirement rows'] == Counter(implemented=104,divergent=1,**{'deferred-with-owner':14})
assert section_counts['Supplemental rows'] == Counter(implemented=4)
assert section_counts['Declarations'] == Counter(implemented=48)
counts = Counter(line.split('|')[2].strip() for line in lines
                 if line.startswith('|') and len(line.split('|')) > 3
                 and line.split('|')[2].strip() in ('implemented','divergent','missing','deferred-with-owner'))
assert counts == Counter(implemented=156, divergent=1, **{'deferred-with-owner':14}), counts
header = '''# C1.3 final register — scoped round-4 closure

Reviewed `205cd97`; inherited round-3 row identities, accepted closures and deferrals retained.
Order: implemented / divergent / missing / deferred-with-owner.

- 119 requirements: **104 / 1 / 0 / 14**.
- 123 including the four parser supplement rows: **108 / 1 / 0 / 14**.
- 48 declarations: **48 / 0 / 0 / 0**.

Only LOCK-01, LOCK-02, Interchange and `doc.import.json` change status from round 3.
REP-01 remains divergent pending SI-044. MED-04's separate cache finding is closed.
The accepted SAVE-01 Windows write-through and RO-02 C1.4 handoff notes remain.
Inherited source coordinates are navigation hints. This is a scoped closure assessment.

'''
Path('reviews/evidence/M1-C1.3-r4/conformance.md').write_text(header+body,encoding='utf-8')
print(json.dumps(dict(counts),indent=2))
