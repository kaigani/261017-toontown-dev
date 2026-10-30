#!/usr/bin/env python3
"""Proves that test-only features are absent from the product dependency graph (C14-R1-10).

Usage: python scripts/check-release-features.py

Runs `cargo tree` on the normal (non-dev) edges of every product package (`tt-headless`, `tt-desktop`) with their
features and fails if a test-only feature (`test-hooks`) is enabled on any crate. Prints the graph lines that mention
`tt-persist` as the proof.
"""
import subprocess
import sys

PRODUCTS = ["tt-headless", "tt-desktop"]
TEST_ONLY = ["test-hooks"]


def main() -> int:
    failed = False
    for package in PRODUCTS:
        cmd = ["cargo", "tree", "--locked", "-p", package, "-e", "normal,build,features", "--prefix", "none"]
        p = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace", check=False)
        print(f"$ {' '.join(cmd)}  (exit {p.returncode})")
        if p.returncode != 0:
            print(p.stderr)
            failed = True
            continue
        persist = sorted({line.strip() for line in p.stdout.splitlines() if "tt-persist" in line})
        for line in persist:
            print(f"  {line}")
        bad = [line for line in p.stdout.splitlines() if any(f'"{f}"' in line or f" {f}" in line for f in TEST_ONLY)]
        if bad:
            failed = True
            print(f"  FAIL: test-only feature in the {package} product graph:")
            for line in sorted(set(bad)):
                print(f"    {line.strip()}")
        else:
            print(f"  ok: no test-only feature ({', '.join(TEST_ONLY)}) in the {package} product graph")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
