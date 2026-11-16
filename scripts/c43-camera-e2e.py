"""M4 C4.3b: the Camera view in the running application (camera note §6; plan §5; rulings D-22, D-24, O-5).

Usage: python scripts/c43-camera-e2e.py <toontown binary> <out dir>

Drives the product binary over attached mode (Streamable HTTP, the official MCP SDK), like scripts/c42-gui-e2e.py,
and checks:

1. The Camera view draws what the final renderer draws (ruling O-5): its real capture against the `activeView` final
   render of the same state. The render is the camera's output image; each capture pixel inside the camera frame is
   compared with the render's pixel under it (the view's pixels mapped to output pixels by
   `inspect.coordinates.convert {from: view, to: image}`), a band of 3 pixels along the frame's edge left out (its
   outline). Under 1 % of the compared pixels may differ by more than 32 in a channel, and at least a quarter of the
   capture must be compared. Plain, zoomed out (the frame and the dimmed outside in view), zoomed in, rolled, and after
   a scene drag.
2. The canvas summary (`cameraView.canvas`'s `data-tt-value`) names the Camera view, its back end, no failures, the
   frame overlay drawn.
3. V9, click to activate (Windows, real OS input): with the Drawing view active, a click on Bob in the Camera view makes
   the Camera view active and selects Bob (the Select scene tool's click), with no undo step for the activation.

Receipts in <out dir>: transcript.json, manifest.json (kind toontown.c43.camera-e2e), the captures and renders of each
comparison. Exit 0 when every check passed.
"""

from __future__ import annotations

import asyncio
import importlib.util
import json
import os
import shutil
import sys
import tempfile
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

RENDER_BOUND = 0.01
RENDER_DELTA = 32
#: Capture pixels this close to the camera frame's edge are left out of the comparison (the frame's outline).
FRAME_BAND = 3.0
#: The comparison must cover at least this share of the capture (a check that compares nothing fails).
MIN_COVERED = 0.25


def affine_of(p0, p1, p2) -> list:
    """The affine taking (0,0), (1,0), (0,1) to the three points."""
    return [p1[0] - p0[0], p1[1] - p0[1], p2[0] - p0[0], p2[1] - p0[1], p0[0], p0[1]]


def compare_mapped(capture: bytes, render: bytes, view_to_image: list, ratio: float) -> dict:
    """Each capture pixel inside the frame (less the band) against the render's pixel under its centre."""
    wc, hc, cc, rc_ = c42.png_rgba(capture)
    wr, hr, cr, rr = c42.png_rgba(render)
    a, b, c, d, e, f = view_to_image
    # Output pixels per view pixel along each axis (the band is in view pixels).
    sx, sy = (a * a + b * b) ** 0.5, (c * c + d * d) ** 0.5
    bx, by = FRAME_BAND * sx, FRAME_BAND * sy
    n = diff = 0
    for y in range(hc):
        row = rc_[y]
        vy = (y + 0.5) / ratio
        for x in range(wc):
            vx = (x + 0.5) / ratio
            u, v = a * vx + c * vy + e, b * vx + d * vy + f
            if not (bx <= u < wr - bx and by <= v < hr - by):
                continue
            ru, rv = int(u), int(v)
            q = rr[rv]
            i, j = x * cc, ru * cr
            n += 1
            if max(abs(row[i] - q[j]), abs(row[i + 1] - q[j + 1]), abs(row[i + 2] - q[j + 2])) > RENDER_DELTA:
                diff += 1
    return {"capture": [wc, hc], "render": [wr, hr], "compared": n, "covered": n / (wc * hc), "different": diff,
            "fraction": diff / n if n else 1.0}


async def canvas_summary(session) -> dict:
    res = (await e2e.tree(session)).get("result") or {}
    node = c42.find(res.get("root") or {}, lambda n: n.get("id") == "cameraView.canvas") or {}
    try:
        return json.loads(node.get("value") or "{}")
    except json.JSONDecodeError:
        return {}


