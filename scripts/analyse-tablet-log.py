"""Analysis of a C0.2 tablet-spike log (`toontown --spike tablet --spike-log <file>`).

Usage: python scripts/analyse-tablet-log.py <log.json | log.json.gz> [--json <out.json>]

Everything is recomputed from the raw samples and packets in the log; summaries stored in the log are ignored
(review C02-R1-01), and so are aggregates inside raw host reports such as `achievedHz` (C02-R2-02): they are
recomputed and the log is rejected if the saved value disagrees.

Log versions (C02-R2-01):
* 3 (current) and 2 are accepted. Version 3 adds the injector's monotonic contact endpoints and focus snapshots.
* 1 is rejected (obsolete: no raw injection report).
* A log without a `version` field is accepted only if it contains no synthetic runs (an unversioned manual/hardware
  capture, e.g. the round-1 reviewer fixture); with synthetic runs it is rejected.

Every synthetic run is validated before anything is printed. There are three outcomes, stated in the output:
* (a) an explicitly failed run (it carries an error such as `spike.notForeground`): analysed and labelled FAILED;
* (b) no synthetic runs (manual hardware log): analysed; there is no injection reference, so loss is "unknown, gap
  heuristic only";
* (c) malformed or missing evidence (a successful run without its raw report, a report missing fields or with array
  lengths that differ from the injected frame count, no time origin, stream bounds outside the streams, a saved rate
  that disagrees with the recomputed one): exit status 2 with a message on stderr; nothing is printed as a result.

Prints, as Markdown that can be pasted into a report:
* environment and run metadata;
* per contact episode (stroke) on each path: samples, duration, rate, interval median/max, intervals longer than twice
  the stroke median (a heuristic, not a loss count), distinct pressure values and the step-implied level count, tilt
  extrema per axis (native: raw OS tilt as well as converted degrees), twist range, and for the web path coalescing
  and handler latency under three estimators;
* loss only against a reference: injected frames (synthetic runs) or host sequence numbers (native channel);
  otherwise "unknown, gap heuristic only";
* every synthetic run: rate estimators, geometry and varying axes, foreground/focus, frames seen per listener and which
  ones are missing, repeated-pixel frames, tilt and twist fidelity (exact / modulo 180 / absent / other).

Episodes: web = pointerdown .. pointerup/pointercancel per pointer; native = `down` (or the first pose with pressure)
.. `up`, proximity-out, or a pose whose pressure returns to zero. Native intervals use one clock: the device clock if
every contact packet has one, else host wall time. Percentiles are nearest-rank, as in desktop/src/lib/latency.ts.
A stroke below 200 Hz is reported as "below target", not as a failure. Standard library only.
"""

import gzip
import json
import math
import sys

SUPPORTED_VERSIONS = (2, 3)
TARGET_HZ = 200
NO_REFERENCE = "unknown, gap heuristic only"
# Saved `achievedHz` versus the recomputed host estimator (same formula, same inputs): float noise only.
HOST_RATE_TOLERANCE = 1e-9
# Version-2 logs lack the host's endpoints; the saved value is checked against the first-to-last estimator instead.
V2_RATE_TOLERANCE = 0.01
REPORT_ARRAYS = ("injectEpochMs", "injectReturnEpochMs", "pressure", "tiltX", "tiltY", "rotation", "screenX", "screenY")
REPORT_FIELDS = ("method", "frames", "hz", "achievedHz", "screenRect", "pattern", "target") + REPORT_ARRAYS
V3_REPORT_FIELDS = ("contactStartNs", "contactEndNs", "contactStartEpochMs", "contactEndEpochMs", "focus")


class LogError(Exception):
    pass


def load(path):
    opener = gzip.open if path.endswith(".gz") else open
    with opener(path, "rt", encoding="utf-8") as f:
        return json.load(f)


