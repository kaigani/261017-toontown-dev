"""Advisory static checks; no product execution; always exits zero."""
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
def read(p): return (ROOT / p).read_text(encoding="utf-8")
def check(name, f):
    try: print(name + ": " + str(f()))
    except Exception as e: print(name + ": inspection error " + type(e).__name__)
check("S1 held saturation", lambda: "FAIL: inline held wait remains" if "if held_until_answered(shared, &token, rid)" in read("crates/mcp/src/server.rs") else "reinspect")
check("S2 allow CI", lambda: "PASS source wiring" if "$PY -I scripts/check-product-allows.py" in read(".github/workflows/ci.yml") else "FAIL")
check("S3 strict parser", lambda: "PASS source guard" if "if (!NUMBER.test(t))" in read("desktop/src/shell/numeric.ts") else "FAIL")
check("S4 constrained width", lambda: "OBSERVATION: min-width 180px; no reachable constrained menu established" if "min-width: 180px" in read("desktop/src/styles.css") else "reinspect")
def reveal_model():
    host_shown = True  # two admitted reveals share one set member
    host_shown = False # hide first completed reveal
    capturing = not host_shown
    painted = True     # second continuation: no generation validation
    return "FAIL: late continuation paints while guard held" if capturing and painted else "PASS"
check("S5 late reveal model", reveal_model)
check("S6 generated inventory", lambda: "PASS source derivation" if '["dom"]' in read("scripts/c43_d39_coverage.py") else "FAIL")
check("S7 reviewer skip", lambda: "FAIL: blanket exception skip" if "except (LookupError,TypeError):return" in read("tests/adversarial/c43b-r2-live.py") else "reinspect")