async def compare_camera(session, run, out: Path, name: str, label: str) -> dict:
    """The Camera view's capture against the activeView render, a few attempts while the canvas catches up."""
    rc = e2e.run_cmd
    best: dict = {}
    await rc(session, "view.active.set", {"viewId": "cameraView"})
    listed = (await rc(session, "view.list", {})).get("result") or []
    views = listed.get("views") or [] if isinstance(listed, dict) else listed
    logical = next((v.get("size") for v in views if v.get("viewId") == "cameraView"), None) or {}
    for k in range(4):
        await asyncio.sleep(0.75 if k == 0 else 1.5)
        conv = await rc(session, "inspect.coordinates.convert", {"from": "view", "to": "image", "view": "cameraView",
                                                                   "points": [{"x": 0, "y": 0}, {"x": 1, "y": 0}, {"x": 0, "y": 1}]})
        pts = [(p["x"], p["y"]) for p in ((conv.get("result") or {}).get("points") or [])]
        shot = await rc(session, "inspect.screenshot", {"target": "cameraView"})
        rend = await rc(session, "render.frame.run", {"composition": "activeView", "background": "#F0F0F0"})
        a, b = c42.png_of(shot), c42.png_of(rend)
        if not a or not b or len(pts) != 3:
            best = best or {"error": [shot.get("error"), rend.get("error"), conv.get("error")]}
            continue
        ratio = c42.png_rgba(a)[0] / logical["width"] if logical.get("width") else 1.0
        d = compare_mapped(a, b, affine_of(*pts), ratio)
        if not best or d["fraction"] < best.get("fraction", 2.0):
            best = d
            (out / f"camera-{name}-capture.png").write_bytes(a)
            (out / f"camera-{name}-final.png").write_bytes(b)
        if d["fraction"] < RENDER_BOUND and d["covered"] >= MIN_COVERED:
            break
    run.check(best.get("fraction", 2.0) < RENDER_BOUND and best.get("covered", 0) >= MIN_COVERED,
              f"the Camera view shows what the final renderer draws: {label} (its capture against the activeView render "
              "inside the frame, under 1 % of the pixels differ)", best)
    return best


