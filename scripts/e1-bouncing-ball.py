#!/usr/bin/env python3
"""End-to-end scenario E1 "bouncing ball" (11 §14.8; M3 C3.4) through the official MCP Python SDK (`mcp==2.0.0`)
against the real `toontown-headless` binary. MP4 export is deferred to M9 (ADR-0008): the movie step is the PNG
sequence with its companion manifest (09 §11.1 items 4-6).

Usage: python scripts/e1-bouncing-ball.py <path to toontown-headless> <delivery dir>

Chain (process A, one MCP session): new project 1920x1080 at 24 fps -> drawing element "Ball" -> circle -> paint -> peg
"Bounce" -> motion keys at frames 1, 12 and 24 with explicit easing (velocity presets) -> validate and read back ->
evaluate -> single renders -> contact sheet -> PNG sequence + manifest -> delivery conflicts, a late read-only
target, a cancelled export -> describe / hit test / convert -> undo / redo -> save, close, reopen, re-render.
Process B: the recorded command script (stable symbolic references) replayed with `toontown-headless --run` into a
fresh project -> canonical content and output hashes identical. A variant (parent peg rotation and scale, camera
roll, default FOV) is rendered and checked too.

Every expected value is computed here, independently of the product: positions from the easing presets' Bezier
control points (05 §10.4) along the straight path, pixel centres from the default camera's table-plane scale
(1 field = cameraWidth / 12 pixels, SI-105), contact-sheet layout from SI-108, and the rendered ball centres are
measured from the PNG pixels.

Delivery folder (shallow): `sequence/` (24 PNGs + manifest), `contact-sheet.png`, `frames/` (frames 1, 12, 24 and
the variant), `evaluate.json`, `manifest.normalized.json`, `e1-commands.json` (the recorded script), `projects/`
(saved and replayed projects), `report.json`. Exit status 0 when every check passes.
"""
import asyncio
import base64
import hashlib
import importlib.metadata
import json
import math
import os
import platform
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import zlib
from pathlib import Path

from mcp import ClientSession, StdioServerParameters, types
from mcp.client.stdio import stdio_client

CHECKS = []
W, H, FPS = 1920, 1080, 24
PX_PER_FIELD = W / 12  # SI-105: 1200 du across the default camera, 100 du per field
TOP, FLOOR = 2.5, -2.5  # fields
RADIUS_DU = 50  # 0.5 field
KEYS = (1, 12, 24)
LENGTH = 30  # scene frames; 25-30 show the hold after the last key
BALL = 'scene:#1/element:"Ball"'
PEG = 'scene:#1/element:"Bounce"'
RIG = 'scene:#1/element:"Rig"'
CAMERA = 'scene:#1/element:"Camera"'
PRESETS = {  # 05 §10.4, normalised P1 and P2
    "slowAcceleration": ((1 / 3, 0.0), (1.0, 1.0)),
    "slowDeceleration": ((0.0, 0.0), (2 / 3, 1.0)),
}


def check(name, ok, detail=""):
    CHECKS.append({"check": name, "ok": bool(ok), "detail": str(detail)[:600]})
    print(("ok   " if ok else "FAIL ") + name, flush=True)


def sc(result):
    return getattr(result, "structuredContent", None) or getattr(result, "structured_content", None) or {}


# --- independent expectations ---------------------------------------------------------------------------------------


def bezier_ease(preset, t):
    """The normalised value of an easing preset at normalised time t: solve x(u) = t (bisection to 1e-12), return y(u)."""
    (x1, y1), (x2, y2) = PRESETS[preset]

    def b(p1, p2, u):
        return 3 * (1 - u) ** 2 * u * p1 + 3 * (1 - u) * u * u * p2 + u ** 3

    lo, hi = 0.0, 1.0
    for _ in range(80):
        mid = (lo + hi) / 2
        if b(x1, x2, mid) < t:
            lo = mid
        else:
            hi = mid
    return b(y1, y2, (lo + hi) / 2)


class MotionPath:
    """The motion path of 05 §9.1-9.2, implemented here: a Kochanek-Bartels spline (t = c = b = 0) through the key
    points with reflected phantom end points, each segment sampled at 64 uniform parameter steps; the cumulative chord
    length of the samples is the arc length, and the point at arc length s is the curve evaluated exactly at the
    parameter interpolated linearly between the bracketing samples."""

    SAMPLES = 64

    def __init__(self, points):
        self.p = points
        n = len(points) - 1
        ext = [tuple(2 * a - b for a, b in zip(points[0], points[1]))] + list(points) + [tuple(2 * a - b for a, b in zip(points[n], points[n - 1]))]
        # t = c = b = 0: Dout(i) = Din(i) = (P[i+1] - P[i-1]) / 2
        self.d = [tuple((ext[i + 2][k] - ext[i][k]) / 2 for k in range(len(points[0]))) for i in range(n + 1)]
        self.samples = []  # (cumulative length, segment, parameter)
        total = 0.0
        prev = None
        for i in range(n):
            for k in range(self.SAMPLES + 1):
                if i > 0 and k == 0:
                    continue
                u = k / self.SAMPLES
                pt = self.at(i, u)
                if prev is not None:
                    total += math.dist(prev, pt)
                self.samples.append((total, i, u))
                prev = pt
        self.key_lengths = [0.0] + [l for (l, i, u) in self.samples if u == 1.0]

    def at(self, i, u):
        h00, h10, h01, h11 = 2 * u ** 3 - 3 * u ** 2 + 1, u ** 3 - 2 * u ** 2 + u, -2 * u ** 3 + 3 * u ** 2, u ** 3 - u ** 2
        return tuple(h00 * self.p[i][k] + h10 * self.d[i][k] + h01 * self.p[i + 1][k] + h11 * self.d[i + 1][k] for k in range(len(self.p[0])))

    def point_at_length(self, s):
        s = min(max(s, 0.0), self.samples[-1][0])
        for (l0, i0, u0), (l1, i1, u1) in zip(self.samples, self.samples[1:]):
            if l0 <= s <= l1:
                if u1 == 0.0:
                    u1 = 1.0
                u = u0 + (u1 - u0) * ((s - l0) / (l1 - l0) if l1 > l0 else 0.0)
                return self.at(i1, u)
        return self.p[-1]


