"""Analysis of a C0.2 tablet-spike log (`toontown --spike tablet --spike-log <file>`).

Usage: python scripts/analyse-tablet-log.py <log.json | log.json.gz> [--json <out.json>]

Everything is recomputed from the raw samples and packets in the log; summaries stored in the log are ignored
(review C02-R1-01), and so are aggregates inside raw host reports such as `achievedHz` (C02-R2-02): they are
recomputed and the log is rejected if the saved value disagrees.

Log versions (C02-R2-01):
* 3 (current) and 2 are accepted. Version 3 adds the injector's monotonic contact endpoints and focus snapshots.
* 1 is rejected (obsolete: no raw injection report).
* A log without a `version` field is accepted only if it carries no synthetic artefact at all: no `runs` or
  `simulations` field (even empty), no `source`, and no `--spike-auto` request (the round-1 reviewer fixture). Any
  of these makes it malformed.

Every synthetic run is validated before anything is printed. There are three outcomes, stated in the output:
* (a) an explicitly failed run (it carries an error such as `spike.notForeground`): analysed and labelled FAILED;
* (b) a manual hardware log: analysed; there is no injection reference, so loss is "unknown, gap heuristic only".
  Allowed only when the log **declares** it (`source: "manual"`, written by the page when no `--spike-auto` run was
  requested and no synthetic stroke was injected) and contains no run or simulation entries (C02-R3-01);
* (c) malformed or missing evidence (a successful run without its raw report, a report missing fields or with array
  lengths that differ from the injected frame count, no time origin, stream bounds outside the streams, a saved rate
  that disagrees with the recomputed one, version-3 focus snapshots missing or incomplete, simulation entries
  without their run records, a `--spike-auto` or synthetic log without runs, a `manual` log with runs): exit status 2
  with a message on stderr; nothing is printed as a result.

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
A stroke below 200 Hz is reported as "below target", not as a failure.

Action markers (C04-R2-02): a manual log from the current page carries `actions`, one record per action of the
manual procedure (A1 pressure ramp, A2 speed, A3 tilt, A4 barrel rotation, A5 hover, A6 stationary pressure). The
person presses Space to start and to finish each action, and K to skip it; a marker is taken on the keyup of a
deliberate press, at least 500 ms after the previous one, and every Space/K event is logged in `keyEvents` (C04-R3-02).
Each record holds its state (`done`,
`skipped`, `unfinished`), and its start and end on the page's performance clock and as wall-clock epoch ms. An
episode belongs to the action whose span contains the episode's first sample: web samples by performance time,
native packets by host wall time. Episodes outside every span (clicks on Save, the metadata form or the listener
button) belong to no action. Per action and path the result is one of:
* omitted (skipped);
* unfinished (saved mid-action);
* performed, no events (done, but no episode on that path);
* episode n;
* split (several episodes; the one with most samples is the measurement and the count is reported).

Malformed action records are outcome (c).

Decision (C04-R2-01): for a manual log with action markers, the analyzer applies the tablet rule set of
docs/decisions/ADR-0009-tablet-input.md and prints the outcome:
* every capability row is classified per measured candidate. W = web path; the native path is H (RealTimeStylus) on
  Windows and A (AppKit monitor) on macOS. Raw HID (R) and a dedicated window (D) need experiments E-R1 / E-D1 and
  are not measured by this log;
* numeric bands are half-open, on the unrounded value;
* the candidate with the highest share of applicable rows at pass is selected (n/a rows excluded), ties by
  priority (W > R > H > D on Windows, W > A on macOS); a candidate with no applicable measured row is excluded;
* pressure (row 2) or rate (row 7) at fail on the selected candidate makes the outcome "no-go on this platform";
* every per-action statistic, including row 6's coalescing, uses only the measured episode's samples inside the
  action's span; samples outside every span are counted and excluded (C04-R3-01);
* the raw Space/K key events (`keyEvents`) are validated and must account for every action transition (C04-R3-02);
* each non-pass row of the selected candidate gets its fixed mitigation. Sources are never combined;
* the decision is also written to `--json` as `decision`. Standard library only.
"""

