#!/usr/bin/env python3
"""Structural comparison of two JSON goldens (M2 C2.4 round 2, C24-R2-05).

Usage: python scripts/golden-structural-diff.py <old.json> <new.json> <out.json>

Descends through every object and array. Arrays of objects with an `id` are aligned by ID: pieces only in the old file
are `removed`, only in the new file `added`; a piece in both is `moved` when its geometry changed with the same anchor
count (corresponding anchors: their displacement is reported), `resplit` when its anchor count changed (its anchors do
not correspond one to one, so no displacement is claimed), else compared field by field. Other arrays of unequal length
are aligned by longest common subsequence of their elements (inserted, deleted and replaced runs; replaced runs of equal
length are compared element by element). Stroke `points` rows are `[x, y, inX, inY, outX, outY]` (anchor, handles
relative to it): the anchor displacement is the distance between `(x, y)` pairs; handle changes are counted separately.
An anchor of a `moved` piece that moved more than `CORRESPONDING_DU` (1 du: far above any snapping or junction
placement change, far below the corpora's feature sizes) is not a displaced junction but a rerouted end (a junction that
appeared or disappeared, so the piece now ends at another vertex); those are listed apart.
Standard library only.
"""
import difflib
import json
import math
import pathlib
import sys

CORRESPONDING_DU = 1.0


def is_number(x):
    return isinstance(x, (int, float)) and not isinstance(x, bool)


