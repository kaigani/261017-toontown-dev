"""Analysis of a C0.2 tablet-spike log (`toontown --spike tablet --spike-log <file>`).

Usage: python scripts/analyse-tablet-log.py <log.json | log.json.gz> [--json <out.json>]

Everything is recomputed from the raw samples and packets in the log; summaries stored in the log are ignored
(review C02-R1-01). Accepted log versions: 1 (or no `version` field) and 2; any other version is rejected.

Prints, as Markdown that can be pasted into a report:
* environment and run metadata;
* per contact episode (stroke) on each path: samples, duration, rate, interval median/max, intervals longer than twice
  the stroke median (a heuristic, not a loss count), distinct pressure values and the step-implied level count, tilt,
  twist, and for the web path coalescing and handler latency under three estimators;
* loss only against a reference: injected frames (synthetic runs) or host sequence numbers (native channel);
  otherwise "unknown, gap heuristic only";
* every synthetic run (version 2): frames seen per layer and which ones are missing, and the listener matrix of the
  native-window probe (C02-R1-04).

Episodes: web = pointerdown .. pointerup/pointercancel per pointer; native = `down` (or the first pose with pressure)
.. `up`, proximity-out, or a pose whose pressure returns to zero. Native intervals use one clock: the device clock if
every contact packet has one, else host wall time. Percentiles are nearest-rank, as in desktop/src/lib/latency.ts.
A stroke below 200 Hz is reported as "below target", not as a failure. Standard library only.
"""

import gzip
import json
import math
import sys

SUPPORTED_VERSIONS = (1, 2)
TARGET_HZ = 200
NO_REFERENCE = "unknown, gap heuristic only"


class LogError(Exception):
    pass


def load(path):
    opener = gzip.open if path.endswith(".gz") else open
    with opener(path, "rt", encoding="utf-8") as f:
        return json.load(f)


def log_version(log):
    version = log.get("version", 1)
    if version not in SUPPORTED_VERSIONS:
        raise LogError(f"unsupported log version {version!r} (this analyzer reads versions {SUPPORTED_VERSIONS})")
    return version


def pct(values, p):
    """Nearest-rank percentile (p in (0, 100]) — the same definition as `percentile` in desktop/src/lib/latency.ts."""
    if not values:
        return None
    s = sorted(values)
    rank = max(1, math.ceil(p / 100 * len(s)))
    return s[min(rank, len(s)) - 1]


def stats(values):
    if not values:
        return None
    return {"n": len(values), "min": min(values), "p50": pct(values, 50), "p95": pct(values, 95),
            "p99": pct(values, 99), "max": max(values), "mean": sum(values) / len(values)}


def pressure_levels(values):
    distinct = sorted({v for v in values if v and v > 0})
    steps = [b - a for a, b in zip(distinct, distinct[1:]) if b > a]
    step = min(steps) if steps else None
    return len(distinct), (round(1 / step) if step else None)


def fmt(v, digits=1):
    if v is None:
        return "-"
    if isinstance(v, float):
        return f"{v:.{digits}f}"
    return str(v)


def ranges(indices):
    """[1,2,3,7] -> '1-3, 7' (compact list of frame indices)."""
    out, start, prev = [], None, None
    for i in sorted(indices):
        if start is None:
            start = prev = i
        elif i == prev + 1:
            prev = i
        else:
            out.append(f"{start}" if start == prev else f"{start}-{prev}")
            start = prev = i
    if start is not None:
        out.append(f"{start}" if start == prev else f"{start}-{prev}")
    return ", ".join(out) if out else "none"


# ---- episodes -----------------------------------------------------------------------------------------------------

def web_episodes(samples):
    """List of (pointer, [samples], truncated). A move with pressure while no episode is open (its pointerdown fell
    outside the capture) opens a truncated one."""
    open_, done = {}, []
    for s in samples:
        key = s.get("pointerType", "?") + (f"#{s['pointerId']}" if s.get("pointerId") is not None else "")
        kind = s.get("kind")
        cur = open_.get(key)
        if kind == "down":
            if cur:
                done.append((key, cur["points"], True))
            open_[key] = {"points": [s], "truncated": False}
        elif kind == "move":
            if cur is not None:
                cur["points"].append(s)
            elif (s.get("pressure") or 0) > 0:
                open_[key] = {"points": [s], "truncated": True}
        elif cur is not None:
            done.append((key, cur["points"], cur["truncated"]))
            del open_[key]
    done.extend((key, cur["points"], True) for key, cur in open_.items())
    return done