from fractions import Fraction
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
FOCUS_NUMBERS = ("ownPid", "targetHwnd", "foregroundHwnd", "foregroundPid", "focusHwnd", "focusPid", "activeHwnd",
                 "captureHwnd")
FOCUS_STRINGS = ("foregroundClass", "focusClass")
SOURCES = ("manual", "interactive", "auto")


class LogError(Exception):
    pass


def load(path):
    opener = gzip.open if path.endswith(".gz") else open
    with opener(path, "rt", encoding="utf-8") as f:
        return json.load(f)


def log_version(log):
    """The log's version, or None for an unversioned log without synthetic runs. Raises LogError otherwise."""
    if "version" not in log:
        artefacts = [k for k in ("runs", "simulations", "source") if k in log]
        if auto_requested(log):
            artefacts.append("environment.mode.auto")
        if artefacts:
            raise LogError(f"log has no `version` field but carries synthetic artefacts {artefacts}; this analyzer "
                           f"only accepts versioned synthetic evidence (versions {SUPPORTED_VERSIONS})")
        return None
    version = log["version"]
    if version == 1:
        raise LogError("log version 1 is obsolete (no raw injection report); re-run with the current build "
                       f"(this analyzer reads versions {SUPPORTED_VERSIONS})")
    if version not in SUPPORTED_VERSIONS:
        raise LogError(f"unsupported log version {version!r} (this analyzer reads versions {SUPPORTED_VERSIONS})")
    return version


def auto_requested(log):
    mode = (log.get("environment") or {}).get("mode") or {}
    return isinstance(mode, dict) and mode.get("auto") is True


def is_manual(log, version):
    """Outcome (b) is chosen only for a log that declares itself manual (C02-R3-01). Raises LogError when the
    log's synthetic artefacts and its run records disagree."""
    if version is None:
        return True  # unversioned: log_version already rejected every synthetic artefact
    runs = log.get("runs")
    sims = log.get("simulations")
    source = log.get("source")
    if source is not None and source not in SOURCES:
        raise LogError(f"unknown log source {source!r} (expected one of {SOURCES})")
    for name, value in (("runs", runs), ("simulations", sims)):
        if value is not None and not isinstance(value, list):
            raise LogError(f"`{name}` is not a list")
    if runs:
        if source == "manual":
            raise LogError("log declares source 'manual' but contains synthetic runs")
        return False
    # No run records from here on.
    if sims:
        raise LogError(f"log has {len(sims)} simulation entries but no run records; synthetic evidence without its "
                       "runs is missing evidence, not a manual capture")
    if auto_requested(log):
        raise LogError("log was saved by a --spike-auto run but has no run records")
    if source != "manual":
        raise LogError("log has no run records and does not declare `source: \"manual\"`; it cannot be treated as a "
                       f"manual/hardware capture (source {source!r})")
    return True


def check_simulations(log):
    """Every simulation entry must be the summary of an existing successful run with the identical raw report."""
    runs = log.get("runs") or []
    for i, sim in enumerate(log.get("simulations") or []):
        label = (sim or {}).get("label") if isinstance(sim, dict) else None
        report = (sim or {}).get("report") if isinstance(sim, dict) else None
        if not any(r.get("label") == label and r.get("report") == report and report is not None for r in runs):
            raise LogError(f"simulation entry {i} ({label!r}) has no run record with the same raw report")


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


def missing_runs(indices):
    """Runs of consecutive missing frame ids as [first, last, length], longest first (ties by first id)."""
    out, start, prev = [], None, None
    for i in sorted(indices):
        if start is None:
            start = prev = i
        elif i == prev + 1:
            prev = i
        else:
            out.append([start, prev, prev - start + 1])
            start = prev = i
    if start is not None:
        out.append([start, prev, prev - start + 1])
    return sorted(out, key=lambda r: (-r[2], r[0]))


def runs_text(runs, limit=5):
    if not runs:
        return "none"
    shown = [f"{a}-{b} ({n})" if n > 1 else f"{a} (1)" for a, b, n in runs[:limit]]
    return ", ".join(shown) + (f", ... {len(runs) - limit} more" if len(runs) > limit else "")


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
        st.update(pointer=pointer, truncated=truncated, startT=points[0].get("t"), _points=points)
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
        st.update(pointer=e["pointer"], truncated=e["truncated"], startEpochMs=pts[0].get("hostEpochMs"), _points=pts)
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


