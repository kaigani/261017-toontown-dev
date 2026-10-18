"""Analysis of a C0.3 canvas-spike log (`toontown --spike canvas --spike-log <file> [--spike-auto]`).

Usage: python scripts/analyse-canvas-log.py <log.json | log.json.gz> [--json <out.json>] [--snapshots <dir>]

Every number is recomputed from the raw samples in the log; the page's own readout is not stored and not used.
The log is validated completely before anything is printed; malformed, incomplete or contradictory evidence exits
with status 2 and prints nothing on stdout (C03-R1-01):
* `spike` must be `canvas` and `version` 1; `source` must be `auto`, `interactive` or `manual`, and agree with
  `environment.mode.auto`;
* the environment must be complete: time origin, user agent, device pixel ratio, viewport, screen, WebGPU API flag,
  forced renderer (consistent with the launch mode), launch mode, and host facts (or the host error);
* parameters must declare the scenes, seed, flattening tolerance, transfer/pixel counts and durations;
* an `auto` log must contain the full matrix: one run per expected back end (the forced one, else WebGPU when its API
  exists, and WebGL2) and declared scene, in order, and one pixel run per back end; an explicitly failed run (with
  its error) counts as an observation, a missing one does not. A `manual` log must contain no runs;
* every successful run must match the declared workload: transfers numbered 0..n-1 in order, every transfer's byte
  count equal to the scene's encoded size (from its logged header counts, which must match the scene, or from the
  workload table for seed 1), decode statistics equal to the workload (paths, cubics, stroke/fill paths, and at
  tolerance 0.5 segments, fill vertices and triangles), non-negative durations, upload completion not before its CPU
  part, idle and pan/zoom timestamps monotonic and covering the declared durations, a CPU time for every frame,
  a GPU completion time for every WebGPU frame and none for WebGL2, renderer facts, and canvas geometry consistent
  with the device pixel ratio;
* pixel runs must have one sample per declared transfer, numbered in order, with the byte count of their size.

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
# Workload of the TTRL v1 scenes for seed 1 (desktop/src-tauri/src/spikes/canvas/renderlist.rs): encoded bytes, header
# counts and the decoder's statistics; segment/vertex/triangle counts hold for the flattening tolerance 0.5.
WORKLOAD = {
    "paths100k": {"bytes": 8_700_096, "groups": 1, "paths": 100_000, "cubics": 200_000, "strokePaths": 100_000,
                  "fillPaths": 0, "segments": 2_832_713, "fillVertices": 0, "triangles": 0},
    "groups1k": {"bytes": 2_024_072, "groups": 1000, "paths": 20_000, "cubics": 50_000, "strokePaths": 10_000,
                 "fillPaths": 10_000, "segments": 101_961, "fillVertices": 175_915, "triangles": 145_915},
    "brush10k": {"bytes": 10_085_716, "groups": 1, "paths": 10_000, "cubics": 0, "strokePaths": 0,
                 "fillPaths": 10_000, "segments": 0, "fillVertices": 1_088_402, "triangles": 1_062_652},
}
TABLE_SEED = 1
TABLE_TOLERANCE = 0.5


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


def nonneg(v):
    return num(v) and v >= 0


def encoded_len(h):
    return 72 + 24 * h["groups"] + 28 * h["paths"] + 8 * h["points"] + math.ceil(h["verbs"] / 4) * 4


def validate_environment(log, source):
    env = log.get("environment")
    if not isinstance(env, dict) or not num(env.get("timeOrigin")):
        raise LogError("environment.timeOrigin is missing")
    if not isinstance(env.get("userAgent"), str) or not env["userAgent"]:
        raise LogError("environment.userAgent is missing")
    if not num(env.get("devicePixelRatio")) or env["devicePixelRatio"] <= 0:
        raise LogError("environment.devicePixelRatio is missing")
    for key in ("viewport", "screen"):
        v = env.get(key)
        if not isinstance(v, dict) or not all(num(v.get(k)) and v[k] > 0 for k in ("width", "height")):
            raise LogError(f"environment.{key} is missing")
    if not isinstance(env.get("webgpuApi"), bool):
        raise LogError("environment.webgpuApi is missing")
    forced = env.get("forcedRenderer", "absent")
    if forced not in (None, "webgpu", "webgl2"):
        raise LogError(f"environment.forcedRenderer {forced!r} is missing or unknown")
    mode = env.get("mode")
    if not isinstance(mode, dict) or mode.get("name") != "canvas" or not isinstance(mode.get("auto"), bool):
        raise LogError("environment.mode (the canvas launch mode) is missing")
    if mode.get("renderer") != forced:
        raise LogError(f"forced renderer {forced!r} disagrees with the launch mode {mode.get('renderer')!r}")
    if mode["auto"] != (source == "auto"):
        raise LogError(f"source {source!r} disagrees with launch mode auto={mode['auto']}")
    host = env.get("host")
    if not (isinstance(host, dict) and isinstance(host.get("os"), str) and isinstance(host.get("arch"), str)):
        if not (isinstance(env.get("hostError"), str) and env["hostError"]):
            raise LogError("environment.host is missing (and no host error is recorded)")
    return env


def validate_parameters(log):
    params = log.get("parameters")
    if not isinstance(params, dict):
        raise LogError("parameters are missing")
    for k in ("transfers", "idleMs", "panZoomMs", "pixelTransfers"):
        if not integer(params.get(k)) or params[k] < 1:
            raise LogError(f"parameters.{k} is not a positive integer")
    scenes = params.get("scenes")
    if not isinstance(scenes, list) or not scenes or any(x not in SCENES for x in scenes) or len(set(scenes)) != len(scenes):
        raise LogError(f"parameters.scenes {scenes!r} is not a list of distinct known scenes")
    if not integer(params.get("seed")):
        raise LogError("parameters.seed is missing")
    if not num(params.get("flattenTolerance")) or params["flattenTolerance"] <= 0:
        raise LogError("parameters.flattenTolerance is missing")
    return params


def expected_backends(env):
    if env["forcedRenderer"]:
        return [env["forcedRenderer"]]
    return ["webgpu", "webgl2"] if env["webgpuApi"] else ["webgl2"]


def validate_run(run, label, params):
    tr = run.get("transfers")
    if not isinstance(tr, list) or len(tr) != params["transfers"]:
        raise LogError(f"{label}: expected {params['transfers']} transfer samples")
    want = WORKLOAD[run["scene"]] if params["seed"] == TABLE_SEED else None
    for k, x in enumerate(tr):
        if not (isinstance(x, dict) and x.get("i") == k and x.get("frame") == k):
            raise LogError(f"{label}: transfer {k} is not numbered {k} (i/frame {x.get('i')!r}/{x.get('frame')!r})")
        if not (integer(x.get("bytes")) and nonneg(x.get("invokeMs")) and integer(x.get("encodeNs"))
                and x["encodeNs"] >= 0 and integer(x.get("generateNs")) and x["generateNs"] >= 0):
            raise LogError(f"{label}: malformed transfer sample {k}")
        header = x.get("header")
        if header is not None:
            if not (isinstance(header, dict) and all(integer(header.get(f)) and header[f] >= 0
                                                     for f in ("groups", "paths", "points", "verbs"))):
                raise LogError(f"{label}: transfer {k} header counts malformed")
            if header.get("kind") != run["scene"] or header.get("seed") != params["seed"] or header.get("frame") != k:
                raise LogError(f"{label}: transfer {k} header describes another scene, seed or frame")
            if x["bytes"] != encoded_len(header):
                raise LogError(f"{label}: transfer {k} is {x['bytes']} bytes; its header counts imply {encoded_len(header)}")
            if want and (header["paths"] != want["paths"] or header["groups"] != want["groups"]):
                raise LogError(f"{label}: transfer {k} header counts differ from the {run['scene']} workload")
        elif want is None:
            raise LogError(f"{label}: no header counts and no workload table for seed {params['seed']}")
        if want and x["bytes"] != want["bytes"]:
            raise LogError(f"{label}: transfer {k} is {x['bytes']} bytes; {run['scene']} encodes to {want['bytes']}")
    if len({x["bytes"] for x in tr}) != 1:
        raise LogError(f"{label}: transfers of one scene differ in size")
    dec = run.get("decode")
    if not (isinstance(dec, dict) and nonneg(dec.get("ms"))
            and all(integer(dec.get(k)) and dec[k] >= 0 for k in ("paths", "cubics", "strokePaths", "fillPaths",
                                                                    "segments", "fillVertices", "triangles"))):
        raise LogError(f"{label}: decode record missing or malformed")
    if dec.get("tolerance") != params["flattenTolerance"]:
        raise LogError(f"{label}: decode tolerance {dec.get('tolerance')!r} differs from the declared parameter")
    if dec["strokePaths"] + dec["fillPaths"] != dec["paths"]:
        raise LogError(f"{label}: decode stroke + fill paths do not add up to its paths")
    if want:
        keys = ["paths", "cubics", "strokePaths", "fillPaths"]
        if dec["tolerance"] == TABLE_TOLERANCE:
            keys += ["segments", "fillVertices", "triangles"]
        for k in keys:
            if dec[k] != want[k]:
                raise LogError(f"{label}: decode {k} = {dec[k]} contradicts the {run['scene']} workload ({want[k]})")
    up = run.get("upload")
    if not (isinstance(up, dict) and nonneg(up.get("cpuMs")) and nonneg(up.get("completeMs"))
            and up["completeMs"] + 1e-6 >= up["cpuMs"]):
        raise LogError(f"{label}: upload record missing, negative or completed before its CPU part")
    idle = run.get("idle")
    if not isinstance(idle, list) or len(idle) < 2 or not monotonic(idle) or idle[-1] - idle[0] < params["idleMs"]:
        raise LogError(f"{label}: idle timestamps missing, non-monotonic or shorter than {params['idleMs']} ms")
    frames = run.get("frames")
    if not isinstance(frames, list) or len(frames) < 2:
        raise LogError(f"{label}: pan/zoom frames missing")
    ts = [f.get("t") if isinstance(f, dict) else None for f in frames]
    if not monotonic(ts) or ts[-1] - ts[0] < params["panZoomMs"] - 100:
        raise LogError(f"{label}: frame timestamps non-monotonic or shorter than {params['panZoomMs']} ms")
    if idle[-1] > ts[0]:
        raise LogError(f"{label}: the idle phase overlaps the pan/zoom phase")
    for k, f in enumerate(frames):
        if not nonneg(f.get("cpuMs")):
            raise LogError(f"{label}: frame {k} has no non-negative CPU time")
        gpu = f.get("gpuMs")
        if run["backend"] == "webgpu" and not nonneg(gpu):
            raise LogError(f"{label}: WebGPU frame {k} has no non-negative GPU completion time")
        if run["backend"] == "webgl2" and gpu is not None:
            raise LogError(f"{label}: WebGL2 frame {k} claims a GPU time the back end cannot measure")
    canvas = run.get("canvas")
    if not (isinstance(canvas, dict) and integer(canvas.get("width")) and integer(canvas.get("height"))
            and canvas["width"] >= 1 and canvas["height"] >= 1 and num(canvas.get("dpr")) and canvas["dpr"] > 0
            and num(canvas.get("cssWidth")) and num(canvas.get("cssHeight"))
            and abs(canvas["width"] - max(1, round(canvas["cssWidth"] * canvas["dpr"]))) <= 1
            and abs(canvas["height"] - max(1, round(canvas["cssHeight"] * canvas["dpr"]))) <= 1):
        raise LogError(f"{label}: canvas geometry missing or inconsistent with its device pixel ratio")
    if not isinstance(run.get("renderer"), dict) or not run["renderer"]:
        raise LogError(f"{label}: renderer facts missing")


def validate(log):
    if log.get("spike") != "canvas":
        raise LogError(f"not a canvas-spike log (spike {log.get('spike')!r})")
    if log.get("version") != VERSION:
        raise LogError(f"unsupported canvas log version {log.get('version')!r} (this analyzer reads {VERSION})")
    source = log.get("source")
    if source not in SOURCES:
        raise LogError(f"unknown or missing source {source!r} (expected one of {SOURCES})")
    env = validate_environment(log, source)
    params = validate_parameters(log)
    runs = log.get("runs")
    pixels = log.get("pixelRuns")
    if not isinstance(runs, list) or not isinstance(pixels, list):
        raise LogError("runs / pixelRuns are missing")
    if source == "manual" and (runs or pixels):
        raise LogError("log declares source 'manual' but contains runs")
    if source != "manual" and not runs:
        raise LogError(f"log source {source!r} has no runs")
    backends = expected_backends(env)
    for i, run in enumerate(runs):
        if not isinstance(run, dict) or run.get("backend") not in BACKENDS or run.get("scene") not in params["scenes"]:
            raise LogError(f"run {i}: unknown backend or a scene that is not declared")
        if run["backend"] not in backends:
            raise LogError(f"run {i}: back end {run['backend']} was not expected (forced {env['forcedRenderer']!r}, "
                           f"WebGPU API {env['webgpuApi']})")
    if source == "auto":
        expected = [(b, sc) for b in backends for sc in params["scenes"]]
        got = [(r["backend"], r["scene"]) for r in runs]
        if got != expected:
            raise LogError(f"automatic run matrix is incomplete or out of order: expected {expected}, got {got}")
        if [p.get("backend") if isinstance(p, dict) else None for p in pixels] != backends:
            raise LogError(f"automatic run needs one pixel run per back end {backends}")
    for i, run in enumerate(runs):
        label = f"run {i} ({run['backend']}/{run['scene']})"
        if "error" not in run:
            raise LogError(f"{label}: no error field")
        if run["error"] is not None:
            if not isinstance(run["error"], str) or not run["error"]:
                raise LogError(f"{label}: failed run without an error message")
            continue
        validate_run(run, label, params)
    for i, p in enumerate(pixels):
        if not isinstance(p, dict) or p.get("backend") not in backends:
            raise LogError(f"pixel run {i}: unknown or unexpected back end")
        if p.get("error"):
            continue
        if not (integer(p.get("width")) and integer(p.get("height")) and p["width"] >= 1 and p["height"] >= 1):
            raise LogError(f"pixel run {i}: size missing")
        samples = p.get("samples")
        if not isinstance(samples, list) or len(samples) != params["pixelTransfers"]:
            raise LogError(f"pixel run {i}: expected {params['pixelTransfers']} samples")
        for k, x in enumerate(samples):
            if not (isinstance(x, dict) and x.get("i") == k and integer(x.get("bytes"))
                    and x["bytes"] == 16 + 4 * p["width"] * p["height"] and nonneg(x.get("invokeMs"))
                    and nonneg(x.get("uploadMs")) and integer(x.get("fillNs")) and x["fillNs"] >= 0):
                raise LogError(f"pixel run {i}: malformed sample {k}")


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