async def session_run(url: str, token: str, run, out: Path, os_input: bool) -> None:
    from contextlib import AsyncExitStack

    async with AsyncExitStack() as stack:
        client = await stack.enter_async_context(
            e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {token}"}, timeout=e2e.httpx2.Timeout(60.0, read=300.0)))
        streams = await stack.enter_async_context(e2e.streamable_http_client(url, http_client=client))
        session = await stack.enter_async_context(e2e.ClientSession(streams[0], streams[1]))
        await session.initialize()
        rc = e2e.run_cmd
        await e2e.gui_tree(session, run, lambda r: e2e.by_id(r, "startScreen") is not None, "the GUI shows the start screen")
        env = await rc(session, "project.create", {"name": "Camera", "onUnsaved": "fail"})
        run.check(env.get("status") == "ok", "project.create", env.get("error"))
        found = (await rc(session, "doc.object.find", {"type": "swatch", "name": "Colour 2"})).get("result") or {}
        items = found.get("items") or found.get("matches") or []
        black = next((it.get("id") or it.get("key") or it.get("colourId") for it in items if isinstance(it, dict)), None)
        run.check(black is not None, "the default palette's Colour 2 is found", None)
        ids = {}
        # Bob, a painted rectangle at the centre; Ann, a painted ellipse moved left and behind (two occurrences, an
        # order to keep).
        for name, shape in (("Bob", ("draw.rectangle.add", {"from": {"x": -120, "y": -90}, "to": {"x": 120, "y": 90}})),
                            ("Ann", ("draw.ellipse.add", {"from": {"x": -80, "y": -80}, "to": {"x": 80, "y": 80}}))):
            el = (await rc(session, "element.create", {"type": "drawing", "name": name})).get("result") or {}
            ids[name] = (el.get("elementIds") or [None])[0]
            await rc(session, "element.drawing.create", {"elementId": ids[name], "name": "1", "exposeFrom": 1, "exposeTo": 24})
            await rc(session, "app.context.set", {"elementId": ids[name], "frame": 1})
            await rc(session, "colour.current.set", {"colourId": black})
            await rc(session, shape[0], shape[1])
            await rc(session, "colour.paint.apply", {"points": [{"x": 0, "y": 0}]})
        await rc(session, "anim.transform.set", {"nodeIds": [ids["Ann"]], "target": "static", "position": {"x": -1.5, "y": 0.5, "z": -1}})
        await rc(session, "app.context.set", {"elementId": ids["Bob"], "frame": 1})
        await rc(session, "edit.selection.set", {"context": "staging", "ids": []})
        await rc(session, "view.active.set", {"viewId": "cameraView"})
        await rc(session, "anim.tool.set", {"tool": "select"})

        # 2. The canvas summary.
        st: dict = {}
        start = time.monotonic()
        while time.monotonic() - start < c42.CANVAS_BOUND_S:
            st = await canvas_summary(session)
            if st.get("viewType") == "camera" and len(st.get("drawings") or []) >= 2:
                break
            await asyncio.sleep(0.25)
        run.save("canvas-summary.json", st)
        run.check(st.get("viewType") == "camera" and st.get("backend") in ("webgl2", "webgpu") and not st.get("lastError"),
                  "the Camera view's canvas draws the camera composition with a back end and no errors", st)
        run.check(len(st.get("drawings") or []) >= 2 and (st.get("camera") or {}).get("frame") is True,
                  "both elements' drawings are resident and the frame overlay is drawn", st)

        # 1. Captures against renders.
        await compare_camera(session, run, out, "plain", "the plain view")
        await rc(session, "view.zoom.set", {"viewId": "cameraView", "zoom": 0.25})
        await compare_camera(session, run, out, "zoomed-out", "zoomed out to 25 % (the frame and the dimmed outside in view)")
        await rc(session, "view.zoom.set", {"viewId": "cameraView", "zoom": 2})
        await compare_camera(session, run, out, "zoomed-in", "zoomed in to 200 %")
        await rc(session, "view.zoom.set", {"viewId": "cameraView", "zoom": 0.5})
        await rc(session, "view.rotation.set", {"viewId": "cameraView", "degrees": 20})
        await compare_camera(session, run, out, "rolled", "at 50 % rolled 20 degrees")
        await rc(session, "view.transform.reset", {"viewId": "cameraView"})
        # After a scene drag (Ann moved right by an agent's pointer, played through the window).
        m = ((await rc(session, "draw.viewMapping.get", {"view": "cameraView"})).get("result") or {}).get("drawingToView")
        env = await e2e.call(session, "pointer", {"view": "cameraView", "tool": "select",
                                                  "events": c42.drag([c42.apply(m, 60, 40), c42.apply(m, 100, 60)])})
        run.check(env.get("status") == "ok", "a scene drag through the window", env.get("error"))
        await compare_camera(session, run, out, "after-drag", "after a scene drag")

        # 3. V9: click to activate (Windows, OS input).
        if os_input:
            await rc(session, "edit.selection.set", {"context": "staging", "ids": []})
            await rc(session, "window.panel.set", {"panelId": "drawingView", "visible": True})
            await asyncio.sleep(1.0)
            await rc(session, "view.active.set", {"viewId": "cameraView"})
            place = await e6.locate_view(session, rc, "cameraView")
            await rc(session, "view.active.set", {"viewId": "drawingView"})
            pos0 = ((await rc(session, "edit.history.get", {"limit": 1000})).get("result") or {}).get("position", 0)
            m = ((await rc(session, "draw.viewMapping.get", {"view": "cameraView"})).get("result") or {}).get("drawingToView")
            run.check(place is not None and isinstance(m, list), "V9: the Camera view is located in the window", place)
            if place and isinstance(m, list):
                v = c42.apply(m, 60, 40)
                p = (place[0] + v[0], place[1] + v[1])
                times: dict = {}
                e6.os_drag(e2e.APP_PID["pid"], [p, p], 0.05, 0.05, times)
                run.check("error" not in times, "V9: the OS click lands in the application window", times.get("error"))
                await asyncio.sleep(1.5)
                state = (await rc(session, "app.state.get", {})).get("result") or {}
                sel = ((await rc(session, "edit.selection.get", {"context": "staging"})).get("result") or {}).get("ids") or []
                hist = (await rc(session, "edit.history.get", {"limit": 1000})).get("result") or {}
                run.check((state.get("activeView") or {}).get("viewId") == "cameraView",
                          "V9: a click in the inactive Camera view makes it the active view", state.get("activeView"))
                run.check(sel == [ids["Bob"]], "V9: the same click is the Select tool's click: Bob is selected", sel)
                run.check(hist.get("position", 0) == pos0, "V9: the activation is no undo step", hist.get("position"))
        run.save("tree-final.json", ((await e2e.tree(session)).get("result") or {}).get("root"))


def main() -> int:
    e6.dpi_aware()
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    app, out = sys.argv[1], Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    os_input = (os.environ.get("TT_E2E_INPUT") or ("os" if sys.platform == "win32" else "played")) == "os"
    user = Path(tempfile.mkdtemp(prefix="tt-c43-camera-"))
    settings = user / "Settings"
    settings.mkdir(parents=True)
    seed = {"agent.policy.defaults": {"policy": "autonomous", "maxJobs": 2, "maxBatchSteps": 1000, "maxInlineImagePx": 2048,
                                      "maxInlineBytes": 52428800, "askInApp": True, "letAgentMoveView": True, "highlightAgentChanges": True}}
    (settings / "preferences.json").write_text(json.dumps({"format": "toontown-settings", "kind": "preferences", "formatVersion": "1.0", "data": seed}), encoding="utf-8")
    discovery = out / "discovery.json"
    started = time.monotonic()
    try:
        proc = e2e.launch(app, user, discovery)
        try:
            d = e2e.wait_discovery(discovery, proc)
            asyncio.run(session_run(d["endpoint"], d["token"], run, out, os_input))
        finally:
            e2e.stop(proc)
    except Exception as ex:  # noqa: BLE001 - reported as a failure with its text
        run.check(False, "the run completed", repr(ex))
    finally:
        shutil.rmtree(user, ignore_errors=True)
    manifest = {"kind": "toontown.c43.camera-e2e", "platform": sys.platform, "input": "os" if os_input else "played",
                "seconds": round(time.monotonic() - started, 1), "failures": run.failures, "checks": len(run.transcript)}
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps(manifest, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