# ---- action markers (C04-R2-02) -----------------------------------------------------------------------------------

ACTION_IDS = ("A1", "A2", "A3", "A4", "A5", "A6")
ACTION_STATES = ("done", "skipped", "unfinished")
ACTION_TIMES = ("startPerf", "endPerf", "startEpochMs", "endEpochMs")


KEY_TYPES = ("keydown", "keyup")
KEY_NAMES = (" ", "k")


def path_outcome(state, indices, strokes, span, clock=None):
    """Outcome of one action on one path. `stats` are computed only from the measured episode's samples that lie
    inside the action's span (C04-R3-01): nothing outside the span reaches a decision input."""
    if state == "skipped":
        return {"outcome": "omitted", "episodes": [], "measured": None, "stats": None}
    if state == "unfinished":
        return {"outcome": "unfinished", "episodes": indices, "measured": None, "stats": None}
    if not indices:
        return {"outcome": "performed, no events", "episodes": [], "measured": None, "stats": None}
    measured = max(indices, key=lambda i: strokes[i - 1]["samples"])
    st = strokes[measured - 1]
    lo, hi = span
    if clock is None:  # web: page performance clock
        pts = [q for q in st["_points"] if is_number(q.get("t")) and lo <= q["t"] < hi]
        stats = stroke_stats(pts, [q["t"] for q in pts])
        stats["coalescedObserved"] = any((q.get("coalescedCount") or 1) > 1 for q in pts)
    else:  # native: assigned by host wall time; intervals on the path's own clock
        pts = [q for q in st["_points"] if is_number(q.get("hostEpochMs")) and lo <= q["hostEpochMs"] < hi]
        stats = stroke_stats(pts, [native_time(q, clock) for q in pts])
    stats.update(pointer=st["pointer"], truncated=st["truncated"])
    return {"outcome": f"episode {indices[0]}" if len(indices) == 1 else f"split: {len(indices)} episodes",
            "episodes": indices, "measured": measured, "stats": stats}


def validate_key_events(log, acts):
    """The page's raw Space/K log (C04-R3-02). Malformed, or disagreeing with the action records: outcome (c)."""
    events = log.get("keyEvents")
    if events is None:
        return None
    if not isinstance(events, list):
        raise LogError("`keyEvents` is not a list")
    prev, transitions = None, 0
    for k, e in enumerate(events):
        if not (isinstance(e, dict) and e.get("type") in KEY_TYPES and e.get("key") in KEY_NAMES
                and is_number(e.get("t")) and isinstance(e.get("repeat"), bool) and isinstance(e.get("outcome"), str)):
            raise LogError(f"key event {k} is malformed")
        if prev is not None and e["t"] < prev:
            raise LogError(f"key event {k} goes back in time")
        prev = e["t"]
        if e["outcome"].startswith("transition"):
            if e["type"] != "keyup" or e["repeat"]:
                raise LogError(f"key event {k} is a transition on a {e['type']}{' repeat' if e['repeat'] else ''}")
            transitions += 1
    expected = sum({"done": 2, "skipped": 1, "unfinished": 1}[a["state"]] for a in acts)
    if transitions != expected:
        raise LogError(f"the raw key events show {transitions} marker transitions but the action records need {expected}")
    return {"events": len(events), "transitions": transitions,
            "ignored": sum(1 for e in events if e["outcome"].startswith("ignored"))}


