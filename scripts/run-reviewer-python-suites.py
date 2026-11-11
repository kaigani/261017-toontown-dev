"""Runs every reviewer Python suite under tests/adversarial/ (CI step "Reviewer Python suites"; C1.1 closure).

Run from anywhere: python scripts/run-reviewer-python-suites.py

Every `tests/adversarial/*.py` is a suite and runs with the current interpreter in isolated mode (`-I`), from the
repository root, unless it is listed in NOT_SUITES with the reason. New reviewer suites therefore run in CI without
editing this file. The exit status is non-zero if any suite fails.

`c04-r3-page.py` starts Vitest through `cmd /c` (Windows only). On other platforms this runner performs the same steps
itself — copy `tests/adversarial/c04-r3-actions.tsx` to `desktop/src/test/c04-r3-actions.test.tsx`, run that one file
with Vitest, delete the copy — so macOS executes the same reviewer test. The reviewer file is not changed.
"""
import os
import pathlib
import shutil
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
SUITES = ROOT / "tests" / "adversarial"
NOT_SUITES = {
    "collect_runtime_evidence.py": "M0 evidence collector, not a test: needs a release build and writes "
    "reviews/evidence/M0-C0.1-r1/",
    # Orchestrator ruling, M2 C2.3 round 3: a round receipt, not a product invariant.
    "c23-r3-finalize.py": "round-3 collector pinned to 989233e; receipts retained under reviews/evidence/M2-C2.3-r3",
    # M3 C3.4 round 2 (declared in reviews/M3-C3.4-r2-response.md): two round collectors, not product invariants.
    "c34-r2-sdk.py": "SDK 2.0.0 probe collector: needs `pip install mcp` and the release binary at a Windows path; "
    "receipts retained under reviews/evidence/M3-C3.4-r2",
    "c34-r2-inventory.py": "round-2 receipt pinned to 1cbd190..967229a that writes reviews/evidence/M3-C3.4-r2/"
    "inventory.json and asserts the register ends at SI-110 (SI-111 is ruled in round 2); its live counterpart is "
    "scripts/check-merge-note-inventory.py",
}
# Single tests of a suite that are fixed-commit receipts of a finding rather than live checks: the suite runs every other
# test (by name). Each entry names its live counterpart.
NOT_TESTS = {
    "c24-r2-audit.py": {
        "Ownership.test_edit38_changes_only_the_two_ruled_union_assertions": "C24-R2-02 receipt comparing the fixed "
        "round-1 head 3c74c34 (the finding itself; it cannot change); the restoration and the declared disposition are "
        "checked live by scripts/check-reviewer-ownership.py (M2 C2.4 round 2, edit 57; for the round-3 ruling)",
    },
}


def test_names(path: pathlib.Path) -> list:
    """`Class.method` of every unittest test in a suite."""
    import ast

    tree = ast.parse(path.read_text(encoding="utf-8"))
    out = []
    for node in tree.body:
        if isinstance(node, ast.ClassDef):
            for item in node.body:
                if isinstance(item, ast.FunctionDef) and item.name.startswith("test"):
                    out.append(f"{node.name}.{item.name}")
    return out


def run_page_shim() -> int:
    target = ROOT / "desktop" / "src" / "test" / "c04-r3-actions.test.tsx"
    if target.exists():
        print(f"{target} already exists", flush=True)
        return 1
    pnpm = shutil.which("pnpm")
    if pnpm is None:
        print("pnpm not found", flush=True)
        return 1
    try:
        target.write_bytes((SUITES / "c04-r3-actions.tsx").read_bytes())
        cmd = [pnpm, "--dir", "desktop", "exec", "vitest", "run", "--config", "vite.config.ts",
               "src/test/c04-r3-actions.test.tsx"]
        return subprocess.run(cmd, cwd=ROOT).returncode
    finally:
        target.unlink(missing_ok=True)


def main() -> int:
    results = []
    for path in sorted(SUITES.glob("*.py")):
        if path.name in NOT_SUITES:
            print(f"skip {path.name}: {NOT_SUITES[path.name]}", flush=True)
            continue
        print(f"=== {path.name}", flush=True)
        start = time.monotonic()
        if path.name == "c04-r3-page.py" and os.name != "nt":
            rc = run_page_shim()
        elif path.name in NOT_TESTS:
            skipped = NOT_TESTS[path.name]
            for name, reason in skipped.items():
                print(f"skip {path.name} {name}: {reason}", flush=True)
            names = [n for n in test_names(path) if n not in skipped]
            rc = subprocess.run([sys.executable, "-I", str(path), *names], cwd=ROOT).returncode
        else:
            rc = subprocess.run([sys.executable, "-I", str(path)], cwd=ROOT).returncode
        results.append((path.name, rc, time.monotonic() - start))
    print("=== summary", flush=True)
    for name, rc, seconds in results:
        print(f"{'ok  ' if rc == 0 else 'FAIL'} {name} ({seconds:.1f} s)", flush=True)
    failed = [name for name, rc, _ in results if rc != 0]
    print(f"{len(results) - len(failed)}/{len(results)} reviewer Python suites passed", flush=True)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