def log_version(log):
    """The log's version, or None for an unversioned log without synthetic runs. Raises LogError otherwise."""
    if "version" not in log:
        if log.get("runs") or log.get("simulations"):
            raise LogError("log has no `version` field but contains synthetic runs; this analyzer only accepts "
                           f"versioned synthetic evidence (versions {SUPPORTED_VERSIONS})")
        return None
    version = log["version"]
    if version == 1:
        raise LogError("log version 1 is obsolete (no raw injection report); re-run with the current build "
                       f"(this analyzer reads versions {SUPPORTED_VERSIONS})")
    if version not in SUPPORTED_VERSIONS:
        raise LogError(f"unsupported log version {version!r} (this analyzer reads versions {SUPPORTED_VERSIONS})")
    return version


def pct(values, p):
    """Nearest-rank percentile (p in (0, 100]) - the same definition as `percentile` in desktop/src/lib/latency.ts."""
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


def extent(values):
    """[min, max] of the non-null values, or None."""
    present = [v for v in values if v is not None]
    return [min(present), max(present)] if present else None


def fmt(v, digits=1):
    if v is None:
        return "-"
    if isinstance(v, float):
        return f"{v:.{digits}f}"
    return str(v)


def fmt_range(r, digits=0):
    return "-" if not r else f"{fmt(r[0], digits)}..{fmt(r[1], digits)}"


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


def is_number(v):
    return isinstance(v, (int, float)) and not isinstance(v, bool) and math.isfinite(v)


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


def stroke_stats(points, times):
    ts = sorted(times)
    iv = [b - a for a, b in zip(ts, ts[1:])]
    med = pct(iv, 50)
    dur = sum(iv)
    distinct, levels = pressure_levels([p.get("pressure") for p in points])
    rate = (len(iv) * 1000 / dur) if dur > 0 else None
    return {
        "samples": len(ts), "durationMs": dur, "rateHz": rate, "medianIntervalMs": med,
        "maxIntervalMs": max(iv) if iv else None,
        "gapsOver2xMedian": sum(1 for d in iv if med and d > 2 * med),
        "pressureDistinct": distinct, "pressureLevelsImplied": levels,
        "tiltPresent": any((p.get("tiltX") or 0) != 0 or (p.get("tiltY") or 0) != 0 for p in points),
        # Extrema per axis (C02-R2-04): converted degrees; for native packets also the raw OS value (macOS -1..1).
        "tiltRangeX": extent([p.get("tiltX") for p in points]),
        "tiltRangeY": extent([p.get("tiltY") for p in points]),
        "rawTiltRangeX": extent([p.get("rawTiltX") for p in points]),
        "rawTiltRangeY": extent([p.get("rawTiltY") for p in points]),
        "twistRange": extent([p.get("twist") for p in points]),
        "twistNonzero": sum(1 for p in points if p.get("twist")),
        "target": None if rate is None else ("meets target" if rate >= TARGET_HZ else "below target"),
        "_intervals": iv,
    }


def web_analysis(log):
    samples = log.get("web", {}).get("samples", [])
    strokes = []
    for pointer, points, truncated in web_episodes(samples):
        st = stroke_stats(points, [p["t"] for p in points])
        st.update(pointer=pointer, truncated=truncated)
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
        st = stroke_stats(pts, [native_time(p, clock) for p in pts])
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


# ---- validation of synthetic runs (C02-R2-01) ---------------------------------------------------------------------

def run_error(run):
    """The explicit failure of a run (outcome a), or None if the run claims success."""
    if run.get("error"):
        return str(run["error"])
    probe = run.get("probe") or {}
    err = probe.get("error")
    if err:
        return f"{err.get('code')}: {err.get('message')}" if isinstance(err, dict) else str(err)
    return None


def run_report(run):
    return (run.get("probe") or {}).get("report") if run.get("target") == "nativeWindow" else run.get("report")


def check_range(name, bounds, length, label):
    if (not isinstance(bounds, list) or len(bounds) != 2 or not all(isinstance(b, int) for b in bounds)
            or not 0 <= bounds[0] <= bounds[1] <= length):
        raise LogError(f"run {label!r}: {name} {bounds!r} is not a range inside the stream (length {length})")


