"""Source-only regression guard for D-7b; run from repository root."""
from pathlib import Path
import re
import sys
s = Path("desktop/src/canvas/CanvasView.tsx").read_text(encoding="utf-8")
cancel = s.split("const cancelGesture = () => {", 1)[1].split("// The display preferences", 1)[0]
event = s.split(".onEvent((event) => {", 1)[1].split("if (event.name === 'tool.active.changed'", 1)[0]
# Both queues must be invalidated on restart, not merely the user's collector.
combined = cancel + event
ok = "playing = null" in combined and "agentQueue.splice" in combined
print("PASS" if ok else "FAIL D-7b: generation cancellation retains playing and queued agent input")
sys.exit(0 if ok else 1)