def action_analysis(log, web, native):
    """Maps action markers to contact episodes on each path; None when the log has no markers."""
    acts = log.get("actions")
    if acts is None:
        return None
    if not isinstance(acts, list):
        raise LogError("`actions` is not a list")
    seen, prev_end, prev_epoch_end, out = set(), None, None, []
    clock = native["clock"]
    for k, a in enumerate(acts):
        if not isinstance(a, dict) or a.get("id") not in ACTION_IDS or a.get("state") not in ACTION_STATES:
            raise LogError(f"action record {k} has no known id/state")
        if a["id"] in seen:
            raise LogError(f"action {a['id']} is recorded twice")
        if ACTION_IDS.index(a["id"]) != len(seen):
            raise LogError(f"action {a['id']} is out of order (expected {ACTION_IDS[len(seen)]})")
        if not all(is_number(a.get(t)) for t in ACTION_TIMES) or a["endPerf"] < a["startPerf"] \
                or a["endEpochMs"] < a["startEpochMs"]:
            raise LogError(f"action {a['id']} has missing or reversed start/end times")
        if prev_end is not None and a["startPerf"] < prev_end:
            raise LogError(f"action {a['id']} starts before the previous action ended")
        if prev_epoch_end is not None and a["startEpochMs"] < prev_epoch_end:
            raise LogError(f"action {a['id']} starts before the previous action ended on the native (epoch) clock")
        seen.add(a["id"])
        prev_end, prev_epoch_end = a["endPerf"], a["endEpochMs"]
        web_idx = [i + 1 for i, st in enumerate(web["strokes"])
                   if is_number(st.get("startT")) and a["startPerf"] <= st["startT"] < a["endPerf"]]
        nat_idx = [i + 1 for i, st in enumerate(native["strokes"])
                   if is_number(st.get("startEpochMs")) and a["startEpochMs"] <= st["startEpochMs"] < a["endEpochMs"]]
        out.append({"id": a["id"], "state": a["state"], "durationMs": a["endPerf"] - a["startPerf"],
                    "web": path_outcome(a["state"], web_idx, web["strokes"], (a["startPerf"], a["endPerf"])),
                    "native": path_outcome(a["state"], nat_idx, native["strokes"],
                                           (a["startEpochMs"], a["endEpochMs"]), clock)})
    keys = validate_key_events(log, acts)
    in_web = {i for r in out for i in r["web"]["episodes"]}
    in_nat = {i for r in out for i in r["native"]["episodes"]}
    spans = [(a["startPerf"], a["endPerf"], a["startEpochMs"], a["endEpochMs"]) for a in acts]
    web_samples = log.get("web", {}).get("samples", [])
    packets = log.get("native", {}).get("packets", [])
    web_out = sum(1 for q in web_samples if not any(is_number(q.get("t")) and lo <= q["t"] < hi
                                                     for lo, hi, _, _ in spans))
    nat_out = sum(1 for q in packets if not any(is_number(q.get("hostEpochMs")) and lo <= q["hostEpochMs"] < hi
                                                 for _, _, lo, hi in spans))
    return {"actions": out, "keyEvents": keys,
            "excludedSamples": {"web": web_out, "native": nat_out},
            "notRecorded": [i for i in ACTION_IDS if i not in seen],
            "webOutside": [i for i in range(1, len(web["strokes"]) + 1) if i not in in_web],
            "nativeOutside": [i for i in range(1, len(native["strokes"]) + 1) if i not in in_nat]}


# ---- ADR-0009 tablet rule set (C04-R2-01) ------------------------------------------------------------------------

INF = math.inf
ROWS = {1: "pen identity", 2: "pressure >= 256 levels", 3: "pressure >= 1024 levels", 4: "tilt",
        5: "twist (barrel rotation)", 6: "coalesced events", 7: "sustained rate >= 120 Hz", 8: "rate >= 200 Hz",
        9: "stationary pressure-only updates"}
