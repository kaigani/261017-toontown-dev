"""M4 C4.4b: each new tool's live preview against its committed state, within 1 view pixel (PV2; design note 3 §8,
design note 4 §9 "preview against commit"; SI-154, SI-190).

Usage: python scripts/c44b-preview-e2e.py <toontown binary> <out dir> [--backend webgl2|webgpu]

Windows only (real OS input held at the drag's end; a workstation receipt, both back ends by `--backend`). In the
Drawing view and the Camera view, on a 120 x 120 du rectangle of Bob's drawing (30 to 150 by -60 to 60 du):

- Contour Editor (an anchor drag; two selected anchors dragged together): while the drag is held, the canvas summary's
  `contourPreview.anchors` (the changed anchors in view pixels); after the release, once the committed revision is
  drawn, the committed anchors through the view's drawing mapping. Every previewed anchor within 1 view pixel of its
  committed place.
- Perspective (a keystone: the top corners 24 du inwards, projective; a concave corner, bilinear) and Reposition All (a
  rotation; an alternate corner): capture A (`inspect.screenshot {target: view}`) while held, 0.6 s after the last
  sample; capture B after the release, once the summary shows the committed revision drawn. Ink mask: pixels with
  luminance below 0.35 (the overlay's accent and white handles are lighter). The symmetric Hausdorff distance of the two
  masks, divided by the device-pixel ratio, is at most 1 view pixel. The receipt keeps the distance, the map's kind and
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

#: PV2's bound, view pixels.
BOUND_PX = 1.0
#: The ink: luminance below this (0-1).
INK = 0.35
REQUESTED = {"backend": None}


def mask_points(png: bytes):
    import numpy as np
    from PIL import Image
    im = np.asarray(Image.open(io.BytesIO(png)).convert("RGB"), dtype=np.float64) / 255.0
    lum = 0.2126 * im[..., 0] + 0.7152 * im[..., 1] + 0.0722 * im[..., 2]
    ys, xs = np.nonzero(lum < INK)
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


def hausdorff(a_png: bytes, b_png: bytes) -> float:
    a, b = mask_points(a_png), mask_points(b_png)
    return max(directed(a, b), directed(b, a))


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
        sq = await res("draw.rectangle.add", {"from": {"x": 30, "y": -60}, "to": {"x": 150, "y": 60}})
        sid = (sq.get("strokeIds") or [None])[0]
        for view in ("drawingView", "cameraView"):
            await rc(s, "window.panel.set", {"panelId": view, "visible": True})
        await asyncio.sleep(1.0)

        async def anchors():
            d = await res("draw.drawing.get", {"flatten": True})
            n = next((x for x in d.get("nodes") or [] if x.get("id") == sid), {}) or {}
            b = n.get("bezier") or []
            return [(b[k * 3]["x"], b[k * 3]["y"]) for k in range((len(b) + 2) // 3)]

        async def held(view: str, dpts: list, mods: list, capture: bool, out_px: float = 0.0) -> dict:
            m = (await res("draw.viewMapping.get", {"view": view})).get("drawingToView") or [1, 0, 0, 1, 0, 0]
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
            vpts = [c42.apply(m, x, y) for x, y in dpts]
            # A press just outside a corner (the rotation zone), in view pixels whatever the view's scale.
            if out_px:
                vpts = [(vpts[0][0] + out_px, vpts[0][1] - out_px)] + [(x + out_px, y - out_px) for x, y in vpts[1:]]
            client = [(place[0] + x * ratio, place[1] + y * ratio) for x, y in vpts] if place else []
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
                    ("Perspective concave corner", "perspective", [(150, 60), (120, 30), (100, 10)], []),
                    ("Reposition rotation", "repositionAll", [(150, 60), (135, 75), (115, 85)], ["out"]),
                    ("Reposition alternate corner", "repositionAll", [(150, 60), (156, 64), (162, 68)], ["alt"])):
                await setup(view, tool)
                h = await held(view, pts, [k for k in mods if k != "out"], True, 10.0 if "out" in mods else 0.0)
                a, b = h.get("shotA"), h.get("shotB")
                kind = ((h.get("held") or {}).get("toolPreview") or {}).get("kind")
                dist = hausdorff(a, b) / (h.get("ratio") or 1.0) if a and b else None
                row = {"name": f"{name} ({view})", "mapKind": kind, "hausdorffPx": dist,
                       "captureA": hashlib.sha256(a).hexdigest() if a else None,
                       "captureB": hashlib.sha256(b).hexdigest() if b else None, "times": h.get("times")}
                if a and b:
                    (out / f"{name.replace(' ', '-')}-{view}-held.png").write_bytes(a)
                    (out / f"{name.replace(' ', '-')}-{view}-committed.png").write_bytes(b)
                run.check(dist is not None and dist <= BOUND_PX,
                          f"PV2 {row['name']}: the held preview and the committed drawing within {BOUND_PX} view px (Hausdorff)", row)
                rows.append(row)
                await rc(s, "edit.undo", {})
            run.save("rows.json", rows)
    return rows


def main() -> int:
    e6.dpi_aware()
    args = sys.argv[1:]
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
