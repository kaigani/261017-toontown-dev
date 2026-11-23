"""Writes the `zones` section of the Timeline's geometry case table (ruling E-35) from `timeline-layout.json` and the rule
its comment states, then copies the table to the window (desktop/src/timeline/, a byte copy kept equal by Vitest).

The cases cover each zone's edge pixels and the pixels just outside, every overlap the rule orders (a keyframe over the
start handle and over the end column, the end column over the start handle on a short bar), the upper and lower half,
loopable and not, and three zoom levels. The kernel's unit test (`timing_gesture::tests`) and the window's Vitest check
every case.

Usage (from the repository root): python scripts/gen-timeline-zone-cases.py
"""
import json
import math
import pathlib
import shutil

ROOT = pathlib.Path(__file__).resolve().parents[1]
LAYOUT = ROOT / "crates" / "executor" / "src" / "timeline-layout.json"
CASES = ROOT / "crates" / "executor" / "src" / "timeline-geometry-cases.json"
WINDOW = ROOT / "desktop" / "src" / "timeline" / "timeline-geometry-cases.json"


def zoom(l, i):
    levels = l["zoomLevels"]
    return levels[max(0, min(i, len(levels) - 1))]


def x_of(l, z, first, frame):
    per = max(1, zoom(l, z)["framesPerCell"])
    return math.floor((frame - first) / per) * zoom(l, z)["cellWidth"]


def zone(l, z, first, x, upper, bar, loopable, keys):
    cw = zoom(l, z)["cellWidth"]
    for k in keys:
        if abs(x - (x_of(l, z, first, k) + cw / 2)) <= l["keyframeRadius"]:
            return {"zone": "keyframe", "frame": k}
    if bar:
        xs, xe = x_of(l, z, first, bar[0]), x_of(l, z, first, bar[1]) + cw
        if abs(x - xe) <= l["endHandleWidth"]:
            return {"zone": "loopHandle" if upper and loopable else "barEnd"}
        if abs(x - xs) <= l["startHandleWidth"]:
            return {"zone": "barStart"}
        if xs < x < xe:
            return {"zone": "barBody"}
    return {"zone": "cell"}


def main():
    l = json.loads(LAYOUT.read_text(encoding="utf-8"))
    table = json.loads(CASES.read_text(encoding="utf-8"))
    cases = []

    def add(z, first, x, upper, bar, loopable, keys):
        cases.append({"zoomLevel": z, "firstFrame": first, "x": x, "upper": upper, "bar": bar, "loopable": loopable,
                      "keys": keys, "zone": zone(l, z, first, x, upper, bar, loopable, keys)})

    for z in (3, 6, 9):
        cw = zoom(l, z)["cellWidth"]
        for bar in ([5, 24], [5, 5]):
            xs, xe = x_of(l, z, 1, bar[0]), x_of(l, z, 1, bar[1]) + cw
            points = set()
            for edge, w in ((xs, l["startHandleWidth"]), (xe, l["endHandleWidth"])):
                points.update([edge - w - 1, edge - w, edge, edge + w, edge + w + 1])
            points.update([(xs + xe) / 2, xs + l["startHandleWidth"] + 1, xe - l["endHandleWidth"] - 1])
            for x in sorted(points):
                for upper in (False, True):
                    for loopable in (True, False):
                        add(z, 1, x, upper, bar, loopable, [])
            # A keyframe on the start frame (over the start handle) and on the last frame (over the end column).
            for k in (bar[0], bar[1]):
                centre = x_of(l, z, 1, k) + cw / 2
                for x in (centre - l["keyframeRadius"] - 1, centre - l["keyframeRadius"], centre,
                          centre + l["keyframeRadius"], centre + l["keyframeRadius"] + 1):
                    add(z, 1, x, True, bar, True, [k])
        # Two keys within the radius of one point: the earliest wins.
        add(z, 1, x_of(l, z, 1, 10) + cw, True, None, True, [10, 11])
        # No bar: a cell.
        add(z, 1, 40.0, False, None, False, [])
    table["zones"] = cases
    text = json.dumps(table, indent=1, ensure_ascii=True)
    CASES.write_text(text + "\n", encoding="utf-8", newline="\n")
    shutil.copyfile(CASES, WINDOW)
    print(f"{len(cases)} zone cases")


if __name__ == "__main__":
    main()
