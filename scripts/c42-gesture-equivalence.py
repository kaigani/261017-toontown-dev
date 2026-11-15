"""M4 C4.2b: gesture equivalence three ways, through the real window (plan §5; ruling D-4; C42B-R1-03).

Usage: python scripts/c42-gesture-equivalence.py <toontown binary> <toontown-headless binary> <out dir>

For every tool of the Drawing view (and the Select tool's handle gestures), one gesture is made in the running
application's window: on Windows through the operating system's real pointer (what the window collects is what the
OS delivered, coalesced; the Polyline ends with a real double click), on macOS as an agent's `pointer` call the window
plays (the runner has no OS input). The window's record of what it committed (the canvas's `lastGesture`, SI-141 item
9, read through `inspect_ui`) gives the samples as collected and the commands they produced. Then, on two further
engines, each the headless binary opened on its own copy of the same saved project:

- engine 2 replays the recorded samples through `pointer` (as drawing points, `space: "drawing"`, converted with the
  mapping the window collected them under: the headless view has another size);
- engine 3 receives the window's produced commands directly (`run_command`).

Each row must give, on all three engines: the same commands with the same parameters (ids of new objects aside;
numbers within 1e-6; a zoom anchor, a view pixel, compared as the drawing point under it on each engine), the same
drawing content (ids and revisions aside: each engine draws ids from its own stream), and at most one undo step with
the same label. Every row is undone on all three engines before the next. Receipts in <out dir>: `rows.json` (each
row's record, commands and comparison), `transcript.json`, `manifest.json` (kind `toontown.c42.gesture-equivalence`).
Exit 0 when every row holds.
"""

from __future__ import annotations

import asyncio
import contextlib
import importlib.util
import json
import os
import shutil
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location("c42e2e", ROOT / "scripts" / "c42-gui-e2e.py")
c42 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(c42)
e2e = c42.e2e

from mcp import ClientSession, StdioServerParameters, types  # noqa: E402
from mcp.client.stdio import stdio_client  # noqa: E402

TOLERANCE = 1e-6
RECORD_BOUND_S = 30.0


def sc(r) -> dict:
    return getattr(r, "structuredContent", None) or getattr(r, "structured_content", None) or {}


