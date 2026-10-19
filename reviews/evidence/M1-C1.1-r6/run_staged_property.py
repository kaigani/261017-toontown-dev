"""Run the builder property for 10,000 cases; change only the temporary copy's case count."""
from run_gates import HERE, ROOT, run
import hashlib
import json
import secrets
import sys

original = ROOT / 'crates/model/tests/staged_commit.rs'
source = original.read_bytes()
assert source.count(b'cases: 256,') == 1
expanded = source.replace(b'cases: 256,', b'cases: 10_000,')
target = original.with_name('c11_r6_staged_property.rs')
assert not target.exists()
seed = sys.argv[1] if len(sys.argv) > 1 else str(secrets.randbits(64))
metadata = dict(seed=seed, cases=10000, builder_source_sha256=hashlib.sha256(source).hexdigest(),
                expanded_source_sha256=hashlib.sha256(expanded).hexdigest(),
                modification='Only cases: 256 -> cases: 10_000 in a temporary test target; builder file unchanged')
(HERE / 'staged-property.json').write_text(json.dumps(metadata, indent=2)+'\n', encoding='utf-8')
target.write_bytes(expanded)
try:
    result = run('staged-property-10000', ['cargo', 'test', '-p', 'tt-model', '--test', target.stem,
        '--locked', 'random_patches_never_commit_a_document_with_integrity_errors', '--', '--nocapture'],
        {'PROPTEST_RNG_SEED': seed})
finally:
    assert target.read_bytes() == expanded
    assert original.read_bytes() == source
    target.unlink()
raise SystemExit(result)
