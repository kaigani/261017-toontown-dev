"""Analysis of a C0.3 canvas-spike log (`toontown --spike canvas --spike-log <file> [--spike-auto]`).

Usage: python scripts/analyse-canvas-log.py <log.json | log.json.gz> [--json <out.json>] [--snapshots <dir>]

Every number is recomputed from the raw samples in the log; the page's own readout is not stored and not used.
The log is validated before anything is printed; malformed or missing evidence exits with status 2:
* `spike` must be `canvas` and `version` 1; `source` must be `auto`, `interactive` or `manual`;
* an `auto` or `interactive` log needs runs; a `manual` log must not contain runs;
* every run that does not carry an explicit `error` needs all its raw records: one transfer sample per requested
  transfer (positive byte counts, non-negative times, integer encode/generate ns), a decode record, an upload record,
  monotonic idle and pan/zoom timestamps covering the requested durations, and per-frame CPU times;
* an explicitly failed run is analysed and labelled FAILED.

Estimators (also printed):
* transfer = performance.now() around one binary `canvasScene` invoke (request, host encode, IPC, envelope check);
  throughput = total bytes / total transfer time;
* display refresh = mean rAF interval of the idle phase (nothing drawn), (last - first) / (n - 1); the median would
  be biased by WebKit's 1 ms timestamp rounding (a 60 Hz median reads 17 ms);
* frame interval = difference of consecutive rAF timestamps during pan/zoom; fps = (n - 1) * 1000 / (last - first);
* long frame = interval > 1.5 x display refresh;
* 60 fps target met when fps >= 57 (0.95 x 60); the display rate is printed next to it (a display below 60 Hz
  cannot meet it);
* CPU frame = JS update + record + submit; GPU frame (WebGPU) = submit to `onSubmittedWorkDone` (includes queueing);
* IPC-per-frame ceiling = 1000 / (p50 transfer ms): the frame rate if the full render list had to cross IPC every
  frame (no decode or draw included).
Percentiles are nearest-rank (as desktop/src/lib/latency.ts). Standard library only.
"""

import base64
import gzip
import json
import math
import os
import sys

VERSION = 1
SOURCES = ("auto", "interactive", "manual")
BACKENDS = ("webgpu", "webgl2")
SCENES = ("paths100k", "groups1k", "brush10k")


class LogError(Exception):
    pass


def load(path):
    opener = gzip.open if path.endswith(".gz") else open
    with opener(path, "rt", encoding="utf-8") as f:
        return json.load(f)


def pct(values, p):
    if not values:
        return None
    s = sorted(values)
    return s[min(max(1, math.ceil(p / 100 * len(s))), len(s)) - 1]


def stats(values):
    if not values:
        return None
    return {"n": len(values), "p50": pct(values, 50), "p95": pct(values, 95), "max": max(values),
            "mean": sum(values) / len(values)}


def num(v):
    return isinstance(v, (int, float)) and not isinstance(v, bool) and math.isfinite(v)


def integer(v):
    return isinstance(v, int) and not isinstance(v, bool)


def monotonic(ts):
    return all(num(t) for t in ts) and all(b >= a for a, b in zip(ts, ts[1:]))


def intervals(ts):
    return [b - a for a, b in zip(ts, ts[1:])]


def fmt(v, d=1):
    return "-" if v is None else f"{v:.{d}f}"


