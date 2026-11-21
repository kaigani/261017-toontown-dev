"""M4 C4.4b: each new tool's live preview against its committed state, within 1 view pixel (PV2; design note 3 §8,
design note 4 §9 "preview against commit"; SI-154, SI-190).

Usage: python scripts/c44b-preview-e2e.py <toontown binary> <out dir> [--backend webgl2|webgpu]
       python scripts/c44b-preview-e2e.py --recheck <out dir>   (the distances again from a run's captures)

Windows only (real OS input held at the drag's end; a workstation receipt, both back ends by `--backend`). In the
Drawing view and the Camera view, on a 120 x 120 du rectangle of Bob's drawing (30 to 150 by -60 to 60 du):

- Contour Editor (an anchor drag; two selected anchors dragged together): while the drag is held, the canvas summary's
  `contourPreview.anchors` (the changed anchors in view pixels); after the release, once the committed revision is
  drawn, the committed anchors through the view's drawing mapping. Every previewed anchor within 1 view pixel of its
  committed place.
- Perspective (a keystone: the top corners 24 du inwards, projective; a concave corner, bilinear) and Reposition All (a
  rotation; an alternate corner): capture A (`inspect.screenshot {target: view}`) while held, 0.6 s after the last
  sample; capture B after the release, once the summary shows the committed revision drawn. The fixture's ink is the
  box's own rectangle, so the held overlay's quadrilateral lies on the mapped ink: A's mask is every pixel darker than
  the background; B's is the dark unsaturated pixels (the black ink). What covers ink is left out: the eight held
  handle squares (6 view px around each, from the summary's `toolPreview`), the selection box the Drawing view keeps
  under the drag (the rectangle through the view's mapping), B's accent (the committed selection box)
  and the capture's 3 px edge (the panel border); 2 view px around it from the set measured to, 6 from the set
  measured from (a point beside a covered stretch would otherwise measure to the stretch's far end). The symmetric Hausdorff distance of the two masks, divided by the
  device-pixel ratio, is at most 1 view pixel. The receipt keeps the distance, the map's kind and
  the captures' hashes.

Receipts: `rows.json`, `transcript.json`, `manifest.json` (kind `toontown.c44b.preview-e2e`). Exit 0 when every row
holds.
"""

from __future__ import annotations

import asyncio
import contextlib
import hashlib
import importlib.util
import io
import json
import sys
import tempfile
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


e6 = _load("c43e6", "c43-e6-coexistence.py")
e2e, c42 = e6.e2e, e6.c42
rc = e2e.run_cmd

from mcp import ClientSession  # noqa: E402

#: The preview map each envelope row exercises (design note 4 §3).
EXPECT_KIND = {"Perspective keystone": "projective", "Perspective concave corner": "bilinear", "Reposition rotation": "affine"}
#: PV2's bound, view pixels.
BOUND_PX = 1.0
#: The ink: luminance below this (0-1). A 1 du line antialiases to mid greys; the overlay is told apart by its colour
#: (`overlay_mask`), not by luminance, so the threshold can be the background's half.
INK = 0.7
REQUESTED = {"backend": None}


#: The overlay's handle squares (design note 4 §2) are left out of the held capture: this far around each handle's
#: centre (view pixels).
HANDLE_GUARD_PX = 6
#: The capture's edge (view pixels) is left out: the panel's border is drawn there.
EDGE_PX = 3
#: The committed capture's overlay (the selection box, axis-aligned, accent) covers ink where it crosses it: ink this
#: close to it is left out of both captures (view pixels).
OVERLAY_GUARD_PX = 2
#: The set measured from leaves out this much more around what covers ink (view pixels): four times PV2's bound.
FROM_MARGIN_PX = 4


def rgb(png: bytes):
    import numpy as np
    from PIL import Image
    return np.asarray(Image.open(io.BytesIO(png)).convert("RGB"), dtype=np.float64) / 255.0


def accent_mask(im):
    """The overlay's accent (blue) pixels."""
    r, b = im[..., 0], im[..., 2]
    return (b > 0.6) & (b - r > 0.25)