def validate_report(report, label, version):
    if not isinstance(report, dict):
        raise LogError(f"run {label!r}: successful run has no raw report (missing evidence, not an empty result)")
    required = REPORT_FIELDS + (V3_REPORT_FIELDS if version == 3 else ())
    missing = [f for f in required if f not in report]
    if missing:
        raise LogError(f"run {label!r}: raw report is missing fields {missing}")
    frames = report["frames"]
    if not isinstance(frames, int) or isinstance(frames, bool) or frames < 1:
        raise LogError(f"run {label!r}: raw report frames {frames!r} is not a positive count")
    for name in REPORT_ARRAYS:
        values = report[name]
        if not isinstance(values, list) or len(values) != frames:
            got = len(values) if isinstance(values, list) else type(values).__name__
            raise LogError(f"run {label!r}: raw report {name} has {got} entries, expected {frames} (one per frame)")
        if not all(is_number(v) for v in values):
            raise LogError(f"run {label!r}: raw report {name} contains a non-numeric value")
    if not isinstance(report["screenRect"], list) or len(report["screenRect"]) != 4:
        raise LogError(f"run {label!r}: raw report screenRect must be [left, top, width, height]")
    if not is_number(report["achievedHz"]):
        raise LogError(f"run {label!r}: raw report achievedHz is not a number")
    expected = [1 + (i % 1024) for i in range(frames)]
    if report["pressure"] != expected:
        raise LogError(f"run {label!r}: injected pressures are not the identifying sequence 1 + (i mod 1024)")
    if version == 3:
        start, end = report["contactStartNs"], report["contactEndNs"]
        if not (isinstance(start, int) and isinstance(end, int) and end > start):
            raise LogError(f"run {label!r}: raw report contact endpoints {start!r}..{end!r} are not an interval")


def validate(log, version):
    """Raises LogError on malformed or missing evidence (outcome c); returns nothing otherwise."""
    runs = log.get("runs", [])
    if not isinstance(runs, list):
        raise LogError("`runs` is not a list")
    samples = log.get("web", {}).get("samples", [])
    packets = log.get("native", {}).get("packets", [])
    for i, run in enumerate(runs):
        label = run.get("label") or f"#{i}"
        target = run.get("target")
        if target not in ("webview", "nativeWindow"):
            raise LogError(f"run {label!r}: unknown target {target!r}")
        if run_error(run):
            continue  # outcome (a): explicitly failed, analysed without a report
        validate_report(run_report(run), label, version)
        if target == "webview":
            if not is_number(log.get("environment", {}).get("timeOrigin")):
                raise LogError(f"run {label!r}: webview evidence needs environment.timeOrigin")
            check_range("webRange", run.get("webRange"), len(samples), label)
            check_range("nativeRange", run.get("nativeRange"), len(packets), label)
        else:
            listeners = (run.get("probe") or {}).get("listeners")
            if not isinstance(listeners, dict) or not isinstance(listeners.get("wmPointer"), dict) \
                    or not isinstance(listeners.get("rawInput"), dict) or "rts" not in listeners:
                raise LogError(f"run {label!r}: native-window probe result lacks its listener records")
            if not isinstance(listeners["wmPointer"].get("entries"), list):
                raise LogError(f"run {label!r}: WM_POINTER record has no entries")


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


def angle_fidelity(observed, injected):
    """Counts of exact / modulo-180 / absent / other agreement between observed and injected angles (degrees)."""
    out = {"exact": 0, "mod180": 0, "absent": 0, "other": 0}
    for o, inj in zip(observed, injected):
        if o is None:
            out["absent"] += 1
        elif abs(o - inj) < 0.5:
            out["exact"] += 1
        elif abs(((o - inj) % 180 + 90) % 180 - 90) < 0.5:
            out["mod180"] += 1
        else:
            out["other"] += 1
    return out


