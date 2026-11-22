"""Source-only selection feedback guard (02 section 4.2 layer 9)."""
from pathlib import Path
import sys
view = Path("desktop/src/canvas/CanvasView.tsx").read_text(encoding="utf-8")
scene = Path("desktop/src/canvas/scene.ts").read_text(encoding="utf-8")
panel = Path("desktop/src/panels/PanelBody.tsx").read_text(encoding="utf-8")
ok = "draw.selection.get" in view or "selection.changed" in view or "selection" in scene.lower() or "onReady=" in panel
print("PASS" if ok else "FAIL: no selected geometry reaches canvas feedback; production caller supplies no overlay")
sys.exit(0 if ok else 1)
