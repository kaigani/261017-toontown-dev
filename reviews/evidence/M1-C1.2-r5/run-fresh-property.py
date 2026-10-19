"""Run the builder's content oracle with a fresh recorded seed without editing its permanent test.

The evidence copy differs only in its seed plus a success print of coverage counters.
"""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
source = ROOT / 'crates/executor/tests/c12_invalidation_property.rs'
target = ROOT / 'crates/executor/tests/c12_r5_fresh_property.rs'
seed = 0xC125A570261019
original = source.read_text(encoding='utf-8')
assert original.count('0xc12_4001_u64') == 1
fresh = original.replace('0xc12_4001_u64', hex(seed) + '_u64')
fresh = fresh.replace('    assert!(diffs > 100', '    println!("seed {seed:#x}: diffs={diffs}, no_op_opportunities={phantom_chances}, real_changes={real_changes}, checkpoints={}, revisions={}", at_checkpoint.len(), at_rev.len());\n    assert!(diffs > 100')
(HERE / 'fresh-invalidation-property.rs').write_text(fresh, encoding='utf-8')
assert not target.exists()
try:
    target.write_text(fresh, encoding='utf-8')
    p = subprocess.run(['cargo', 'test', '-p', 'tt-executor', '--test', target.stem, '--locked', '--', '--nocapture'], cwd=ROOT)
finally:
    target.unlink()
receipt = {'seed': hex(seed), 'source': str(source.relative_to(ROOT)),
           'sourceSha256': hashlib.sha256(source.read_bytes()).hexdigest(),
           'evidenceCopy': 'fresh-invalidation-property.rs', 'permanentTestUnmodified': source.read_text(encoding='utf-8') == original,
           'edits': ['fresh seed', 'success-only coverage counter print'], 'exitCode': p.returncode}
(HERE / 'fresh-property.json').write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
sys.exit(p.returncode)