def is_contact(p):
    return p.get("kind") in ("pose", "down") and (p.get("pressure") or 0) > 0


def native_clock(packets):
    contact = [p for p in packets if is_contact(p)]
    return "device" if contact and all(p.get("deviceMs") is not None for p in contact) else "host"


def native_time(p, clock):
    return p.get("deviceMs") if clock == "device" else p.get("hostEpochMs")


def native_episodes(packets):
    done, cur, first = [], None, True
    for p in packets:
        kind = p.get("kind")
        if kind == "down":
            if cur:
                done.append(cur)
            cur = {"pointer": p.get("tool"), "points": [p] if is_contact(p) else [], "truncated": False}
        elif kind == "pose":
            if is_contact(p):
                if cur is None:
                    cur = {"pointer": p.get("tool"), "points": [], "truncated": first}
                cur["points"].append(p)
            elif cur is not None:
                done.append(cur)
                cur = None
        elif kind in ("up", "out"):
            if cur:
                done.append(cur)
            cur = None
        first = False
    if cur:
        cur["truncated"] = True
        done.append(cur)
    return [e for e in done if e["points"]]


def stroke_stats(times, pressures, tilts, twists):
    ts = sorted(times)
    iv = [b - a for a, b in zip(ts, ts[1:])]
    med = pct(iv, 50)
    dur = sum(iv)
    distinct, levels = pressure_levels(pressures)
    rate = (len(iv) * 1000 / dur) if dur > 0 else None
    return {
        "samples": len(ts), "durationMs": dur, "rateHz": rate, "medianIntervalMs": med,
        "maxIntervalMs": max(iv) if iv else None,
        "gapsOver2xMedian": sum(1 for d in iv if med and d > 2 * med),
        "pressureDistinct": distinct, "pressureLevelsImplied": levels,
        "tiltPresent": any((x or 0) != 0 or (y or 0) != 0 for x, y in tilts),
        "twistNonzero": sum(1 for t in twists if t),
        "target": None if rate is None else ("meets target" if rate >= TARGET_HZ else "below target"),
        "_intervals": iv,
    }


def web_analysis(log):
    samples = log.get("web", {}).get("samples", [])
    strokes = []
    episodes = web_episodes(samples)
    for pointer, points, truncated in episodes:
        st = stroke_stats([p["t"] for p in points], [p.get("pressure") for p in points],
                          [(p.get("tiltX"), p.get("tiltY")) for p in points], [p.get("twist") for p in points])
        st.update(pointer=pointer, truncated=truncated, _points=points)
        strokes.append(st)
    # Handler latency per dispatch, three estimators (C02-R1-01).
    by_dispatch = {}
    for s in samples:
        key = ("m", s.get("dispatch")) if s.get("kind") == "move" else (s.get("kind"), s.get("t"), s.get("dispatch"))
        by_dispatch.setdefault(key, []).append(s)
    oldest, newest, every, counts = [], [], [], []
    for key, group in by_dispatch.items():
        lo = min(group, key=lambda s: s["t"])
        hi = max(group, key=lambda s: s["t"])
        oldest.append(lo["handlerT"] - lo["t"])
        newest.append(hi["handlerT"] - hi["t"])
        every.extend(s["handlerT"] - s["t"] for s in group)
        if key[0] == "m":
            counts.append(group[0].get("coalescedCount", len(group)))
    frames = log.get("web", {}).get("frames", [])
    return {
        "samples": len(samples),
        "strokes": strokes,
        "coalescedApi": log.get("environment", {}).get("coalescedApi"),
        "coalescedObserved": any((s.get("coalescedCount") or 1) > 1 for s in samples),
        "coalescedPerDispatch": stats(counts),
        "samplesPerFrame": stats([f["samples"] for f in frames if f.get("samples", 0) > 0]),
        "handlerLatencyMs": {"oldestCoalesced": stats(oldest), "newestCoalesced": stats(newest),
                             "allSamples": stats(every)},
        "loss": NO_REFERENCE,
        "truncated": log.get("web", {}).get("truncated"),
    }