ROW_ACTION = {1: "A1-A3", 2: "A1", 3: "A1", 4: "A3", 5: "A4", 6: "A2", 7: "A2", 8: "A2", 9: "A6"}
# Half-open bands [low, high) on the unrounded value, in the order pass, partial, fail.
BANDS = {
    2: ((256, INF, "pass"), (16, 256, "partial"), (0, 16, "fail")),
    3: ((1024, INF, "pass"), (256, 1024, "partial"), (0, 256, "fail")),
    7: ((120, INF, "pass"), (60, 120, "partial"), (0, 60, "fail")),
    8: ((200, INF, "pass"), (120, 200, "partial"), (0, 120, "fail")),
}
STATIONARY_PASS_HZ = 60
TILT_SPAN_DEG = 30
TWIST_PASS_RANGE_DEG = 180
PRIORITY = {"windows": ("W", "R", "H", "D"), "macos": ("W", "A")}
NO_GO_ROWS = (2, 7)
NATIVE_CANDIDATE = {"windows": "H", "macos": "A"}
UNMEASURED_NOTE = {"R": "raw HID, needs experiment E-R1", "D": "dedicated input window, needs experiment E-D1"}
# Mitigation for the selected candidate's non-pass rows: (partial, fail).
MITIGATIONS = {
    1: ("pen recognised by pressure > 0; eraser not distinguished",
        "pen not identified on this path: contacts with pressure > 0 are treated as pen; no eraser"),
    2: ("accept the measured resolution of 16 to 255 levels; brush response quantised",
        "no-go on this platform (Step 3)"),
    3: ("accept >= 256 levels", "as row 2"),
    4: ("accept the limited or quantised tilt", "tilt unsupported on this platform: tilt-driven brush options off"),
    5: ("twist limited to the measured range (e.g. modulo 180)", "barrel rotation unsupported on this platform"),
    6: ("accept one sample per dispatch", "accept one sample per dispatch; a floor change is a user decision (ADR-0005)"),
    7: ("interpolated sampling off; the >= 200 Hz target is recorded as below target",
        "no-go on this platform (Step 3)"),
    8: ("recorded as below the 200 Hz target", "recorded as below the 200 Hz target"),
    9: ("pressure interpolated between updates", "pressure changes without movement are ignored"),
}


def band(row, value):
    for low, high, result in BANDS[row]:
        if low <= value < high:
            return result
    return "fail"


def platform_of(log):
    host = ((log.get("environment") or {}).get("host") or {}).get("os")
    if host in ("windows", "macos"):
        return host
    ua = (log.get("environment") or {}).get("userAgent") or ""
    return "macos" if "Mac OS X" in ua or "Macintosh" in ua else "windows"


def capability(meta, key):
    v = str((meta or {}).get(key) or "").strip().lower()
    return True if v in ("yes", "y", "true") else False if v in ("no", "n", "false") else None


def measured(actions, aid, path, strokes=None):
    """('stats', st) | ('none', None) for performed without events | ('missing', reason). `st` holds only samples
    inside the action's span (C04-R3-01)."""
    rec = next((r for r in actions["actions"] if r["id"] == aid), None)
    if rec is None:
        return "missing", f"{aid} not recorded"
    out = rec[path]
    if out["outcome"] in ("omitted", "unfinished"):
        return "missing", f"{aid} {out['outcome']}"
    if out["measured"] is None:
        return "none", None
    return "stats", out["stats"]


