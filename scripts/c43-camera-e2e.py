"""M4 C4.3b: the Camera view in the running application (camera note §6; plan §5; rulings D-22, D-24, O-5).

Usage: python scripts/c43-camera-e2e.py <toontown binary> <out dir> [--backend webgl2|webgpu]

`--backend` starts the canvas on that back end (`--canvas-backend`); without it the product chooses (the rule set).

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
4. The live preview (preview note 8; Windows, real OS input, the press held at the drag's end): PV1, a drag sends no
   command before its release (no history step while held, one after) and Escape during a drag sends nothing; PV2, the
   preview's affine (the summary's `preview.transform`, read while held) takes the selection box's corners before the
   drag to within 1 view pixel of the committed box's corners, for exact cases of the preview note §4: a scene move, a
   scene rotation and a scene corner scale of one node (its factor taken from the box, applied about Bob's pivot, as
   the commit does: SI-150) in the Camera view, a drawing Select move in the Drawing view.

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
#: The back end asked for on the command line (None: the product's choice).
REQUESTED = {"backend": None}


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


async def canvas_summary(session, view: str = "cameraView") -> dict:
    res = (await e2e.tree(session)).get("result") or {}
    node = c42.find(res.get("root") or {}, lambda n: n.get("id") == f"{view}.canvas") or {}
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
        if REQUESTED["backend"]:
            run.check(st.get("backend") == REQUESTED["backend"] and not st.get("fallback"),
                      f"the canvas runs on the requested back end, {REQUESTED['backend']}, with no fallback", st)

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
        # The comparison is of the composition: the drag's selection (its box, pivot and handles, an overlay) is cleared.
        await rc(session, "edit.selection.set", {"context": "staging", "ids": []})
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
        # 4. The live preview (Windows, OS input).
        if os_input:
            await preview_rows(session, run, ids)
        run.save("tree-final.json", ((await e2e.tree(session)).get("result") or {}).get("root"))


def corners(box) -> list:
    x0, y0, x1, y1 = box
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]


async def box_of(session, view: str) -> list | None:
    """The selection's box in the view's pixels: the current drawing's bounds through the mapping (the scene tools'
    furniture, the Camera view), or the drawing selection's bounds (the Drawing view)."""
    rc = e2e.run_cmd
    m = ((await rc(session, "draw.viewMapping.get", {"view": view})).get("result") or {}).get("drawingToView")
    if view == "cameraView":
        b = ((await rc(session, "draw.bounds.get", {})).get("result") or {}).get("bounds")
    else:
        b = ((await rc(session, "draw.selection.get", {})).get("result") or {}).get("bounds")
    if not (isinstance(m, list) and b):
        return None
    pts = [c42.apply(m, x, y) for x in (b["x"], b["x"] + b["width"]) for y in (b["y"], b["y"] + b["height"])]
    return [min(q[0] for q in pts), min(q[1] for q in pts), max(q[0] for q in pts), max(q[1] for q in pts)]