def native_analysis(log):
    native = log.get("native", {})
    packets = native.get("packets", [])
    clock = native_clock(packets)
    strokes = []
    for e in native_episodes(packets):
        pts = e["points"]
        st = stroke_stats([native_time(p, clock) for p in pts], [p.get("pressure") for p in pts],
                          [(p.get("tiltX"), p.get("tiltY")) for p in pts], [p.get("twist") for p in pts])
        st.update(pointer=e["pointer"], truncated=e["truncated"])
        strokes.append(st)
    seqs = [p.get("seq") for p in packets]
    if packets and all(s is not None for s in seqs):
        unique = set(seqs)
        expected = max(unique) - min(unique) + 1
        loss = {"reference": "host sequence numbers (host-to-page channel loss only)", "expected": expected,
                "received": len(unique), "missing": expected - len(unique)}
    else:
        loss = NO_REFERENCE
    return {
        "packets": len(packets), "clock": clock, "strokes": strokes, "loss": loss,
        "hostDropped": native.get("hostDropped"), "sendFailures": native.get("sendFailures"),
        "invalidBatches": native.get("invalidBatches", log.get("summary", {}).get("invalidNativeBatches")),
        "truncated": native.get("truncated"),
        "info": log.get("nativeInfo", log.get("summary", {}).get("nativeInfo")),
    }


# ---- synthetic runs -----------------------------------------------------------------------------------------------

def pressure_scale(values):
    """Normalised pressures are raw / scale. Web pointer events use 1024 for the synthetic pen; octotablet's Ink
    backend divides by the device range, which is 1025 for the synthetic device. Inferred from the smallest step
    between distinct values when that lies near 1024, else 1024."""
    distinct = sorted({v for v in values if v is not None and v > 0})
    steps = [b - a for a, b in zip(distinct, distinct[1:]) if b - a > 1e-9]
    if not steps:
        return 1024
    scale = round(1 / min(steps))
    return scale if 1000 <= scale <= 1100 else 1024


def frames_by_pressure(values, frames, scale=1024):
    """{k: first index into values} for pressure values identifying frame k-1 (k = round(p * scale))."""
    seen = {}
    for i, p in enumerate(values):
        if p is None or p <= 0:
            continue
        k = round(p * scale)
        if 1 <= k <= frames and k not in seen:
            seen[k] = i
    return seen


def match(report, values, times=None, scale=1024):
    frames = min(report.get("frames", 0), 1024)
    seen = frames_by_pressure(values, frames, scale)
    missing = [k - 1 for k in range(1, frames + 1) if k not in seen]
    out = {"injected": frames, "received": len(seen), "missing": len(missing), "missingFrames": missing,
           "pressureScale": scale}
    inject = report.get("injectEpochMs", [])
    if times is not None:
        lat = [times[i] - inject[k - 1] for k, i in seen.items() if k - 1 < len(inject) and times[i] is not None]
        out["latencyMs"] = stats(lat)
    sx, sy = report.get("screenX"), report.get("screenY")
    if sx and sy:
        repeated = [i for i in range(1, min(frames, len(sx))) if sx[i] == sx[i - 1] and sy[i] == sy[i - 1]]
        out["repeatedPixelFrames"] = len(repeated)
        out["missingAmongRepeatedPixel"] = sum(1 for i in repeated if i in set(missing))
    return out


