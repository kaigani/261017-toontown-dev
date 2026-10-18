"""Per-stroke analysis of a C0.2 tablet-spike log (`toontown --spike tablet --spike-log <file>`).

Usage: python scripts/analyse-tablet-log.py <log.json | log.json.gz>

Prints the environment, the native backend, every synthetic-pen run (Windows) and, per pen stroke on each path
(web = pointer events in the webview, native = host listener), the numbers the C0.2 pass criteria use: contact rate,
interval median/max, gaps longer than twice the median, distinct pressure values and the step-implied level count,
tilt ranges, twist, coalesced events per dispatch and event-to-handler latency. Standard library only.
"""

import gzip
import json
import statistics as st
import sys


def load(path):
    opener = gzip.open if path.endswith(".gz") else open
    with opener(path, "rt", encoding="utf-8") as f:
        return json.load(f)


def pct(values, p):
    if not values:
        return None
    s = sorted(values)
    return s[min(len(s) - 1, max(0, int(round(p / 100 * len(s) + 0.5)) - 1))]


def pressure_levels(values):
    distinct = sorted({v for v in values if v and v > 0})
    steps = [b - a for a, b in zip(distinct, distinct[1:]) if b > a]
    step = min(steps) if steps else None
    return len(distinct), (round(1 / step) if step else None)


def stroke_report(name, points):
    """points: list of dicts with t, pressure, tiltX, tiltY, twist (and optional dispatch/coalesced/handler)."""
    ts = [p["t"] for p in points]
    iv = [b - a for a, b in zip(ts, ts[1:])]
    med = st.median(iv) if iv else None
    dur = (ts[-1] - ts[0]) if len(ts) > 1 else 0
    distinct, levels = pressure_levels([p["pressure"] for p in points])
    tx = [p["tiltX"] for p in points if p["tiltX"] is not None]
    ty = [p["tiltY"] for p in points if p["tiltY"] is not None]
    line = (
        f"  {name}: samples {len(points)}, rate {((len(points) - 1) * 1000 / dur) if dur else 0:.1f} Hz, "
        f"interval median {med if med is not None else float('nan'):.2f} ms max {max(iv) if iv else 0:.1f} ms, "
        f"gaps>2x {sum(1 for d in iv if med and d > 2 * med)}, pressure distinct {distinct} implied {levels}, "
        f"tilt x {min(tx) if tx else '-'}..{max(tx) if tx else '-'} y {min(ty) if ty else '-'}..{max(ty) if ty else '-'}, "
        f"twist nonzero {sum(1 for p in points if p.get('twist'))}"
    )
    print(line)
    if points and "dispatch" in points[0]:
        first = {}
        for p in points:
            first.setdefault(p["dispatch"], p)
        cc = [p["coalesced"] for p in first.values()]
        hl = [p["handler"] - p["t"] for p in first.values()]
        print(
            f"      dispatches {len(first)}, coalesced per dispatch p50 {pct(cc, 50)} max {max(cc)}, "
            f"event->handler latency p50 {pct(hl, 50):.1f} p95 {pct(hl, 95):.1f} ms"
        )


def main():
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    r = load(sys.argv[1])
    env = r["environment"]
    print(f"userAgent: {env['userAgent']}")
    print(f"devicePixelRatio {env['devicePixelRatio']}, getCoalescedEvents available: {env['coalescedApi']}")
    print(f"native: {r['summary']['nativeInfo']}, packets {len(r['native']['packets'])}, invalid batches {r['summary']['invalidNativeBatches']}")
    for m in r.get("statusLog", []):
        print(f"  status: {m['message']}")
    for s in r.get("simulations", []):
        w, n = s["web"], s["native"]
        lat = w["latencyMs"] or {}
        print(
            f"synthetic pen {s['request']['hz']} Hz (achieved {s['achievedHz']:.1f}): web received {w['received']}/{w['injected']} "
            f"(tilt exact {w['tiltMatches']}), injection->event p50 {lat.get('p50', float('nan')):.2f} p95 {lat.get('p95', float('nan')):.2f} "
            f"max {lat.get('max', float('nan')):.1f} ms; native received {n['received']}/{n['injected']}"
        )
    strokes, cur = [], None
    for s in r["web"]["samples"]:
        if s["kind"] == "down":
            cur = []
            strokes.append(cur)
        elif cur is not None and s["kind"] == "move" and s["pressure"] > 0:
            cur.append({"t": s["t"], "pressure": s["pressure"], "tiltX": s["tiltX"], "tiltY": s["tiltY"], "twist": s["twist"],
                        "dispatch": s["dispatch"], "coalesced": s["coalescedCount"], "handler": s["handlerT"]})
    print(f"web path: {len(strokes)} pen-down strokes")
    for i, stroke in enumerate(strokes):
        if stroke:
            stroke_report(f"web stroke {i}", stroke)
    nstrokes, cur = [], None
    for p in r["native"]["packets"]:
        if p["kind"] == "down" or (cur is None and p["kind"] == "pose"):
            cur = []
            nstrokes.append(cur)
        if p["kind"] == "up":
            cur = None
        elif cur is not None and p["kind"] == "pose" and (p["pressure"] or 0) > 0:
            cur.append({"t": p["deviceMs"] if p["deviceMs"] is not None else p["hostEpochMs"], "pressure": p["pressure"],
                        "tiltX": p["tiltX"], "tiltY": p["tiltY"], "twist": p["twist"]})
    print(f"native path: {len(nstrokes)} strokes")
    for i, stroke in enumerate(nstrokes):
        if stroke:
            stroke_report(f"native stroke {i}", stroke)
    return 0


if __name__ == "__main__":
    sys.exit(main())