def classify(path, strokes, actions, meta, web=None):
    """Row -> (result, note) for one candidate. Results: pass, partial, fail, n/a, insufficient."""
    res = {}
    # Row 1: pen identity over A1-A3.
    pens, missing = 0, []
    for aid in ("A1", "A2", "A3"):
        kind, st = measured(actions, aid, path, strokes)
        if kind == "missing":
            missing.append(st)
        elif kind == "stats" and str(st.get("pointer") or "").split("#")[0] in ("pen", "eraser"):
            pens += 1
    res[1] = (("insufficient", "; ".join(missing)) if missing else
              ("pass" if pens == 3 else "partial" if pens else "fail", f"{pens} of 3 actions report a pen"))
    # Rows 2 and 3: pressure resolution on A1.
    kind, st = measured(actions, "A1", path, strokes)
    if kind == "missing":
        res[2] = res[3] = ("insufficient", st)
    else:
        distinct = st["pressureDistinct"] if kind == "stats" else 0
        implied = st.get("pressureLevelsImplied") if kind == "stats" else None
        levels = implied if implied is not None and distinct >= 64 else distinct
        res[2] = (band(2, levels), f"{levels} levels")
        res[3] = (band(3, levels), f"{levels} levels")
    # Row 4: tilt on A3.
    has_tilt = capability(meta, "penTilt")
    kind, st = measured(actions, "A3", path, strokes)
    if has_tilt is False:
        res[4] = ("n/a", "pen has no tilt")
    elif kind == "missing":
        res[4] = ("insufficient", st)
    else:
        rx, ry = (st["tiltRangeX"], st["tiltRangeY"]) if kind == "stats" else (None, None)
        full = all(r and r[0] <= -TILT_SPAN_DEG and r[1] >= TILT_SPAN_DEG for r in (rx, ry))
        some = any(r and r[1] > r[0] for r in (rx, ry))
        result = "pass" if full else "partial" if some else "fail"
        res[4] = ("insufficient", "tilt absent and the pen's tilt support is not stated") \
            if result == "fail" and has_tilt is None else (result, f"x {rx}, y {ry}")
    # Row 5: twist on A4.
    has_rot = capability(meta, "penRotation")
    kind, st = measured(actions, "A4", path, strokes)
    if has_rot is False:
        res[5] = ("n/a", "pen has no barrel rotation")
    elif kind == "missing":
        res[5] = ("insufficient", st + ("; state whether the pen has barrel rotation" if has_rot is None else ""))
    else:
        r = st["twistRange"] if kind == "stats" else None
        width = (r[1] - r[0]) if r else 0
        nonzero = st["twistNonzero"] if kind == "stats" else 0
        result = "pass" if width > TWIST_PASS_RANGE_DEG else "partial" if nonzero and width > 0 else "fail"
        res[5] = ("insufficient", "twist absent and the pen's rotation support is not stated") \
            if result == "fail" and has_rot is None else (result, f"range {r}")
    # Row 6: coalesced events (web only), observed in A2's own samples (C04-R3-01).
    kind, st = measured(actions, "A2", path, strokes)
    if web is None:
        res[6] = ("n/a", "native path")
    elif kind == "missing":
        res[6] = ("insufficient", st)
    else:
        seen_batch = bool(st and st.get("coalescedObserved"))
        res[6] = ("pass" if web["coalescedApi"] and seen_batch else "partial" if web["coalescedApi"] else "fail",
                  f"API {web['coalescedApi']}, batching in A2 {seen_batch}")
    # Rows 7 and 8: rate on A2.
    kind, st = measured(actions, "A2", path, strokes)
    if kind == "missing":
        res[7] = res[8] = ("insufficient", st)
    elif kind == "none":
        res[7] = res[8] = ("fail", "performed, no events")
    elif st["rateHz"] is None:
        res[7] = res[8] = ("insufficient", "A2 has a single sample: repeat A2")
    else:
        res[7] = (band(7, st["rateHz"]), f"{st['rateHz']:.3f} Hz")
        res[8] = (band(8, st["rateHz"]), f"{st['rateHz']:.3f} Hz")
    # Row 9: stationary pressure on A6. One sample is the failure this row tests, not missing evidence.
    kind, st = measured(actions, "A6", path, strokes)
    if kind == "missing":
        res[9] = ("insufficient", st)
    elif kind == "none" or st["samples"] <= 1 or st["pressureDistinct"] <= 1:
        res[9] = ("fail", "only the down sample or one pressure value")
    else:
        hz = st["rateHz"]
        res[9] = ("pass" if hz is not None and hz >= STATIONARY_PASS_HZ else "partial",
                  f"{st['samples']} samples, {fmt(hz)} Hz")
    return res


