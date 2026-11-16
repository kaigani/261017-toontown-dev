"""M4 C4.3b: gesture equivalence three ways in the Camera view, through the real window (plan §3.4, §5; rulings D-4,
D-22, D-23).

Usage: python scripts/c43-gesture-equivalence.py <toontown binary> <toontown-headless binary> <out dir>

The Camera view's counterpart of scripts/c42-gesture-equivalence.py, whose engines and helpers it reuses. For each row
(the scene tools' click, moves, corner scale, skew and pivot; drawing tools used in the Camera view), one gesture is
made in the running application's Camera view: on Windows through the operating system's real pointer, on macOS as an
agent's `pointer` call the window plays. The window's record of what it committed (the canvas's `lastGesture`) gives
the samples and the commands they produced. Then, on two further engines, each the headless binary opened on its own
copy of the same saved project, with the Camera view active:

- engine 2 replays the recorded samples through `pointer` in a space that does not depend on the view's size: a scene
  tool's samples in camera space (the window's view pixels converted by `inspect.coordinates.convert`), a drawing
  tool's in drawing units (through the mapping the window collected them under);
- engine 3 receives the window's produced commands directly (`run_command`).

Each row must give, on all three engines, exactly (ruling D-4): the same commands with the same parameters, the same
content (Bob's element for a scene tool, the current drawing for a drawing tool), and at most one undo step with the
same label. Every row is undone on all three engines before the next. Receipts: `rows.json`, `transcript.json`,
`manifest.json` (kind `toontown.c43.gesture-equivalence`). Exit 0 when every row holds.
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


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


eq = _load("c42equiv", "c42-gesture-equivalence.py")
e6 = _load("c43e6", "c43-e6-coexistence.py")
c42, e2e = eq.c42, eq.e2e
Engine = eq.Engine

from mcp import ClientSession, StdioServerParameters, types  # noqa: E402
from mcp.client.stdio import stdio_client  # noqa: E402

RECORD_BOUND_S = 30.0
#: The commands that choose a tool: not the gesture's command (an agent's `pointer` carries them, the user chose the
#: tool beforehand).
TOOL_CHOICE = {"tool.active.set", "anim.tool.set"}


#: Engine 2 replays through the headless Camera view, whose size differs from the window's: a quantity computed through
#: the view's geometry (a scene move's parent-space delta) can differ in its last bits (SI-158). Engine 2 is compared
#: exactly, else within this relative tolerance, and each row says which; the window and engine 3 are compared exactly.
REPLAY_TOLERANCE = 1e-9


def largest_difference(a, b) -> float | None:
    """The largest relative difference between two values equal in shape, or None when they differ otherwise."""
    if isinstance(a, bool) or isinstance(b, bool):
        return 0.0 if a == b else None
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return abs(a - b) / max(1.0, abs(a), abs(b))
    if isinstance(a, dict) and isinstance(b, dict):
        if a.keys() != b.keys():
            return None
        ds = [largest_difference(a[k], b[k]) for k in a]
        return None if any(d is None for d in ds) else max(ds, default=0.0)
    if isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            return None
        ds = [largest_difference(x, y) for x, y in zip(a, b)]
        return None if any(d is None for d in ds) else max(ds, default=0.0)
    return 0.0 if a == b else None


def replay_agrees(window, replay) -> tuple:
    """(agrees, exact, largest difference) of engine 2's value against the window's."""
    if window == replay:
        return True, True, 0.0
    d = largest_difference(window, replay)
    return d is not None and d <= REPLAY_TOLERANCE, False, d


def normalised(cmds: list) -> list:
    return [{"commandId": c.get("commandId"), "params": c.get("params")} for c in cmds if c.get("commandId") not in TOOL_CHOICE]


async def camera_state(engine: Engine) -> dict:
    res = (await engine.tool("inspect_ui", {"target": "window"})).get("result") or {}
    node = c42.find(res.get("root") or {}, lambda n: n.get("id") == "cameraView.canvas") or {}
    try:
        return json.loads(node.get("value") or "{}")
    except json.JSONDecodeError:
        return {}


async def main(app: Engine, h2: Engine, h3: Engine, run, out: Path, work: Path, os_input: bool) -> list:
    rc = app.run
    await rc("project.create", {"name": "CameraEquivalence", "onUnsaved": "fail"})
    found = await app.result("doc.object.find", {"type": "swatch", "name": "Colour 2"})
    items = found.get("items") or found.get("matches") or []
    black = next((it.get("id") or it.get("key") or it.get("colourId") for it in items if isinstance(it, dict)), None)
    found = await app.result("doc.object.find", {"type": "swatch", "name": "Colour 3"})
    items = found.get("items") or found.get("matches") or []
    other = next((it.get("id") or it.get("key") or it.get("colourId") for it in items if isinstance(it, dict)), None)
    run.check(black is not None and other is not None, "the default palette's Colour 2 and Colour 3 are found", [black, other])
    ids = {}
    for name, shape in (("Bob", ("draw.rectangle.add", {"from": {"x": -120, "y": -90}, "to": {"x": 120, "y": 90}})),
                        ("Ann", ("draw.ellipse.add", {"from": {"x": -80, "y": -80}, "to": {"x": 80, "y": 80}}))):
        ids[name] = ((await app.result("element.create", {"type": "drawing", "name": name})).get("elementIds") or [None])[0]
        await rc("element.drawing.create", {"elementId": ids[name], "name": "1", "exposeFrom": 1, "exposeTo": 24})
        await rc("app.context.set", {"elementId": ids[name], "frame": 1})
        await rc("colour.current.set", {"colourId": black})
        await rc(shape[0], shape[1])
        await rc("colour.paint.apply", {"points": [{"x": 0, "y": 0}]})
    await rc("anim.transform.set", {"nodeIds": [ids["Ann"]], "target": "static", "position": {"x": -1.5, "y": 0.5, "z": -1}})
    saved = work / "camera-equivalence.animpkg"
    env = await eq.job(app, await rc("project.saveAs", {"path": str(saved), "onExists": "overwrite"}))
    run.check(env.get("status") == "ok", "the application saves the fixture project", env.get("error"))
    for engine, copy in ((h2, work / "engine2.animpkg"), (h3, work / "engine3.animpkg")):
        shutil.copyfile(saved, copy)
        await eq.open_project(engine, copy)
    engines = (app, h2, h3)
    for engine in engines:
        await engine.run("app.context.set", {"elementId": ids["Bob"], "frame": 1})
        await engine.run("colour.current.set", {"colourId": black})
        await engine.run("view.active.set", {"viewId": "cameraView"})

    # The canvas's place in the window (OS input) and its device-pixel ratio.
    start = time.monotonic()
    while time.monotonic() - start < c42.CANVAS_BOUND_S and len((await camera_state(app)).get("drawings") or []) < 2:
        await asyncio.sleep(0.25)
    place, ratio = None, 1.0
    if os_input:
        place = await e6.locate_view(app.s, e2e.run_cmd, "cameraView")
        listed = await app.result("view.list", {})
        views = listed.get("views") or [] if isinstance(listed, dict) else listed
        logical = next((v.get("size") for v in views if v.get("viewId") == "cameraView"), None) or {}
        if place and logical.get("width"):
            ratio = place[2] / logical["width"]
        run.check(place is not None, "the Camera view's canvas is located in the window (OS input)", place)

    async def mapping() -> list:
        return (await app.result("draw.viewMapping.get", {"view": "cameraView"})).get("drawingToView") or [1, 0, 0, 1, 0, 0]

    # Bob's box in view pixels (the union of his drawing's bounds through the mapping: the scene tools' furniture).
    async def furniture() -> tuple:
        m = await mapping()
        b = (await app.result("draw.bounds.get", {})).get("bounds") or {}
        xs, ys = [b["x"], b["x"] + b["width"]], [b["y"], b["y"] + b["height"]]
        corners = [c42.apply(m, x, y) for x in xs for y in ys]
        u = [min(p[0] for p in corners), min(p[1] for p in corners), max(p[0] for p in corners), max(p[1] for p in corners)]
        return u, c42.apply(m, 0, 0), m

    # name, family, tool, the view points (a function of Bob's box, his origin and the mapping), Bob selected before.
    def du(*pts):
        return lambda _u, _o, m: [c42.apply(m, x, y) for x, y in pts]

    rows = [
        ("scene select: click", "scene", "select", du((60, 40), (60, 40)), False),
        ("scene select: move", "scene", "select", du((60, 40), (80, 50), (100, 60)), True),
        ("scene transform: move", "scene", "transform", du((60, 40), (80, 50), (100, 60)), True),
        ("scene transform: corner scale", "scene", "transform", lambda u, _o, _m: [(u[2], u[1]), (u[2] + 10, u[1] - 6), (u[2] + 20, u[1] - 12)], True),
        ("scene transform: skew", "scene", "transform", lambda u, _o, _m: [((u[0] + u[2]) / 2, u[1] - 12), ((u[0] + u[2]) / 2 + 10, u[1] - 12), ((u[0] + u[2]) / 2 + 20, u[1] - 12)], True),
        ("scene transform: pivot", "scene", "transform", lambda _u, o, _m: [o, (o[0] + 8, o[1] - 3), (o[0] + 15, o[1] - 5)], True),
        ("brush", "drawing", "brush", du((-60, 20), (-40, 35), (-20, 45), (5, 30), (30, 10)), False),
        ("rectangle", "drawing", "rectangle", du((-60, -40), (-10, -25), (40, -10)), False),
        ("ellipse", "drawing", "ellipse", du((-40, -60), (0, -40), (40, -20)), False),
        ("line", "drawing", "line", du((-100, 60), (0, 65), (100, 70)), False),
        ("eraser", "drawing", "eraser", du((-100, 0), (-80, 5), (-60, 10), (-40, 5), (-20, 0)), False),
        ("cutter", "drawing", "cutter", du((50, 100), (140, 100), (140, -100), (50, -100), (50, 100)), False),
        ("paint", "drawing", "paint", du((0, 0), (0, 0)), False),
        ("select (drawing)", "drawing", "select", du((-130, 100), (0, 0), (130, -100)), False),
    ]
    results = []
    only = {x for x in os.environ.get("C43_EQUIV_ROWS", "").split(",") if x}
    for name, family, tool, points, selected in rows:
        if only and name not in only:
            continue
        for engine in engines:
            await engine.run("edit.selection.set", {"context": "staging", "ids": [ids["Bob"]] if selected else []})
            await engine.run("draw.selection.set", {"by": "none"})
            await engine.run("view.transform.reset", {"viewId": "cameraView"})
            # The Paint row paints Bob's black rectangle in another colour (a change, so an undo step).
            await engine.run("colour.current.set", {"colourId": other if tool == "paint" else black})
        # The user's choice of tool, before the gesture: the scene tool, or the drawing tool (which makes the Camera
        # view run the drawing family, 05 lines 29-31).
        if family == "scene":
            await rc("anim.tool.set", {"tool": tool})
        else:
            await rc("tool.active.set", {"toolId": tool})
        await asyncio.sleep(0.3)
        before = json.dumps((await camera_state(app)).get("lastGesture"))
        pos = [await engine.position() for engine in engines]
        u, o, m = await furniture()
        vpts = points(u, o, m)
        listed = await app.result("view.list", {})
        views = listed.get("views") or [] if isinstance(listed, dict) else listed
        size = next((v.get("size") for v in views if v.get("viewId") == "cameraView"), None) or {}
        on_canvas = all(4 <= q[0] <= size.get("width", 0) - 4 and 4 <= q[1] <= size.get("height", 0) - 4 for q in vpts)
        client = lambda q: (place[0] + q[0] * ratio, place[1] + q[1] * ratio) if place else q  # noqa: E731
        if os_input and place is not None and on_canvas and eq.on_screen(e2e.APP_PID["pid"], [client(q) for q in vpts]):
            times: dict = {}
            e6.os_drag(e2e.APP_PID["pid"], [client(q) for q in vpts], 0.03 * (len(vpts) - 1), 0.05, times)
            how = "os" if "error" not in times else f"os refused: {times['error']}"
        else:
            req = {"view": "cameraView", "events": c42.drag(vpts)}
            if not (family == "drawing" and tool == "select"):
                req["tool"] = tool
            await app.tool("pointer", req)
            how = "agent"
        start = time.monotonic()
        rec = None
        while time.monotonic() - start < RECORD_BOUND_S:
            st = await camera_state(app)
            if st.get("lastGesture") is not None and json.dumps(st.get("lastGesture")) != before:
                rec = st["lastGesture"]
                break
            await asyncio.sleep(0.25)
        row = {"name": name, "family": family, "tool": tool, "input": how, "record": rec}
        if rec is None or not rec.get("events"):
            run.check(False, f"three-way {name}: the window committed and recorded the gesture", row)
            results.append(row)
            run.save("rows.json", results)
            continue
        # 2. Engine 2: the recorded samples in a size-independent space.
        events = [dict(ev) for ev in rec["events"]]
        if family == "scene":
            conv = await app.result("inspect.coordinates.convert", {"from": "view", "to": "camera", "view": "cameraView",
                                                                   "points": [{"x": ev["x"], "y": ev["y"]} for ev in events]})
            for ev, p in zip(events, conv.get("points") or []):
                ev["x"], ev["y"] = p["x"], p["y"]
            req2 = {"view": "cameraView", "tool": rec.get("tool") or tool, "space": "camera", "events": events}
        else:
            to_d = eq.inverse(rec["drawingToView"])
            for ev in events:
                ev["x"], ev["y"] = c42.apply(to_d, ev["x"], ev["y"])
            req2 = {"view": "cameraView", "space": "drawing", "pointerType": rec.get("pointerType", "mouse"), "events": events}
            if tool == "select":
                await h2.run("tool.active.set", {"toolId": "select"})
            else:
                req2["tool"] = rec.get("tool") or tool
        env2 = await h2.tool("pointer", req2)
        cmds2 = (env2.get("result") or {}).get("commands") or []
        # 3. Engine 3: the window's commands, sent directly.
        direct = [await h3.run(c["commandId"], c.get("params") or {}) for c in rec.get("commands") or []]
        if family == "scene":
            contents = [(await e.result("doc.object.get", {"ref": ids["Bob"]})).get("object") for e in engines]
        else:
            contents = [await e.result("draw.drawing.get", {}) for e in engines]
        labels = [await e.labels_since(p) for e, p in zip(engines, pos)]
        win_cmds, h2_cmds = normalised(rec.get("commands") or []), normalised(cmds2)
        statuses = [rec.get("status"), env2.get("status"), "ok" if all(d.get("status") == "ok" for d in direct) else "error"]
        cmds_ok, cmds_exact, cmds_diff = replay_agrees(win_cmds, h2_cmds)
        content_ok, content_exact, content_diff = replay_agrees(contents[0], contents[1])
        # The window and the commands sent directly: exactly.
        same_cmds = cmds_ok
        same_content = content_ok and contents[0] == contents[2]
        same_labels = labels[0] == labels[1] == labels[2] and len(labels[0]) <= 1
        row.update({"commands": {"window": win_cmds, "engine2": h2_cmds}, "labels": labels, "statuses": statuses,
                    "sameCommands": same_cmds, "sameContent": same_content, "sameLabels": same_labels,
                    "replayExact": cmds_exact and content_exact, "replayLargestDifference": max(cmds_diff or 0.0, content_diff or 0.0)})
        if not same_content:
            row["contents"] = contents
        run.check(same_cmds and same_content and same_labels and statuses == ["ok", "ok", "ok"] and len(win_cmds) >= 1,
                  f"three-way {name} ({how}): the window, the recorded samples replayed headless and the produced commands "
                  "sent directly give the same commands, content and undo step",
                  {k: row[k] for k in ("sameCommands", "sameContent", "sameLabels", "statuses", "labels", "replayExact", "replayLargestDifference")})
        results.append(row)
        run.save("rows.json", results)
        run.save("transcript.json", run.transcript)
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
            os_input = (os.environ.get("TT_E2E_INPUT") or ("os" if sys.platform == "win32" else "played")) == "os"
            return await main(Engine("window", a), heads[0], heads[1], run, out, work, os_input)
    finally:
        e2e.stop(proc)


def run_main() -> int:
    e6.dpi_aware()
    if len(sys.argv) != 4:
        print(__doc__)
        return 2
    app_bin, headless, out = sys.argv[1], sys.argv[2], Path(sys.argv[3])
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    work = Path(tempfile.mkdtemp(prefix="tt-c43-equiv-"))
    started = time.monotonic()
    rows: list = []
    try:
        rows = asyncio.run(session_all(app_bin, headless, run, out, work))
    except Exception as ex:  # noqa: BLE001 - reported as a failure with its text
        run.check(False, "the comparison ran", repr(ex))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    manifest = {"kind": "toontown.c43.gesture-equivalence", "platform": sys.platform, "seconds": round(time.monotonic() - started, 1),
                "rows": len(rows), "input": sorted({r.get("input") for r in rows if r.get("input")}),
                "replayNotExact": [r["name"] for r in rows if r.get("replayExact") is False], "failures": run.failures}
    run.save("rows.json", rows)
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps(manifest, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(run_main())