async def preview_rows(session, run, ids) -> None:
    import threading

    rc = e2e.run_cmd

    async def position() -> int:
        return ((await rc(session, "edit.history.get", {"limit": 1000})).get("result") or {}).get("position", 0)

    async def held_drag(view: str, pts_view: list, escape: bool = False) -> dict:
        """An OS drag in `view` through view points, held 1.2 s at its end: the summary and the history while held."""
        await rc(session, "view.active.set", {"viewId": view})
        # The view is found in the window capture once the layout has settled (a workspace switched just before; a slow
        # runner had not drawn it 0.8 s later, runs 43837502373 and 43847318713, nor 5 s later in run 44130018843): a
        # wait for that state (ruling D-45), the 30 s bound only guarding a hang; the attempts are recorded.
        place, attempts, t_locate = None, 0, time.monotonic()
        while time.monotonic() - t_locate < 30.0:
            attempts += 1
            place = await e6.locate_view(session, rc, view, keep=Path(run.out))
            if place:
                break
            await asyncio.sleep(0.5)
        out: dict = {"place": place, "locate": {"attempts": attempts, "seconds": round(time.monotonic() - t_locate, 1)}}
        if not place:
            return out
        client = [(place[0] + x, place[1] + y) for x, y in pts_view]
        times: dict = {}
        pos0 = await position()
        th = threading.Thread(target=e6.os_drag, args=(e2e.APP_PID["pid"], client, 0.5, 1.2, times))
        th.start()
        t0 = time.monotonic()
        while "release" not in times and "error" not in times and th.is_alive():
            if "press" in times and time.monotonic() - times["press"] > 0.6 and "held" not in out:
                out["held"] = (await canvas_summary(session, view)).get("preview")
                out["heldPosition"] = await position()
                if escape:
                    c42.os_key(0x1B)
            await asyncio.sleep(0.05)
            if time.monotonic() - t0 > 10:
                break
        th.join()
        await asyncio.sleep(1.5)
        out.update({"times": times, "pos0": pos0, "after": await position(),
                    "previewAfter": (await canvas_summary(session, view)).get("preview")})
        return out

    async def row(name: str, view: str, setup, pts_of, exact: bool = True) -> None:
        await setup()
        await asyncio.sleep(0.8)
        before = await box_of(session, view)
        if not before:
            run.check(False, f"PV {name}: the selection box is known", before)
            return
        r = await held_drag(view, pts_of(before))
        after = await box_of(session, view)
        held = r.get("held") or {}
        t = held.get("transform")
        run.check(r.get("place") is not None and "error" not in (r.get("times") or {}), f"PV {name}: the OS drag lands in the {view}", {"times": r.get("times"), "locate": r.get("locate")})
        if r.get("place") is None or "pos0" not in r:
            return  # no drag was made: nothing further of this row can be checked (the failure is above)
        run.check(r.get("heldPosition") == r.get("pos0") and r.get("after") == r.get("pos0") + 1,
                  f"PV1 {name}: no command while the drag is held, one step after its release",
                  {k: r.get(k) for k in ("pos0", "heldPosition", "after")})
        if isinstance(t, list) and len(t) == 6 and after:
            # The box of the previewed corners (a rotation turns the box; the committed box is axis-aligned).
            moved = [c42.apply(t, x, y) for x, y in corners(before)]
            box = [min(q[0] for q in moved), min(q[1] for q in moved), max(q[0] for q in moved), max(q[1] for q in moved)]
            err = max(abs(a - b) for a, b in zip(box, after))
            detail = {"before": before, "after": after, "previewed": box, "transform": t, "previewKind": held.get("kind"), "error": err}
            if exact:
                run.check(err <= 1.0, f"PV2 {name}: the held preview takes the box's corners to the committed box's, within 1 view pixel", detail)
            else:
                run.transcript.append({"approximate": f"PV2 {name} (an approximate case, preview note §4)", "detail": detail})
        else:
            run.check(False, f"PV2 {name}: the summary shows the preview while the drag is held", {"held": held, "after": after})
        # Every row starts from the same Bob: the row's step is undone (a rotated node scales along its own axes, which
        # the box's preview only approximates, preview note §4).
        if r.get("after") == r.get("pos0", -1) + 1:
            await rc(session, "edit.undo", {})

    async def scene_setup(tool):
        async def f():
            await rc(session, "app.context.set", {"elementId": ids["Bob"], "frame": 1})
            await rc(session, "edit.selection.set", {"context": "staging", "ids": [ids["Bob"]]})
            await rc(session, "anim.tool.set", {"tool": tool})
        return f

    await row("scene move", "cameraView", await scene_setup("select"),
              lambda b: [((b[0] + b[2]) / 2 + 30, (b[1] + b[3]) / 2 + 20), ((b[0] + b[2]) / 2 + 60, (b[1] + b[3]) / 2 + 35)])
    def rotation(b):
        import math
        cx, cy = (b[0] + b[2]) / 2, (b[1] + b[3]) / 2
        p0 = (b[2] + 8, b[1] - 8)
        a = math.radians(-20)
        dx, dy = p0[0] - cx, p0[1] - cy
        return [p0, (cx + dx * math.cos(a / 2) - dy * math.sin(a / 2), cy + dx * math.sin(a / 2) + dy * math.cos(a / 2)),
                (cx + dx * math.cos(a) - dy * math.sin(a), cy + dx * math.sin(a) + dy * math.cos(a))]

    await row("scene rotation", "cameraView", await scene_setup("transform"), rotation)
    await row("scene corner scale", "cameraView", await scene_setup("transform"),
              lambda b: [(b[2], b[1]), (b[2] + 20, b[1] - 12), (b[2] + 30, b[1] - 18)])

    # PV1, Escape: a scene drag cancelled by Escape while held sends nothing and the preview ends.
    await (await scene_setup("select"))()
    await asyncio.sleep(0.8)
    b = await box_of(session, "cameraView")
    if b:
        c = ((b[0] + b[2]) / 2 + 30, (b[1] + b[3]) / 2 + 20)
        r = await held_drag("cameraView", [c, (c[0] + 40, c[1] + 25)], escape=True)
        run.check(r.get("after") == r.get("pos0") and not r.get("previewAfter") and (r.get("held") or {}).get("transform") is not None,
                  "PV1 Escape: a drag cancelled by Escape sends nothing and its preview ends",
                  {k: r.get(k) for k in ("pos0", "after", "held", "previewAfter")})

    async def drawing_setup():
        # The Drawing workspace: the Drawing view central and large, as scripts/c42-gui-e2e.py uses it (a docked
        # extra panel on a runner's small screen is small and changes the Camera view's place).
        await rc(session, "workspace.select", {"name": "Drawing"})
        await rc(session, "app.context.set", {"elementId": ids["Bob"], "frame": 1})
        await rc(session, "tool.active.set", {"toolId": "select"})
        await rc(session, "draw.selection.set", {"by": "all"})
        # The Drawing view laid out and drawing before it is looked for (a wait for the state, D-45; 30 s guard).
        await c42.wait_canvas(session, lambda s_: bool(s_.get("drawings")) and s_.get("viewType") == "drawing", 30)
    await row("drawing select move", "drawingView", drawing_setup,
              lambda b: [((b[0] + b[2]) / 2 + 10, (b[1] + b[3]) / 2 + 10), ((b[0] + b[2]) / 2 + 40, (b[1] + b[3]) / 2 + 25)])


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
    REQUESTED["backend"] = backend
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
        if backend:
            # The back end alone (`--canvas-backend`): with `--canvas-metrics` the summary is published only every
            # 500 ms, and the held preview would be read stale.
            import subprocess
            if discovery.exists():
                discovery.unlink()
            log = open(discovery.with_suffix(".log"), "ab")
            proc = subprocess.Popen([app, "--user-dir", str(user), "--mcp-http", "127.0.0.1:0", "--mcp-discovery", str(discovery),
                                     "--canvas-backend", backend], stdout=log, stderr=subprocess.STDOUT)
            e2e.APP_PID["pid"] = proc.pid
        else:
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
                "requestedBackend": backend,
                "seconds": round(time.monotonic() - started, 1), "failures": run.failures, "checks": len(run.transcript)}
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps(manifest, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
