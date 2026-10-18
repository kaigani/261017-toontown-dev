"""Sequential local gate groups; every command has a raw log and receipt."""
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
groups = {
    "rust": [
        ("fmt", ["cargo", "fmt", "--all", "--check"]),
        ("clippy", ["cargo", "clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"]),
        ("clippy-diagnostics", ["cargo", "clippy", "-p", "tt-desktop", "--all-targets", "--locked", "--features", "m0-diagnostics", "--", "-D", "warnings"]),
        ("test-diagnostics", ["cargo", "test", "-p", "tt-desktop", "--locked", "--features", "m0-diagnostics"]),
        ("deny", ["cargo", "deny", "--locked", "check"]),
        ("headless-build", ["cargo", "build", "--release", "--locked", "-p", "tt-headless"]),
        ("headless-1", [r".\target\release\toontown-headless.exe", "--selftest", "--json"]),
        ("headless-2", [r".\target\release\toontown-headless.exe", "--selftest", "--json"]),
        ("headless-guard", ["python", "scripts/check-headless-deps.py", "target/release/toontown-headless.exe"]),
        ("raster", ["cargo", "run", "--release", "--locked", "-q", "-p", "tt-render", "--example", "c03_raster", "--", "--out", "target/c11-r2-raster", "--report", "reviews/evidence/M1-C1.1-r2/raster.json", "--check", "crates/render/examples/c03_raster/hashes.json"]),
        ("tooling", ["python", "tests/adversarial/c03-r1-tooling.py"]),
    ],
    "frontend": [(name, ["pnpm", name]) for name in ["lint", "typecheck", "format:check", "licenses:check"]] + [
        ("build-diagnostics", ["pnpm", "--dir", "desktop", "build:diagnostics"]),
        ("build-default", ["pnpm", "--dir", "desktop", "build"]),
    ],
    "tauri": [
        ("tauri-release", ["cmd", "/c", "pnpm", "--dir", "desktop", "tauri", "build", "--no-bundle", "--", "--locked"]),
        ("diagnostics-guard", ["python", "scripts/check-diagnostics-gating.py", "target/release/toontown.exe", "--expect", "default"]),
    ],
    "scale": [(f"scale-{i}", ["cargo", "run", "--release", "--locked", "-p", "tt-model", "--example", "scale"]) for i in [1, 2]],
}
prefix = sys.argv[2] if len(sys.argv) > 2 else "gate"
results = {}
for name, args in groups[sys.argv[1]]:
    p = subprocess.run([sys.executable, str(HERE / "run.py"), prefix + "-" + name, *args], cwd=ROOT)
    results[name] = p.returncode
print(results)
sys.exit(int(any(results.values())))