def validate(log):
    if log.get("spike") != "canvas":
        raise LogError(f"not a canvas-spike log (spike {log.get('spike')!r})")
    if log.get("version") != VERSION:
        raise LogError(f"unsupported canvas log version {log.get('version')!r} (this analyzer reads {VERSION})")
    source = log.get("source")
    if source not in SOURCES:
        raise LogError(f"unknown or missing source {source!r} (expected one of {SOURCES})")
    env = log.get("environment")
    if not isinstance(env, dict) or not num(env.get("timeOrigin")):
        raise LogError("environment.timeOrigin is missing")
    params = log.get("parameters")
    if not isinstance(params, dict):
        raise LogError("parameters are missing")
    for k in ("transfers", "idleMs", "panZoomMs", "pixelTransfers"):
        if not integer(params.get(k)) or params[k] < 1:
            raise LogError(f"parameters.{k} is not a positive integer")
    runs = log.get("runs")
    pixels = log.get("pixelRuns")
    if not isinstance(runs, list) or not isinstance(pixels, list):
        raise LogError("runs / pixelRuns are missing")
    if source == "manual" and (runs or pixels):
        raise LogError("log declares source 'manual' but contains runs")
    if source != "manual" and not runs:
        raise LogError(f"log source {source!r} has no runs")
    for i, run in enumerate(runs):
        label = f"run {i} ({run.get('backend')}/{run.get('scene')})"
        if run.get("backend") not in BACKENDS or run.get("scene") not in SCENES:
            raise LogError(f"{label}: unknown backend or scene")
        if run.get("error"):
            continue
        tr = run.get("transfers")
        if not isinstance(tr, list) or len(tr) != params["transfers"]:
            raise LogError(f"{label}: expected {params['transfers']} transfer samples")
        for s in tr:
            if not (integer(s.get("bytes")) and s["bytes"] > 0 and num(s.get("invokeMs")) and s["invokeMs"] >= 0
                    and integer(s.get("encodeNs")) and integer(s.get("generateNs"))):
                raise LogError(f"{label}: malformed transfer sample {s}")
        dec, up = run.get("decode"), run.get("upload")
        if not (isinstance(dec, dict) and num(dec.get("ms")) and integer(dec.get("paths"))):
            raise LogError(f"{label}: decode record missing")
        if not (isinstance(up, dict) and num(up.get("cpuMs")) and num(up.get("completeMs"))):
            raise LogError(f"{label}: upload record missing")
        idle = run.get("idle")
        if not isinstance(idle, list) or len(idle) < 2 or not monotonic(idle) or idle[-1] - idle[0] < params["idleMs"]:
            raise LogError(f"{label}: idle timestamps missing, non-monotonic or shorter than {params['idleMs']} ms")
        frames = run.get("frames")
        if not isinstance(frames, list) or len(frames) < 2:
            raise LogError(f"{label}: pan/zoom frames missing")
        ts = [f.get("t") for f in frames]
        if not monotonic(ts) or ts[-1] - ts[0] < params["panZoomMs"] - 100:
            raise LogError(f"{label}: frame timestamps non-monotonic or shorter than {params['panZoomMs']} ms")
        for f in frames:
            if not num(f.get("cpuMs")) or not (f.get("gpuMs") is None or num(f.get("gpuMs"))):
                raise LogError(f"{label}: malformed frame sample {f}")
        canvas = run.get("canvas")
        if not isinstance(canvas, dict) or not (integer(canvas.get("width")) and integer(canvas.get("height"))):
            raise LogError(f"{label}: canvas geometry missing")
    for i, p in enumerate(pixels):
        if p.get("error"):
            continue
        s = p.get("samples")
        if not isinstance(s, list) or len(s) != params["pixelTransfers"]:
            raise LogError(f"pixel run {i}: expected {params['pixelTransfers']} samples")
        for x in s:
            if not (integer(x.get("bytes")) and x["bytes"] == 16 + 4 * p["width"] * p["height"]
                    and num(x.get("invokeMs")) and num(x.get("uploadMs"))):
                raise LogError(f"pixel run {i}: malformed sample {x}")


def analyse_run(run):
    if run.get("error"):
        return {"backend": run["backend"], "scene": run["scene"], "outcome": "failed", "error": run["error"]}
    tr = run["transfers"]
    tms = [s["invokeMs"] for s in tr]
    total_bytes = sum(s["bytes"] for s in tr)
    it = run["idle"]
    ts = [f["t"] for f in run["frames"]]
    iv = intervals(ts)
    fps = (len(ts) - 1) * 1000 / (ts[-1] - ts[0])
    refresh = (it[-1] - it[0]) / (len(it) - 1) if len(it) > 1 and it[-1] > it[0] else None
    gpu = [f["gpuMs"] for f in run["frames"] if f.get("gpuMs") is not None]
    transfer = stats(tms)
    info = run.get("renderer") or {}
    return {
        "backend": run["backend"], "scene": run["scene"], "outcome": "ok", "error": None,
        "gpu": info.get("unmaskedRenderer") or info.get("renderer") or " ".join(
            str(info.get(k)) for k in ("vendor", "architecture", "device", "description") if info.get(k)) or "-",
        "canvas": run["canvas"],
        "bytesPerTransfer": total_bytes / len(tr),
        "transferMs": transfer,
        "transferMBps": total_bytes / 1e6 / (sum(tms) / 1000) if sum(tms) > 0 else None,
        "encodeMs": stats([s["encodeNs"] / 1e6 for s in tr]),
        "generateMs": max(s["generateNs"] for s in tr) / 1e6,
        "ipcPerFrameCeilingFps": 1000 / transfer["p50"] if transfer and transfer["p50"] > 0 else None,
        "decodeMs": run["decode"]["ms"],
        "decode": {k: v for k, v in run["decode"].items() if k != "ms"},
        "uploadMs": run["upload"],
        "refreshMs": refresh,
        "frames": len(ts),
        "intervalMs": stats(iv),
        "fps": fps,
        "longFrames": sum(1 for d in iv if refresh and d > 1.5 * refresh),
        "cpuMs": stats([f["cpuMs"] for f in run["frames"]]),
        "gpuMs": stats(gpu),
        "meetsTarget": fps >= 57,
    }


def analyse_pixels(p):
    if p.get("error"):
        return {"backend": p["backend"], "outcome": "failed", "error": p["error"]}
    s = p["samples"]
    inv = [x["invokeMs"] for x in s]
    up = [x["uploadMs"] for x in s]
    total = sum(x["bytes"] for x in s)
    return {"backend": p["backend"], "outcome": "ok", "width": p["width"], "height": p["height"],
            "bytes": s[0]["bytes"], "transferMs": stats(inv), "uploadMs": stats(up),
            "transferMBps": total / 1e6 / (sum(inv) / 1000) if sum(inv) > 0 else None,
            "frameCostP50Ms": (pct(inv, 50) or 0) + (pct(up, 50) or 0)}