def raw_pressure(raw, report):
    """Locates the pressure field in raw HID reports: the 16-bit little-endian offset whose values cover the most
    injected pressure values (each identifies one contact frame). Returns frames found that way, or None."""
    reports = [bytes.fromhex(h) for h in (raw or {}).get("reports", [])]
    if not reports or not report:
        return None
    frames = min(report.get("frames", 0), 1024)
    wanted = set(range(1, frames + 1))
    best = None
    for off in range(0, min(len(r) for r in reports) - 1):
        values = {int.from_bytes(r[off:off + 2], "little") for r in reports}
        hit = len(values & wanted)
        if best is None or hit > best[1]:
            best = (off, hit)
    if best is None or best[1] < frames // 2:
        return {"offset": None, "received": 0, "note": "no 16-bit field matches the injected pressures"}
    off = best[0]
    seen = {int.from_bytes(r[off:off + 2], "little") for r in reports} & wanted
    missing = [k - 1 for k in range(1, frames + 1) if k not in seen]
    return {"offset": off, "injected": frames, "received": len(seen), "missing": len(missing), "missingFrames": missing}


def run_analysis(log):
    samples = log.get("web", {}).get("samples", [])
    packets = log.get("native", {}).get("packets", [])
    origin = log.get("environment", {}).get("timeOrigin")
    results = []
    for run in log.get("runs", []):
        entry = {"label": run.get("label"), "target": run.get("target"), "error": run.get("error"),
                 "request": run.get("request")}
        report = run.get("report")
        probe = run.get("probe")
        if run.get("target") == "nativeWindow" and probe:
            report = probe.get("report")
            entry["error"] = entry["error"] or (probe.get("error") or {}).get("message")
            listeners = probe.get("listeners") or {}
            wm = listeners.get("wmPointer") or {}
            contact = [e for e in wm.get("entries", []) if e.get("inContact")]
            entry["wmPointer"] = None if report is None else match(report, [e["pressure"] / 1024 for e in contact])
            entry["wmPointerCounts"] = {k: wm.get(k) for k in ("down", "update", "up", "historyEntries",
                                                                "historyErrors", "truncated")}
            rts = listeners.get("rts")
            if rts is None:
                entry["rts"] = {"attached": False}
            else:
                entry["rts"] = {"attached": True, "poses": rts.get("poses"), "error": rts.get("error"),
                                "tablets": rts.get("tablets"),
                                "frames": None if report is None else match(
                                    report, rts.get("pressures", []), scale=pressure_scale(rts.get("pressures", [])))}
            entry["rawInput"] = listeners.get("rawInput")
        elif report:
            w0, w1 = run.get("webRange", [0, 0])
            n0, n1 = run.get("nativeRange", [0, 0])
            web = [s for s in samples[w0:w1] if s.get("kind") != "up"]
            times = [(origin + s["t"]) if origin is not None else None for s in web]
            entry["web"] = match(report, [s.get("pressure") for s in web], times)
            if origin is not None:
                handler = [origin + s["handlerT"] for s in web]
                entry["web"]["handlerLatencyMs"] = match(report, [s.get("pressure") for s in web], handler)["latencyMs"]
            poses = [p for p in packets[n0:n1] if p.get("kind") == "pose"]
            pressures = [p.get("pressure") for p in poses]
            entry["rts"] = {"poses": len(poses), "frames": match(report, pressures, [p.get("hostEpochMs") for p in poses],
                                                                  scale=pressure_scale(pressures))}
            raw = run.get("rawInput") or {}
            entry["rawInput"] = (raw.get("listeners") or {}).get("rawInput") if "listeners" in raw else raw or None
        if report and entry.get("rawInput"):
            entry["rawPressure"] = raw_pressure(entry["rawInput"], report)
        if report:
            entry["achievedHz"] = report.get("achievedHz")
            entry["pattern"] = report.get("pattern")
            ret = report.get("injectReturnEpochMs")
            if ret:
                entry["injectCallMs"] = stats([b - a for a, b in zip(report["injectEpochMs"], ret)])
        results.append(entry)
    return results


def v1_simulation_strokes(web):
    """Version-1 logs carry no raw injection report. Strokes whose pressures look like the standard synthetic stroke
    (k / 1024 for k = 1..1024) are matched by pressure alone, and labelled as such."""
    out = []
    for i, st in enumerate(web["strokes"]):
        seen = frames_by_pressure([p.get("pressure") for p in st["_points"]], 1024)
        if len(seen) > 512:
            out.append({"stroke": i, "received": len(seen),
                        "missingFrames": [k - 1 for k in range(1, 1025) if k not in seen]})
    return out


