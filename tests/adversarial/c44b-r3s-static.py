#!/usr/bin/env python3
"""Read-only round-3 diagnostics; deliberately never fails CI."""
from pathlib import Path
import subprocess
import json

ROOT = Path(__file__).resolve().parents[2]
def read(p):
    return (ROOT / p).read_text(encoding="utf-8")
def check(name, fn):
    try:
        print(name + ": " + str(fn()))
    except Exception as e:
        print(name + ": diagnostic unavailable: " + str(e))
lock = read("crates/persist/src/lock.rs")
check("S1 helper proof has no aside", lambda: "Boundary::ArbiterAbandonedProven, false, false, valid" in lock)
check("S1 rounds can repeat", lambda: "const DECIDE_BOUND: Duration = Duration::from_secs(2)" in lock and "Nothing is held: decide again" in lock)
check("S2 named schedule", lambda: "type Key = (String, Boundary)" in read("crates/executor/tests/c44b_lock_takeover.rs"))
check("S3 MCP cannot set person", lambda: "confirm, person: false" in read("crates/mcp/src/server.rs"))
check("S4 acting snapshot missing", lambda: "acting: false" in read("crates/executor/src/commands/app.rs"))
check("S5 extended segmentation", lambda: "text.graphemes(true)" in read("crates/vector/src/text.rs"))
check("S6 M1 reviewer unchanged", lambda: not subprocess.check_output(["git", "diff", "25b99718", "71ef95f3", "--", "crates/executor/tests/c13_r1_persistence.rs"], cwd=ROOT))
check("S6 oversized receipts", lambda: [str(p.relative_to(ROOT)) for p in (ROOT / "reviews/evidence/M4-C4.4b-r3").rglob("*") if p.is_file() and p.stat().st_size > 200000])
check("S7 deferred owners", lambda: sorted(set(v["owner"] for v in json.loads(read("desktop/src/ui-bindings/deferred.json"))["deferred"].values())))
check("report ASCII and LF", lambda: read("reviews/M4-C4.4b-r3-static.md").isascii() and b"\r" not in (ROOT / "reviews/M4-C4.4b-r3-static.md").read_bytes())
