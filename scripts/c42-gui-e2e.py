"""M4 C4.2b end-to-end test of the interactive Drawing view in the running application (both CI OSes).

Usage: python scripts/c42-gui-e2e.py <toontown binary> <out dir>

Drives the product binary over attached mode (Streamable HTTP, the official MCP SDK), like scripts/c41-gui-e2e.py,
whose launch, session and capture helpers it reuses, and checks:

1. The canvas shows the current drawing at the kernel's revision (design note 1 §9, T1 end to end): the canvas's
   `data-tt-value` in the rendered UI tree names the drawing, its revision and the back end, with no failures. And it
   draws what the final renderer draws (ruling O-5: the check fails when the canvas draws nothing, the wrong drawing
   or a mirror of it): a real capture of the view against the `activeView` final render of the same state, in the
   plain view, zoomed and panned, and rotated, and after every drawing gesture of item 2, with under 1 % of the pixels
   differing by more than 32 in a channel (the reviewer measured 0.03 to 0.15 %).
2. Gestures through the window equal the headless path (ruling D-4; design note 2 §6): for every covered tool, the
   agent's `pointer` call is routed through the window (played through the window's collector and overlay, committed
   through gesture_commit), undone, then sent again with the Drawing view hidden (no window shows it: the headless
   path). The produced commands' parameters must be equal value for value, the drawing's content equal (ids and
   revisions aside: the second pass allocates from a later point of the id stream), and each gesture one undo step
   with the same label. The Select tool's handle gestures (corner scale, constrained corner, side scale, rotate, the
   drawing pivot; frame-loop note §7.3) are rows of the same table.
2b. The selection overlay (frame-loop note §7; C42B-R1-22): selecting an object with the Select tool shows eight
   handles in the canvas summary and, in a real capture, the handle squares at the handle positions; deselecting
   removes them.
3. Screenshots capture what the user sees (ruling D-6; design note 5 §2): the window capture is the window's size and
   contains the Drawing view capture pixel for pixel at the canvas's place (the chrome is in the window capture and
   not in the view's); with the Preferences dialog open, the window capture changes where the dialog is.
4. Windows only (OS input): a real mouse drag in the canvas commits one Brush Tool step drawn at the new revision; a
   user press during a long agent gesture aborts it with `ui.busy`, `reason: "userInput"` (P2, SI-135), nothing
   committed.
5. The float lifecycle (C42B-R1-05): the Drawing view floated draws what the final renderer draws; after the float
   closes and the view is docked again, `inspect.screenshot` and `pointer` reach the docked view.

Receipts in <out dir>: transcript.json, manifest.json (kind toontown.c42.gui-e2e), the PNGs of the attached captures,
the UI trees. Exit 0 when every check passed.
"""

from __future__ import annotations

import asyncio
import base64
import importlib.util
import io
import json
import shutil
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location("c41e2e", ROOT / "scripts" / "c41-gui-e2e.py")
e2e = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(e2e)

#: How long the canvas may take to show a new revision (generous: CI runners draw with software WebGL).
CANVAS_BOUND_S = 60.0


def find(node: dict, pred) -> dict | None:
    if pred(node):
        return node
    for c in node.get("children", []) or []:
        f = find(c, pred)
        if f is not None:
            return f
    return None


async def canvas_state(session) -> dict:
    res = (await e2e.tree(session)).get("result") or {}
    root = res.get("root") or {}
    node = find(root, lambda n: n.get("id") == "drawingView.canvas") or {}
    try:
        return json.loads(node.get("value") or "{}")
    except json.JSONDecodeError:
        return {}


async def wait_canvas(session, pred, bound=CANVAS_BOUND_S) -> dict:
    start = time.monotonic()
    last = {}
    while time.monotonic() - start < bound:
        last = await canvas_state(session)
        if pred(last):
            return last
        await asyncio.sleep(0.25)
    return last


def strip_ids(v):
    """Content modulo the id stream: object ids, revisions and id-valued references dropped."""
    if isinstance(v, dict):
        return {k: strip_ids(x) for k, x in v.items() if k not in ("id", "rev", "strokeIds", "leftZoneId", "rightZoneId",
                                                                    "boundaryStrokeIds", "parentZoneId", "zoneIds", "objectIds")}
    if isinstance(v, list):
        return [strip_ids(x) for x in v]
    if isinstance(v, str) and len(v) > 4 and v[2] == "_" and v[:2] in ("st", "zn", "nd", "tx"):
        return "<id>"
    return v


def params_of(env: dict) -> list:
    """The produced commands' ids and parameters, ids of new objects aside."""
    cmds = ((env.get("result") or {}).get("commands")) or []
    return [{"commandId": c.get("commandId"), "params": strip_ids(c.get("params"))} for c in cmds]