def dilate(mask, k: int):
    import numpy as np
    out = mask.copy()
    for dy in range(-k, k + 1):
        for dx in range(-k, k + 1):
            out |= np.roll(np.roll(mask, dy, axis=0), dx, axis=1)
    return out


def lum(im):
    return 0.2126 * im[..., 0] + 0.7152 * im[..., 1] + 0.0722 * im[..., 2]


def points(mask):
    import numpy as np
    ys, xs = np.nonzero(mask)
    return np.stack([xs, ys], axis=1).astype(np.float64)


def directed(a, b) -> float:
    import numpy as np
    if len(a) == 0 or len(b) == 0:
        return float("inf")
    worst = 0.0
    for i in range(0, len(a), 2048):
        chunk = a[i:i + 2048]
        d2 = ((chunk[:, None, :] - b[None, :, :]) ** 2).sum(axis=2)
        worst = max(worst, float(np.sqrt(d2.min(axis=1)).max()))
    return worst


def map_point(pm: dict, x: float, y: float):
    """A preview map (the canvas summary's `toolPreview`, drawing units) applied to a point, as `mapPoint`."""
    k = pm.get("kind")
    if k == "affine":
        m = pm["m"]
        return (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
    if k == "projective":
        h = pm["h"]
        w = h[6] * x + h[7] * y + 1
        return ((h[0] * x + h[1] * y + h[2]) / w, (h[3] * x + h[4] * y + h[5]) / w)
    if k == "bilinear":
        (x0, y0), (x1, y1) = pm["min"], pm["max"]
        u, v = (x - x0) / (x1 - x0), (y - y0) / (y1 - y0)
        c0, c1, c2, c3 = pm["corners"]
        return tuple((1 - u) * (1 - v) * c0[i] + u * (1 - v) * c1[i] + u * v * c2[i] + (1 - u) * v * c3[i] for i in range(2))
    return (x, y)


def held_handles(box, pm: dict, d2v) -> list:
    """The held overlay's eight handle centres in view pixels: the box's corners and side midpoints through the map."""
    x0, y0, x1, y1 = box
    corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    mids = [((corners[i][0] + corners[(i + 1) % 4][0]) / 2, (corners[i][1] + corners[(i + 1) % 4][1]) / 2) for i in range(4)]
    return [c42.apply(d2v, *map_point(pm, x, y)) for x, y in corners + mids]


def kept_box(view: str, d2v):
    """The selection box the Drawing view keeps under a Perspective or Reposition All drag (view pixels), or None."""
    return [c42.apply(d2v, 30, -60), c42.apply(d2v, 150, 60)] if view == "drawingView" else None


def hausdorff(a_png: bytes, b_png: bytes, handles: list, scale: float, kept=None) -> tuple:
    """PV2's distance in capture pixels and the two point counts. The held capture (A): every pixel darker than the
    background (the ink, and the overlay's quadrilateral, which lies on the mapped rectangle's sides). The committed
    capture (B): the dark unsaturated pixels (the black ink; the overlay is accent and white). What covers ink in
    either capture (the held handle squares, the selection box the Drawing view keeps under the drag, the committed
    selection box, the view's edge) is left out of the set
    measured to (`OVERLAY_GUARD_PX` around it) and, wider by `FROM_MARGIN_PX`, of the set measured from: a point next
    to a covered stretch would otherwise measure to the stretch's far end."""
    import numpy as np
    ia, ib = rgb(a_png), rgb(b_png)
    if ia.shape != ib.shape:
        return float("inf"), 0, 0
    h, w = ia.shape[:2]
    bg_a = float(np.median(lum(ia)))
    yy, xx = np.mgrid[0:h, 0:w]
    cover = np.zeros((h, w), dtype=bool)
    r = HANDLE_GUARD_PX * scale
    for hx, hy in handles:
        cover |= (np.abs(xx - hx * scale) <= r) & (np.abs(yy - hy * scale) <= r)
    # The Drawing view keeps the selection's box under the drag (`kept`, view pixels): not ink, and it covers ink.
    if kept:
        (x0, y0), (x1, y1) = kept
        lo_x, hi_x, lo_y, hi_y = min(x0, x1) * scale, max(x0, x1) * scale, min(y0, y1) * scale, max(y0, y1) * scale
        inside_x, inside_y = (xx >= lo_x - 1) & (xx <= hi_x + 1), (yy >= lo_y - 1) & (yy <= hi_y + 1)
        cover |= inside_x & ((np.abs(yy - lo_y) <= 1) | (np.abs(yy - hi_y) <= 1))
        cover |= inside_y & ((np.abs(xx - lo_x) <= 1) | (np.abs(xx - hi_x) <= 1))
    cover |= accent_mask(ib)
    # The view's own edge (a panel border) is not ink.
    m = max(1, round(EDGE_PX * scale))
    cover[:m, :] = cover[-m:, :] = True
    cover[:, :m] = cover[:, -m:] = True
    near = dilate(cover, max(1, round(OVERLAY_GUARD_PX * scale)))
    wide = dilate(cover, max(1, round((OVERLAY_GUARD_PX + FROM_MARGIN_PX) * scale)))
    ink_a = lum(ia) < bg_a - 0.15
    ink_b = (lum(ib) < INK) & (np.abs(ib[..., 2] - ib[..., 0]) < 0.15)
    a_to, a_from = points(ink_a & ~near), points(ink_a & ~wide)
    b_to, b_from = points(ink_b & ~near), points(ink_b & ~wide)
    return max(directed(a_from, b_to), directed(b_from, a_to)), len(a_from), len(b_from)


async def summary(session, view: str) -> dict:
    res = (await e2e.tree(session)).get("result") or {}
    node = c42.find(res.get("root") or {}, lambda n: n.get("id") == f"{view}.canvas") or {}
    try:
        return json.loads(node.get("value") or "{}")
    except json.JSONDecodeError:
        return {}


async def session_run(endpoint: str, token: str, run, out: Path) -> list:
    rows: list = []
    async with contextlib.AsyncExitStack() as stack:
        client = await stack.enter_async_context(
            e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {token}"}, timeout=e2e.httpx2.Timeout(60.0, read=300.0)))
        streams = await stack.enter_async_context(e2e.streamable_http_client(endpoint, http_client=client))
        s = await stack.enter_async_context(ClientSession(streams[0], streams[1]))
        await s.initialize()
        await e2e.gui_tree(s, run, lambda r: e2e.by_id(r, "startScreen") is not None, "the attached GUI is up")

        async def res(cid, params=None):
            return (await rc(s, cid, params or {})).get("result") or {}

        await rc(s, "project.create", {"name": "PreviewE2E", "onUnsaved": "fail"})
        bob = ((await res("element.create", {"type": "drawing", "name": "Bob"})).get("elementIds") or [None])[0]
        await rc(s, "element.drawing.create", {"elementId": bob, "name": "1", "exposeFrom": 1, "exposeTo": 24})
        await rc(s, "app.context.set", {"elementId": bob, "frame": 1})
        # Black ink (the default palette's Colour 2): the captures' ink mask needs a line colour.
        found = await res("doc.object.find", {"type": "swatch", "name": "Colour 2"})
        items = found.get("items") or found.get("matches") or []
        black = next((it.get("id") or it.get("key") for it in items if isinstance(it, dict)), None)
        if black:
            await rc(s, "colour.current.set", {"colourId": black})
        # A thin line (design note 4 §9's fixture is an unpainted rectangle): a projective preview scales a line's width
        # locally while the commit keeps it constant (SI-190), which a thick line would measure.
        sq = await res("draw.rectangle.add", {"from": {"x": 30, "y": -60}, "to": {"x": 150, "y": 60}, "thickness": 1})
        sid = (sq.get("strokeIds") or [None])[0]
        for view in ("drawingView", "cameraView"):
            await rc(s, "window.panel.set", {"panelId": view, "visible": True})
        await asyncio.sleep(1.0)

        async def anchors():
            d = await res("draw.drawing.get", {"flatten": True})
            n = next((x for x in d.get("nodes") or [] if x.get("id") == sid), {}) or {}
            b = n.get("bezier") or []
            return [(b[k * 3]["x"], b[k * 3]["y"]) for k in range((len(b) + 2) // 3)]

        located: dict = {}

        async def locate(view: str):
            """The view's canvas in the window and a calibration: one Select click at its centre, whose recorded
            position corrects the located place (the capture match can be a few pixels off)."""
            if view in located:
                return located[view]
            place = None
            t0 = time.monotonic()
            while time.monotonic() - t0 < 30.0 and not place:
                place = await e6.locate_view(s, rc, view)
                if not place:
                    await asyncio.sleep(0.5)
            listed = await res("view.list", {})
            views = listed.get("views") or [] if isinstance(listed, dict) else listed
            logical = next((v.get("size") for v in views if v.get("viewId") == view), None) or {}
            ratio = place[2] / logical["width"] if place and logical.get("width") else 1.0
            offset = (0.0, 0.0)
            if place and logical.get("width"):
                await rc(s, "tool.active.set", {"toolId": "select"})
                await asyncio.sleep(0.4)
                want = (logical["width"] / 2, logical["height"] / 2)
                before = json.dumps((await summary(s, view)).get("lastGesture"))
                times: dict = {}
                c = (place[0] + want[0] * ratio, place[1] + want[1] * ratio)
                e6.os_drag(e2e.APP_PID["pid"], [c, c], 0.05, 0.05, times)
                t1 = time.monotonic()
                while time.monotonic() - t1 < 10.0:
                    rec = (await summary(s, view)).get("lastGesture")
                    if rec is not None and json.dumps(rec) != before and rec.get("events"):
                        offset = (rec["events"][0]["x"] - want[0], rec["events"][0]["y"] - want[1])
                        break
                    await asyncio.sleep(0.2)
            located[view] = (place, ratio, offset)
            run.check(place is not None, f"the {view} canvas is located and calibrated", {"place": place, "ratio": ratio, "offset": offset})
            return located[view]

        async def held(view: str, dpts: list, mods: list, capture: bool, out_px: float = 0.0) -> dict:
            m = (await res("draw.viewMapping.get", {"view": view})).get("drawingToView") or [1, 0, 0, 1, 0, 0]
            place, ratio, offset = located[view]
            vpts = [c42.apply(m, x, y) for x, y in dpts]
            # A press just outside a corner (the rotation zone), in view pixels whatever the view's scale.
            if out_px:
                vpts = [(vpts[0][0] + out_px, vpts[0][1] - out_px)] + [(x + out_px, y - out_px) for x, y in vpts[1:]]
            client = [(place[0] + (x - offset[0]) * ratio, place[1] + (y - offset[1]) * ratio) for x, y in vpts] if place else []
            out_: dict = {"place": place, "ratio": ratio}
            if not place:
                return out_
            rev0 = (await summary(s, view)).get("drawings")
            times: dict = {}
            vks = [{"alt": 0x12, "shift": 0x10, "ctrl": 0x11}[k] for k in mods]
            import ctypes
            for vk in vks:
                ctypes.windll.user32.keybd_event(vk, 0, 0, 0)
            th = threading.Thread(target=e6.os_drag, args=(e2e.APP_PID["pid"], client, 0.5, 1.6, times))
            th.start()
            t1 = time.monotonic()
            while th.is_alive() and "error" not in times:
                if "press" in times and time.monotonic() - times["press"] > 0.5 + 0.6 and "held" not in out_:
                    out_["held"] = await summary(s, view)
                    if capture:
                        out_["shotA"] = c42.png_of(await rc(s, "inspect.screenshot", {"target": view}))
                await asyncio.sleep(0.05)
                if time.monotonic() - t1 > 15:
                    break
            th.join()
            for vk in reversed(vks):
                ctypes.windll.user32.keybd_event(vk, 0, 2, 0)
            if 0x12 in vks:
                # A released Alt can leave Windows' menu mode on: Escape leaves it (the gesture has ended).
                time.sleep(0.1)
                c42.os_key(0x1B)
            # Committed and drawn: the summary's drawings at a new revision (a wait for the state, D-45).
            t2 = time.monotonic()
            while time.monotonic() - t2 < 15.0:
                now = await summary(s, view)
                if now.get("drawings") != rev0 and not now.get("contourPreview") and not now.get("toolPreview"):
                    break
                await asyncio.sleep(0.2)
            await asyncio.sleep(0.6)
            out_["after"] = await summary(s, view)
            out_["times"] = times
            out_["mapping"] = m
            if capture:
                out_["shotB"] = c42.png_of(await rc(s, "inspect.screenshot", {"target": view}))
            return out_

        async def setup(view, tool, points=()):
            await rc(s, "view.active.set", {"viewId": view})
            await rc(s, "view.transform.reset", {"viewId": view})
            if view == "drawingView":
                # The Drawing view's canvas is short: the rectangle centred at three quarters' zoom keeps every drag's
                # ink inside the capture.
                await rc(s, "view.zoom.set", {"viewId": view, "zoom": 0.75})
                await rc(s, "view.pan.set", {"viewId": view, "centre": {"x": 90, "y": 0}})
            await asyncio.sleep(0.5)
            await locate(view)
            await rc(s, "app.context.set", {"elementId": bob, "frame": 1})
            await rc(s, "tool.snapToContour.set", {"enabled": False})
            await rc(s, "draw.selection.set", {"by": "ids", "ids": [sid], "mode": "replace"})
            await rc(s, "draw.pointSelection.set", {"points": [{"strokeId": sid, "anchor": a} for a in points]})
            await rc(s, "tool.active.set", {"toolId": tool})
            await asyncio.sleep(0.8)

        for view in ("drawingView", "cameraView"):
            # Contour Editor: the previewed anchors against the committed ones.
            for name, pts, sel in (("anchor drag", [(30, -60), (40, -55), (50, -50)], ()),
                                   ("two selected anchors", [(150, -60), (160, -50), (165, -45)], (1, 2))):
                await setup(view, "contourEditor", sel)
                h = await held(view, pts, [], False)
                prev = ((h.get("held") or {}).get("contourPreview") or {}).get("anchors") or []
                m = h.get("mapping") or [1, 0, 0, 1, 0, 0]
                committed = [c42.apply(m, x, y) for x, y in await anchors()]
                worst = max((min(((p[0] - q[0]) ** 2 + (p[1] - q[1]) ** 2) ** 0.5 for q in committed) for p in prev), default=None)
                row = {"name": f"Contour Editor › {name} ({view})", "previewed": prev, "committed": committed, "worstPx": worst,
                       "times": h.get("times")}
                run.check(bool(prev) and worst is not None and worst <= BOUND_PX,
                          f"PV2 {row['name']}: every previewed anchor within {BOUND_PX} view px of its committed place", row)
                rows.append(row)
                await rc(s, "edit.undo", {})
            # Perspective and Reposition All: captures while held and after the commit.
            for name, tool, pts, mods in (
                    ("Perspective keystone", "perspective", [(90, 60), (78, 60), (66, 60)], ["alt"]),
                    ("Perspective concave corner", "perspective", [(150, 60), (110, 35), (60, 10)], []),
                    ("Reposition rotation", "repositionAll", [(150, 60), (135, 75), (115, 85)], ["out"]),
                    ("Reposition alternate corner", "repositionAll", [(150, 60), (165, 75), (180, 90)], ["alt"])):
                await setup(view, tool)
                h = await held(view, pts, [k for k in mods if k != "out"], True, 10.0 if "out" in mods else 0.0)
                a, b = h.get("shotA"), h.get("shotB")
                pm = (h.get("held") or {}).get("toolPreview") or {}
                kind = pm.get("kind")
                scale = h.get("ratio") or 1.0
                handles = held_handles((30, -60, 150, 60), pm, h.get("mapping") or [1, 0, 0, 1, 0, 0]) if kind else []
                kept = kept_box(view, h.get("mapping") or [1, 0, 0, 1, 0, 0])
                dist, na, nb = hausdorff(a, b, handles, scale, kept) if a and b and kind else (None, 0, 0)
                dist = dist / scale if dist is not None else None
                row = {"name": f"{name} ({view})", "mapKind": kind, "map": pm, "mapping": h.get("mapping"), "scale": scale,
                       "handles": handles, "hausdorffPx": dist, "inkPoints": [na, nb],
                       "captureA": hashlib.sha256(a).hexdigest() if a else None,
                       "captureB": hashlib.sha256(b).hexdigest() if b else None, "times": h.get("times")}
                if a and b:
                    (out / f"{name.replace(' ', '-')}-{view}-held.png").write_bytes(a)
                    (out / f"{name.replace(' ', '-')}-{view}-committed.png").write_bytes(b)
                # The map the row is for (design note 4 §3): a keystone is projective, a concave corner bilinear.
                want_kind = EXPECT_KIND.get(name)
                row["expectedKind"] = want_kind
                run.check(want_kind is None or kind == want_kind, f"PV2 {row['name']}: the preview's map is {want_kind}", row)
                run.check(dist is not None and dist <= BOUND_PX,
                          f"PV2 {row['name']}: the held preview and the committed drawing within {BOUND_PX} view px (Hausdorff)", row)
                rows.append(row)
                await rc(s, "edit.undo", {})
            run.save("rows.json", rows)
    return rows


def recheck(out: Path) -> int:
    """`--recheck <out dir>`: the distances again from a run's saved captures and rows (no app)."""
    ok = True
    for row in json.loads((out / "rows.json").read_text(encoding="utf-8")):
        if "map" not in row:
            continue
        stem = row["name"].rsplit(" (", 1)[0].replace(" ", "-")
        view = row["name"].rsplit(" (", 1)[1].rstrip(")")
        a = (out / f"{stem}-{view}-held.png").read_bytes()
        b = (out / f"{stem}-{view}-committed.png").read_bytes()
        handles = held_handles((30, -60, 150, 60), row["map"], row["mapping"])
        d, na, nb = hausdorff(a, b, handles, row["scale"], kept_box(view, row["mapping"]))
        print(f"{row['name']}: {d / row['scale']:.3f} view px ({na}, {nb} ink points)")
        ok = ok and d / row["scale"] <= BOUND_PX
    return 0 if ok else 1


def main() -> int:
    args = sys.argv[1:]
    if len(args) == 2 and args[0] == "--recheck":
        return recheck(Path(args[1]))
    e6.dpi_aware()
    backend = None
    if "--backend" in args:
        i = args.index("--backend")
        backend = args[i + 1] if i + 1 < len(args) else None
        del args[i:i + 2]
    if len(args) != 2 or backend not in (None, "webgl2", "webgpu"):
        print(__doc__)
        return 2
    app, out = args[0], Path(args[1])
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    rows: list = []
    started = time.monotonic()
    if sys.platform != "win32":
        run.check(True, "real OS input held at a drag's end is a Windows workstation receipt (named in the handoff)", None)
    else:
        user = Path(tempfile.mkdtemp(prefix="tt-c44b-preview-"))
        (user / "Settings").mkdir(parents=True)
        seed = {"agent.policy.defaults": {"policy": "autonomous", "maxJobs": 2, "maxBatchSteps": 1000, "maxInlineImagePx": 4096,
                                          "maxInlineBytes": 52428800, "askInApp": True, "letAgentMoveView": True, "highlightAgentChanges": True}}
        (user / "Settings" / "preferences.json").write_text(json.dumps({"format": "toontown-settings", "kind": "preferences", "formatVersion": "1.0", "data": seed}), encoding="utf-8")
        discovery = out / "discovery.json"
        try:
            import subprocess
            extra = ["--canvas-backend", backend] if backend else []
            log = open(discovery.with_suffix(".log"), "ab")
            proc = subprocess.Popen([app, "--user-dir", str(user), "--mcp-http", "127.0.0.1:0", "--mcp-discovery", str(discovery), *extra],
                                    stdout=log, stderr=subprocess.STDOUT)
            e2e.APP_PID["pid"] = proc.pid
            try:
                d = e2e.wait_discovery(discovery, proc)
                rows = asyncio.run(session_run(d["endpoint"], d["token"], run, out))
            finally:
                e2e.stop(proc)
        except Exception as ex:  # noqa: BLE001 - reported as a failure with its text
            run.check(False, "the comparison ran", repr(ex))
    manifest = {"kind": "toontown.c44b.preview-e2e", "platform": sys.platform, "backend": backend,
                "seconds": round(time.monotonic() - started, 1), "rows": len(rows), "failures": run.failures}
    run.save("rows.json", rows)
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps(manifest, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