def match(report, items, pressure, times=None, scale=1024, tilt=None, twist=None):
    """Matches items to injected frames by their identifying pressure. `pressure`, `times`, `tilt` and `twist` are
    accessors on one item."""
    frames = min(report["frames"], 1024)
    seen = frames_by_pressure([pressure(x) for x in items], frames, scale)
    missing = [k - 1 for k in range(1, frames + 1) if k not in seen]
    sx, sy = report["screenX"], report["screenY"]
    repeated = [i for i in range(1, frames) if sx[i] == sx[i - 1] and sy[i] == sy[i - 1]]
    missing_set = set(missing)
    out = {"injected": frames, "received": len(seen), "missing": len(missing), "missingFrames": missing,
           "pressureScale": scale, "repeatedPixelFrames": len(repeated), "repeatedPixelIndices": repeated,
           "missingAmongRepeatedPixel": sum(1 for i in repeated if i in missing_set)}
    inject = report["injectEpochMs"]
    first = {k - 1: items[i] for k, i in seen.items()}
    if times is not None:
        out["latencyMs"] = stats([times(x) - inject[f] for f, x in first.items() if times(x) is not None])
    if tilt is not None:
        out["tiltExact"] = sum(1 for f, x in first.items()
                               if tilt(x) == (report["tiltX"][f], report["tiltY"][f]))
    if twist is not None:
        out["twistFidelity"] = angle_fidelity([twist(x) for x in first.values()],
                                              [report["rotation"][f] for f in first])
    return out


def rate_estimators(report, label, version):
    """Recomputes the injector rate (C02-R2-02) and checks the saved aggregate against it."""
    frames = report["frames"]
    times = report["injectEpochMs"]
    first_to_last = (frames - 1) * 1000 / (times[-1] - times[0]) if frames > 1 and times[-1] > times[0] else None
    saved = report["achievedHz"]
    host = None
    if version == 3:
        host = frames * 1e9 / (report["contactEndNs"] - report["contactStartNs"])
        if abs(saved - host) > HOST_RATE_TOLERANCE * host:
            raise LogError(f"run {label!r}: saved achievedHz {saved} disagrees with the host estimator recomputed "
                           f"from contactStartNs/contactEndNs ({host}); tolerance {HOST_RATE_TOLERANCE} relative")
        check = f"saved value equals the recomputed host estimator (relative tolerance {HOST_RATE_TOLERANCE})"
    else:
        if first_to_last is None or abs(saved - first_to_last) > V2_RATE_TOLERANCE * first_to_last:
            raise LogError(f"run {label!r}: saved achievedHz {saved} is not within {V2_RATE_TOLERANCE:.0%} of the "
                           f"first-to-last estimator ({first_to_last}); version-2 logs cannot reproduce the host "
                           "estimator exactly")
        check = (f"version 2: host estimator not reproducible (no endpoints); saved value within "
                 f"{V2_RATE_TOLERANCE:.0%} of first-to-last")
    return {"achievedHz": host if host is not None else first_to_last, "hostEstimatorHz": host,
            "firstToLastHz": first_to_last, "savedAchievedHz": saved, "rateCheck": check}


def varying_axes(report):
    return {k: len(set(report[k])) for k in ("pressure", "tiltX", "tiltY", "rotation")} | {
        "position": len(set(zip(report["screenX"], report["screenY"])))}


def focus_summary(report):
    out = []
    for f in report.get("focus") or []:
        own = f.get("ownPid")
        out.append({"when": f.get("when"),
                    "foreground": "this process" if f.get("foregroundPid") == own else f"pid {f.get('foregroundPid')}",
                    "foregroundClass": f.get("foregroundClass"),
                    "foregroundIsTarget": f.get("foregroundHwnd") == f.get("targetHwnd"),
                    "focus": ("none" if not f.get("focusHwnd") else
                              "this process" if f.get("focusPid") == own else f"pid {f.get('focusPid')}"),
                    "focusClass": f.get("focusClass")})
    return out


