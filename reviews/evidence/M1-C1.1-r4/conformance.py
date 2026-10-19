"""Carry the stable register forward; change only the five scoped closure rows."""
from pathlib import Path
from collections import Counter
import json

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
prior = json.loads((HERE.parent / 'M1-C1.1-r3/conformance.json').read_text(encoding='utf-8'))
rows = prior['rows']
M = 'crates/model/tests/c11_r3_adversarial.rs'
N = 'crates/model/tests/c11_r4_exposure_bounds.rs'

def loc(file, anchor):
    lines = (ROOT / file).read_text(encoding='utf-8').splitlines()
    return file + ':' + str(next(i for i, line in enumerate(lines, 1) if anchor in line))

decisions = {
    388: ('divergent', 'crates/model/src/change.rs', 'fn expand_runs', N,
          'exposure_range_rejects_total_above_scene_limit',
          'R3-02 removal and R3-03 zero-count checks verified. C11-R4-01: oversized totals/windows still accepted at the patch boundary; integrity rejects afterward. Row remains divergent.'),
    392: ('implemented', 'crates/model/src/schema.rs', 'fn annotate_since', M,
          'anonymous_variant_property_declarations_have_since',
          'R3-06 closed: all 522 recursively discovered real property declarations have x-since=1.0; named readOnly controls still pass.'),
    394: ('implemented', 'crates/model/src/schema.rs', 'pub fn property_schema', M,
          'property_schema_preserves_nullable_value_domains',
          'R3-04 closed: nullable and all enum alternatives, conditional paths, and reference annotations pass.'),
    56: ('implemented', 'crates/model/src/change.rs', 'if ty == ObjectType::SoundPlacement', M,
         'opaque_mouth_track_lookalikes_are_ordinary_json_changes',
         'R3-01 closed for meta: wholesale text diffs/replacements accepted, ranged edits rejected with typed arg.invalid; fresh escaped-key/Unicode controls pass.'),
    57: ('implemented', 'crates/model/src/change.rs', 'if ty == ObjectType::SoundPlacement', M,
         'opaque_mouth_track_lookalikes_are_ordinary_json_changes',
         'R3-01 closed for ext: namespace values remain opaque; wholesale text changes round-trip and ranged edits reject with typed arg.invalid.'),
}
closure = []
for row, (status, file, anchor, test_file, test_name, note) in decisions.items():
    item = rows[row - 1]
    item['r3_status'] = item['status']
    item['r3_note'] = item['note']
    item.update(status=status, implementation=loc(file, anchor), note=note,
                r4_test=dict(name=test_name, location=loc(test_file, 'fn '+test_name+'('),
                             command=f'cargo test -p tt-model --test {Path(test_file).stem} {test_name} --locked -- --exact --nocapture'))
    closure.append(item)
counts = dict(Counter(row['status'] for row in rows))
counts.setdefault('missing', 0)
assert counts == dict(implemented=234, divergent=1, deferred=193, missing=0), counts
assert len(rows) == 428
old_rows = json.loads((HERE.parent / 'M1-C1.1-r3/conformance.json').read_text(encoding='utf-8'))['rows']
assert all(row == old for row, old in zip(rows, old_rows) if row['row'] not in decisions)
out = dict(base='3b6179f57bbdf65ac847787f34ef937f88035178', counts=counts, rows=rows,
           scope='Only rows 388, 392, 394, 56 and 57 re-evaluated; all other rows and 193 deferral owners preserved.')
(HERE / 'conformance.json').write_text(json.dumps(out, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
(HERE / 'register-closure.json').write_text(json.dumps(closure, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
print(json.dumps(dict(requirements=len(rows), counts=counts, changed_rows=list(decisions)), indent=2))
