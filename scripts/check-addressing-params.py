"""Every point-taking `draw.*` and `colour.*` command declares the common addressing parameters (M4 C4.2a, C42A-R2-01
and -02): spec 02 §13.1 item 5 ("every point-taking `draw.*` command also accepts the common addressing parameters
`space: enum "drawing"|"scene"|"view" = "drawing"; view?: string; frame?: int`") and spec 03's point targets (03 §16.2:
`space`, `view`, `frame` on the point-taking colour commands).

Mechanical: the generated parameter schemas (`desktop/src/ipc/generated/commands.ts`) are read; a command is
point-taking when a top-level parameter is a point (an object with numeric `x` and `y`), an array of points, or an
object holding points one level down (a rectangle `{x, y, width, height}`, a segment `{from, to}`, a mapping drag). Each
such command must declare `space` (with `drawing`, `scene` and `view`), `view` and `frame`, and, as 02 §13.1 item 4
requires of every command that edits or reads a drawing, `drawingId` and `elementId` (NOT_DRAWING_ADDRESSED lists the
exception). The few commands whose
coordinates are not point targets are listed in EXEMPT with the reason. Exit 0 when every command complies, 1 with the
list otherwise. Standard library only; run from the repository root.
"""
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]

# Point-taking commands that do not address one drawing (02 §13.1 item 4 does not apply), and why.
NOT_DRAWING_ADDRESSED = {
    "draw.repositionAll.apply": "edits every drawing of an element (02 §13.2 line 1336: `elementId?: id = current`)",
}

# Coordinates that are not point targets of the drawing (and why).
EXEMPT = {
    "draw.drawing.render": "`region` is the rendered area of a thumbnail render, not a point target (02 §13.2 row)",
    "draw.vectorize.preview": "no point parameters; listed for clarity",
}


def ts_object(text, start):
    s = text.split(start, 1)[1].split("\n};", 1)[0] + "\n}"
    tok = re.compile(r'"(?:\\.|[^"\\])*"|\b\d+n\b|(?<![\w$])[A-Za-z_$][\w$]*(?=\s*:)|,(?=\s*[}\]])')

    def sub(m):
        t = m[0]
        if t.startswith('"'):
            return t
        if t.endswith("n") and t[0].isdigit():
            return t[:-1]
        if t == ",":
            return ""
        return json.dumps(t)

    return json.loads(tok.sub(sub, s))


def main():
    ts = (ROOT / "desktop/src/ipc/generated/commands.ts").read_text(encoding="utf-8")
    defs = ts_object(ts, "export const COMMAND_DEFS: Readonly<Record<string, Schema>> = ")
    index = ts_object(ts, "export const COMMAND_INDEX = ")

    def expand(v, depth=0):
        if depth > 12:
            return v
        if isinstance(v, dict):
            if "$ref" in v and v["$ref"].startswith("#/$defs/"):
                return expand(defs[v["$ref"][8:]], depth + 1)
            return {k: expand(x, depth + 1) for k, x in v.items()}
        if isinstance(v, list):
            return [expand(x, depth + 1) for x in v]
        return v

    def variants(s):
        out = [s]
        for k in ("anyOf", "oneOf"):
            for x in s.get(k, []) if isinstance(s, dict) else []:
                out.extend(variants(x))
        return out

    def is_point(s):
        return any(isinstance(v, dict) and {"x", "y"} <= set(v.get("properties", {})) for v in variants(s))

    def takes_points(s, depth=0):
        for v in variants(s):
            if not isinstance(v, dict):
                continue
            if is_point(v):
                return True
            if v.get("type") == "array" or "items" in v:
                if takes_points(v.get("items", {}), depth + 1):
                    return True
            if depth < 1 and isinstance(v.get("properties"), dict):
                if any(takes_points(x, depth + 1) for x in v["properties"].values()):
                    return True
        return False

    missing = []
    checked = 0
    for cid, entry in sorted(index.items()):
        if not (cid.startswith("draw.") or cid.startswith("colour.")) or entry.get("kind") == "event":
            continue
        key = cid + ".params"
        if key not in defs:
            continue
        params = expand(defs[key])
        props = params.get("properties", {})
        coords = [k for k, v in props.items() if k not in ("space", "view", "frame") and takes_points(v)]
        if not coords or cid in EXEMPT:
            continue
        checked += 1
        lack = [k for k in ("space", "view", "frame") if k not in props]
        # 02 §13.1 item 4: the target is `drawingId`, else `elementId` + `frame`, else the current drawing.
        if cid not in NOT_DRAWING_ADDRESSED:
            lack += [k for k in ("drawingId", "elementId") if k not in props]
        if "space" in props:
            text = json.dumps(props["space"])
            lack += [f"space:{v}" for v in ("drawing", "scene", "view") if f'"{v}"' not in text]
        if lack:
            missing.append(f"{cid}: point parameters {coords}; lacks {lack}")
    print(f"{checked} point-taking draw.*/colour.* commands checked; {len(missing)} without the common addressing parameters")
    for m in missing:
        print("  " + m)
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
