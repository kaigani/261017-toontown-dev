import importlib.util
import pathlib
import tempfile
s = importlib.util.spec_from_file_location('checker', 'scripts/check-message-help.py')
c = importlib.util.module_from_spec(s)
s.loader.exec_module(c)
with tempfile.TemporaryDirectory() as d:
    c.ROOT = pathlib.Path(d)
    p = c.ROOT / 'fixture.rs'
    p.write_text('fn f() { const CODE: &str = "review.missingHelp"; CommandError::new(CODE, "failure"); }', encoding='utf-8')
    c.rust_sources = lambda: iter([p])
    assert 'review.missingHelp' in c.produced_codes(), 'constructor constant escapes help coverage'