PATH = MotionPath([(0.0, TOP), (0.0, FLOOR), (0.0, TOP)])


def velocity(f):
    """The velocity channel V(f) (05 §10.5): pinned at the keys' arc lengths, shaped by the segment presets."""
    s = PATH.key_lengths
    if f <= KEYS[0]:
        return s[0]
    if f <= KEYS[1]:
        return s[0] + (s[1] - s[0]) * bezier_ease("slowAcceleration", (f - KEYS[0]) / (KEYS[1] - KEYS[0]))
    if f <= KEYS[2]:
        return s[1] + (s[2] - s[1]) * bezier_ease("slowDeceleration", (f - KEYS[1]) / (KEYS[2] - KEYS[1]))
    return s[2]


def expected_xy(f):
    return PATH.point_at_length(velocity(f))


def expected_y(f):
    """The ball's height in fields at frame f."""
    return expected_xy(f)[1]


def ideal_y(f):
    """The height with exact arc length (a straight path at the eased distance), to bound the sampling's effect."""
    span = TOP - FLOOR
    if f <= KEYS[1]:
        return TOP - span * bezier_ease("slowAcceleration", max(0.0, (f - KEYS[0]) / (KEYS[1] - KEYS[0])))
    if f <= KEYS[2]:
        return FLOOR + span * bezier_ease("slowDeceleration", (f - KEYS[1]) / (KEYS[2] - KEYS[1]))
    return TOP


def to_pixels(x, y, rot_deg=0.0, scale=1.0, roll_deg=0.0):
    """Image position of a table-plane point under a parent rotation/scale about the origin and a camera roll."""
    a = math.radians(rot_deg)
    wx, wy = scale * (x * math.cos(a) - y * math.sin(a)), scale * (x * math.sin(a) + y * math.cos(a))
    r = math.radians(-roll_deg)  # a camera rolled by +r shows the world turned by -r
    cx, cy = wx * math.cos(r) - wy * math.sin(r), wx * math.sin(r) + wy * math.cos(r)
    return W / 2 + PX_PER_FIELD * cx, H / 2 - PX_PER_FIELD * cy