def run_analysis(log):
    version = log_version(log)
    validate(log, version)
    samples = log.get("web", {}).get("samples", [])
    packets = log.get("native", {}).get("packets", [])
    origin = log.get("environment", {}).get("timeOrigin")
    results = []
    for i, run in enumerate(log.get("runs", [])):
        label = run.get("label") or f"#{i}"
        entry = {"label": label, "target": run.get("target"), "request": run.get("request")}
        error = run_error(run)
        if error:
            entry.update(outcome="failed", error=error)
            results.append(entry)
            continue
        report = run_report(run)
        entry.update(outcome="ok", error=None, pattern=report["pattern"], screenRect=report["screenRect"],
                     varyingAxes=varying_axes(report), focus=focus_summary(report))
        entry.update(rate_estimators(report, label, version))
        entry["injectCallMs"] = stats([b - a for a, b in zip(report["injectEpochMs"], report["injectReturnEpochMs"])])
        if run["target"] == "nativeWindow":
            listeners = run["probe"]["listeners"]
            wm = listeners["wmPointer"]
            contact = [e for e in wm["entries"] if e.get("inContact")]
            entry["wmPointer"] = match(report, contact, lambda e: e["pressure"] / 1024,
                                       tilt=lambda e: (e.get("tiltX"), e.get("tiltY")),
                                       twist=lambda e: e.get("rotation"))
            entry["wmPointerCounts"] = {k: wm.get(k) for k in ("down", "update", "up", "historyEntries",
                                                                "historyErrors", "truncated")}
            rts = listeners["rts"]
            if rts is None:
                entry["rts"] = {"attached": False}
            else:
                pressures = rts.get("pressures", [])
                entry["rts"] = {"attached": True, "poses": rts.get("poses"), "error": rts.get("error"),
                                "tablets": rts.get("tablets"),
                                "frames": match(report, pressures, lambda p: p, scale=pressure_scale(pressures))}
            entry["rawInput"] = listeners["rawInput"]
        else:
            w0, w1 = run["webRange"]
            n0, n1 = run["nativeRange"]
            web = [s for s in samples[w0:w1] if s.get("kind") != "up"]
            entry["web"] = match(report, web, lambda s: s.get("pressure"), times=lambda s: origin + s["t"],
                                 tilt=lambda s: (s.get("tiltX"), s.get("tiltY")), twist=lambda s: s.get("twist"))
            entry["web"]["handlerLatencyMs"] = match(report, web, lambda s: s.get("pressure"),
                                                     times=lambda s: origin + s["handlerT"])["latencyMs"]
            poses = [p for p in packets[n0:n1] if p.get("kind") == "pose"]
            pressures = [p.get("pressure") for p in poses]
            entry["rts"] = {"attached": True, "poses": len(poses),
                            "frames": match(report, poses, lambda p: p.get("pressure"),
                                            times=lambda p: p.get("hostEpochMs"), scale=pressure_scale(pressures))}
            raw = run.get("rawInput") or {}
            entry["rawInput"] = (raw.get("listeners") or {}).get("rawInput") if "listeners" in raw else (raw or None)
            menus = log.get("web", {}).get("contextMenus")
            p0, p1 = run.get("perfStart"), run.get("perfEnd")
            if isinstance(menus, list) and is_number(p0) and is_number(p1):
                # Pen press-and-hold surfaces as `contextmenu` (suppressed by the page); counted per run.
                entry["contextMenus"] = sum(1 for m in menus if p0 <= m.get("t", -1) <= p1)
        results.append(entry)
    return results


# ---- output -------------------------------------------------------------------------------------------------------