# ---- output -------------------------------------------------------------------------------------------------------

def print_strokes(title, strokes):
    print(f"\n### {title}\n")
    if not strokes:
        print("No contact episodes.")
        return
    print("| stroke | pointer | samples | ms | Hz | vs 200 Hz | median / max interval ms | >2x median | "
          "pressure levels (distinct / implied) | tilt | twist |")
    print("|---|---|---|---|---|---|---|---|---|---|---|")
    for i, s in enumerate(strokes):
        print(f"| {i + 1}{'*' if s['truncated'] else ''} | {s['pointer']} | {s['samples']} | {fmt(s['durationMs'], 0)} | "
              f"{fmt(s['rateHz'])} | {s['target'] or '-'} | {fmt(s['medianIntervalMs'], 2)} / {fmt(s['maxIntervalMs'], 1)} | "
              f"{s['gapsOver2xMedian']} | {s['pressureDistinct']} / {fmt(s['pressureLevelsImplied'])} | "
              f"{'yes' if s['tiltPresent'] else 'no'} | {s['twistNonzero']} nonzero |")
    print("\n`*` = episode cut off at the start or end of the capture. `>2x median` is a heuristic, not a loss count.")


def lat_text(s):
    return "-" if not s else f"p50 {s['p50']:.2f} / p95 {s['p95']:.2f} / max {s['max']:.1f}"


def print_runs(runs):
    if not runs:
        return
    print("\n### Synthetic runs (InjectSyntheticPointerInput; simulation, not tablet hardware)\n")
    print("| run | pattern | achieved Hz | web events: received / missing | WM_POINTER (history) | "
          "RTS: poses, frames received | raw HID reports (all frames incl. hover) | inject call ms (p50) | error |")
    print("|---|---|---|---|---|---|---|---|---|")
    for r in runs:
        web = r.get("web")
        wm = r.get("wmPointer")
        rts = r.get("rts") or {}
        rf = rts.get("frames") or {}
        raw = r.get("rawInput") or {}
        web_cell = f"{web['received']} / {web['missing']}" if web else "n/a (native window)"
        if r.get("target") == "nativeWindow":
            wm_cell = f"{wm['received']} / {wm['missing']} missing" if wm else "-"
        else:
            wm_cell = "not observable (WebView2 input window is in another process)"
        if rts.get("attached") is False:
            rts_cell = "not attached"
        else:
            rts_cell = f"{fmt(rts.get('poses'))}, {fmt(rf.get('received'))}" + (f" ({rts['error']})" if rts.get("error") else "")
        raw_cell = fmt(raw.get("hidReports")) if raw.get("registered", True) else f"not registered ({raw.get('error')})"
        rp = r.get("rawPressure")
        if rp and rp.get("offset") is not None:
            raw_cell += f"; frames by pressure {rp['received']} / {rp['missing']} missing"
        if raw.get("error") and not raw.get("hidReports"):
            raw_cell = f"- ({raw.get('error')})"
        call = (r.get("injectCallMs") or {}).get("p50")
        print(f"| {r['label']} | {r.get('pattern') or '-'} | {fmt(r.get('achievedHz'))} | {web_cell} | {wm_cell} | "
              f"{rts_cell} | {raw_cell} | {fmt(call, 3)} | {r.get('error') or ''} |")
    print()
    for r in runs:
        parts = []
        if r.get("web"):
            w = r["web"]
            parts.append(f"web missing frames: {ranges(w['missingFrames'])}")
            if "repeatedPixelFrames" in w:
                parts.append(f"repeated-pixel frames {w['repeatedPixelFrames']}, of them missing "
                             f"{w['missingAmongRepeatedPixel']}")
            parts.append(f"event - injection ms {lat_text(w.get('latencyMs'))}; "
                         f"handler - injection ms {lat_text(w.get('handlerLatencyMs'))}")
        rp = r.get("rawPressure")
        if rp and rp.get("offset") is not None and rp["missing"]:
            parts.append(f"raw HID missing frames: {ranges(rp['missingFrames'])}")
        raw = r.get("rawInput") or {}
        if raw.get("deviceNames"):
            parts.append(f"raw devices: {raw['deviceNames']}")
        if r.get("wmPointer"):
            w = r["wmPointer"]
            parts.append(f"WM_POINTER missing frames: {ranges(w['missingFrames'])}; counts {r['wmPointerCounts']}")
            rf = (r.get("rts") or {}).get("frames")
            if rf:
                union = set(range(w["injected"])) - (set(w["missingFrames"]) & set(rf["missingFrames"]))
                parts.append(f"RTS missing frames (pressure scale {rf['pressureScale']}): {ranges(rf['missingFrames'])}; "
                             f"WM_POINTER or RTS together: {len(union)} / {w['injected']}")
        if parts:
            print(f"- **{r['label']}**: " + "; ".join(parts))