def aggregate(platform, results):
    """The ADR-0009 Step 3 decision from per-candidate row results ({candidate: {row: result}})."""
    order = PRIORITY[platform]
    insufficient = sorted({f"row {row} ({c})" for c, rows in results.items() for row, r in rows.items()
                           if r == "insufficient"})
    if insufficient:
        return {"outcome": "insufficient evidence: repeat the actions named for " + ", ".join(insufficient),
                "selected": None, "conditions": [], "scores": {}, "eligible": []}
    # Ruling on the round-3 disputed note, option (a): the score is the share of applicable rows at pass (n/a rows are
    # in neither numerator nor denominator); a candidate with no applicable measured row is excluded. Exact fractions,
    # so equal shares tie exactly and fall to the fixed order. `scores` stays the plain pass count (diagnostic).
    applicable = {c: [r for r in rows.values() if r in ("pass", "partial", "fail")] for c, rows in results.items()}
    eligible = [c for c in order if c in results and applicable[c]]
    scores = {c: sum(1 for r in results[c].values() if r == "pass") for c in results}
    shares = {c: Fraction(scores[c], len(applicable[c])) for c in eligible}
    unmeasured = [c for c in order if c not in results]
    if not eligible:
        return {"outcome": "not evidence: no candidate has a valid measurement; repeat the procedure",
                "selected": None, "conditions": [], "scores": scores, "shares": {}, "eligible": [],
                "unmeasured": unmeasured}
    best = max(eligible, key=lambda c: (shares[c], -order.index(c)))
    # Option (c): pressure (row 2) or rate (row 7) at fail on the selected candidate makes the platform a no-go; the
    # mitigation table still applies to its other rows.
    blocking = [row for row in NO_GO_ROWS if results[best].get(row) == "fail"]
    conditions = [{"row": row, "result": r, "mitigation": MITIGATIONS[row][0 if r == "partial" else 1]}
                  for row, r in sorted(results[best].items()) if r in ("partial", "fail") and row not in blocking]
    if blocking:
        outcome = f"no-go on this platform: escalate (plan §6 decision 7); selected {best}, row(s) " \
                  f"{', '.join(str(r) for r in blocking)} at fail"
    else:
        outcome = f"go: {best}" if not conditions else f"go-with-conditions: {best}"
    return {"outcome": outcome, "selected": best, "conditions": conditions, "scores": scores,
            "shares": {c: f"{v.numerator}/{v.denominator}" if v.denominator != 1 else str(v.numerator)
                       for c, v in shares.items()},
            "blocking": blocking, "eligible": eligible, "unmeasured": unmeasured}


def tablet_decision(log, web, native, actions):
    if actions is None:
        return {"outcome": "not evidence: the log has no action markers (Step 0); repeat the procedure with the "
                           "current build", "selected": None, "conditions": []}
    platform = platform_of(log)
    meta = log.get("metadata") or {}
    rows = {"W": classify("web", web["strokes"], actions, meta, web=web)}
    if log.get("nativeInfo"):
        rows[NATIVE_CANDIDATE[platform]] = classify("native", native["strokes"], actions, meta)
    decision = aggregate(platform, {c: {k: v[0] for k, v in r.items()} for c, r in rows.items()})
    decision.update(platform=platform, rows={c: {str(k): {"result": v[0], "note": v[1]} for k, v in r.items()}
                                             for c, r in rows.items()})
    return decision


def print_decision(d):
    print("\n### Decision (ADR-0009 tablet rule set)\n")
    if d.get("rows"):
        cands = list(d["rows"])
        print("| row | capability | action | " + " | ".join(cands) + " |")
        print("|---|---|---|" + "---|" * len(cands))
        for row, name in ROWS.items():
            cells = [f"{d['rows'][c][str(row)]['result']} ({d['rows'][c][str(row)]['note']})" for c in cands]
            print(f"| {row} | {name} | {ROW_ACTION[row]} | " + " | ".join(cells) + " |")
        print(f"\n- platform: {d['platform']}; share of applicable rows at pass: {d.get('shares')}; rows at pass: "
              f"{d.get('scores')}; candidates with a valid measurement: {d.get('eligible')}")
        if d.get("unmeasured"):
            print("- not scored: " + ", ".join(f"{c} ({UNMEASURED_NOTE.get(c, 'not measured')})" for c in d["unmeasured"]))
    print(f"- outcome: **{d['outcome']}**")
    for c in d.get("conditions", []):
        print(f"- condition: row {c['row']} {ROWS[c['row']]} at {c['result']}: {c['mitigation']}")


