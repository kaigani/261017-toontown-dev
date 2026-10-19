"""Stage the evidence-only Rust target, run it, and remove only the unchanged copy."""
from run_gates import HERE, ROOT, run

target = ROOT / 'crates/model/tests/c11_r5_bounds_probe.rs'
assert not target.exists(), target
source = (HERE / 'bounds_probe.rs').read_bytes()
target.write_bytes(source)
try:
    result = run('bounds-probe', ['cargo', 'test', '-p', 'tt-model', '--test', target.stem,
                                '--locked', '--', '--nocapture'])
finally:
    assert target.read_bytes() == source
    target.unlink()
raise SystemExit(result)