def s3(st, d=2):
    return "-" if not st else f"{st['p50']:.{d}f} / {st['p95']:.{d}f} / {st['max']:.{d}f}"


def report(log, runs, pixels):
    env = log["environment"]
    host = env.get("host") or {}
    print("## Canvas spike log (version 1)\n")
    print(f"- source: {log['source']}; userAgent: {env.get('userAgent')}")
    print(f"- host: {host.get('os')} {host.get('arch')}, {host.get('osVersion')}, webview {host.get('webviewVersion')}, "
          f"CPUs {host.get('cpuCount')}; DPR {env.get('devicePixelRatio')}, viewport {env.get('viewport')}")
    print(f"- WebGPU API present: {env.get('webgpuApi')}; forced renderer: {env.get('forcedRenderer')}")
    print(f"- parameters: {log['parameters']}")
    print("\n### Runs (p50 / p95 / max)\n")
    print("| back end | scene | GPU | canvas px | MB / transfer | transfer ms | MB/s | encode ms | decode ms | "
          "upload ms (cpu / complete) | refresh ms | fps | interval ms | long frames | CPU frame ms | GPU frame ms | "
          "60 fps target | IPC-per-frame ceiling fps |")
    print("|" + "---|" * 18)
    for r in runs:
        if r["outcome"] != "ok":
            print(f"| {r['backend']} | {r['scene']} | **FAILED**: {r['error']} |" + " |" * 15)
            continue
        c = r["canvas"]
        print(f"| {r['backend']} | {r['scene']} | {r['gpu']} | {c['width']}x{c['height']} | "
              f"{r['bytesPerTransfer'] / 1e6:.2f} | {s3(r['transferMs'], 1)} | {fmt(r['transferMBps'], 0)} | "
              f"{s3(r['encodeMs'])} | {r['decodeMs']:.1f} | {r['uploadMs']['cpuMs']:.1f} / {r['uploadMs']['completeMs']:.1f} | "
              f"{fmt(r['refreshMs'], 2)} | {r['fps']:.1f} | {s3(r['intervalMs'])} | {r['longFrames']} of {r['frames'] - 1} | "
              f"{s3(r['cpuMs'])} | {s3(r['gpuMs'])} | {'met' if r['meetsTarget'] else 'missed'} | "
              f"{fmt(r['ipcPerFrameCeilingFps'])} |")
    print("\nDecode details (segments = stroke instances; triangles = fill triangles):\n")
    for r in runs:
        if r["outcome"] == "ok":
            d = r["decode"]
            print(f"- {r['backend']} {r['scene']}: paths {d.get('paths')}, cubics {d.get('cubics')}, segments "
                  f"{d.get('segments')}, fill vertices {d.get('fillVertices')}, triangles {d.get('triangles')}, "
                  f"GPU bytes {(d.get('strokeBytes', 0) + d.get('fillBytes', 0)) / 1e6:.1f} MB, tolerance {d.get('tolerance')}")
    print("\n### Pixel-frame transfer (host-raster fallback, transfer + texture upload only)\n")
    print("| back end | size | MB / frame | transfer ms | upload ms | MB/s | transfer + upload p50 ms |")
    print("|---|---|---|---|---|---|---|")
    for p in pixels:
        if p["outcome"] != "ok":
            print(f"| {p['backend']} | **FAILED**: {p['error']} | | | | | |")
            continue
        print(f"| {p['backend']} | {p['width']}x{p['height']} | {p['bytes'] / 1e6:.2f} | {s3(p['transferMs'], 1)} | "
              f"{s3(p['uploadMs'], 1)} | {fmt(p['transferMBps'], 0)} | {p['frameCostP50Ms']:.1f} |")


def main(argv):
    args = argv[1:]
    out_json = snap_dir = None
    for flag in ("--json", "--snapshots"):
        if flag in args:
            i = args.index(flag)
            if i + 1 >= len(args):
                print(__doc__)
                return 2
            if flag == "--json":
                out_json = args[i + 1]
            else:
                snap_dir = args[i + 1]
            del args[i:i + 2]
    if len(args) != 1:
        print(__doc__)
        return 2
    log = load(args[0])
    try:
        validate(log)
    except LogError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    runs = [analyse_run(r) for r in log["runs"]]
    pixels = [analyse_pixels(p) for p in log["pixelRuns"]]
    report(log, runs, pixels)
    if snap_dir:
        os.makedirs(snap_dir, exist_ok=True)
        for r in log["runs"]:
            s = r.get("snapshot")
            for mime, ext in (("image/png", "png"), ("image/jpeg", "jpg")):
                if isinstance(s, str) and s.startswith(f"data:{mime};base64,"):
                    with open(os.path.join(snap_dir, f"{r['backend']}-{r['scene']}.{ext}"), "wb") as f:
                        f.write(base64.b64decode(s.split(",", 1)[1]))
    if out_json:
        with open(out_json, "w", encoding="utf-8") as f:
            json.dump({"version": VERSION, "runs": runs, "pixelRuns": pixels}, f, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