def print_actions(actions):
    print("\n### Actions (markers recorded by the page; C04-R2-02)\n")
    if actions is None:
        print("No action markers in this log: it predates them, so the ADR-0009 rule set cannot use it (Step 0).")
        return
    print("| action | state | ms | web | web measured episode | native | native measured episode |")
    print("|---|---|---|---|---|---|---|")
    for r in actions["actions"]:
        w, n = r["web"], r["native"]
        print(f"| {r['id']} | {r['state']} | {fmt(r['durationMs'], 0)} | {w['outcome']} | {fmt(w['measured'], 0)} | "
              f"{n['outcome']} | {fmt(n['measured'], 0)} |")
    print(f"\nNot recorded: {', '.join(actions['notRecorded']) or 'none'}. Episodes outside every action (UI clicks, "
          f"ignored): web {actions['webOutside'] or 'none'}, native {actions['nativeOutside'] or 'none'}.")
    ex = actions["excludedSamples"]
    print(f"Samples outside every action span, excluded from every decision input: web {ex['web']}, native "
          f"{ex['native']}.")
    keys = actions.get("keyEvents")
    print("Raw key events: " + (f"{keys['events']} recorded, {keys['transitions']} marker transitions, "
                                f"{keys['ignored']} ignored (repeat, debounce or typing)." if keys else
                                "none in this log."))


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
        validate_focus(report["focus"], label)


def validate_focus(focus, label):
    """Version 3 needs exactly one complete snapshot for the start and one for the end of the stroke."""
    if not isinstance(focus, list):
        raise LogError(f"run {label!r}: focus snapshots must be a list")
    roles = [f.get("when") if isinstance(f, dict) else None for f in focus]
    if sorted(roles, key=str) != ["end", "start"]:
        raise LogError(f"run {label!r}: version 3 needs one start and one end focus snapshot, found {roles}")
    for f in focus:
        bad = [k for k in FOCUS_NUMBERS if not (isinstance(f.get(k), int) and not isinstance(f.get(k), bool))]
        bad += [k for k in FOCUS_STRINGS if not isinstance(f.get(k), str)]
        if bad:
            raise LogError(f"run {label!r}: focus snapshot {f.get('when')!r} lacks valid fields {bad}")


def validate(log, version):
    """Raises LogError on malformed or missing evidence (outcome c); returns True for a manual log (outcome b)."""
    if is_manual(log, version):
        return True
    check_simulations(log)
    runs = log["runs"]
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
    return False


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
           "missingRuns": missing_runs(missing),
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
    if validate(log, version):
        return []
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
            # Validation guarantees complete snapshots in version 3, so an empty list only occurs in version 2.
            parts.append("focus not recorded (version 2 log; focus snapshots were added in version 3)")
        if r.get("web"):
            w = r["web"]
            parts.append(f"web missing frames: {ranges(w['missingFrames'])}")
            parts.append(f"longest runs of consecutive missing frames (first-last (count)): "
                         f"{runs_text(w['missingRuns'])}")
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
            parts.append(f"WM_POINTER missing frames: {ranges(w['missingFrames'])} (longest runs "
                         f"{runs_text(w['missingRuns'], 3)}); counts {r['wmPointerCounts']}; "
                         f"repeated-pixel frames {ranges(w['repeatedPixelIndices'])}; tilt exact {w['tiltExact']}; "
                         f"rotation: {fidelity_text(w['twistFidelity'])}")
            rf = (r.get("rts") or {}).get("frames")
            if rf:
                union = set(range(w["injected"])) - (set(w["missingFrames"]) & set(rf["missingFrames"]))
                parts.append(f"RTS missing frames (pressure scale {rf['pressureScale']}): {ranges(rf['missingFrames'])}; "
                             f"WM_POINTER or RTS together: {len(union)} / {w['injected']}")
        print(f"- **{r['label']}**: " + "; ".join(parts))


def print_report(log, version, web, native, runs, actions=None):
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
    for key in ("computer", "osVersion", "webkit", "tablet", "driver", "penTilt", "penRotation", "tester", "notes"):
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
    if not runs:
        print_actions(actions)
        print_decision(tablet_decision(log, web, native, actions))
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
        web = web_analysis(log)
        native = native_analysis(log)
        actions = action_analysis(log, web, native)
    except LogError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    print_report(log, version, web, native, runs, actions)
    if out_json:
        with open(out_json, "w", encoding="utf-8") as f:
            decision = None if runs else tablet_decision(log, web, native, actions)
            json.dump(strip({"version": version, "web": web, "native": native, "runs": runs, "actions": actions,
                             "decision": decision}), f, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
