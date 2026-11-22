"""Read-only source probe; run from repository root. No runtime claim."""
from pathlib import Path
s = Path("crates/executor/src/gesture.rs").read_text(encoding="utf-8")
g = Path("crates/executor/src/scene_gesture.rs").read_text(encoding="utf-8")
start = s.index("let tool = match scene.as_deref()")
branch = s[start:s.index("let g = Gesture", start)]
assert "alternate" in branch or "alternate" in g, "C43A-R1-21: Alt never affects the Camera scene-tool dispatch or gesture command"