def print_report(log, version, web, native, runs):
    env = log.get("environment", {})
    meta = log.get("metadata") or {}
    print(f"## Tablet spike log (version {version})\n")
    print(f"- userAgent: {env.get('userAgent')}")
    host = env.get("host") or {}
    if host:
        print(f"- host: {host.get('os')} {host.get('arch')}, {host.get('osVersion')}, webview {host.get('webviewVersion')}, "
              f"CPUs {host.get('cpuCount')}")
    if env.get("gpu"):
        print(f"- GPU (navigator.gpu): {env['gpu']}")
    print(f"devicePixelRatio {env.get('devicePixelRatio')}, getCoalescedEvents available: {env.get('coalescedApi')}")
    for key in ("computer", "osVersion", "webkit", "tablet", "driver", "tester", "notes"):
        if meta.get(key):
            print(f"- {key}: {meta[key]}")
    print(f"native: {native['info']}, packets {native['packets']}, invalid batches {native['invalidBatches']}")
    print(f"- native clock used: {native['clock']}; channel loss: {native['loss']}; host dropped "
          f"{fmt(native['hostDropped'])}, send failures {fmt(native['sendFailures'])}, truncated {fmt(native['truncated'])}")
    print(f"- web samples {web['samples']}, truncated {fmt(web['truncated'])}; web loss: {web['loss']}")
    print(f"- coalescing: API available {web['coalescedApi']}, batching observed {web['coalescedObserved']}, "
          f"per dispatch {lat_text(web['coalescedPerDispatch'])}")
    hl = web["handlerLatencyMs"]
    print(f"- handler - event ms: oldest coalesced {lat_text(hl['oldestCoalesced'])}; newest {lat_text(hl['newestCoalesced'])}; "
          f"all samples {lat_text(hl['allSamples'])}")
    print(f"web path: {len(web['strokes'])} pen-down strokes")
    print(f"native path: {len(native['strokes'])} strokes")
    print_strokes("Web path strokes (pointer events)", web["strokes"])
    print_strokes(f"Native path strokes (host listener, {native['clock']} clock)", native["strokes"])
    print_runs(runs)
    if version == 1:
        sims = v1_simulation_strokes(web)
        if sims:
            print("\n### Version-1 log: synthetic strokes matched by pressure alone (no raw injection report)\n")
            for s in sims:
                print(f"- web stroke {s['stroke'] + 1}: {s['received']}/1024 frames, missing {ranges(s['missingFrames'])}")


def strip(value):
    if isinstance(value, dict):
        return {k: strip(v) for k, v in value.items() if not k.startswith("_")}
    if isinstance(value, list):
        return [strip(v) for v in value]
    return value


def main(argv):
    args = argv[1:]
    out_json = None
    if "--json" in args:
        i = args.index("--json")
        if i + 1 >= len(args):
            print(__doc__)
            return 2
        out_json = args[i + 1]
        del args[i:i + 2]
    if len(args) != 1:
        print(__doc__)
        return 2
    log = load(args[0])
    try:
        version = log_version(log)
    except LogError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    web = web_analysis(log)
    native = native_analysis(log)
    runs = run_analysis(log) if version >= 2 else []
    print_report(log, version, web, native, runs)
    if out_json:
        with open(out_json, "w", encoding="utf-8") as f:
            json.dump(strip({"version": version, "web": web, "native": native, "runs": runs}), f, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
