from pathlib import Path
text = Path("desktop/src/ipc/generated/commands.ts").read_text(encoding="utf-8")
assert "renderedSeq" in text, "generated command results omit renderedSeq"