class Diff:
    def __init__(self):
        self.changes = []  # leaf changes: path, old, new
        self.pieces = {"added": [], "removed": [], "moved": [], "resplit": []}
        self.anchor_displacements = []  # (distance, path, old, new)
        self.rerouted = []  # anchors of moved pieces that moved more than CORRESPONDING_DU
        self.handle_changes = 0
        self.array_edits = []

    def leaf(self, path, a, b):
        self.changes.append({"path": path, "old": a, "new": b})

    def compare(self, a, b, path):
        if isinstance(a, dict) and isinstance(b, dict):
            for k in sorted(set(a) | set(b)):
                p = f"{path}.{k}" if path else k
                if k not in a:
                    self.leaf(p, None, b[k])
                elif k not in b:
                    self.leaf(p, a[k], None)
                elif k == "points" and isinstance(a[k], list) and isinstance(b[k], list):
                    self.points(a[k], b[k], p)
                elif k == "bezier" and isinstance(a[k], list) and isinstance(b[k], list) and len(a[k]) == len(b[k]):
                    self.bezier(a[k], b[k], p)
                else:
                    self.compare(a[k], b[k], p)
        elif isinstance(a, list) and isinstance(b, list):
            if a and b and all(isinstance(x, dict) and "id" in x for x in a + b):
                self.by_id(a, b, path)
            elif len(a) == len(b):
                for i, (x, y) in enumerate(zip(a, b)):
                    self.compare(x, y, f"{path}[{i}]")
            else:
                self.lcs(a, b, path)
        elif a != b:
            self.leaf(path, a, b)

    def by_id(self, a, b, path):
        old = {x["id"]: x for x in a}
        new = {x["id"]: x for x in b}
        for i in sorted(set(old) - set(new)):
            self.pieces["removed"].append({"path": path, "id": i, "type": old[i].get("type")})
        for i in sorted(set(new) - set(old)):
            self.pieces["added"].append({"path": path, "id": i, "type": new[i].get("type")})
        order_a = [x["id"] for x in a if x["id"] in new]
        order_b = [x["id"] for x in b if x["id"] in old]
        if order_a != order_b:
            self.array_edits.append({"path": path, "kind": "reordered"})
        for i in order_b:
            x, y = old[i], new[i]
            p = f"{path}[id={i}]"
            if x.get("type") == "stroke" and isinstance(x.get("points"), list) and isinstance(y.get("points"), list):
                if x["points"] != y["points"]:
                    if len(x["points"]) == len(y["points"]):
                        self.pieces["moved"].append({"path": path, "id": i})
                    else:
                        self.pieces["resplit"].append(
                            {"path": path, "id": i, "anchors": [len(x["points"]), len(y["points"])]}
                        )
                rest_x = {k: v for k, v in x.items() if k != "points"}
                rest_y = {k: v for k, v in y.items() if k != "points"}
                if len(x["points"]) == len(y["points"]):
                    self.points(x["points"], y["points"], p + ".points")
                self.compare(rest_x, rest_y, p)
            else:
                self.compare(x, y, p)

    def points(self, a, b, path):
        if len(a) != len(b):
            self.lcs(a, b, path)
            return
        for k, (r, s) in enumerate(zip(a, b)):
            if r == s:
                continue
            if isinstance(r, list) and isinstance(s, list) and len(r) >= 2 and all(map(is_number, r[:2] + s[:2])):
                d = math.hypot(s[0] - r[0], s[1] - r[1])
                if d > CORRESPONDING_DU:
                    self.rerouted.append({"du": d, "path": f"{path}[{k}]", "old": r[:2], "new": s[:2]})
                elif d > 0:
                    self.anchor_displacements.append((d, f"{path}[{k}]", r[:2], s[:2]))
                if r[2:] != s[2:]:
                    self.handle_changes += 1
            else:
                self.compare(r, s, f"{path}[{k}]")

    def bezier(self, a, b, path):
        """A flat Bezier list `p0, c1, c2, p1, ...` of `{x, y}`: anchors at every third entry, handles between."""
        for k, (r, s) in enumerate(zip(a, b)):
            if r == s:
                continue
            if all(isinstance(v, dict) and is_number(v.get("x")) and is_number(v.get("y")) for v in (r, s)):
                if k % 3 == 0:
                    d = math.hypot(s["x"] - r["x"], s["y"] - r["y"])
                    if d > CORRESPONDING_DU:
                        self.rerouted.append({"du": d, "path": f"{path}[{k}]", "old": r, "new": s})
                    else:
                        self.anchor_displacements.append((d, f"{path}[{k}]", [r["x"], r["y"]], [s["x"], s["y"]]))
                else:
                    self.handle_changes += 1
            else:
                self.compare(r, s, f"{path}[{k}]")

    def lcs(self, a, b, path):
        key = lambda x: json.dumps(x, sort_keys=True)
        sm = difflib.SequenceMatcher(a=[key(x) for x in a], b=[key(x) for x in b], autojunk=False)
        for op, i1, i2, j1, j2 in sm.get_opcodes():
            if op == "equal":
                continue
            edit = {"path": path, "kind": op, "old": [i1, i2], "new": [j1, j2]}
            self.array_edits.append(edit)
            if op == "replace" and i2 - i1 == j2 - j1:
                for k in range(i2 - i1):
                    self.compare(a[i1 + k], b[j1 + k], f"{path}[{i1 + k}->{j1 + k}]")


def main():
    if len(sys.argv) != 4:
        print(__doc__)
        return 2
    old = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
    new = json.loads(pathlib.Path(sys.argv[2]).read_text(encoding="utf-8"))
    d = Diff()
    d.compare(old, new, "")
    disp = sorted(d.anchor_displacements, key=lambda t: -t[0])
    out = {
        "old": sys.argv[1],
        "new": sys.argv[2],
        "pieces": {k: len(v) for k, v in d.pieces.items()},
        "pieceDetails": d.pieces,
        "correspondingAnchorsMoved": len(disp),
        "maxAnchorDisplacementDu": disp[0][0] if disp else 0.0,
        "largestAnchorDisplacements": [
            {"du": t[0], "path": t[1], "old": t[2], "new": t[3]} for t in disp[:20]
        ],
        "reroutedAnchors": d.rerouted,
        "handleRowsChanged": d.handle_changes,
        "arrayEdits": d.array_edits,
        "leafChanges": len(d.changes),
        "leafChangeDetails": d.changes[:200],
    }
    pathlib.Path(sys.argv[3]).write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: out[k] for k in ("pieces", "correspondingAnchorsMoved", "maxAnchorDisplacementDu",
                                          "handleRowsChanged", "leafChanges")} | {"rerouted": len(d.rerouted)}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