def apply(m, x, y):
    return (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])


def drag(points):
    out = []
    for i, (x, y) in enumerate(points):
        t = "down" if i == 0 else ("up" if i == len(points) - 1 else "move")
        out.append({"type": t, "x": x, "y": y, "dt": 0 if i == 0 else 16})
    return out


def png_rgba(data: bytes):
    """Decodes an 8-bit RGB/RGBA PNG (the captures) with the standard library: (w, h, rows of RGBA tuples)."""
    import struct
    import zlib
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    pos, idat, w = 8, b"", 0
    while pos < len(data):
        n = struct.unpack(">I", data[pos:pos + 4])[0]
        kind = data[pos + 4:pos + 8]
        body = data[pos + 8:pos + 8 + n]
        if kind == b"IHDR":
            w, h, depth, ctype = struct.unpack(">IIBB", body[:10])
            assert depth == 8 and ctype in (2, 6), (depth, ctype)
            ch = 4 if ctype == 6 else 3
        elif kind == b"IDAT":
            idat += body
        pos += 12 + n
    raw = zlib.decompress(idat)
    stride = w * ch
    rows, prev = [], bytearray(stride)
    i = 0
    for _ in range(h):
        f = raw[i]
        line = bytearray(raw[i + 1:i + 1 + stride])
        i += 1 + stride
        for x in range(stride):
            a = line[x - ch] if x >= ch else 0
            b = prev[x]
            c = prev[x - ch] if x >= ch else 0
            if f == 1:
                line[x] = (line[x] + a) & 255
            elif f == 2:
                line[x] = (line[x] + b) & 255
            elif f == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else (b if pb <= pc else c))) & 255
        rows.append(bytes(line))
        prev = line
    return w, h, ch, rows


#: Item 1's bound: the fraction of pixels whose largest channel difference exceeds `RENDER_DELTA`.
RENDER_BOUND = 0.01
RENDER_DELTA = 32
#: The tools whose gestures change the drawing and show no overlay afterwards: compared with the final render.
DRAWING_TOOLS = {"brush", "pencil", "rectangle", "ellipse", "line", "polyline", "eraser", "cutter", "scissor", "paint",
                 "unpaint", "paintUnpainted", "closeGap", "dropper"}


def png_of(env: dict) -> bytes | None:
    r = env.get("result") or {}
    data = r.get("png") or r.get("data")
    return base64.b64decode(data) if data else None


def differing(a: bytes, b: bytes) -> dict:
    """The fraction of pixels of two equal-sized PNGs whose largest RGB difference exceeds `RENDER_DELTA`."""
    wa, ha, ca, ra = png_rgba(a)
    wb, hb, cb, rb = png_rgba(b)
    if (wa, ha) != (wb, hb):
        return {"sizes": [[wa, ha], [wb, hb]], "fraction": 1.0}
    n = 0
    for y in range(ha):
        ar, br = ra[y], rb[y]
        for x in range(wa):
            i, j = x * ca, x * cb
            if max(abs(ar[i] - br[j]), abs(ar[i + 1] - br[j + 1]), abs(ar[i + 2] - br[j + 2])) > RENDER_DELTA:
                n += 1
    return {"size": [wa, ha], "different": n, "fraction": n / (wa * ha)}


async def compare_render(session, run, out: Path, name: str, label: str) -> dict:
    """Item 1: the view's real capture against the `activeView` final render, a few attempts while the canvas catches
    up (a bucket reload after a zoom, the overlay clearing after a gesture); the best attempt is the receipt."""
    best: dict = {}
    # The final render of the Drawing view: it is the active view (after a float, the Camera view may be central).
    await e2e.run_cmd(session, "view.active.set", {"viewId": "drawingView"})
    for k in range(4):
        await asyncio.sleep(0.75 if k == 0 else 1.5)
        shot = await e2e.run_cmd(session, "inspect.screenshot", {"target": "drawingView"})
        rend = await e2e.run_cmd(session, "render.frame.run", {"composition": "activeView", "background": "#F0F0F0"})
        a, b = png_of(shot), png_of(rend)
        if not a or not b:
            best = best or {"error": [shot.get("error"), rend.get("error")]}
            continue
        d = differing(a, b)
        if not best or d["fraction"] < best.get("fraction", 2.0):
            best = d
            (out / f"render-{name}-capture.png").write_bytes(a)
            (out / f"render-{name}-final.png").write_bytes(b)
        if d["fraction"] < RENDER_BOUND:
            break
    run.check(best.get("fraction", 2.0) < RENDER_BOUND,
              f"the canvas shows what the final renderer draws: {label} (a real capture against the activeView render, under 1 % of pixels differ)",
              best)
    return best