def print_strokes(title, strokes, native=False):
    print(f"\n### {title}\n")
    if not strokes:
        print("No contact episodes.")
        return
    raw_head = " | raw tilt x / y (OS units)" if native else ""
    print("| stroke | pointer | samples | ms | Hz | vs 200 Hz | median / max interval ms | >2x median | "
          f"pressure levels (distinct / implied) | tilt x / y (deg){raw_head} | twist range (nonzero) |")
    print("|---|---|---|---|---|---|---|---|---|---|" + ("---|" if native else "") + "---|")
    for i, s in enumerate(strokes):
        raw_cell = (f" | {fmt_range(s['rawTiltRangeX'], 3)} / {fmt_range(s['rawTiltRangeY'], 3)}" if native else "")
        print(f"| {i + 1}{'*' if s['truncated'] else ''} | {s['pointer']} | {s['samples']} | {fmt(s['durationMs'], 0)} | "
              f"{fmt(s['rateHz'])} | {s['target'] or '-'} | {fmt(s['medianIntervalMs'], 2)} / {fmt(s['maxIntervalMs'], 1)} | "
              f"{s['gapsOver2xMedian']} | {s['pressureDistinct']} / {fmt(s['pressureLevelsImplied'])} | "
              f"{fmt_range(s['tiltRangeX'])} / {fmt_range(s['tiltRangeY'])}{raw_cell} | "
              f"{fmt_range(s['twistRange'])} ({s['twistNonzero']}) |")
    print("\n`*` = episode cut off at the start or end of the capture. `>2x median` is a heuristic, not a loss count.")


def lat_text(s):
    return "-" if not s else f"p50 {s['p50']:.2f} / p95 {s['p95']:.2f} / max {s['max']:.1f}"


def fidelity_text(f):
    return "-" if not f else f"exact {f['exact']}, mod 180 {f['mod180']}, absent {f['absent']}, other {f['other']}"


