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
}


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
