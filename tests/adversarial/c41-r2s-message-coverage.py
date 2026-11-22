from pathlib import Path
import importlib.util
import tempfile

root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('checker', root / 'scripts/check-message-help.py')
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)
with tempfile.TemporaryDirectory() as folder:
    fixture = Path(folder) / 'fixture.rs'
    fixture.write_text('fn example() { CommandError::new("reviewMissingHelp", "failure"); }', encoding='utf-8')
    checker.ROOT = Path(folder)
    checker.rust_sources = lambda: iter([fixture])
    assert 'reviewMissingHelp' in checker.produced_codes(), 'O-11: undotted error code evades help coverage'