def close(a, b) -> bool:
    """Equal values, numbers within `TOLERANCE`."""
    if isinstance(a, bool) or isinstance(b, bool):
        return a == b
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return abs(a - b) <= TOLERANCE
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(close(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(close(x, y) for x, y in zip(a, b))
    return a == b


def inverse(m: list) -> list:
    a, b, c, d, e, f = m
    det = a * d - b * c
    ia, ib, ic, id_ = d / det, -b / det, -c / det, a / det
    return [ia, ib, ic, id_, -(ia * e + ic * f), -(ib * e + id_ * f)]


def normalised(cmds: list, view_to_drawing: list) -> list:
    """Commands with new-object ids aside, without the tool's activation, and view-pixel anchors as the drawing point
    under them."""
    out = []
    for cmd in cmds:
        # The tool's activation is not the gesture's command: an agent's `pointer` call carries it, the user chose
        # the tool beforehand.
        if cmd.get("commandId") == "tool.active.set":
            continue
        params = c42.strip_ids(json.loads(json.dumps(cmd.get("params"))))
        if cmd.get("commandId") == "view.zoom.set" and isinstance(params, dict) and isinstance(params.get("anchor"), dict):
            x, y = c42.apply(view_to_drawing, params["anchor"]["x"], params["anchor"]["y"])
            params["anchor"] = {"drawingX": x, "drawingY": y}
        out.append({"commandId": cmd.get("commandId"), "params": params})
    return out


class Engine:
    """One engine's MCP session: `run` a command, `tool` call a tool."""

    def __init__(self, name: str, session: ClientSession):
        self.name, self.s = name, session

    async def run(self, cid: str, params: dict | None = None) -> dict:
        return sc(await self.s.call_tool("run_command", {"id": cid, "params": params or {}}))

    async def tool(self, name: str, args: dict) -> dict:
        return sc(await self.s.call_tool(name, args))

    async def result(self, cid: str, params: dict | None = None):
        return (await self.run(cid, params)).get("result") or {}

    async def position(self) -> int:
        return (await self.result("edit.history.get", {"limit": 1000})).get("position", 0)

    async def labels_since(self, pos0: int) -> list:
        h = await self.result("edit.history.get", {"limit": 1000})
        return [x.get("label") for x in (h.get("entries") or [])[pos0:h.get("position", 0)]]

    async def mapping(self) -> dict:
        return await self.result("draw.viewMapping.get", {"view": "drawingView"})


async def job(engine: Engine, env: dict) -> dict:
    jid = (env.get("result") or {}).get("jobId")
    if not jid:
        return env
    return await engine.tool("job_wait", {"jobId": jid, "timeoutMs": 60000})


async def open_project(engine: Engine, path: Path) -> None:
    env = await engine.run("project.open", {"path": str(path), "onUnsaved": "fail"})
    if env.get("status") != "ok":
        raise RuntimeError(f"{engine.name}: project.open: {json.dumps(env)[:400]}")
    start = time.monotonic()
    while time.monotonic() - start < 60:
        if (await engine.run("doc.state.get", {})).get("status") == "ok":
            return
        await asyncio.sleep(0.25)
    raise RuntimeError(f"{engine.name}: the project did not open")


def canvas_offset(window_png: bytes, view_png: bytes):
    """Where the view's capture sits in the window's capture (device pixels of the webview's client area)."""
    W, H, cw, wrows = c42.png_rgba(window_png)
    w, h, cv, vrows = c42.png_rgba(view_png)
    first = [vrows[0][i:i + 3] for i in range(0, w * cv, cv)]
    for y in range(0, H - h + 1):
        row = wrows[y]
        for x in range(0, W - w + 1):
            if all(row[(x + i) * cw:(x + i) * cw + 3] == first[i] for i in range(0, w, max(1, w // 16))):
                if all(wrows[y + yy][x * cw:(x + w) * cw][::cw] == vrows[yy][::cv] for yy in range(0, h, max(1, h // 32))):
                    return x, y
    return None


def on_screen(pid: int, pts) -> bool:
    """Whether client points of the application's window are on the screen (Windows: a runner's display is small and
    the window may extend below it, where the cursor cannot go)."""
    import ctypes
    from ctypes import wintypes
    u = ctypes.windll.user32
    hwnds = []

    def cb(hw, _):
        q = wintypes.DWORD()
        u.GetWindowThreadProcessId(hw, ctypes.byref(q))
        if q.value == pid and u.IsWindowVisible(hw):
            hwnds.append(hw)
        return True

    u.EnumWindows(ctypes.WINFUNCTYPE(ctypes.c_bool, wintypes.HWND, wintypes.LPARAM)(cb), 0)
    if not hwnds:
        return False
    origin = wintypes.POINT(0, 0)
    u.ClientToScreen(hwnds[0], ctypes.byref(origin))
    w, h = u.GetSystemMetrics(0), u.GetSystemMetrics(1)
    return all(0 <= origin.x + x < w - 2 and 0 <= origin.y + y < h - 2 for x, y in pts)


def os_double_click(pid: int, p) -> None:
    """Two quick left clicks at client point `p` (a real double click; Windows)."""
    import ctypes
    from ctypes import wintypes
    u = ctypes.windll.user32
    hwnds = []

    def cb(hw, _):
        q = wintypes.DWORD()
        u.GetWindowThreadProcessId(hw, ctypes.byref(q))
        if q.value == pid and u.IsWindowVisible(hw):
            hwnds.append(hw)
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


async def main(app: Engine, h2: Engine, h3: Engine, run, out: Path, work: Path, os_input: bool) -> list:
    rc = app.run
    await rc("project.create", {"name": "Equivalence", "onUnsaved": "fail"})
    await rc("workspace.select", {"name": "Drawing"})
    eid = ((await app.result("element.create", {"type": "drawing", "name": "Ink"})).get("elementIds") or [None])[0]
    await rc("element.drawing.create", {"elementId": eid, "name": "1", "exposeFrom": 1, "exposeTo": 1})
    found_sw = await app.result("doc.object.find", {"type": "swatch", "name": "Colour 2"})
    items = found_sw.get("items") or found_sw.get("matches") or []
    black = next((it.get("id") or it.get("key") or it.get("colourId") for it in items if isinstance(it, dict)), None)
    # The fixtures of scripts/c42-gui-e2e.py: a painted rectangle, an unpainted one, two nearly touching lines.
    await rc("app.context.set", {"elementId": eid, "frame": 1})
    await rc("colour.current.set", {"colourId": black})
    await rc("draw.rectangle.add", {"from": {"x": -150, "y": -60}, "to": {"x": -30, "y": 60}})
    await rc("colour.paint.apply", {"points": [{"x": -90, "y": 0}]})
    await rc("draw.rectangle.add", {"from": {"x": 30, "y": -60}, "to": {"x": 150, "y": 60}})
    await rc("draw.line.add", {"from": {"x": -150, "y": 120}, "to": {"x": -10, "y": 120}, "layer": "top"})
    await rc("draw.line.add", {"from": {"x": -6, "y": 118}, "to": {"x": 150, "y": 118}, "layer": "top"})
    saved = work / "equivalence.animpkg"
    env = await job(app, await rc("project.saveAs", {"path": str(saved), "onExists": "overwrite"}))
    run.check(env.get("status") == "ok", "the application saves the fixture project", env.get("error"))
    for engine, copy in ((h2, work / "engine2.animpkg"), (h3, work / "engine3.animpkg")):
        shutil.copyfile(saved, copy)
        await open_project(engine, copy)
    engines = (app, h2, h3)
    for engine in engines:
        await engine.run("app.context.set", {"elementId": eid, "frame": 1})
        await engine.run("colour.current.set", {"colourId": black})

    # The canvas's place in the window's client area (Windows OS input) and the mapping of the window's view.
    await c42.wait_canvas(app.s, lambda s_: bool(s_.get("drawings")))
    m_app = (await app.mapping()).get("drawingToView")
    offset, ratio = None, 1.0
    if os_input:
        wpng = c42.png_of(await rc("inspect.screenshot", {"target": "window"}))
        vpng = c42.png_of(await rc("inspect.screenshot", {"target": "drawingView"}))
        if wpng and vpng:
            offset = canvas_offset(wpng, vpng)
            ratio = c42.png_rgba(vpng)[0] / (((await app.mapping()).get("viewSize") or {}).get("width") or 1)
        run.check(offset is not None, "the canvas is located in the window (OS input)", offset)

    sel_rect = {"by": "rect", "rect": {"x": -160, "y": -70, "width": 140, "height": 140}, "rectRule": "inside", "mode": "replace"}
    await rc("draw.selection.set", sel_rect)
    sb = (await app.result("draw.selection.get", {})).get("bounds") or {"x": -150, "y": -60, "width": 120, "height": 120}
    await rc("draw.selection.set", {"by": "none"})
    x0, y0 = sb["x"], sb["y"]
    x1, y1 = x0 + sb["width"], y0 + sb["height"]
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
    pivot = (await app.result("draw.drawing.get", {})).get("pivot") or {"x": 0, "y": 0}
    out_c = (x1 + 8, y1 + 8)
    # name, tool, drawing points (a drag through them), modifiers, the selection set before it.
    rows = [
        ("brush", "brush", [(-200, 140), (-160, 150), (-120, 160), (-80, 155), (-40, 150)], [], False),
        ("pencil", "pencil", [(-200, -100), (-150, -115), (-100, -130), (-50, -115), (0, -100)], [], False),
        ("rectangle", "rectangle", [(-60, -150), (-10, -125), (40, -100)], [], False),
        ("ellipse", "ellipse", [(60, -150), (110, -125), (160, -100)], [], False),
        ("line", "line", [(-200, 150), (0, 155), (200, 160)], [], False),
        ("polyline", "polyline", "polyline", [], False),
        ("select", "select", [(-170, -80), (-90, 0), (-10, 80)], [], False),
        ("eraser", "eraser", [(-160, 0), (-140, 5), (-120, 10), (-100, 5), (-80, 0)], [], False),
        ("cutter", "cutter", [(-170, -80), (-90, -80), (-90, 80), (-170, 80), (-170, -80)], [], False),
        ("scissor", "scissor", [(-160, -20), (-150, 0), (-140, 20)], [], False),
        ("paint", "paint", [(90, 0), (90, 0)], [], False),
        ("unpaint", "unpaint", [(-90, 0), (-90, 0)], [], False),
        ("paintUnpainted", "paintUnpainted", [(90, 0), (90, 0)], [], False),
        ("dropper", "dropper", [(-90, 0), (-90, 0)], [], False),
        ("closeGap", "closeGap", [(-10, 120), (-8, 119), (-6, 118)], [], False),
        ("zoom (click)", "zoom", [(40, 40), (40, 40)], [], False),
        ("grabber", "grabber", [(0, 0), (20, 10), (40, 20)], [], False),
        ("select: corner scale", "select", [(x1, y1), (x1 + 10, y1 + 6), (x1 + 20, y1 + 12)], [], True),
        ("select: constrained corner", "select", [(x1, y1), (x1 + 10, y1 + 6), (x1 + 20, y1 + 12)], ["shift"], True),
        ("select: side scale", "select", [(x1, cy), (x1 + 20, cy + 3), (x1 + 40, cy + 5)], [], True),
        ("select: rotate", "select", [out_c, (cx - (out_c[1] - cy), cy + (out_c[0] - cx))], [], True),
        ("select: drawing pivot", "select", [(pivot["x"], pivot["y"]), (pivot["x"] + 30, pivot["y"] + 20)], [], False),
    ]
    results = []
    # `C42_EQUIV_ROWS=brush,pencil` runs only those rows (a debugging aid; CI runs every row).
    only = {x for x in os.environ.get("C42_EQUIV_ROWS", "").split(",") if x}
    for name, tool, pts, mods, selected in rows:
        if only and name not in only:
            continue
        # The same starting state on every engine.
        for engine in engines:
            await engine.run("draw.selection.set", sel_rect if selected else {"by": "none"})
            await engine.run("draw.pivot.set", {"point": pivot})
            await engine.run("view.transform.reset", {"viewId": "drawingView"})
            await engine.run("colour.current.set", {"colourId": black})
        await rc("tool.active.set", {"toolId": tool})
        before = json.dumps((await c42.canvas_state(app.s)).get("lastGesture"))
        pos = [await engine.position() for engine in engines]
        m_app = (await app.mapping()).get("drawingToView")
        v = lambda p: c42.apply(m_app, p[0], p[1])  # noqa: E731
        if pts == "polyline":
            vpts = [v((170, 100)), v((220, 140))]
        else:
            vpts = [v(p) for p in pts]
        # 1. The gesture in the window. OS input needs every point on the canvas (a runner's window is smaller).
        size = (await app.mapping()).get("viewSize") or {}
        on_canvas = all(4 <= q[0] <= size.get("width", 0) - 4 and 4 <= q[1] <= size.get("height", 0) - 4 for q in vpts)
        client = lambda q: (offset[0] + q[0] * ratio, offset[1] + q[1] * ratio) if offset else q  # noqa: E731
        if os_input and offset is not None and not mods and on_canvas and on_screen(e2e.APP_PID["pid"], [client(q) for q in vpts]):
            if pts == "polyline":
                # One click, then a real double click at the second anchor (its two clicks and the double click end
                # the gesture; a further click would start the next Polyline and hold the user's gesture open).
                c42.os_drag(e2e.APP_PID["pid"], [client(vpts[0]), client(vpts[0])])
                os_double_click(e2e.APP_PID["pid"], client(vpts[1]))
            else:
                c42.os_drag(e2e.APP_PID["pid"], [client(q) for q in vpts])
            how = "os"
        else:
            # macOS, a modifier the OS drag does not hold, or points off a small canvas or screen: the agent's
            # pointer, played through the window.
            if pts == "polyline":
                events = c42.drag([vpts[0], vpts[0]]) + c42.drag([vpts[1], vpts[1]]) + [{"type": "doubleClick", "x": vpts[1][0], "y": vpts[1][1]}]
            else:
                events = c42.drag(vpts)
            for ev in events:
                ev["modifiers"] = list(mods)
            await app.tool("pointer", {"view": "drawingView", "tool": tool, "events": events})
            how = "agent"
        start = time.monotonic()
        rec = None
        while time.monotonic() - start < RECORD_BOUND_S:
            st = await c42.canvas_state(app.s)
            if st.get("lastGesture") is not None and json.dumps(st.get("lastGesture")) != before:
                rec = st["lastGesture"]
                break
            await asyncio.sleep(0.25)
        row = {"name": name, "tool": tool, "input": how, "record": rec}
        if rec is None or not rec.get("events"):
            run.check(False, f"three-way {name}: the window committed and recorded the gesture", row)
            results.append(row)
            run.save("rows.json", results)
            continue
        # 2. Engine 2: the recorded samples, as drawing points through the window's own mapping.
        to_d = inverse(rec["drawingToView"])
        replay = []
        for ev in rec["events"]:
            ev = dict(ev)
            ev["x"], ev["y"] = c42.apply(to_d, ev["x"], ev["y"])
            replay.append(ev)
        m2 = await h2.mapping()
        env2 = await h2.tool("pointer", {"view": "drawingView", "tool": rec["tool"], "space": "drawing",
                                         "pointerType": rec.get("pointerType", "mouse"), "events": replay})
        cmds2 = (env2.get("result") or {}).get("commands") or []
        # 3. Engine 3: the window's commands, sent directly.
        direct = []
        for cmd in rec.get("commands") or []:
            direct.append(await h3.run(cmd["commandId"], cmd.get("params") or {}))
        m3 = await h3.mapping()
        contents = [c42.strip_ids(await engine.result("draw.drawing.get", {})) for engine in engines]
        labels = [await engine.labels_since(p) for engine, p in zip(engines, pos)]
        win_cmds = normalised(rec.get("commands") or [], m_app and inverse(m_app))
        h2_cmds = normalised(cmds2, m2.get("viewToDrawing") or [1, 0, 0, 1, 0, 0])
        statuses = [rec.get("status"), env2.get("status"), "ok" if all(d.get("status") == "ok" for d in direct) else "error"]
        same_cmds = close(win_cmds, h2_cmds)
        same_content = close(contents[0], contents[1]) and close(contents[0], contents[2])
        same_labels = labels[0] == labels[1] == labels[2] and len(labels[0]) <= 1
        row.update({"commands": {"window": win_cmds, "engine2": h2_cmds}, "labels": labels, "statuses": statuses,
                    "sameCommands": same_cmds, "sameContent": same_content, "sameLabels": same_labels,
                    "mappings": {"window": m_app, "engine2": m2.get("drawingToView"), "engine3": m3.get("drawingToView")}})
        if not same_content:
            row["contents"] = contents
        run.check(same_cmds and same_content and same_labels and statuses == ["ok", "ok", "ok"],
                  f"three-way {name} ({how}): the window, the recorded samples replayed headless and the produced commands sent "
                  "directly give the same commands, content and undo step",
                  {k: row[k] for k in ("sameCommands", "sameContent", "sameLabels", "statuses", "labels")})
        results.append(row)
        run.save("rows.json", results)
        run.save("transcript.json", run.transcript)
        # Back to the fixture on every engine.
        for engine, lab in zip(engines, labels):
            for _ in lab:
                await engine.run("edit.undo", {})
    return results


async def session_all(app_bin: str, headless: str, run, out: Path, work: Path) -> list:
    user = work / "user-a"
    (user / "Settings").mkdir(parents=True)
    seed = {"agent.policy.defaults": {"policy": "autonomous", "maxJobs": 2, "maxBatchSteps": 1000, "maxInlineImagePx": 2048,
                                      "maxInlineBytes": 52428800, "askInApp": True, "letAgentMoveView": True, "highlightAgentChanges": True}}
    (user / "Settings" / "preferences.json").write_text(json.dumps({"format": "toontown-settings", "kind": "preferences", "formatVersion": "1.0", "data": seed}), encoding="utf-8")

    async def roots(_ctx):
        return types.ListRootsResult(roots=[types.Root(uri=work.resolve().as_uri(), name="work")])

    discovery = work / "discovery.json"
    proc = e2e.launch(app_bin, user, discovery)
    try:
        d = e2e.wait_discovery(discovery, proc)
        async with contextlib.AsyncExitStack() as stack:
            client = await stack.enter_async_context(
                e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {d['token']}"}, timeout=e2e.httpx2.Timeout(60.0, read=300.0)))
            streams = await stack.enter_async_context(e2e.streamable_http_client(d["endpoint"], http_client=client))
            a = await stack.enter_async_context(ClientSession(streams[0], streams[1], list_roots_callback=roots))
            await a.initialize()
            await e2e.gui_tree(a, run, lambda r: e2e.by_id(r, "startScreen") is not None, "the attached GUI is up")
            heads = []
            for k in (2, 3):
                params = StdioServerParameters(command=headless, args=["--headless", "--mcp-stdio", "--user-dir", str(work / f"user-h{k}"), "--policy", "autonomous"])
                r, w = await stack.enter_async_context(stdio_client(params))
                hs = await stack.enter_async_context(ClientSession(r, w, list_roots_callback=roots))
                await hs.initialize()
                heads.append(Engine(f"engine{k}", hs))
            return await main(Engine("window", a), heads[0], heads[1], run, out, work, sys.platform == "win32")
    finally:
        e2e.stop(proc)


def run_main() -> int:
    if len(sys.argv) != 4:
        print(__doc__)
        return 2
    app_bin, headless, out = sys.argv[1], sys.argv[2], Path(sys.argv[3])
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    work = Path(tempfile.mkdtemp(prefix="tt-c42-equiv-"))
    started = time.monotonic()
    rows: list = []
    try:
        rows = asyncio.run(session_all(app_bin, headless, run, out, work))
    except Exception as ex:  # noqa: BLE001 - reported as a failure with its text
        run.check(False, "the comparison ran", repr(ex))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    manifest = {"kind": "toontown.c42.gesture-equivalence", "platform": sys.platform, "seconds": round(time.monotonic() - started, 1),
                "rows": len(rows), "input": sorted({r.get("input") for r in rows if r.get("input")}), "failures": run.failures}
    run.save("rows.json", rows)
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps(manifest, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(run_main())
