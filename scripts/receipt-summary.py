"""Ruling D-42: a committed receipt is a summary. Turns a run's output folder into one `SUMMARY.json` and, with
`--delete`, removes the raw files from the folder (they stay outside the repository, or in git history at `--raw-at`).

Usage: python scripts/receipt-summary.py <folder> --command "<the command that regenerates it>"
           [--keep NAME ...] [--raw-at COMMIT] [--delete]

The summary holds:
- `verdict` (`pass` when the run's `manifest.json` lists no failures) and the manifest itself;
- `checks`: every check of the run's `transcript.json` with its verdict (a check's detail is kept only when short, and
  not at all when the run has rows, which carry it);
- `rows`: the verdict per row of a `rows.json` (each row's name, verdict and short fields; a row's clicks reduced to
  what was expected, what was observed, the effect and the undo steps);
- `regenerate`: the command; `rawAt`: where the raw files can be read again (a commit), when given;
- `raw`: each raw file's path, size and SHA-256.

`--keep` names files kept beside the summary (figures that are already a summary, such as `measurements.json`). The
summary must stay under 200 KB (D-42's limit for one receipt file); the script refuses otherwise. Every deletion is a
file the script listed inside the folder. Standard library only.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

LIMIT = 200 * 1024
SHORT = 300


def short(v) -> bool:
    return len(json.dumps(v, ensure_ascii=False)) <= SHORT


def row_summary(r) -> dict:
    if not isinstance(r, dict):
        return {"value": r if short(r) else "(long)"}
    out = {}
    for k, v in r.items():
        if k == "clicks" and isinstance(v, list):
            out["clicks"] = [{x: c.get(x) for x in ("input", "expected", "declared", "sent", "effect", "steps", "shown", "ok")
                              if x in c and short(c.get(x))} for c in v if isinstance(c, dict)]
        elif k == "disabled" and isinstance(v, dict):
            out["disabled"] = {x: v.get(x) for x in ("target", "ok", "note", "nativeEnabled", "treeEnabled", "enabled", "unchanged")
                               if x in v and short(v.get(x))}
        elif short(v):
            out[k] = v
    return out


def summarise(folder: Path, command: str, keep: set, raw_at: str | None) -> tuple[dict, list[Path]]:
    files = sorted(p for p in folder.rglob("*") if p.is_file() and p.name != "SUMMARY.json")
    manifest, checks, rows = None, None, None
    for p in files:
        name = p.relative_to(folder).as_posix()
        try:
            if p.name == "manifest.json" and manifest is None:
                manifest = json.loads(p.read_text(encoding="utf-8"))
            elif p.name == "transcript.json" and checks is None:
                t = json.loads(p.read_text(encoding="utf-8"))
                if isinstance(t, list):
                    checks = [{"ok": c.get("ok"), "check": c.get("check"), **({"detail": c.get("detail")} if short(c.get("detail")) else {})}
                              for c in t if isinstance(c, dict)]
            elif p.name == "rows.json" and rows is None:
                r = json.loads(p.read_text(encoding="utf-8"))
                if isinstance(r, list):
                    rows = [row_summary(x) for x in r]
        except (ValueError, UnicodeDecodeError):
            print(f"{name}: not JSON, kept as a raw file only", file=sys.stderr)
    if rows is not None and checks is not None:
        # The rows carry the detail; the checks keep their verdicts only.
        checks = [{"ok": c["ok"], "check": c["check"]} for c in checks]
    raw, removable = [], []
    for p in files:
        name = p.relative_to(folder).as_posix()
        if name in keep:
            continue
        raw.append({"file": name, "bytes": p.stat().st_size, "sha256": hashlib.sha256(p.read_bytes()).hexdigest()})
        removable.append(p)
    failures = (manifest or {}).get("failures")
    summary = {
        "receipt": folder.as_posix(),
        "verdict": ("pass" if failures == [] else "fail") if manifest is not None and failures is not None else "see checks",
        "regenerate": command,
        **({"rawAt": raw_at} if raw_at else {}),
        "manifest": manifest,
        **({"checks": checks} if checks is not None else {}),
        **({"rows": rows} if rows is not None else {}),
        "kept": sorted(keep),
        "raw": raw,
    }
    return summary, removable


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("folder")
    ap.add_argument("--command", required=True)
    ap.add_argument("--keep", nargs="*", default=[])
    ap.add_argument("--raw-at")
    ap.add_argument("--delete", action="store_true")
    a = ap.parse_args()
    folder = Path(a.folder)
    if not folder.is_dir():
        print(f"no folder {folder}", file=sys.stderr)
        return 2
    summary, removable = summarise(folder, a.command, set(a.keep), a.raw_at)
    text = json.dumps(summary, indent=1, ensure_ascii=False) + "\n"
    if len(text.encode("utf-8")) > LIMIT:
        print(f"the summary is {len(text.encode('utf-8'))} bytes, over D-42's {LIMIT}: shorten it", file=sys.stderr)
        return 1
    (folder / "SUMMARY.json").write_text(text, encoding="utf-8", newline="\n")
    if a.delete:
        for p in removable:
            p.unlink()
        for d in sorted((d for d in folder.rglob("*") if d.is_dir()), key=lambda d: len(d.parts), reverse=True):
            if not any(d.iterdir()):
                d.rmdir()
    print(f"{folder}: SUMMARY.json {len(text.encode('utf-8'))} bytes; {len(removable)} raw files"
          f"{' removed' if a.delete else ' (kept: --delete removes them)'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
