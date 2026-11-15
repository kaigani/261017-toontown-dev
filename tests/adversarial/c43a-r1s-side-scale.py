"""Read-only source probe; run from repository root. No runtime claim."""
from pathlib import Path
s = Path("crates/executor/src/scene_gesture.rs").read_text(encoding="utf-8")
start = s.index("Handle::Side(i) =>", s.index("fn handle_drag"))
body = s[start:s.index("Handle::Skew(i) =>", start)]
h = Path("crates/executor/src/commands/anim_transform.rs").read_text(encoding="utf-8")
assert not ('"x": sx, "y": sy' in body and 'if let Some(v) = s.x' in h and 'if let Some(v) = s.y' in h), "C43A-R1-22: side drag supplies both axes; key handler keys each supplied axis including unchanged factor 1"