def handle_points(b: list, m: list) -> list:
    """The Select tool's eight handle centres in view pixels: corners counter-clockwise from the bottom-left (drawing
    space), then the side midpoints (selection.ts `handlePoints`, the kernel's order)."""
    c = [apply(m, b[0], b[1]), apply(m, b[2], b[1]), apply(m, b[2], b[3]), apply(m, b[0], b[3])]
    mids = [((c[i][0] + c[(i + 1) % 4][0]) / 2, (c[i][1] + c[(i + 1) % 4][1]) / 2) for i in range(4)]
    return c + mids


def handles_in(data: bytes, centres: list, ratio: float) -> int:
    """How many handle squares a capture shows: a white centre (the square's fill) and accent pixels around it."""
    w, h, ch, rows = png_rgba(data)

    def px(x, y):
        if 0 <= x < w and 0 <= y < h:
            return rows[y][x * ch:x * ch + 3]
        return None

    found = 0
    for vx, vy in centres:
        cx, cy = round(vx * ratio), round(vy * ratio)
        white = any((q := px(cx + dx, cy + dy)) is not None and min(q) >= 250 for dx in range(-2, 3) for dy in range(-2, 3))
        accent = any((q := px(cx + dx, cy + dy)) is not None and q[2] >= 180 and q[2] - q[0] >= 60 and 90 <= q[1] <= 210
                     for dx in range(-6, 7) for dy in range(-6, 7))
        found += 1 if white and accent else 0
    return found


