"""Evidence-only targets; originals unchanged, temporary targets always removed."""
from run_gates import HERE, ROOT, run
import hashlib
import json
import secrets
import sys

def execute(name, source, args=(), env=None):
    target = ROOT / 'crates/model/tests' / (name + '.rs')
    assert not target.exists()
    target.write_bytes(source)
    try:
        return run(name, ['cargo', 'test', '-p', 'tt-model', '--test', name, '--locked', *args, '--', '--nocapture'], env)
    finally:
        assert target.read_bytes() == source
        target.unlink()

if sys.argv[1] == 'probe':
    product = (ROOT/'crates/model/src/document/mod.rs').read_text(encoding='utf-8')
    block = product.split('        if crate::change::apply(&committed, &undo).ok().as_ref() != Some(&before) {', 1)[1].split('        Ok((next, undo))', 1)[0]
    block = '        if crate::change::apply(&committed, &undo).ok().as_ref() != Some(&before) {' + block
    # Only the crate import path changes; the branch and fallback body are verbatim product code.
    source = (HERE/'closure_probe.rs').read_text(encoding='utf-8').replace('// PRODUCTION_SELF_CHECK', block.replace('crate::change::', 'tt_model::change::')).encode()
    (HERE/'fallback-extraction.json').write_text(json.dumps(dict(
        source='crates/model/src/document/mod.rs', block=block,
        sha256=hashlib.sha256(block.encode()).hexdigest(),
        technique='Test double supplies a bad diff candidate; exact production guard/body extracted without changing product files. Tests both Ok(wrong state) and Err(apply).'), indent=2)+'\n',encoding='utf-8')
    raise SystemExit(execute('c11_r7_closure_probe', source))

assert sys.argv[1] == 'properties'
results = []
for index, (stem, test) in enumerate([
        ('staged_commit', 'random_patches_never_commit_a_document_with_integrity_errors'),
        ('staged_inverse', 'staged_exposure_and_mouth_edits_undo_and_redo_exactly')]):
    original = ROOT/'crates/model/tests'/f'{stem}.rs'
    source = original.read_bytes()
    assert source.count(b'cases: 256,') == 1
    expanded = source.replace(b'cases: 256,', b'cases: 10_000,')
    seed = sys.argv[index + 2] if len(sys.argv) > index + 2 else str(secrets.randbits(64))
    metadata = dict(test=test, seed=seed, cases=10000, builder_source_sha256=hashlib.sha256(source).hexdigest(),
        expanded_source_sha256=hashlib.sha256(expanded).hexdigest(),
        modification='Only cases: 256 -> cases: 10_000 in a temporary test target; builder file unchanged')
    (HERE/f'{stem}-property.json').write_text(json.dumps(metadata,indent=2)+'\n',encoding='utf-8')
    results.append(execute(f'c11_r7_{stem}_10000', expanded, [test], {'PROPTEST_RNG_SEED': seed}))
    assert original.read_bytes() == source
raise SystemExit(any(results))