def png_pixels(png):
    """(width, height, RGBA rows) of an 8-bit RGBA, non-interlaced PNG."""
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    pos, idat, width, height = 8, b"", 0, 0
    while pos < len(png):
        n = int.from_bytes(png[pos:pos + 4], "big")
        kind, data = png[pos + 4:pos + 8], png[pos + 8:pos + 8 + n]
        if kind == b"IHDR":
            width, height = int.from_bytes(data[:4], "big"), int.from_bytes(data[4:8], "big")
            assert data[8] == 8 and data[9] == 6, "8-bit RGBA"
        elif kind == b"IDAT":
            idat += data
        pos += 12 + n
    raw, stride, rows, prev = zlib.decompress(idat), width * 4, [], bytearray(width * 4)
    for y in range(height):
        f, line = raw[y * (stride + 1)], bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        if f == 1:
            for i in range(4, stride):
                line[i] = (line[i] + line[i - 4]) & 255
        elif f == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 255
        elif f == 3:
            for i in range(stride):
                line[i] = (line[i] + ((line[i - 4] if i >= 4 else 0) + prev[i]) // 2) & 255
        elif f == 4:
            for i in range(stride):
                a = line[i - 4] if i >= 4 else 0
                b, c = prev[i], prev[i - 4] if i >= 4 else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(bytes(line))
        prev = line
    return width, height, rows


def ball_centre(png, ball_rgb, bg_rgb):
    """Coverage-weighted centroid of the ball's pixels (pixel centres at +0.5), and its pixel count."""
    width, height, rows = png_pixels(png)
    ch = max(range(3), key=lambda c: abs(ball_rgb[c] - bg_rgb[c]))
    span = bg_rgb[ch] - ball_rgb[ch]
    sx = sy = sw = 0.0
    count = 0
    for y, row in enumerate(rows):
        for x in range(width):
            v = row[x * 4 + ch]
            if v == bg_rgb[ch]:
                continue
            a = (bg_rgb[ch] - v) / span
            if a <= 0:
                continue
            sx += a * (x + 0.5)
            sy += a * (y + 0.5)
            sw += a
            count += 1
    return (sx / sw, sy / sw, sw, count) if sw else (None, None, 0.0, 0)


def sha(b):
    return hashlib.sha256(b).hexdigest()


# Times are volatile; revision counters count edits (an undo and a redo are two more), so content comparisons leave
# them out too (10 §2: `rev`, `documentRev`).
VOLATILE = {"exportedAt", "createdAt", "modifiedAt", "savedAt", "openedAt", "lastOpened", "time", "startedAt", "finishedAt", "rev", "documentRev"}


def canonical(value):
    """IDs replaced by names in order of first appearance and volatile times dropped (stable symbolic references)."""
    names = {}

    def is_id(s):
        # Object IDs are a lower-case type prefix, `_` and a 26-character ULID (10 §2.1).
        if not isinstance(s, str) or "_" not in s:
            return False
        prefix, rest = s.split("_", 1)
        return 2 <= len(prefix) <= 4 and prefix.isalpha() and prefix.islower() and len(rest) == 26 and rest.isalnum() and rest.upper() == rest

    def walk(v, key=None):
        if isinstance(v, dict):
            return {names.setdefault(k, f"<id{len(names) + 1}>") if is_id(k) else k: walk(x, k) for k, x in sorted(v.items()) if k not in VOLATILE}
        if isinstance(v, list):
            return [walk(x, key) for x in v]
        if is_id(v):
            return names.setdefault(v, f"<id{len(names) + 1}>")
        return v

    return json.dumps(walk(value), sort_keys=True, indent=1) + "\n"


# --- the scenario ---------------------------------------------------------------------------------------------------


class Session:
    """One MCP session on the headless binary: `cmd` raises on a refusal; document-changing steps are recorded."""

    def __init__(self, s):
        self.s = s
        self.script = []

    async def call(self, cid, p=None):
        return sc(await self.s.call_tool("run_command", {"id": cid, "params": p or {}}))

    async def cmd(self, cid, p=None, record=False):
        r = await self.call(cid, p)
        if r.get("status") != "ok":
            raise RuntimeError(f"{cid} {json.dumps(p)[:300]}: {json.dumps(r)[:600]}")
        if record:
            self.script.append({"id": cid, "params": p or {}})
        return r["result"]

    async def tool(self, name, args):
        r = sc(await self.s.call_tool(name, args))
        if r.get("status") not in (None, "ok"):
            raise RuntimeError(f"{name} {json.dumps(args)[:300]}: {json.dumps(r)[:600]}")
        return r.get("result", r)

    async def record_of(self, job):
        """The job's record through MCP `job_status` (generic tool)."""
        r = await self.tool("job_status", {"jobId": job})
        if isinstance(r, list):
            return r[0] if r else {}
        return r.get("record", r)

    async def wait(self, job, timeout=300000):
        r = await self.tool("job_wait", {"jobId": job, "timeoutMs": timeout})
        return r.get("record", r)


def frame_png(r):
    return base64.b64decode(r.get("data") or r.get("png"))


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(data, str):
        path.write_text(data, encoding="utf-8", newline="\n")
    else:
        path.write_bytes(data)


def normalized_manifest(manifest):
    m = json.loads(json.dumps(manifest))
    for key in ("outputPath", "manifestPath"):
        m.pop(key, None)
    for f in m.get("files", []):
        f["path"] = Path(f["path"]).name
    return json.dumps(m, sort_keys=True, indent=1) + "\n"


async def build(a, red):
    """The E1 document: Ball, circle, paint, peg, keys at 1, 12, 24 with explicit easing (recorded steps)."""
    steps = [
        ("element.create", {"type": "drawing", "name": "Ball", "sceneId": "scene:#1"}),
        ("draw.ellipse.add", {"elementId": BALL, "frame": 1, "from": {"x": -RADIUS_DU, "y": -RADIUS_DU}, "to": {"x": RADIUS_DU, "y": RADIUS_DU}, "circle": True, "layer": "new", "colourId": red}),
        ("colour.paint.apply", {"elementId": BALL, "frame": 1, "points": [{"x": 0, "y": 0}], "colourId": red}),
        ("xsheet.exposures.set", {"columns": [{"elementId": BALL, "from": 1, "to": LENGTH, "exposures": [{"drawing": "1", "from": 1, "to": LENGTH}]}]}),
        ("element.create", {"type": "peg", "name": "Bounce", "sceneId": "scene:#1"}),
        ("element.parent.set", {"elementIds": [BALL], "parentId": PEG}),
        ("element.timing.set", {"elementIds": [PEG, CAMERA], "duration": LENGTH}),
        ("anim.path.set", {"nodeId": PEG, "points": [{"kind": "key", "frame": KEYS[0], "x": 0, "y": TOP, "z": 0}, {"kind": "key", "frame": KEYS[1], "x": 0, "y": FLOOR, "z": 0}, {"kind": "key", "frame": KEYS[2], "x": 0, "y": TOP, "z": 0}]}),
        ("anim.function.preset.apply", {"nodeId": PEG, "channel": "velocity", "preset": "slowAcceleration", "frame": KEYS[0], "scope": "segment"}),
        ("anim.function.preset.apply", {"nodeId": PEG, "channel": "velocity", "preset": "slowDeceleration", "frame": KEYS[1], "scope": "segment"}),
    ]
    for cid, p in steps:
        await a.cmd(cid, p, record=True)


def heights(ev, peg_id):
    ys = {}
    for fr in ev.get("frames", []):
        for n in fr.get("nodes", []):
            if n.get("id") == peg_id:
                ys[int(fr["frame"])] = (n["world"]["position"]["x"], n["world"]["position"]["y"])
    return ys


async def run(binary, out, work):
    async def on_roots(context):
        return types.ListRootsResult(roots=[types.Root(uri=work.resolve().as_uri(), name="work"), types.Root(uri=out.resolve().as_uri(), name="out")])

    params = StdioServerParameters(command=binary, args=["--headless", "--mcp-stdio", "--user-dir", str(work / "user"), "--policy", "autonomous"])
    async with stdio_client(params) as (read, write_stream):
        async with ClientSession(read, write_stream, list_roots_callback=on_roots) as s:
            await s.initialize()
            await asyncio.sleep(0.5)
            a = Session(s)
            return await scenario(a, binary, out, work)


async def scenario(a, binary, out, work):
    projects = out / "projects"
    projects.mkdir(parents=True, exist_ok=True)
    pkg = projects / "E1.animpkg"

    # ---- Item 1: project, Ball, circle, paint, peg through registered commands; validate and read back.
    create = {"name": "E1", "path": str(pkg), "frameRate": FPS, "cameraWidth": W, "cameraHeight": H}
    await a.cmd("project.create", create)
    export = await a.cmd("doc.export.json", {"scope": "project", "binaries": "omit"})
    red_index = None
    for i, sw in enumerate(export["bundle"]["palettes"][0]["swatches"], start=1):
        val = next(iter(sw.get("values", {}).values()), {})
        if isinstance(val, dict) and str(val.get("rgba", "")).upper() == "#FF0000FF":
            red_index = red_index or i
    red = f"palette:#1/swatch:#{red_index}"  # a stable symbolic reference (11 §5), resolved in each project
    await build(a, red)
    issues = (await a.cmd("doc.validate", {})).get("issues", [])
    check("1 doc.validate: no errors", not [i for i in issues if i.get("severity") == "error"], issues[:3])
    proj = (await a.cmd("doc.object.get", {"ref": "project"}))["object"]
    scene = (await a.cmd("doc.object.get", {"ref": "scene:#1"}))["object"]
    ball = (await a.cmd("doc.object.get", {"ref": BALL}))["object"]
    peg = (await a.cmd("doc.object.get", {"ref": PEG}))["object"]
    cam = await a.cmd("anim.camera.get", {"cameraId": CAMERA})
    check("1 read back: 24 fps", proj.get("frameRate") == FPS, proj.get("frameRate"))
    check("1 read back: camera 1920x1080", (cam.get("resolution") or {}).get("w") == W and (cam.get("resolution") or {}).get("h") == H, cam.get("resolution"))
    check("1 read back: drawing element Ball, a child of peg Bounce", ball.get("elementType") == "drawing" and ball.get("name") == "Ball" and peg.get("elementType") == "peg", (ball.get("elementType"), peg.get("elementType")))
    tree = json.dumps(scene)
    check("1 read back: Ball under Bounce in the scene tree", ball["id"] in tree and peg["id"] in tree, "")
    swatch = (await a.cmd("doc.object.get", {"ref": red}))["object"]
    drawing = await a.cmd("draw.drawing.get", {"drawingId": f"{BALL}/drawing:#1"})
    painted = json.dumps(drawing)
    check("1 read back: the circle is drawn and painted with the red swatch", painted.count(swatch["colourId"]) >= 2, painted[:300])
    keys = await a.cmd("anim.key.list", {"nodeId": PEG})
    key_frames = sorted({k.get("frame") for k in keys.get("keys", []) if isinstance(k, dict)})
    check("1 read back: motion keyframes at frames 1, 12, 24", set(KEYS) <= set(key_frames), json.dumps(keys)[:300])

    # ---- Item 2: evaluated positions, extrema and holds, against the easing formulas.
    ev = await a.cmd("anim.evaluate", {"nodeIds": [PEG, BALL], "frames": list(range(1, LENGTH + 1))})
    write(out / "evaluate.json", canonical(ev))
    ys = heights(ev, peg["id"])
    err = max((max(abs(ys[f][1] - expected_y(f)), abs(ys[f][0] - expected_xy(f)[0])) for f in range(1, LENGTH + 1) if f in ys), default=math.inf)
    check("2 evaluated positions follow the easing presets along the sampled path of 05 §9.2 (|error| <= 1e-9 field)", len(ys) == LENGTH and err <= 1e-9, f"max error {err:.3e}")
    sampling = max(abs(expected_y(f) - ideal_y(f)) for f in range(1, LENGTH + 1))
    check("2 the 64-sample arc length keeps the ball within 1e-3 field of the exact eased distance", sampling <= 1e-3, f"{sampling:.3e}")
    y = {f: ys[f][1] for f in ys}
    check("2 extrema: the floor at frame 12, the top at the keys 1 and 24", len(y) == LENGTH and min(y, key=y.get) == 12 and abs(y[12] - FLOOR) < 1e-9 and abs(y[1] - TOP) < 1e-9 and abs(y[24] - TOP) < 1e-9, {k: y.get(k) for k in KEYS})
    check("2 hold after the last key: frames 25-30 at the top", all(abs(y.get(f, 0) - TOP) < 1e-9 for f in range(25, LENGTH + 1)), "")
    check("2 the ball falls to frame 12 and rises to frame 24", all(y[f] > y[f + 1] for f in range(1, 12)) and all(y[f] < y[f + 1] for f in range(12, 24)), "")
    check("2 explicit easing: slow start, fastest at the floor (bounce)", (y[1] - y[2]) < (y[10] - y[11]) and (y[13] - y[12]) > (y[24] - y[23]), "")

    # ---- Item 5 (and 3, process A): the 24-frame sequence with its manifest into the delivery folder.
    seq = out / "sequence"
    started = await a.cmd("render.range.run", {"from": 1, "to": 24, "directory": str(seq), "createDirs": True})
    rec = await a.wait(started["jobId"])
    check("5 render.range.run succeeded", rec.get("state") == "succeeded", json.dumps(rec)[:300])
    manifest = rec.get("result") or {}
    names = sorted(p.name for p in seq.iterdir())
    want = sorted([f"Scene-1_{f:04d}.png" for f in range(1, 25)] + ["Scene-1_0001-0024.manifest.json"])
    check("5 the delivery folder holds 24 frames and the manifest, nothing else", names == want, names[:4])
    files = manifest.get("files", [])
    check("5 manifest: 24 frame files in frame order", [f.get("frame") for f in files] == list(range(1, 25)) and [Path(f["path"]).name for f in files] == [f"Scene-1_{f:04d}.png" for f in range(1, 25)], [f.get("frame") for f in files][:5])
    check("5 manifest: count, size, fps, duration, range", manifest.get("frameCount") == 24 and manifest.get("width") == W and manifest.get("height") == H and manifest.get("fps") == FPS and abs(manifest.get("durationSec", 0) - 1.0) < 1e-12 and manifest.get("range", {}).get("from") == 1 and manifest.get("range", {}).get("to") == 24, {k: manifest.get(k) for k in ("frameCount", "width", "height", "fps", "durationSec", "range")})
    on_disk = json.loads((seq / "Scene-1_0001-0024.manifest.json").read_text(encoding="utf-8"))
    check("5 the published manifest is the job's result", on_disk.get("files") == files, "")
    seq_png = {}
    ok_hash = ok_dims = True
    for f in files:
        b = Path(f["path"]).read_bytes()
        seq_png[f["frame"]] = b
        hashes[f"sequence/{Path(f['path']).name}"] = sha(b)
        ok_hash &= sha(b) == f["sha256"] and len(b) == f["bytes"]
        ok_dims &= int.from_bytes(b[16:20], "big") == W and int.from_bytes(b[20:24], "big") == H and f.get("width") == W and f.get("height") == H
    check("5 every frame's SHA-256 and byte count match the manifest", ok_hash, "")
    check("5 every frame is 1920x1080 (PNG header and manifest)", ok_dims, "")
    write(out / "manifest.normalized.json", normalized_manifest(manifest))

    # ---- Item 2/3: rendered centres measured from the PNG pixels; single renders equal the sequence frames.
    _, _, rows = png_pixels(seq_png[1])
    bg = tuple(rows[0][0:3])
    worst, areas = 0.0, []
    for f in range(1, 25):
        cx, cy, area, _ = ball_centre(seq_png[f], (255, 0, 0), bg)
        ex, ey = to_pixels(0.0, expected_y(f))
        worst = max(worst, abs(cx - ex), abs(cy - ey)) if cx is not None else math.inf
        areas.append(area)
    check("2 rendered ball centres (pixel centroids) match the eased positions within 0.05 px", worst <= 0.05, f"max error {worst:.4f} px")
    r_px = math.sqrt(sum(areas) / len(areas) / math.pi)
    check("2 the ball is a disc of the drawn radius (0.5 field = 80 px, plus half the line), the same area in every frame", 80 <= r_px <= 90 and max(areas) - min(areas) < 1.0, f"radius {r_px:.3f} px, area spread {max(areas) - min(areas):.4f}")
    for f in KEYS:
        r = await a.cmd("render.frame.run", {"frame": f})
        png = frame_png(r)
        write(out / "frames" / f"frame-{f:02d}.png", png)
        hashes[f"frames/frame-{f:02d}.png"] = sha(png)
        check(f"3 render.frame.run frame {f} equals the sequence frame byte for byte", png == seq_png[f] and r.get("sha256") == sha(png), "")

    # ---- Item 4: the labelled contact sheet; each cell is the frame rendered at the cell size.
    sheet = await a.cmd("inspect.contactSheet.render", {"frames": {"from": 1, "to": 24, "step": 1}, "cellWidth": 240})
    sheet_png = frame_png(sheet)
    write(out / "contact-sheet.png", sheet_png)
    hashes["contact-sheet.png"] = sha(sheet_png)
    sw_, sh_, srows = png_pixels(sheet_png)
    cols, cw, chh, gap, band = math.ceil(math.sqrt(24)), 240, 240 * H // W, 4, 14
    rows_n = math.ceil(24 / cols)
    cells = sheet.get("cells", [])
    expect_cells = [{"frame": f, "x": gap + (i % cols) * (cw + gap), "y": gap + (i // cols) * (chh + band + gap), "w": cw, "h": chh} for i, f in enumerate(range(1, 25))]
    got_cells = [{k: c.get(k) for k in ("frame", "x", "y", "w", "h")} for c in cells]
    check("4 contact sheet: 5 columns, 24 cells, layout of SI-108", got_cells == expect_cells, (got_cells[:2], expect_cells[:2]))
    check("4 contact sheet: dimensions", (sw_, sh_) == (cols * cw + (cols + 1) * gap, rows_n * (chh + band) + (rows_n + 1) * gap), (sw_, sh_))
    same, labelled, label_bands = True, True, set()
    for c in expect_cells:
        single = await a.cmd("render.frame.run", {"frame": c["frame"], "width": cw})
        _, _, crows = png_pixels(frame_png(single))
        for yy in range(chh):
            if srows[c["y"] + yy][c["x"] * 4:(c["x"] + cw) * 4] != crows[yy]:
                same = False
                break
        band_px = b"".join(srows[c["y"] + chh + yy][c["x"] * 4:(c["x"] + cw) * 4] for yy in range(band))
        labelled &= any(band_px[i] < 128 for i in range(0, len(band_px), 4))
        label_bands.add(sha(band_px))
    check("4 every cell equals its frame rendered at the cell size, pixel for pixel", same, "")
    check("4 every cell has a frame label, and the 24 labels differ", labelled and len(label_bands) == 24, len(label_bands))
    write(out / "contact-sheet.json", json.dumps({"cells": got_cells, "width": sw_, "height": sh_}, indent=1) + "\n")

    # ---- Item 6: delivery conflicts, a late read-only target, sidecar-owned recovery, cancellation.
    trial = work / "trial"
    shutil.copytree(seq, trial)
    before = {p.name: p.read_bytes() for p in trial.iterdir()}
    r = await a.call("render.range.run", {"from": 1, "to": 24, "directory": str(trial)})
    check("6 onConflict fail (default): export.fileExists, nothing written", (r.get("error") or {}).get("code") == "export.fileExists" and {p.name: p.read_bytes() for p in trial.iterdir()} == before, (r.get("error") or {}).get("code"))
    started = await a.cmd("render.range.run", {"from": 1, "to": 24, "step": 12, "directory": str(trial), "onConflict": "rename"})
    rec = await a.wait(started["jobId"])
    renamed = sorted(set(p.name for p in trial.iterdir()) - set(before))
    check("6 onConflict rename: new names, originals untouched", rec.get("state") == "succeeded" and len(renamed) == 3 and all((trial / n).read_bytes() == b for n, b in before.items()), renamed)
    # A late read-only target: the publication fails, the originals stay byte for byte, no manifest is published.
    late = work / "late"
    shutil.copytree(seq, late)
    (late / "Scene-1_0001-0024.manifest.json").unlink()
    before = {p.name: p.read_bytes() for p in late.iterdir()}
    os.chmod(late / "Scene-1_0024.png", stat.S_IREAD)
    started = await a.cmd("render.range.run", {"from": 1, "to": 24, "directory": str(late), "onConflict": "overwrite"})
    rec = await a.wait(started["jobId"])
    os.chmod(late / "Scene-1_0024.png", stat.S_IREAD | stat.S_IWRITE)
    after = {p.name: p.read_bytes() for p in late.iterdir()}
    check("6 a late read-only target: failed, export.targetInvalid", rec.get("state") == "failed" and (rec.get("error") or {}).get("code") == "export.targetInvalid", json.dumps(rec.get("error"))[:200])
    check("6 the failed delivery keeps every original byte for byte and publishes no manifest", after == before, sorted(set(after) ^ set(before)))
    failed_job = started["jobId"]
    # Sidecar-owned recovery: a dead job's listed staging is recovered and reported; an unowned look-alike is kept.
    rec_dir = work / "recovery"
    rec_dir.mkdir()
    # What a job of a process that has exited leaves (ADR-0020 decision 11): the host name as the product reads it, the
    # ID of a child process that has finished.
    host = (os.environ.get("COMPUTERNAME") or os.environ.get("HOSTNAME") or "").strip()
    if not host:
        host = subprocess.run(["hostname"], capture_output=True, text=True).stdout.strip() or "unknown"
    gone = subprocess.Popen([sys.executable, "-c", "pass"])
    gone.wait()
    token = "e1e1e1e1e1e1"
    sidecar = {"kind": "toontown.render.range.staging", "version": 1, "token": token, "jobId": "job-dead", "head": "unknown", "productVersion": "0.0.1", "host": host, "pid": gone.pid, "published": False, "targets": ["old.png"], "staged": [f".old.png.tt-partial-{token}"], "backups": [f".old.png.tt-backup-{token}"]}
    write(rec_dir / f".tt-partial-{token}.job", json.dumps(sidecar))
    write(rec_dir / f".old.png.tt-backup-{token}", b"original old.png")
    write(rec_dir / ".notes.tt-partial-draft", b"user file")
    started = await a.cmd("render.range.run", {"from": 1, "to": 1, "directory": str(rec_dir)})
    rec = await a.wait(started["jobId"])
    warn = [w for w in (rec.get("result") or {}).get("warnings", []) if w.get("code") == "render.staleStagingRemoved"]
    check("6 a dead job's sidecar-listed backup is restored and reported", bool(warn) and (rec_dir / "old.png").read_bytes() == b"original old.png" and warn[0].get("restored") == ["old.png"], json.dumps(rec.get("result", {}).get("warnings"))[:300])
    check("6 an unowned look-alike is kept and not reported", (rec_dir / ".notes.tt-partial-draft").read_bytes() == b"user file" and ".notes" not in json.dumps(warn), "")
    # Cancellation: nothing is published; the record keeps the state, the typed error and the receipt.
    cancel_dir = work / "cancel"
    started = await a.cmd("render.range.run", {"from": 1, "to": 30, "scale": 2, "directory": str(cancel_dir), "createDirs": True})
    await a.cmd("job.cancel", {"jobId": started["jobId"]})
    rec = await a.wait(started["jobId"])
    cancelled_job = started["jobId"]
    check("6 a cancelled export publishes nothing", rec.get("state") == "cancelled" and (not cancel_dir.exists() or not any(cancel_dir.iterdir())), rec.get("state"))

    # ---- Item 7: cancelled and failed records through list, wait and MCP job_status.
    async def observed(job, state, code):
        views = [await a.record_of(job), await a.wait(job, 1000)]
        listed = await a.cmd("job.list", {"jobId": job})
        listed = listed if isinstance(listed, list) else listed.get("jobs", [listed])
        views.append(listed[0] if listed else {})
        return all(v.get("state") == state and (v.get("error") or {}).get("code") == code and any(w.get("code") == "render.stagingDiscarded" for w in v.get("warnings", [])) for v in views), views

    ok_c, v_c = await observed(cancelled_job, "cancelled", "job.cancelled")
    check("7 cancelled record: state, job.cancelled and the discard receipt through job_status, job.wait and job.list", ok_c, json.dumps(v_c[0])[:300])
    ok_f, v_f = await observed(failed_job, "failed", "export.targetInvalid")
    check("7 failed record: state, export.targetInvalid and the discard receipt through job_status, job.wait and job.list", ok_f, json.dumps(v_f[0])[:300])

    # ---- Item 8: describe / hit test / convert at frame 12 against the analytic geometry and the measured centre.
    cx12, cy12, _, _ = ball_centre(seq_png[12], (255, 0, 0), bg)
    d = await a.tool("describe_frame", {"frame": 12})
    el = next((e for e in d.get("elements", []) if e.get("name") == "Ball"), {})
    wpos = (el.get("worldTransform") or {}).get("position")
    check("8 describe_frame: Ball at the floor (world position)", wpos is not None and abs(wpos[0]) < 1e-9 and abs(wpos[1] - FLOOR) < 1e-9, wpos)
    hits = await a.tool("hit_test", {"space": "image", "point": {"x": cx12, "y": cy12}, "frame": 12})
    top = (hits.get("hits") or [{}])[0]
    check("8 hit_test at the measured centre: Ball is the top hit", top.get("elementId") == ball["id"], json.dumps(hits)[:200])
    miss = await a.tool("hit_test", {"space": "image", "point": {"x": cx12 + 120, "y": cy12}, "frame": 12})
    check("8 hit_test beside the ball: no hit on Ball", all(h.get("elementId") != ball["id"] for h in miss.get("hits", [])), json.dumps(miss)[:200])
    conv = await a.tool("convert_coordinates", {"from": "image", "to": "scene", "points": [{"x": cx12, "y": cy12}], "frame": 12})
    pt = (conv.get("points") or [{}])[0]
    check("8 convert_coordinates: the measured centre maps to the ball's world position (<= 0.001 field)", abs(pt.get("x", 9)) <= 1e-3 and abs(pt.get("y", 9) - FLOOR) <= 1e-3, pt)

    # ---- Item 10: undo / redo restore the document and the renders; anim.changed is covered by builder tests.
    before_doc = canonical((await a.cmd("doc.export.json", {"scope": "project", "binaries": "omit"}))["bundle"])
    await a.tool("undo", {})
    undone = heights(await a.cmd("anim.evaluate", {"nodeIds": [PEG], "frames": [18]}), peg["id"])
    await a.tool("redo", {})
    redone = heights(await a.cmd("anim.evaluate", {"nodeIds": [PEG], "frames": [18]}), peg["id"])
    after_doc = canonical((await a.cmd("doc.export.json", {"scope": "project", "binaries": "omit"}))["bundle"])
    check("10 undo changes the eased segment, redo restores it exactly", abs(undone[18][1] - expected_y(18)) > 1e-3 and abs(redone[18][1] - expected_y(18)) <= 1e-6, (undone.get(18), redone.get(18)))
    check("10 undo then redo: identical canonical document", before_doc == after_doc, "")
    if os.environ.get("E1_DEBUG"):
        write(work.parent / "e1-debug-before.json", before_doc)
        write(work.parent / "e1-debug-after.json", after_doc)

    # ---- Item 10/3: save, close, reopen in the same session; the same document and the same bytes.
    r = await a.cmd("project.save", {})
    if isinstance(r, dict) and r.get("jobId"):
        await a.wait(r["jobId"])
    saved = canonical((await a.cmd("doc.export.json", {"scope": "project", "binaries": "omit"}))["bundle"])
    await a.cmd("project.close", {"onUnsaved": "fail"})
    r = await a.cmd("project.open", {"path": str(pkg)})
    if isinstance(r, dict) and r.get("jobId"):
        await a.wait(r["jobId"])
    reopened = canonical((await a.cmd("doc.export.json", {"scope": "project", "binaries": "omit"}))["bundle"])
    check("10 save, close, reopen: identical canonical document", saved == reopened, "")
    same = True
    for f in (1, 6, 12, 18, 24):
        same &= frame_png(await a.cmd("render.frame.run", {"frame": f})) == seq_png[f]
    check("3 after reopening, frames 1, 6, 12, 18, 24 re-render identical bytes", same, "")
    ok_c, _ = await observed(cancelled_job, "cancelled", "job.cancelled")
    check("7 the cancelled record survives save and reopen in the same session", ok_c, "")

    # ---- Item 3: a parent-transform / camera-roll variant at the default FOV, not saved.
    variant = [
        ("element.create", {"type": "peg", "name": "Rig", "sceneId": "scene:#1"}),
        ("element.parent.set", {"elementIds": [PEG], "parentId": RIG}),
        ("anim.transform.set", {"nodeIds": [RIG], "target": "static", "rotation": 30, "scale": {"x": 0.8, "y": 0.8}}),
        ("anim.camera.set", {"cameraId": CAMERA, "roll": 15}),
    ]
    for cid, p in variant:
        await a.cmd(cid, p)
    cam = await a.cmd("anim.camera.get", {"cameraId": CAMERA})
    check("3 variant: the camera keeps the default FOV (6 / tan(FOV/2) = 16, no alias)", abs(6 / math.tan(math.radians(cam.get("effectiveFov", cam.get("fov", 0))) / 2) - 16) < 1e-9, cam.get("effectiveFov"))
    worst = 0.0
    for f in KEYS + (6, 18):
        png = frame_png(await a.cmd("render.frame.run", {"frame": f}))
        if f in KEYS:
            write(out / "frames" / f"variant-{f:02d}.png", png)
            hashes[f"frames/variant-{f:02d}.png"] = sha(png)
        cx, cy, _, _ = ball_centre(png, (255, 0, 0), bg)
        ex, ey = to_pixels(0.0, expected_y(f), rot_deg=30, scale=0.8, roll_deg=15)
        worst = max(worst, abs(cx - ex), abs(cy - ey)) if cx is not None else math.inf
    check("3 variant: rendered centres match the rotated, scaled, rolled positions within 0.05 px", worst <= 0.05, f"max error {worst:.4f} px")

    write(out / "e1-commands.json", json.dumps({"label": "E1 bouncing ball (recorded command script)", "steps": [{"id": "project.create", "params": {k: v for k, v in create.items() if k != "path"}}] + a.script}, indent=1) + "\n")
    return {"seq": seq_png, "saved": saved, "cancelled_job": cancelled_job, "create": create}


async def replay(binary, out, work, state):
    """Process B: the recorded script with `--run` into a fresh project; the same canonical content and bytes."""
    recorded = json.loads((out / "e1-commands.json").read_text(encoding="utf-8"))["steps"]
    pkg = out / "projects" / "E1-replay.animpkg"
    seq = work / "replay-sequence"
    steps = [{"id": "project.create", "params": dict(recorded[0]["params"], path=str(pkg))}] + recorded[1:]
    steps += [
        {"id": "render.range.run", "params": {"from": 1, "to": 24, "directory": str(seq), "createDirs": True, "wait": True}},
        {"id": "project.save", "params": {}},
        {"id": "doc.export.json", "params": {"scope": "project", "binaries": "omit"}},
        {"id": "job.list", "params": {"jobId": state["cancelled_job"]}},
    ]
    batch = work / "replay.json"
    write(batch, json.dumps({"label": "E1 replay", "atomic": False, "stopOnError": True, "steps": steps}))
    log = work / "replay.jsonl"
    p = subprocess.run([binary, "--headless", "--user-dir", str(work / "user-b"), "--allow-path", str(out), "--allow-path", str(work), "--run", str(batch), "--out", str(log)], capture_output=True, text=True, timeout=600)
    check("3 process B (--run replay into a fresh project) exits 0", p.returncode == 0, (p.stderr or p.stdout)[-400:])
    env = {}
    for line in log.read_text(encoding="utf-8").splitlines():
        e = json.loads(line)
        if e.get("record") == "step":
            env[e.get("index")] = e.get("envelope") or {}
    n = len(steps)
    export = (env.get(n - 2) or {}).get("result") or {}
    replayed = canonical(export.get("bundle", {}))
    write(work / "canonical-a.json", state["saved"])
    write(work / "canonical-b.json", replayed)
    check("3 the replayed project's canonical content equals the original's (stable symbolic references)", replayed == state["saved"], "")
    same = all((seq / f"Scene-1_{f:04d}.png").read_bytes() == b for f, b in state["seq"].items()) if seq.exists() else False
    check("3 process B renders the 24 frames with identical bytes and SHA-256", same, "")
    jobs = (env.get(n - 1) or {}).get("result")
    check("7 a fresh session does not see the first session's job records", jobs in ([], None) or (env.get(n - 1) or {}).get("status") == "error", json.dumps(env.get(n - 1))[:200])


hashes = {}


async def run_all(binary, out, work):
    state = await run(binary, out, work)
    if state:
        await replay(binary, out, work, state)
    write(out / "hashes.json", json.dumps(hashes, indent=1, sort_keys=True) + "\n")


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    binary, out = str(Path(sys.argv[1]).resolve()), Path(sys.argv[2]).resolve()
    out.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="tt-e1-"))
    t = time.time()
    try:
        asyncio.run(run_all(binary, out, work))
    except Exception as e:  # noqa: BLE001 - every failure is a failed check in the report
        import traceback

        check("scenario ran to the end", False, repr(e) + " " + traceback.format_exc()[-400:])
    failed = [c for c in CHECKS if not c["ok"]]
    sha_head = os.environ.get("GITHUB_SHA")
    if not sha_head:
        try:
            sha_head = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
        except (OSError, subprocess.CalledProcessError):
            sha_head = "unknown"
    report = {"seconds": round(time.time() - t, 3), "passed": len(CHECKS) - len(failed), "failed": len(failed), "checks": CHECKS, "headSha": sha_head, "platform": platform.platform(), "python": platform.python_version(), "mcp": importlib.metadata.version("mcp")}
    (out / "report.json").write_text(json.dumps(report, indent=1) + "\n", encoding="utf-8", newline="\n")
    if os.environ.get("E1_DEBUG"):
        print(f"work folder kept: {work}")
    else:
        shutil.rmtree(work, ignore_errors=True)
    print(f"{len(CHECKS) - len(failed)}/{len(CHECKS)} checks passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
