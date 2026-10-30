#!/usr/bin/env python3
"""Fails when product code gains a lint `allow` attribute (standing review rule; C14-R3-05).

Usage: python scripts/check-product-allows.py [--update]

Scans the tracked product sources (`crates/*/src`, `bins/*/src`, `desktop/src`, `desktop/src-tauri/src`) for
`#[allow(...)]` and `#![allow(...)]` and compares each file's lints with the inventory in
`scripts/product-allows.json`. A lint allowed more often than the inventory says is a new allow: the check prints it and
exits 1. One allowed less often is reported too (exit 1) so the inventory stays exact: run with `--update` after removing
an allow. Adding to the inventory is a visible, declared change in review, never a side effect of a gate run.
"""
import collections
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
INVENTORY = ROOT / "scripts" / "product-allows.json"
SOURCES = ["crates/*/src/**", "bins/*/src/**", "desktop/src/**", "desktop/src-tauri/src/**"]
ATTR = re.compile(r"#!?\[\s*allow\s*\((.*?)\)\s*\]", re.S)
REASON = re.compile(r"reason\s*=\s*\"(?:[^\"\\]|\\.)*\"", re.S)


def scan():
    files = subprocess.run(["git", "ls-files", "--", *SOURCES], cwd=ROOT, capture_output=True, text=True,
                           check=True).stdout.split()
    found = {}
    for f in sorted(set(files)):
        p = ROOT / f
        if not p.is_file():
            continue
        text = p.read_text(encoding="utf-8", errors="replace")
        lints = collections.Counter()
        for m in ATTR.finditer(text):
            for lint in REASON.sub("", m.group(1)).split(","):
                lint = lint.strip()
                if lint:
                    lints[lint] += 1
        if lints:
            found[f] = dict(sorted(lints.items()))
    return found


def main(argv):
    found = scan()
    if "--update" in argv:
        INVENTORY.write_text(json.dumps(found, indent=2) + "\n", encoding="utf-8")
        print(f"inventory updated: {sum(sum(v.values()) for v in found.values())} allowed lints in {len(found)} files")
        return 0
    known = json.loads(INVENTORY.read_text(encoding="utf-8"))
    added, removed = [], []
    for f in sorted(set(found) | set(known)):
        now, before = found.get(f, {}), known.get(f, {})
        for lint in sorted(set(now) | set(before)):
            d = now.get(lint, 0) - before.get(lint, 0)
            if d > 0:
                added.append(f"{f}: allow({lint}) +{d}")
            elif d < 0:
                removed.append(f"{f}: allow({lint}) {d}")
    for a in added:
        print(f"NEW ALLOW  {a}")
    for r in removed:
        print(f"removed    {r} (run with --update to shrink the inventory)")
    total = sum(sum(v.values()) for v in found.values())
    print(f"{total} allowed lints in {len(found)} product files; {len(added)} new, {len(removed)} removed")
    return 1 if added or removed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
