"""ADR-0007 experiment completion evidence, source only."""
from pathlib import Path
import re
import sys
s = Path("docs/decisions/ADR-0007-interactive-canvas.md").read_text(encoding="utf-8")
missing = sorted(set(re.findall(r"CI-(?:WIN|MAC)-(?:NUMBERS|CELLS|OUTCOME)", s)))
print("FAIL: unresolved experiment placeholders: " + ", ".join(missing) if missing else "PASS")
sys.exit(bool(missing))
