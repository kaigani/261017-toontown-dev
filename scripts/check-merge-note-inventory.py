#!/usr/bin/env python3
"""The M3 merge note's open-issue inventory follows the register (live counterpart of the round-2 receipt
`tests/adversarial/c34-r2-inventory.py`; M3 C3.4 round 2): every `## SI-` heading of plan/spec-issues.md whose status is
open, unresolved, awaiting, interim or pending confirmation is listed in reviews/M3-merge-note.md section 4, and the
note's next free id is one past the register's highest.

Usage: python scripts/check-merge-note-inventory.py   (exit 0 when consistent)
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]


def main():
    register = (ROOT / "plan" / "spec-issues.md").read_text(encoding="utf-8")
    note = (ROOT / "reviews" / "M3-merge-note.md").read_text(encoding="utf-8")
    section = note.split("## 4.", 1)[1].split("## 5.", 1)[0]
    lines = [l for l in register.splitlines() if l.startswith("## SI-")]
    active = [l for l in lines if re.search(r"open|unresolved|awaiting|interim|pending confirmation", l, re.I)]
    missing = [re.search(r"SI-\d+", l)[0] for l in active if re.search(r"SI-\d+", l)[0] not in section]
    highest = max(int(re.search(r"SI-(\d+)", l)[1]) for l in lines)
    problems = []
    if missing:
        problems.append("missing from section 4: " + ", ".join(missing))
    if f"next free id **SI-{highest + 1:03d}**" not in section:
        problems.append(f"section 4 does not give the next free id SI-{highest + 1:03d}")
    for p in problems:
        print("FAIL " + p)
    print(f"{len(lines)} headings, {len(active)} active; " + ("consistent" if not problems else "inconsistent"))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
