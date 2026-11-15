"""Source guard for C42B-R3-20; no runtime or third-party dependency."""
from pathlib import Path
import sys
root = Path(__file__).resolve().parents[2]
gesture = (root / "crates/executor/src/gesture.rs").read_text(encoding="utf-8")
transform = (root / "crates/executor/src/commands/draw_transform.rs").read_text(encoding="utf-8")
errors = []
if '\"scissor\" if st.mods.alternate => (\"draw.cut.apply\", obj(json!({ \"polygon\": mapped })))' in gesture and 'options: InvokeOptions::default()' in gesture:
    errors.append("alternate Scissor inherits polygon Cutter Tool label, contrary to 02 section 7.4.3:654")
print("\n".join(errors) if errors else "PASS: audited defect signatures absent; re-review semantics after changes")
sys.exit(bool(errors))