async def first_session(url: str, token: str, run, out: Path) -> None:
    client = e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {token}"}, timeout=e2e.httpx2.Timeout(60.0, read=300.0))
    from mcp import types as mt

    async def roots(_ctx):
        return mt.ListRootsResult(roots=[mt.Root(uri=out.resolve().as_uri())])

    async with client:
        async with e2e.streamable_http_client(url, http_client=client) as streams:
            async with e2e.ClientSession(streams[0], streams[1], list_roots_callback=roots) as session:
                await session.initialize()
                await e2e.gui_tree(session, run, lambda r: e2e.by_id(r, "startScreen") is not None, "the GUI shows the start screen")
                rc = e2e.run_cmd
                env = await rc(session, "project.create", {"name": "Canvas", "onUnsaved": "fail"})
                run.check(env.get("status") == "ok", "project.create", env.get("error"))
                await rc(session, "workspace.select", {"name": "Drawing"})
                el = (await rc(session, "element.create", {"type": "drawing", "name": "Ink"})).get("result") or {}
                eid = (el.get("elementIds") or [None])[0]
                dr = (await rc(session, "element.drawing.create", {"elementId": eid, "name": "1", "exposeFrom": 1, "exposeTo": 1})).get("result") or {}
                did = dr.get("drawingId")
                await rc(session, "app.context.set", {"elementId": eid, "frame": 1})
                # Black: the default palette's second swatch, "Colour 2" (03 §1.8; the first is transparent).
                found_sw = (await rc(session, "doc.object.find", {"type": "swatch", "name": "Colour 2"})).get("result") or {}
                items = found_sw.get("items") or found_sw.get("matches") or (found_sw if isinstance(found_sw, list) else [])
                black = next((it.get("id") or it.get("key") or it.get("colourId") for it in items if isinstance(it, dict)), None)
                run.check(black is not None, "the default palette's Colour 2 is found", None)
                await rc(session, "colour.current.set", {"colourId": black})
                m = ((await rc(session, "draw.viewMapping.get", {"view": "drawingView"})).get("result") or {}).get("drawingToView")
                run.check(isinstance(m, list) and len(m) == 6, "the view's mapping", m)
                v = lambda x, y: apply(m, x, y)  # noqa: E731

                # Fixtures, in drawing units around the origin: a painted rectangle, an unpainted one, two nearly
                # touching lines.
                await rc(session, "draw.rectangle.add", {"from": {"x": -150, "y": -60}, "to": {"x": -30, "y": 60}})
                await rc(session, "colour.paint.apply", {"points": [{"x": -90, "y": 0}]})
                await rc(session, "draw.rectangle.add", {"from": {"x": 30, "y": -60}, "to": {"x": 150, "y": 60}})
                await rc(session, "draw.line.add", {"from": {"x": -150, "y": 120}, "to": {"x": -10, "y": 120}, "layer": "top"})
                await rc(session, "draw.line.add", {"from": {"x": -6, "y": 118}, "to": {"x": 150, "y": 118}, "layer": "top"})

                # 1. The canvas shows the drawing at the kernel's revision.
                rev = ((await rc(session, "draw.drawing.get", {})).get("result") or {}).get("rev")
                st = await wait_canvas(session, lambda s: any(d.get("id") == did and d.get("rev") == rev for d in s.get("drawings", [])))
                run.check(any(d.get("id") == did and d.get("rev") == rev for d in st.get("drawings", [])) and not st.get("failures"),
                          "T1: the canvas holds the current drawing at the kernel's revision", {"canvas": st, "rev": rev})
                run.check(st.get("backend") in ("webgl2", "webgpu") and not st.get("lastError"), "a back end draws without errors", st)
                run.save("canvas-state.json", st)

                # 1b. The canvas draws what the final renderer draws: plain, zoomed and panned, rotated (no overlay:
                # a creation tool, nothing selected; the aids off by default).
                await rc(session, "tool.active.set", {"toolId": "brush"})
                await rc(session, "draw.selection.set", {"by": "none"})
                renders = {"plain": await compare_render(session, run, out, "plain", "the plain view")}
                await rc(session, "view.zoom.set", {"viewId": "drawingView", "zoom": 2})
                await rc(session, "view.pan.set", {"viewId": "drawingView", "by": {"x": 30, "y": 20}})
                renders["zoomed"] = await compare_render(session, run, out, "zoomed", "zoomed 200 % and panned")
                await rc(session, "view.rotation.set", {"viewId": "drawingView", "degrees": 30})
                renders["rotated"] = await compare_render(session, run, out, "rotated", "zoomed, panned and rotated 30°")
                await rc(session, "view.transform.reset", {"viewId": "drawingView"})

                # 2. Every covered tool: the window path equals the headless path. The mapping is the view's as the
                # window reported its size (read after the canvas showed the drawing).
                m = ((await rc(session, "draw.viewMapping.get", {"view": "drawingView"})).get("result") or {}).get("drawingToView")
                run.save("mapping.json", m)
                cases = [
                    ("brush", drag([v(-200, 160), v(-120, 190), v(-40, 170)])),
                    ("pencil", drag([v(-200, -120), v(-100, -150), v(0, -120)])),
                    ("rectangle", drag([v(-60, -200), v(40, -140)])),
                    ("ellipse", drag([v(60, -200), v(160, -140)])),
                    ("line", drag([v(-200, 220), v(200, 230)])),
                    ("polyline", drag([v(170, 100), v(170, 100)]) + drag([v(220, 140), v(220, 140)]) + [{"type": "doubleClick", "x": v(220, 140)[0], "y": v(220, 140)[1]}]),
                    ("select", drag([v(-170, -80), v(-10, 80)])),
                    ("eraser", drag([v(-160, 0), v(-120, 10), v(-80, 0)])),
                    ("cutter", drag([v(-170, -80), v(-90, -80), v(-90, 80), v(-170, 80), v(-170, -80)])),
                    ("scissor", drag([v(-160, -20), v(-150, 0), v(-140, 20)])),
                    ("paint", drag([v(90, 0), v(90, 0)])),
                    ("unpaint", drag([v(-90, 0), v(-90, 0)])),
                    ("paintUnpainted", drag([v(90, 0), v(90, 0)])),
                    ("dropper", drag([v(-90, 0), v(-90, 0)])),
                    ("closeGap", drag([v(-10, 120), v(-6, 118)])),
                    ("zoom", drag([v(-100, -100), v(100, 100)])),
                    ("grabber", drag([v(0, 0), v(40, 20)])),
                ]
                cases = [(tool, tool, events, None) for tool, events in cases]

                # The Select tool's handles (frame-loop note §7.3) on the painted rectangle, selected before each pass.
                async def select_fixture():
                    await rc(session, "draw.selection.set", {"by": "rect", "rect": {"x": -160, "y": -70, "width": 140, "height": 140},
                                                             "rectRule": "inside", "mode": "replace"})

                await select_fixture()
                sb = ((await rc(session, "draw.selection.get", {})).get("result") or {}).get("bounds") or {}
                await rc(session, "draw.selection.set", {"by": "none"})
                bx0, by0 = sb.get("x", -150), sb.get("y", -60)
                bx1, by1 = bx0 + sb.get("width", 120), by0 + sb.get("height", 120)
                bcx, bcy = (bx0 + bx1) / 2, (by0 + by1) / 2
                pv = ((await rc(session, "draw.drawing.get", {})).get("result") or {}).get("pivot") or {"x": 0, "y": 0}
                out_c = (bx1 + 8, by1 + 8)
                constrained = drag([v(bx1, by1), v(bx1 + 10, by1 + 6), v(bx1 + 20, by1 + 12)])
                for ev_ in constrained:
                    ev_["modifiers"] = ["shift"]

                async def reset_pivot():
                    await rc(session, "draw.selection.set", {"by": "none"})
                    await rc(session, "draw.pivot.set", {"point": pv})

                cases += [
                    ("select: corner scale", "select", drag([v(bx1, by1), v(bx1 + 10, by1 + 6), v(bx1 + 20, by1 + 12)]), select_fixture),
                    ("select: constrained corner", "select", constrained, select_fixture),
                    ("select: side scale", "select", drag([v(bx1, bcy), v(bx1 + 20, bcy + 3), v(bx1 + 40, bcy + 5)]), select_fixture),
                    ("select: rotate", "select", drag([v(*out_c), v(bcx - (out_c[1] - bcy), bcy + (out_c[0] - bcx))]), select_fixture),
                    ("select: drawing pivot", "select", drag([v(pv["x"], pv["y"]), v(pv["x"] + 30, pv["y"] + 20)]), reset_pivot),
                ]
                results = []
                renders["gestures"] = {}
                for name, tool, events, setup in cases:
                    async def one(hidden: bool):
                        if hidden:
                            await rc(session, "window.panel.set", {"panelId": "drawingView", "visible": False})
                            await asyncio.sleep(0.5)
                        if setup:
                            await setup()
                        pos0 = ((await rc(session, "edit.history.get", {"limit": 1000})).get("result") or {}).get("position", 0)
                        envp = await e2e.call(session, "pointer", {"view": "drawingView", "tool": tool, "events": events})
                        if not hidden and name in DRAWING_TOOLS and envp.get("status") == "ok":
                            # Item 1 after a drawing gesture: the canvas at the new revision draws what the final renderer
                            # draws (the gesture's ink cleared, the committed drawing shown).
                            rev_now = ((await rc(session, "draw.drawing.get", {})).get("result") or {}).get("rev")
                            await wait_canvas(session, lambda s_: any(d_.get("rev") == rev_now for d_ in s_.get("drawings", [])))
                            renders["gestures"][name] = await compare_render(session, run, out, f"after-{name}", f"after the {name} gesture")
                        content = (await rc(session, "draw.drawing.get", {})).get("result") or {}
                        sel = (await rc(session, "draw.selection.get", {})).get("result")
                        view = (await rc(session, "view.transform.get", {"viewId": "drawingView"})).get("result")
                        hist = (await rc(session, "edit.history.get", {"limit": 1000})).get("result") or {}
                        labels = [e.get("label") for e in (hist.get("entries") or [])[pos0:hist.get("position", 0)]]
                        # Back to the starting state: undo the gesture's step, reset what is not undoable.
                        for _ in labels:
                            await rc(session, "edit.undo", {})
                        await rc(session, "view.transform.reset", {"viewId": "drawingView"})
                        await rc(session, "colour.current.set", {"colourId": black})
                        await rc(session, "draw.selection.set", {"by": "none"})
                        if hidden:
                            await rc(session, "window.panel.set", {"panelId": "drawingView", "visible": True})
                            await e2e.gui_tree(session, run, lambda r: e2e.by_id(r, "drawingView.canvas") is not None, "the Drawing view returns")
                            await asyncio.sleep(1.0)
                        return envp, {"commands": params_of(envp), "content": strip_ids(content), "selection": strip_ids(sel),
                                      "view": view, "labels": labels, "status": envp.get("status"),
                                      "error": (envp.get("error") or {}).get("code")}
                    env_w, window = await one(False)
                    env_h, headless = await one(True)
                    same = window == headless
                    results.append({"tool": name, "same": same, "window": window, "headless": headless})
                    run.check(same and window["status"] == "ok" and len(window["labels"]) <= 1,
                              f"D-4 {name}: the window path equals the headless path (commands, content, one step, label)",
                              {"window": window, "headless": headless} if not same or window["status"] != "ok" else window["labels"])
                run.save("equivalence.json", results)
                run.save("renders.json", renders)

                # 2b. The selection overlay: the summary and a real capture show the handles; deselecting removes them.
                await rc(session, "tool.active.set", {"toolId": "select"})
                await select_fixture()
                st = await wait_canvas(session, lambda s_: (s_.get("selection") or {}).get("handles") == 8)
                run.check((st.get("selection") or {}).get("handles") == 8 and (st.get("selection") or {}).get("objects", 0) >= 1,
                          "selecting an object shows the selection overlay: eight handles and the outlines (canvas summary)", st.get("selection"))
                mp = (await rc(session, "draw.viewMapping.get", {"view": "drawingView"})).get("result") or {}
                sb = ((await rc(session, "draw.selection.get", {})).get("result") or {}).get("bounds") or {}
                centres = handle_points([sb.get("x", 0), sb.get("y", 0), sb.get("x", 0) + sb.get("width", 0), sb.get("y", 0) + sb.get("height", 0)],
                                        mp.get("drawingToView") or [1, 0, 0, -1, 0, 0])
                await asyncio.sleep(0.5)
                shot = png_of(await rc(session, "inspect.screenshot", {"target": "drawingView"}))
                view_w = (mp.get("viewSize") or {}).get("width") or 1
                seen = 0
                if shot:
                    (out / "selection-shown.png").write_bytes(shot)
                    seen = handles_in(shot, centres, png_rgba(shot)[0] / view_w)
                run.check(seen == 8, "a real capture shows the eight handle squares at the handle positions", {"found": seen, "centres": centres})
                await rc(session, "draw.selection.set", {"by": "none"})
                st = await wait_canvas(session, lambda s_: (s_.get("selection") or {}).get("handles") == 0)
                await asyncio.sleep(0.5)
                shot = png_of(await rc(session, "inspect.screenshot", {"target": "drawingView"}))
                gone = handles_in(shot, centres, png_rgba(shot)[0] / view_w) if shot else -1
                if shot:
                    (out / "selection-cleared.png").write_bytes(shot)
                run.check(gone == 0 and (st.get("selection") or {}).get("handles") == 0,
                          "deselecting removes the handles (summary and capture)", {"found": gone, "summary": st.get("selection")})
                await rc(session, "tool.active.set", {"toolId": "brush"})
                # The canvas reads its tool asynchronously: the next captures wait until it no longer draws the Select
                # tool's pivot (the window and view captures must show the same canvas).
                await wait_canvas(session, lambda s_: not (s_.get("selection") or {}).get("pivot"))

                # 3. Screenshots of what the user sees.
                shots = {}
                found = None
                for tgt in ("window", "drawingView"):
                    sh = await rc(session, "inspect.screenshot", {"target": tgt})
                    r = sh.get("result") or {}
                    run.check(sh.get("status") == "ok" and r.get("source") == "gui", f"D-6: inspect.screenshot {tgt} is the GUI capture",
                              {k: r.get(k) for k in ("source", "width", "height")} | {"error": sh.get("error")})
                    if r.get("png"):
                        data = base64.b64decode(r["png"])
                        (out / f"gui-{tgt}.png").write_bytes(data)
                        shots[tgt] = png_rgba(data)
                if "window" in shots and "drawingView" in shots:
                    W, H, cw, wrows = shots["window"]
                    w, h, cv, vrows = shots["drawingView"]
                    run.check(W > w and H > h, "the window capture is larger than the view's (chrome included)", {"window": [W, H], "view": [w, h]})
                    # Find the view capture inside the window capture (its first row, then every row).
                    first = [vrows[0][i:i + 3] for i in range(0, w * cv, cv)]
                    found = None
                    for y in range(0, H - h + 1):
                        row = wrows[y]
                        for x in range(0, W - w + 1):
                            if all(row[(x + i) * cw:(x + i) * cw + 3] == first[i] for i in range(0, w, max(1, w // 16))):
                                if all(wrows[y + yy][x * cw:(x + w) * cw][::cw] == vrows[yy][::cv] for yy in range(0, h, max(1, h // 32))):
                                    found = (x, y)
                                    break
                        if found:
                            break
                    run.check(found is not None, "the view capture is the window capture's pixels at the canvas's place", found)
                # The Preferences dialog appears in the window capture.
                await rc(session, "ui.preferences.open", {"category": "interface"})
                await e2e.gui_tree(session, run, lambda r: e2e.by_id(r, "preferences") is not None, "the preferences dialog opens")
                await asyncio.sleep(0.5)
                sh = await rc(session, "inspect.screenshot", {"target": "window"})
                r = sh.get("result") or {}
                if r.get("png") and "window" in shots:
                    data = base64.b64decode(r["png"])
                    (out / "gui-window-dialog.png").write_bytes(data)
                    W2, H2, c2, rows2 = png_rgba(data)
                    W, H, cw, wrows = shots["window"]
                    changed = sum(1 for y in range(0, min(H, H2), 4) for x in range(0, min(W, W2), 4)
                                  if rows2[y][x * c2:x * c2 + 3] != wrows[y][x * cw:x * cw + 3])
                    run.check(changed > 500, "D-6: the open dialog is in the window capture", {"changedSamples": changed})
                pressed = e2e.os_escape(e2e.APP_PID["pid"])
                await e2e.gui_tree(session, run, lambda r: e2e.by_id(r, "preferences") is None, "Escape closes Preferences")
                run.check(pressed.get("ok") is True, "Escape pressed through the OS", pressed)

                # 4. Windows: OS-level input.
                if sys.platform == "win32":
                    await rc(session, "tool.active.set", {"toolId": "brush"})
                    pos0 = ((await rc(session, "edit.history.get", {"limit": 1000})).get("result") or {}).get("position", 0)
                    # The canvas's place in the window's client area: where the view capture was found in the window
                    # capture (both are the webview's device pixels).
                    if found:
                        x0, y0 = found
                        os_drag(e2e.APP_PID["pid"], [(x0 + 120, y0 + 120), (x0 + 200, y0 + 160), (x0 + 280, y0 + 120)])
                        await asyncio.sleep(1.5)
                        hist = (await rc(session, "edit.history.get", {"limit": 1000})).get("result") or {}
                        labels = [e.get("label") for e in (hist.get("entries") or [])[pos0:hist.get("position", 0)]]
                        rev = ((await rc(session, "draw.drawing.get", {})).get("result") or {}).get("rev")
                        st = await wait_canvas(session, lambda s: any(d.get("rev") == rev for d in s.get("drawings", [])))
                        run.check(labels == ["Brush Tool"] and any(d.get("rev") == rev for d in st.get("drawings", [])),
                                  "an OS mouse drag in the canvas commits one Brush Tool step, drawn at the new revision",
                                  {"labels": labels, "canvas": st, "rev": rev})
                        # P2: a long agent gesture, interrupted by a user press.
                        long_events = drag([v(-200 + i * 0.2, 200 + (i % 50)) for i in range(3000)])
                        task = asyncio.create_task(e2e.call(session, "pointer", {"view": "drawingView", "tool": "brush", "events": long_events}))
                        await asyncio.sleep(0.3)
                        os_drag(e2e.APP_PID["pid"], [(x0 + 50, y0 + 50), (x0 + 52, y0 + 52)])
                        envp = await task
                        run.check((envp.get("error") or {}).get("code") == "ui.busy"
                                  and ((envp.get("error") or {}).get("details") or {}).get("reason") == "userInput",
                                  "P2: a user press during an agent gesture answers ui.busy, reason userInput (SI-135)", envp.get("error") or envp.get("status"))

                        # A Polyline ends its gesture however it ends (02 §7.2.5; C42B-R2-03): click, click and a real
                        # double click; click, click and Enter. The modal state is gone and agent writes run again.
                        async def modal_after(bound=3.0):
                            t1 = time.monotonic()
                            st_ = {}
                            while time.monotonic() - t1 < bound:
                                st_ = (await rc(session, "app.state.get", {})).get("result") or {}
                                if st_.get("modal") is None:
                                    break
                                await asyncio.sleep(0.1)
                            return st_.get("modal")

                        for ending in ("double click", "Enter"):
                            await rc(session, "tool.active.set", {"toolId": "polyline"})
                            await asyncio.sleep(0.5)
                            pos0 = ((await rc(session, "edit.history.get", {"limit": 1000})).get("result") or {}).get("position", 0)
                            os_drag(e2e.APP_PID["pid"], [(x0 + 100, y0 + 260), (x0 + 100, y0 + 260)])
                            os_drag(e2e.APP_PID["pid"], [(x0 + 200, y0 + 240), (x0 + 200, y0 + 240)])
                            if ending == "Enter":
                                os_key(0x0D)
                            else:
                                os_double_click(e2e.APP_PID["pid"], (x0 + 260, y0 + 260))
                            modal = await modal_after()
                            hist = (await rc(session, "edit.history.get", {"limit": 1000})).get("result") or {}
                            labels = [e.get("label") for e in (hist.get("entries") or [])[pos0:hist.get("position", 0)]]
                            write = await rc(session, "tool.active.set", {"toolId": "brush"})
                            run.check(modal is None and labels == ["Polyline Tool"] and write.get("status") == "ok",
                                      f"a Polyline ended by {ending} commits one Polyline Tool step and ends the gesture's modal state",
                                      {"modal": modal, "labels": labels, "write": write.get("error")})
                            if modal is not None:
                                e2e.os_escape(e2e.APP_PID["pid"])

                # 5. The float lifecycle (C42B-R1-05; 11 §7.5, §15.2): the Drawing view in a float window draws what the
                # final renderer draws; after the float closes and the view is docked again, the screenshot and pointer
                # reach the docked view (the window showing it is resolved at each call).
                mv = await rc(session, "window.panel.move", {"panelId": "cameraView", "area": "central"})
                fl = await rc(session, "window.panel.set", {"panelId": "drawingView", "floating": True,
                                                            "geometry": {"x": 100, "y": 100, "width": 900, "height": 700}})
                run.check(mv.get("status") == "ok" and fl.get("status") == "ok", "the Drawing view floats", [mv.get("error"), fl.get("error")])
                await wait_canvas(session, lambda s_: bool(s_.get("drawings")))
                await compare_render(session, run, out, "float", "the Drawing view in a float window")
                await rc(session, "window.panel.set", {"panelId": "drawingView", "visible": False})
                await asyncio.sleep(1.0)
                await rc(session, "window.panel.set", {"panelId": "drawingView", "visible": True, "floating": False})
                await rc(session, "window.panel.move", {"panelId": "drawingView", "area": "central"})
                await e2e.gui_tree(session, run, lambda r: e2e.by_id(r, "drawingView.canvas") is not None, "the Drawing view is docked again")
                await wait_canvas(session, lambda s_: bool(s_.get("drawings")))
                sh = await rc(session, "inspect.screenshot", {"target": "drawingView"})
                run.check(sh.get("status") == "ok" and (sh.get("result") or {}).get("source") == "gui",
                          "after the float closed and the view docked, inspect.screenshot drawingView captures the docked view", sh.get("error"))
                await compare_render(session, run, out, "docked", "the Drawing view docked again after its float closed")
                m = ((await rc(session, "draw.viewMapping.get", {"view": "drawingView"})).get("result") or {}).get("drawingToView") or m
                envp = await e2e.call(session, "pointer", {"view": "drawingView", "tool": "brush",
                                                           "events": drag([apply(m, -200, -230), apply(m, -100, -240)])})
                run.check(envp.get("status") == "ok", "pointer reaches the docked view", envp.get("error"))
                run.save("tree-final.json", (await e2e.tree(session)).get("root"))


def os_key(vk: int) -> None:
    """A key pressed and released through the operating system in the foreground window (Windows)."""
    import ctypes
    u = ctypes.windll.user32
    u.keybd_event(vk, 0, 0, 0)
    time.sleep(0.03)
    u.keybd_event(vk, 0, 2, 0)


def os_double_click(pid, p) -> None:
    """Two quick left clicks at client point `p` of the application's window: a real double click (Windows)."""
    import ctypes
    from ctypes import wintypes
    u = ctypes.windll.user32
    hwnds = []

    def cb(h, _):
        q = wintypes.DWORD()
        u.GetWindowThreadProcessId(h, ctypes.byref(q))
        if q.value == pid and u.IsWindowVisible(h):
            hwnds.append(h)
        return True

    u.EnumWindows(ctypes.WINFUNCTYPE(ctypes.c_bool, wintypes.HWND, wintypes.LPARAM)(cb), 0)
    if not hwnds:
        return
    origin = wintypes.POINT(0, 0)
    u.ClientToScreen(hwnds[0], ctypes.byref(origin))
    u.SetCursorPos(origin.x + int(p[0]), origin.y + int(p[1]))
    for _ in range(2):
        time.sleep(0.03)
        u.mouse_event(0x0002, 0, 0, 0, 0)
        time.sleep(0.03)
        u.mouse_event(0x0004, 0, 0, 0, 0)


def os_drag(pid, pts):
    """A left drag through points of the window's client area (Windows; moves the real cursor)."""
    import ctypes
    from ctypes import wintypes
    u = ctypes.windll.user32
    hwnds = []

    def cb(h, _):
        p = wintypes.DWORD()
        u.GetWindowThreadProcessId(h, ctypes.byref(p))
        if p.value == pid and u.IsWindowVisible(h):
            hwnds.append(h)
        return True

    u.EnumWindows(ctypes.WINFUNCTYPE(ctypes.c_bool, wintypes.HWND, wintypes.LPARAM)(cb), 0)
    if not hwnds:
        return
    h = hwnds[0]
    u.SetForegroundWindow(h)
    origin = wintypes.POINT(0, 0)
    u.ClientToScreen(h, ctypes.byref(origin))
    time.sleep(0.3)
    u.SetCursorPos(origin.x + int(pts[0][0]), origin.y + int(pts[0][1]))
    time.sleep(0.05)
    u.mouse_event(0x0002, 0, 0, 0, 0)
    for x, y in pts[1:]:
        time.sleep(0.03)
        u.SetCursorPos(origin.x + int(x), origin.y + int(y))
    time.sleep(0.05)
    u.mouse_event(0x0004, 0, 0, 0, 0)


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    app, out = sys.argv[1], Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    user = Path(tempfile.mkdtemp(prefix="tt-c42-e2e-"))
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
            asyncio.run(first_session(d["endpoint"], d["token"], run, out))
        finally:
            e2e.stop(proc)
    except Exception as ex:  # noqa: BLE001 - reported as a failure with its text
        run.check(False, "the run completed", repr(ex))
    finally:
        shutil.rmtree(user, ignore_errors=True)
    manifest = {"kind": "toontown.c42.gui-e2e", "platform": sys.platform, "seconds": round(time.monotonic() - started, 1),
                "failures": run.failures, "checks": len(run.transcript)}
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps(manifest, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
