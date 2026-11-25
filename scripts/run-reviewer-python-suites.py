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
    # M4 C4.1 round 1 (declared in reviews/M4-C4.1-r1-response.md): a probe of the running application.
    "c41-r1-http-lifecycle.py": "attached-mode probe: needs the application binary and a receipt path; CI runs it in "
    "the `desktop gui` job on both OSes against the bundled product build (receipt desktop-gui-<OS>)",
    # M4 C4.1 round 2 (declared in reviews/M4-C4.1-r2-response.md): Windows desktop probes of the running application
    # (Win32 window APIs through ctypes); without a binary argument they print `skipped: no application`.
    "c41-r2-desktop-floats.py": "Windows float-window probe: needs the application binary and a receipt path; CI runs "
    "it in the `desktop gui` job on Windows against the bundled product build (receipt desktop-gui-Windows); macOS "
    "float checks are the builder e2e's",
    "c41-r2-desktop-keys.py": "Windows keymap and menu probe: needs the application binary and a receipt path; CI runs "
    "it in the `desktop gui` job on Windows against the bundled product build (receipt desktop-gui-Windows)",
    # M4 C4.1 round 3 (declared in reviews/M4-C4.1-r3-response.md): Windows desktop probes of the running application.
    "c41-r3-desktop-changes.py": "Windows agent-server, freshness and policy probe: needs the application binary and a "
    "receipt path; CI runs it in the `desktop gui` job on Windows against the bundled product build (receipt "
    "desktop-gui-Windows)",
    "c41-r3-desktop-visibility.py": "Windows float-visibility sequence probe: needs the application binary and a receipt "
    "path; the reviewer's receipt is reviews/evidence/M4-C4.1-r3/c2-visibility.json (C41-R2-10 closed); the same "
    "sequences are in the builder e2e and the round-2 float probe in CI",
    # M4 C4.2b round 1 (declared in reviews/M4-C4.2b-r1-response.md): probes of the running application.
    "c42b-r1-live.py": "Windows OS-input and live-budget probe of the running application: needs the release binary "
    "and a mode; CI runs modes c2, c3, d2, d2-exact-attached, d2-exact-headless and d3-webgl2 in the `desktop gui` "
    "job on Windows against the default release build (receipt desktop-gui-Windows/c42b-r1-live); the budget modes "
    "d1-long-webgpu, d1-long-webgl2, d1-extra and d3-webgpu (a WebGPU adapter) are workstation receipts "
    "(reviews/evidence/M4-C4.2b/live/, copied from the probe's output folder)",
    # M4 C4.2b round 2 (declared in reviews/M4-C4.2b-r1-response.md, round-2 section): the reviewer's live runner.
    "c42b-r2-live.py": "Windows live runner of the running application: needs the release binary, a mode and for "
    "some modes the headless binary; CI runs modes c2, d3-webgl2, gui, regression and selection in the "
    "`desktop gui` job and byte, exact, exact-select and exact-tools (which need the headless binary) in the "
    "`drawing byte gate` job, both on Windows against the default build (receipts desktop-gui-Windows/c42b-r2-live, "
    "drawing-byte-gate-Windows/c42b-r2-live); measure-webgl2, measure-webgpu, d1-long-webgl2, d1-long-webgpu and "
    "d3-webgpu are workstation receipts (reviews/evidence/M4-C4.2b/live/, copied from the runner's output folder); "
    "observer asserts that a read-only session sees the gesture samples, which ruling D-19 removed, so it is not run",
    # M4 C4.2b round 3 (declared in reviews/M4-C4.2b-r1-response.md, round-3 section): the reviewer's live runner.
    "c42b-r3-live.py": "Windows live runner of the running application (round 3; other modes pass through to the "
    "round-2 runner): needs the release binary, a mode and for labels the headless binary; CI runs endings in the "
    "`desktop gui` job and labels in the `drawing byte gate` job, both on Windows against the default release build "
    "(receipts desktop-gui-Windows/c42b-r3-live, drawing-byte-gate-Windows/c42b-r3-live); fixed-webgl2 and "
    "fixed-webgpu are workstation receipts",
    "c42b-r1-conformance.py": "attached-mode MCP conformance through the running application: needs the release "
    "binary; the same suite (scripts/mcp-conformance.py --http) runs in the `desktop gui` job on both OSes",
    # M4 C4.3b round 1 (declared in reviews/M4-C4.3b-handoff-r2.md): a wrapper, not a suite.
    "c43b-r1-regression-runner.py": "the reviewer's wrapper that its live runner (c43b-r1-live.py case7) calls with a "
    "probe's path and arguments to run an earlier probe with its receipts redirected; without them it has nothing to "
    "run",
    # M4 C4.4a round 1 (declared in reviews/M4-C4.4a-handoff-r2.md): the reviewer's live runner.
    "c44a-r1-live.py": "Windows live runner of the running application (round 1 sessions l1a to l4b): needs the release "
    "binary and a session and step argument (without one it stops at its argument check); its steps are run by hand "
    "for the round-2 handoff, results in reviews/M4-C4.4a-handoff-r2.md",
    # M4 C4.4a round 2 (declared in reviews/M4-C4.4a-handoff-r3.md): the reviewer's round-2 live runner.
    "c44a-r2-live.py": "Windows live runner of the running application (round 2 cases m1a, m1b, m2a, m2b): needs the "
    "release binary and a case and step argument (without one it fails at its argument read); its steps are run by "
    "hand for the round-3 handoff, results in reviews/M4-C4.4a-handoff-r3.md",
    # M4 C4.4a round 3 (declared in reviews/M4-C4.4a-handoff-r4.md): the reviewer's round-3 live runner.
    "c44a-r3-live.py": "Windows live runner of the running application (round 3 cases n1a, n1b, n2a, n2b): needs the "
    "release binary and a case and step argument (without one it fails at its argument read); its steps are run by "
    "hand for the round-4 handoff, results in reviews/M4-C4.4a-handoff-r4.md",
    # M4 C4.4a closure (declared in reviews/M4-C4.4a-handoff-r5.md): the reviewer's closure live runner.
    "c44a-r4-live.py": "Windows live runner of the running application (closure cases p1a, p1b): needs the release "
    "binary and a case and step argument (without one it fails at its argument read); its steps are run by hand for the "
    "round-5 handoff, results in reviews/M4-C4.4a-handoff-r5.md",
    # M4 C4.4b round 1 (declared in reviews/M4-C4.4b-handoff-r2.md): the reviewer's live runner.
    "c44b-r1-live.py": "Windows live runner of the running application (round 1 cases l1a to l3b): needs the release "
    "binary and a case and step argument (without one it fails at its argument read); its steps are run by hand for the "
    "round-2 handoff, results in reviews/M4-C4.4b-handoff-r2.md",
    # M4 C4.4b round 2 (declared in reviews/M4-C4.4b-handoff-r3.md): the reviewer's live runner.
    "c44b-r2-live.py": "Windows live runner of the running application (round 2 cases m1 and m2): needs the release "
    "binary and a case and step argument (without one it fails at its argument read); its steps are run by hand for the "
    "round-3 handoff, results in reviews/M4-C4.4b-handoff-r3.md",
    # M4 C4.4b round 3 (declared in reviews/M4-C4.4b-handoff-r4.md): the reviewer's live runner.
    "c44b-r3-live.py": "Windows live runner of the running application (round 3 cases n1 and n2): needs the release "
    "binary and a case and step argument (without one it fails at its argument read); its steps are run by hand for the "
    "closure handoff, results in reviews/M4-C4.4b-handoff-r4.md",
    # M4 C4.4b closure (declared in reviews/M4-C4.4b-handoff-r5.md): the reviewer's live closure runner.
    "c44b-r4-live.py": "Windows live runner of the running application (the scoped closure check): needs the release "
    "binary and a case and step argument (without one it fails at its argument read); the reviewer ran it for the "
    "closure check, results in reviews/M4-C4.4b-r4-closure-live.md",
    "c34-r2-inventory.py": "round-2 receipt pinned to 1cbd190..967229a that writes reviews/evidence/M3-C3.4-r2/"
    "inventory.json and asserts the register ends at SI-110 (SI-111 is ruled in round 2); its live counterpart is "
    "scripts/check-merge-note-inventory.py",
    # M5 C5.1b round 1 (declared in reviews/M5-C5.1b-r1-response.md): the live reviewer's runner.
    "c51b-r1-live.py": "Windows live runner of the running application (round 1 of C5.1b): needs a case and a step "
    "number; receipts retained under reviews/evidence/M5-C5.1b-r1/",
    # M5 C5.1b round 2 (declared in reviews/M5-C5.1b-handoff-r3.md): the live reviewer's runner, as round 1's.
    "c51b-r2-live.py": "Windows live runner of the running application (round 2 of C5.1b, importing round 1's): needs "
    "a step number; receipts retained under reviews/evidence/M5-C5.1b-r2/review/",
    # M5 C5.1b closure (declared in the round 3 handoff's addendum): the live closure runner, as rounds 1 and 2's.
    "c51b-r3-live.py": "Windows live runner of the running application (the C5.1b closure, importing rounds 1 and 2): "
    "needs a step number and 120 GB free; receipts retained under reviews/evidence/M5-C5.1b-r3/review/",
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
