"""Stage only this evidence probe as an integration target, then remove that exact file."""
from pathlib import Path
import sys
from run_gates import ROOT, HERE, run

target = ROOT / 'crates/model/tests/c11_r4_closure_probe.rs'
assert not target.exists(), target
source = (HERE / 'closure_probe.rs').read_bytes()
target.write_bytes(source)
try:
    code = run(sys.argv[1] if len(sys.argv) > 1 else 'closure-probe', ['cargo', 'test', '-p', 'tt-model', '--test', target.stem, '--locked', '--', '--nocapture'])
finally:
    assert target.read_bytes() == source, 'Do not delete an externally edited probe'
    target.unlink()
sys.exit(code)
