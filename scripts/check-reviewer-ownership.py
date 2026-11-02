#!/usr/bin/env python3
"""Live check of the reviewer-owned files changed in M2 C2.4 (CI `check`; standard library only).

Usage: python scripts/check-reviewer-ownership.py

Each reviewer-owned file differs from its reviewer revision exactly by the edits the orchestrator ruled and the builder
declared, and by nothing else:

- `crates/executor/tests/c24_r1_topology.rs` (reviewer revision `700868e`): the two union bounds become the oracle's
  snapping envelope (ruling C24-R1-02(ii), edit 38) and the area-preservation tolerance becomes the envelope
  (C24-R2-02 disposition, edit 46, subject to round-3 confirmation);
- `crates/executor/tests/c24_r2_topology.rs` (`9b37e9d`): the same tolerance in the reviewer's `edit38…` reproduction
  (ruling (b), edit 46);
- `crates/executor/tests/c24_r2_routes.rs` (`9b37e9d`): every `#[test]` function byte-identical (ruling (a): only the
  embedded oracle helpers mirror the builder oracle, edits 49 and 53);
- every other round-2 reviewer file unchanged.

This is the live counterpart of the reviewer's `Ownership` receipt in `tests/adversarial/c24-r2-audit.py`, which compares
the fixed round-1 head `3c74c34` (the finding itself) and cannot change; exit status 1 on any difference.
"""
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]


def show(rev, path):
    return subprocess.run(["git", "show", f"{rev}:{path}"], cwd=ROOT, capture_output=True, check=True).stdout.decode("utf-8")


def now(path):
    return (ROOT / path).read_text(encoding="utf-8")


def replace_once(text, old, new, what):
    if text.count(old) != 1:
        raise SystemExit(f"{what}: expected one occurrence of {old!r}")
    return text.replace(old, new)


def tests(text):
    """The `#[test]` functions of a Rust file, by name."""
    out = {}
    for m in re.finditer(r"^#\[test\]\n(?:#\[[^\n]*\]\n)*fn (\w+)\(.*?^\}\n", text, re.S | re.M):
        out[m.group(1)] = m.group(0)
    return out


def main():
    failures = []

    path = "crates/executor/tests/c24_r1_topology.rs"
    want = show("700868e", path).replace(
        "sa <= union + 1e-3 * union.max(1.0)", "sa <= union + oracle::snapping_envelope(&segs, 64)"
    )
    want = replace_once(
        want,
        "prop_assert!((sa - sb).abs() <= 1e-3 * sb.max(1.0), ",
        "prop_assert!((sa - sb).abs() <= oracle::snapping_envelope(&[segments(&list, 0), vec![(from, to)]].concat(), 64), ",
        path,
    )
    if now(path) != want:
        failures.append(f"{path} differs from 700868e beyond edits 38 and 46")

    path = "crates/executor/tests/c24_r2_topology.rs"
    want = replace_once(
        show("9b37e9d", path),
        '    assert!((sa - sb).abs() <= 1e-3 * sb.max(1.0), "the cut keeps the zones\' total area: {sb} vs {sa}");\n',
        "    assert!(\n"
        "        (sa - sb).abs() <= oracle::snapping_envelope(&[segments(&list, 0), vec![((80, 60), (0, 0))]].concat(), 64),\n"
        '        "the cut keeps the zones\' total area: {sb} vs {sa}"\n'
        "    );\n",
        path,
    )
    if now(path) != want:
        failures.append(f"{path} differs from 9b37e9d beyond edit 46")

    path = "crates/executor/tests/c24_r2_routes.rs"
    before, after = tests(show("9b37e9d", path)), tests(now(path))
    if before != after:
        changed = sorted(n for n in set(before) | set(after) if before.get(n) != after.get(n))
        failures.append(f"{path}: #[test] functions changed: {changed}")
    if len(before) < 13:
        failures.append(f"{path}: only {len(before)} #[test] functions found")

    for path in [
        "crates/executor/tests/c24_r2_variants.rs",
        "crates/vector/tests/c24_r2_noding.rs",
        "crates/vector/tests/c24_r2_properties.rs",
        "tests/adversarial/c24-r2-audit.py",
        "tests/adversarial/c24-r2-run.py",
    ]:
        if now(path) != show("9b37e9d", path):
            failures.append(f"{path} changed")

    for f in failures:
        print(f"FAIL {f}")
    if not failures:
        print("ok: reviewer-owned C2.4 files differ only by the ruled, declared edits")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
