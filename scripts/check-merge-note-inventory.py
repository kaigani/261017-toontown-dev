#!/usr/bin/env python3
"""The M3 merge note's open-issue inventory follows the register (live counterpart of the round-2 receipt
`tests/adversarial/c34-r2-inventory.py`; M3 C3.4 rounds 2 and 3): every `## SI-` heading SI-001 to SI-111 of
plan/spec-issues.md whose status is open, unresolved, awaiting, interim or pending confirmation is listed in
reviews/M3-merge-note.md section 4. Bounded to M3's issues (O-3): issues recorded later do not concern this note.

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
    lines = [l for l in register.splitlines() if l.startswith("## SI-") and 1 <= int(re.search(r"SI-(\d+)", l)[1]) <= 111]
    active = [l for l in lines if re.search(r"open|unresolved|awaiting|interim|pending confirmation", l, re.I)]
    missing = [re.search(r"SI-\d+", l)[0] for l in active if re.search(r"SI-\d+", l)[0] not in section]
    problems = []
    if missing:
        problems.append("missing from section 4: " + ", ".join(missing))
    for p in problems:
        print("FAIL " + p)
    print(f"{len(lines)} headings, {len(active)} active; " + ("consistent" if not problems else "inconsistent"))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