def print_runs(runs):
    print("\n### Synthetic runs (InjectSyntheticPointerInput; simulation, not tablet hardware)\n")
    if not runs:
        print("No synthetic runs: outcome (b), a manual/hardware log. There is no injection reference, so loss is "
              f"\"{NO_REFERENCE}\".")
        return
    rects = {tuple(r["screenRect"]) for r in runs if r["outcome"] == "ok"}
    print(f"Geometry: {'all successful runs share' if len(rects) == 1 else 'runs DIFFER in'} screen rectangle "
          f"{' / '.join(str(list(r)) for r in sorted(rects)) or '-'} (physical px, [left, top, width, height]).\n")
    print("| run | outcome | pattern (varying axes) | Hz: host / first-to-last | web events: received / missing | "
          "WM_POINTER on target (history) | RTS: poses, frames | raw-input sink HID reports (not decoded) | "
          "inject call ms p50 |")
    print("|---|---|---|---|---|---|---|---|---|")
    for r in runs:
        if r["outcome"] == "failed":
            print(f"| {r['label']} | **FAILED**: {r['error']} | - | - | - | - | - | - | - |")
            continue
        web, wm = r.get("web"), r.get("wmPointer")
        rts = r.get("rts") or {}
        rf = rts.get("frames") or {}
        raw = r.get("rawInput") or {}
        axes = r["varyingAxes"]
        varying = ", ".join(k for k in ("position", "pressure", "tiltX", "tiltY", "rotation") if axes[k] > 1)
        web_cell = f"{web['received']} / {web['missing']}" if web else "n/a (native window)"
        if r["target"] == "nativeWindow":
            wm_cell = f"{wm['received']} / {wm['missing']} missing"
        else:
            wm_cell = "not measured (the webview's input window belongs to msedgewebview2.exe)"
        rts_cell = "not attached" if rts.get("attached") is False else (
            f"{fmt(rts.get('poses'))}, {fmt(rf.get('received'))}" + (f" ({rts['error']})" if rts.get("error") else ""))
        if raw.get("error") and not raw.get("hidReports"):
            raw_cell = f"- ({raw.get('error')})"
        else:
            raw_cell = fmt(raw.get("hidReports"))
        print(f"| {r['label']} | ok | {r['pattern']} ({varying}) | {fmt(r['hostEstimatorHz'], 3)} / "
              f"{fmt(r['firstToLastHz'], 3)} | {web_cell} | {wm_cell} | {rts_cell} | {raw_cell} | "
              f"{fmt((r.get('injectCallMs') or {}).get('p50'), 3)} |")
    print()
    for r in runs:
        if r["outcome"] != "ok":
            continue
        parts = [f"rate: saved {r['savedAchievedHz']}; {r['rateCheck']}"]
        for f in r["focus"]:
            parts.append(f"focus {f['when']}: foreground {f['foreground']} ({f['foregroundClass']}"
                         f"{', the target' if f['foregroundIsTarget'] else ''}), keyboard focus {f['focus']}"
                         f"{' (' + f['focusClass'] + ')' if f['focusClass'] else ''}")
        if not r["focus"]:
            parts.append("focus not recorded (log predates version 3)")
        if r.get("web"):
            w = r["web"]
            parts.append(f"web missing frames: {ranges(w['missingFrames'])}")
            parts.append(f"repeated-pixel frames {ranges(w['repeatedPixelIndices'])}, of them missing "
                         f"{w['missingAmongRepeatedPixel']}")
            parts.append(f"web tilt exact {w['tiltExact']} of {w['received']}; web twist vs injected rotation: "
                         f"{fidelity_text(w['twistFidelity'])}")
            parts.append(f"event - injection ms {lat_text(w.get('latencyMs'))}; "
                         f"handler - injection ms {lat_text(w.get('handlerLatencyMs'))}")
            if "contextMenus" in r:
                parts.append(f"contextmenu events (press-and-hold) during the run: {r['contextMenus']}")
        raw = r.get("rawInput") or {}
        if raw.get("deviceNames"):
            parts.append(f"raw devices: {raw['deviceNames']}")
        if r.get("wmPointer"):
            w = r["wmPointer"]
            parts.append(f"WM_POINTER missing frames: {ranges(w['missingFrames'])}; counts {r['wmPointerCounts']}; "
                         f"repeated-pixel frames {ranges(w['repeatedPixelIndices'])}; tilt exact {w['tiltExact']}; "
                         f"rotation: {fidelity_text(w['twistFidelity'])}")
            rf = (r.get("rts") or {}).get("frames")
            if rf:
                union = set(range(w["injected"])) - (set(w["missingFrames"]) & set(rf["missingFrames"]))
                parts.append(f"RTS missing frames (pressure scale {rf['pressureScale']}): {ranges(rf['missingFrames'])}; "
                             f"WM_POINTER or RTS together: {len(union)} / {w['injected']}")
        print(f"- **{r['label']}**: " + "; ".join(parts))


def print_report(log, version, web, native, runs):
    env = log.get("environment", {})
    meta = log.get("metadata") or {}
    print(f"## Tablet spike log ({'unversioned, no synthetic runs' if version is None else f'version {version}'})\n")
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
    failed = sum(1 for r in runs if r["outcome"] == "failed")
    print(f"- outcome: {len(runs)} synthetic runs, {len(runs) - failed} ok, {failed} explicitly failed"
          if runs else "- outcome: no synthetic runs (manual/hardware log; loss unknown)")
    print(f"web path: {len(web['strokes'])} pen-down strokes")
    print(f"native path: {len(native['strokes'])} strokes")
    print_strokes("Web path strokes (pointer events)", web["strokes"])
    print_strokes(f"Native path strokes (host listener, {native['clock']} clock)", native["strokes"], native=True)
    print_runs(runs)


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
        # Everything is validated and computed before anything is printed (outcome c prints no results).
        version = log_version(log)
        runs = run_analysis(log)
    except LogError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    web = web_analysis(log)
    native = native_analysis(log)
    print_report(log, version, web, native, runs)
    if out_json:
        with open(out_json, "w", encoding="utf-8") as f:
            json.dump(strip({"version": version, "web": web, "native": native, "runs": runs}), f, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
