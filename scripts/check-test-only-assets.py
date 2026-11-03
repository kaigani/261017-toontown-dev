#!/usr/bin/env python3
"""Test-only assets under their own licences never reach a product (ADR-0003 rule 7; M2 C2.4, C21-R1-11).

Usage:
  python scripts/check-test-only-assets.py                 repository check
  python scripts/check-test-only-assets.py --bundle PATH   also scan a built product (file or directory) for them

The manifest `tests/fixtures/test-only-assets.json` lists every test-only asset with its licence file and SHA-256.
Repository check: every font file tracked by git is listed; every listed asset exists with its hash, lies under
`tests/fixtures/` and has its licence file beside it; no product source (crates/*/src, bins, desktop/src,
desktop/src-tauri) names an asset's file name. Bundle check: no file of the product contains a 64-byte sample of any
listed asset. Exit status 0 when every check passes, 1 otherwise. Standard library only.
"""
import hashlib
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "tests" / "fixtures" / "test-only-assets.json"
FONT_SUFFIXES = (".ttf", ".otf", ".ttc", ".woff", ".woff2")
SOURCE_SUFFIXES = (".rs", ".ts", ".tsx", ".js", ".mjs", ".json", ".toml", ".html", ".css")


def tracked() -> list[str]:
    out = subprocess.run(["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True).stdout
    return [line for line in out.splitlines() if line]


def samples(data: bytes) -> list[bytes]:
    """Three 64-byte samples from the middle of an asset (distinctive, not header bytes shared by every font)."""
    n = len(data)
    return [data[k : k + 64] for k in (n // 4, n // 2, 3 * n // 4) if k + 64 <= n]


def main(argv: list[str]) -> int:
    failures: list[str] = []
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    assets = manifest["assets"]
    listed = {a["path"] for a in assets}
    files = tracked()
    for f in files:
        if f.lower().endswith(FONT_SUFFIXES) and f not in listed:
            failures.append(f"font file not in the test-only manifest: {f}")
    for a in assets:
        path = ROOT / a["path"]
        if not a["path"].startswith("tests/fixtures/"):
            failures.append(f"test-only asset outside tests/fixtures/: {a['path']}")
        if not path.is_file():
            failures.append(f"missing asset: {a['path']}")
            continue
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest != a["sha256"]:
            failures.append(f"hash differs for {a['path']}: {digest}")
        licence = path.parent / a["licenceFile"]
        if not licence.is_file():
            failures.append(f"licence file missing beside {a['path']}: {a['licenceFile']}")
    names = {pathlib.PurePosixPath(a["path"]).name for a in assets}
    for f in files:
        parts = f.split("/")
        product = f.startswith(("bins/", "desktop/src/", "desktop/src-tauri/")) or (
            len(parts) > 3 and parts[0] == "crates" and parts[2] == "src"
        )
        if not product or not f.endswith(SOURCE_SUFFIXES):
            continue
        text = (ROOT / f).read_text(encoding="utf-8", errors="replace")
        for name in names:
            if name in text:
                failures.append(f"product source names the test-only asset {name}: {f}")
    if "--bundle" in argv:
        target = pathlib.Path(argv[argv.index("--bundle") + 1])
        needles = [s for a in assets for s in samples((ROOT / a["path"]).read_bytes())]
        paths = [target] if target.is_file() else [p for p in target.rglob("*") if p.is_file()]
        if not paths:
            failures.append(f"nothing to scan at {target}")
        for p in paths:
            data = p.read_bytes()
            for needle in needles:
                if needle in data:
                    failures.append(f"product file contains a test-only asset: {p}")
                    break
        print(f"scanned {len(paths)} product file(s) under {target} for {len(needles)} samples")
    for f in failures:
        print(f"FAIL: {f}")
    if not failures:
        print(f"ok: {len(assets)} test-only asset(s), licences beside them, none in product sources"
              + (" or the bundle" if "--bundle" in argv else ""))
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
