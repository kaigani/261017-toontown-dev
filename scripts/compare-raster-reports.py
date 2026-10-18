"""Compares C0.3 rasteriser reports (`c03_raster --report`) with the committed hashes.

Usage: python scripts/compare-raster-reports.py <hashes.json> <report.json> [<report.json> ...]

Prints, from the reports' raw data only:
* hash identity: for every rasteriser configuration and scene, whether every report's pixel and PNG hashes equal the
  committed ones (and therefore each other);
* per-scene median render time (ms) per rasteriser per report (the median of the report's `msAll` samples, recomputed);
* the `--canvas-bench` rows (vello_cpu on the canvas-spike scenes) when present.
Exits with status 1 if any hash differs, 2 on malformed input. Standard library only.
"""

import json
import math
import sys


def median(values):
    s = sorted(values)
    return s[len(s) // 2] if s else math.nan


def main(argv):
    if len(argv) < 3:
        print(__doc__)
        return 2
    with open(argv[1], encoding="utf-8") as f:
        committed = json.load(f)
    if committed.get("format") != "tt-c03-raster-hashes/1":
        print("error: not a tt-c03-raster-hashes/1 file", file=sys.stderr)
        return 2
    reports = []
    for path in argv[2:]:
        with open(path, encoding="utf-8") as f:
            r = json.load(f)
        if r.get("format") != "tt-c03-raster-report/1":
            print(f"error: {path} is not a tt-c03-raster-report/1 file", file=sys.stderr)
            return 2
        reports.append((path, r))
    base = committed["renderers"]
    differences = 0
    print("## Hash identity against the committed hashes\n")
    print("| rasteriser | scenes | " + " | ".join(f"{r['os']} {r['arch']} ({r['detectedSimdLevel'].split('(')[0]})"
                                            for _, r in reports) + " |")
    print("|---|---|" + "---|" * len(reports))
    for name in sorted(base):
        cells = []
        for _, r in reports:
            got = r["hashes"]["renderers"].get(name, {})
            same = sum(1 for s, h in base[name].items()
                       if got.get(s, {}).get("pixels") == h["pixels"] and got.get(s, {}).get("png") == h["png"])
            differences += len(base[name]) - same
            cells.append(f"{same}/{len(base[name])} identical")
        print(f"| {name} | {len(base[name])} | " + " | ".join(cells) + " |")
    for path, r in reports:
        if r.get("repeatFailures"):
            differences += len(r["repeatFailures"])
            print(f"\n{path}: repeats differ: {r['repeatFailures']}")
    print("\n## Median render time per scene (ms; recomputed from msAll)\n")
    names = sorted(base)
    for path, r in reports:
        print(f"\n### {r['os']} {r['arch']} - {path} (repeat {r['repeat']})\n")
        print("| scene | " + " | ".join(names) + " |")
        print("|---|" + "---|" * len(names))
        for s in r["scenes"]:
            row = [f"{median(r['timingsMs'][n][s['name']]['msAll']):.3f}" for n in names]
            print(f"| {s['name']} | " + " | ".join(row) + " |")
        bench = (r.get("canvasBench") or {}).get("rows") or []
        if bench:
            print("\ncanvas-spike scenes with vello_cpu (host-raster fallback), median ms:\n")
            print("| scene | size | renderer | ms |")
            print("|---|---|---|---|")
            for b in bench:
                print(f"| {b['scene']} | {b['width']}x{b['height']} | {b['renderer']} | {median(b['msAll']):.1f} |")
    print(f"\n{differences} hash differences")
    return 1 if differences else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
