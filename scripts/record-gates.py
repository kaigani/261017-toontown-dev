"""Runs the local gates and records raw logs with a receipt (evidence hygiene: reviews/evidence/**).

Usage: python scripts/record-gates.py <out-dir> [gate ...]

Each gate's combined stdout/stderr goes to <out-dir>/gates/<name>.log (the command line first, the exit status last).
<out-dir>/receipt.json lists the commands, the exit status of every recorded gate, the base commit, the dirty files,
the machine and the sha256 of every file under <out-dir>/gates. Gates can be recorded in several calls (each call
updates the receipt); with no gate names, every gate runs. The exit status is non-zero if a gate of this call failed.
"""
import datetime
import hashlib
import json
import pathlib
import platform
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
PY = sys.executable
EXE = ROOT / "target" / "release" / ("toontown-headless" + (".exe" if sys.platform == "win32" else ""))
QEXE = f'"{EXE}"'

GATES = {
    "fmt": ["cargo fmt --all --check"],
    "clippy": ["cargo clippy --workspace --all-targets --locked -- -D warnings"],
    "clippy-diag": ["cargo clippy -p tt-desktop --all-targets --locked --features m0-diagnostics -- -D warnings"],
    "test": ["cargo test --workspace --locked --no-fail-fast"],
    "test-diag": ["cargo test -p tt-desktop --locked --features m0-diagnostics"],
    "deny": ["cargo deny --locked check"],
    "pnpm-lint": ["pnpm lint"],
    "pnpm-typecheck": ["pnpm typecheck"],
    "pnpm-test": ["pnpm test"],
    "pnpm-format": ["pnpm format:check"],
    "pnpm-licenses": ["pnpm licenses:check"],
    "build": ["pnpm --dir desktop build:diagnostics", "pnpm --dir desktop build"],
    "headless": [
        "cargo build --release --locked -p tt-headless",
        f"{QEXE} --version",
        f"{QEXE} --selftest",
        f'"{PY}" scripts/check-headless-deps.py {QEXE}',
    ],
    "reviewer-python-suites": [f'"{PY}" scripts/run-reviewer-python-suites.py'],
    # C1.4: the official MCP Python SDK client against the release binary (needs `pip install mcp`).
    "mcp-conformance": [f'"{PY}" scripts/mcp-conformance.py {QEXE}'],
    # C14-R1-10: test-only features never reach the product dependency graph.
    "release-features": [f'"{PY}" scripts/check-release-features.py'],
    # C14-R1-07: the C1.4 measurement summaries recomputed from their observations (after the test gate).
    "measurements": [f'"{PY}" scripts/analyse-measurements.py target/evidence/c14'],
    # Declared changes: generated declarations are current, and no product `allow` attribute appeared outside the
    # reviewed inventory `scripts/product-allows.json` (C14-R3-05).
    "declarations": [
        f'"{PY}" scripts/check-product-allows.py',
        f'"{PY}" scripts/gen-command-declarations.py',
        "git diff --exit-code -- crates/registry/src/declared.rs crates/registry/src/declared.json plan/spec-issues.md",
    ],
}


def sh(cmd: str, log) -> int:
    log.write(f"$ {cmd}\n")
    log.flush()
    p = subprocess.run(cmd, shell=True, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=False)
    log.write(f"\n[exit {p.returncode}]\n")
    log.flush()
    return p.returncode


def selftests(gates: pathlib.Path) -> int:
    outs = []
    for i in (1, 2):
        p = subprocess.run([str(EXE), "--selftest", "--json"], cwd=ROOT, capture_output=True, check=False)
        (gates / f"selftest-{i}.json").write_bytes(p.stdout)
        outs.append(p)
    strip = [[(c["id"], c["ok"]) for c in json.loads(o.stdout)["checks"]] for o in outs]
    same = strip[0] == strip[1] and all(ok for _, ok in strip[0])
    (gates / "selftest-compare.log").write_text(f"{strip[0]}\nidentical and all ok: {same}\n", encoding="utf-8")
    return 0 if same else 1


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    out = (ROOT / sys.argv[1]).resolve()
    names = sys.argv[2:] or list(GATES)
    gates = out / "gates"
    gates.mkdir(parents=True, exist_ok=True)
    receipt_path = out / "receipt.json"
    receipt = json.loads(receipt_path.read_text(encoding="utf-8")) if receipt_path.exists() else {}
    status = receipt.get("status", {})
    failed = False
    for name in names:
        with open(gates / f"{name}.log", "w", encoding="utf-8", newline="\n") as log:
            code = 0
            for cmd in GATES[name]:
                code = sh(cmd, log)
                if code != 0:
                    break
        if name == "headless" and code == 0:
            code = selftests(gates)
        status[f"gates/{name}.log"] = code
        failed |= code != 0
        print(f"{name}: exit {code}")
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=False)
    dirty = subprocess.run(["git", "status", "--porcelain"], cwd=ROOT, capture_output=True, text=True, check=False)
    receipt.update(
        {
            "what": receipt.get("what", "Local gate logs recorded by scripts/record-gates.py."),
            "commands": {f"gates/{k}.log": " && ".join(v) for k, v in GATES.items()},
            "status": status,
            "recordedAt": datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0).isoformat(),
            "baseCommit": head.stdout.strip(),
            "dirtyFiles": [l[3:] for l in dirty.stdout.splitlines() if l.strip()],
            "machine": {
                "os": platform.platform(),
                "cpu": platform.processor(),
                "rustc": subprocess.run(["rustc", "--version"], capture_output=True, text=True, check=False).stdout.strip(),
            },
            "sha256": {
                str(p.relative_to(out)).replace("\\", "/"): hashlib.sha256(p.read_bytes()).hexdigest()
                for p in sorted(gates.iterdir())
                if p.is_file()
            },
        }
    )
    receipt_path.write_text(json.dumps(receipt, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
