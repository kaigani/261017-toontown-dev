"""Checks a built desktop executable for the M0 diagnostics gating (C0.4).

Usage: python scripts/check-diagnostics-gating.py <toontown executable> --expect default|diagnostics

The command names come from the host's single registry in desktop/src-tauri/src/lib.rs (`command_registry![...]`,
one block per build). Tauri's invoke handler matches a command by its name, so a registered command's name is in
the executable as plain bytes.
* `--expect default`: the executable contains every product command name and **no** diagnostic one (no
  `--ipc-bench` benchmark or spike command is registered).
* `--expect diagnostics`: it contains every product and every diagnostic name. This is the control: it shows the
  scan finds names that are registered.
The embedded front end is not scanned here (release builds compress it); desktop/src/diagnostics/gating.build.test.ts
checks the bundle itself. Exit status 0 = as expected, 1 = not, 2 = usage. Standard library only.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
LIB = ROOT / "desktop/src-tauri/src/lib.rs"


def registry(source, cfg):
    m = re.search(r"#\[cfg\(" + re.escape(cfg) + r"\)\]\s*command_registry!\[(.*?)\]", source, re.S)
    if not m:
        raise SystemExit(f"no command_registry! block for cfg({cfg}) in {LIB}")
    return [p.strip().split("::")[-1] for p in m.group(1).split(",") if p.strip()]


def main(argv):
    if len(argv) != 4 or argv[2] != "--expect" or argv[3] not in ("default", "diagnostics"):
        print(__doc__)
        return 2
    exe, expect = pathlib.Path(argv[1]), argv[3]
    source = LIB.read_text(encoding="utf-8")
    product = registry(source, 'not(feature = "m0-diagnostics")')
    diagnostic = [n for n in registry(source, 'feature = "m0-diagnostics"') if n not in product]
    data = exe.read_bytes()
    found = {n: n.encode() in data for n in product + diagnostic}
    problems = [f"product command {n} not found" for n in product if not found[n]]
    for n in diagnostic:
        if expect == "default" and found[n]:
            problems.append(f"diagnostic command {n} is in a default build")
        if expect == "diagnostics" and not found[n]:
            problems.append(f"diagnostic command {n} is missing from a diagnostics build")
    present = sorted(n for n in diagnostic if found[n])
    print(f"{exe.name} ({len(data)} bytes), expected {expect}: product {len(product)}/{len(product)} names, "
          f"diagnostic {len(present)}/{len(diagnostic)} names present")
    for p in problems:
        print(f"  {p}")
    print("diagnostics gating: " + ("FAIL" if problems else "ok"))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
