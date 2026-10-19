"""Run evidence-only probes and preserve canonical before/after SHA-256 receipts."""
from run_gates import HERE, ROOT, run
import hashlib
import json

target = ROOT / 'crates/model/tests/c11_r6_closure_probe.rs'
source = (HERE / 'closure_probe.rs').read_bytes()
snapshots = ROOT / 'target/review-m1-c11-r6-snapshots'
assert not target.exists()
target.write_bytes(source)
try:
    result = run('closure-probe', ['cargo', 'test', '-p', 'tt-model', '--test', target.stem,
                                 '--locked', '--', '--nocapture', '--test-threads=1'],
                 {'C11_R6_SNAPSHOTS': str(snapshots)})
finally:
    assert target.read_bytes() == source
    target.unlink()
receipts = []
for before in sorted(snapshots.glob('*.before.json')):
    after = before.with_name(before.name.replace('.before.', '.after.'))
    a, b = before.read_bytes(), after.read_bytes()
    receipts.append(dict(case=before.name.removesuffix('.before.json'), bytes=len(a),
                         before_sha256=hashlib.sha256(a).hexdigest(),
                         after_sha256=hashlib.sha256(b).hexdigest(), identical=a == b))
assert receipts and all(r['identical'] for r in receipts)
(HERE / 'rollback-hashes.json').write_text(json.dumps(receipts, indent=2)+'\n', encoding='utf-8')
print(f'Canonical JSON SHA-256 rollback checks: {len(receipts)} identical')
raise SystemExit(result)
