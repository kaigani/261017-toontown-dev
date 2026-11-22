"""Developer tool (not run in CI): prunes stale build artefacts from the one `target/` directory between cleans.

For each profile (`target/debug`, `target/release`):
- `deps/`: files are named `<stem>-<16 hex>[.<ext>]` (`.exe`, `.pdb`, `.d`, `.rlib`, `.rmeta`, `.dll`, ...). For each stem
  only the newest hash's file set is kept (newest by the latest modification time among that hash's files); the older
  hashes' files are deleted.
- `incremental/`: directories are named `<stem>-<hash>`. For each stem only the newest directory is kept.

Run it after a local test cycle, never while cargo is running (it refuses when a cargo or rustc process is found).
`--dry-run` prints what would be deleted and how much it would free, deleting nothing. Every deletion is a literal path
the script computed inside `target/`; nothing outside `target/<profile>/deps` and `target/<profile>/incremental` is
touched. Standard library only; run from the repository root.
"""
import argparse
import os
import pathlib
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
TARGET = ROOT / "target"
DEP = re.compile(r"^(?P<stem>.+)-(?P<hash>[0-9a-f]{16})(?P<ext>\.[A-Za-z0-9.]+)?$")
INC = re.compile(r"^(?P<stem>.+)-(?P<hash>[0-9a-z]+)$")


def cargo_running():
    try:
        if os.name == "nt":
            out = subprocess.run(["tasklist", "/FO", "CSV", "/NH"], capture_output=True, text=True, timeout=30).stdout
        else:
            out = subprocess.run(["ps", "-A", "-o", "comm="], capture_output=True, text=True, timeout=30).stdout
    except (OSError, subprocess.SubprocessError):
        return False
    names = [line.split(",")[0].strip('"').lower() for line in out.splitlines()]
    return any(n.startswith(("cargo", "rustc")) for n in names)


def size_of(p: pathlib.Path) -> int:
    if p.is_file():
        return p.stat().st_size
    total = 0
    for dirpath, _dirs, files in os.walk(p):
        for f in files:
            try:
                total += (pathlib.Path(dirpath) / f).stat().st_size
            except OSError:
                pass
    return total


def mtime(p: pathlib.Path) -> float:
    try:
        return p.stat().st_mtime
    except OSError:
        return 0.0


def stale_deps(deps: pathlib.Path):
    groups = {}
    for f in deps.iterdir():
        if not f.is_file():
            continue
        m = DEP.match(f.name)
        if not m:
            continue
        groups.setdefault(m["stem"], {}).setdefault(m["hash"], []).append(f)
    for stem, by_hash in groups.items():
        if len(by_hash) < 2:
            continue
        newest = max(by_hash, key=lambda h: max(mtime(f) for f in by_hash[h]))
        for h, files in by_hash.items():
            if h != newest:
                yield from files


def stale_incremental(inc: pathlib.Path):
    groups = {}
    for d in inc.iterdir():
        if not d.is_dir():
            continue
        m = INC.match(d.name)
        if m:
            groups.setdefault(m["stem"], []).append(d)
    for dirs in groups.values():
        if len(dirs) < 2:
            continue
        newest = max(dirs, key=mtime)
        yield from (d for d in dirs if d != newest)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--dry-run", action="store_true", help="print what would be freed; delete nothing")
    args = ap.parse_args()
    if not TARGET.is_dir():
        print("no target/ directory")
        return 0
    if not args.dry_run and cargo_running():
        print("a cargo or rustc process is running; not pruning (run again when it has finished)")
        return 2
    totals = {"deps": 0, "incremental": 0}
    count = 0
    for profile in ("debug", "release"):
        base = TARGET / profile
        for kind, finder in (("deps", stale_deps), ("incremental", stale_incremental)):
            where = base / kind
            if not where.is_dir():
                continue
            for p in list(finder(where)):
                # Only literal paths inside target/<profile>/<kind> are ever removed.
                if where.resolve() not in p.resolve().parents:
                    continue
                n = size_of(p)
                totals[kind] += n
                count += 1
                if not args.dry_run:
                    if p.is_dir():
                        shutil.rmtree(p, ignore_errors=True)
                    else:
                        try:
                            p.unlink()
                        except OSError:
                            pass
    gib = lambda n: n / (1 << 30)
    verb = "would free" if args.dry_run else "freed"
    print(f"{count} stale entries; {verb} {gib(totals['deps'] + totals['incremental']):.2f} GiB "
          f"(deps {gib(totals['deps']):.2f} GiB, incremental {gib(totals['incremental']):.2f} GiB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
