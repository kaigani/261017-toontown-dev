"""M4 C4.4b: gesture equivalence three ways for the four drawing tools, in the Drawing view and the Camera view, through
the real window (plan §5; rulings D-4, D-23, D-32; design notes 3 §8, 4 §9, 5 §8).

Usage: python scripts/c44b-gesture-equivalence.py <toontown binary> <toontown-headless binary> <out dir>

For each row (the Contour Editor's anchor drag, segment drag, temporary-select insert and alternate reset; Perspective's
corner and alternate side drags; Reposition All's offset, rotation and alternate corner; the Text tool's click, and a
click then typing) one gesture is made in the running application's view: on Windows through the operating system's
real pointer (and keyboard for typing), elsewhere as an agent's `pointer` (and `type_text`) call the window plays. The
window's record of what it committed (the canvas's `lastGesture`) gives the samples and the commands. Then, on two
headless engines opened on copies of the same saved project, with the window's view sizes (`--view-size`, D-32):
engine 2 replays the recorded samples through `pointer` (exact when the view geometry matches, else within 1e-9
relative with the difference recorded); engine 3 receives the window's commands directly (`run_command`). Each row must
give on all three engines the same commands and parameters, the same drawing content and the same undo steps, and each
path is compared with the spec's statement written before the run (`EXPECTED`, D-23). Every row is undone on all three
engines before the next. Receipts: `rows.json`, `transcript.json`, `manifest.json` (kind
`toontown.c44b.gesture-equivalence`). Exit 0 when every row holds.
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


c43 = _load("c43equiv", "c43-gesture-equivalence.py")
eq, e6 = c43.eq, c43.e6
c42, e2e = eq.c42, eq.e2e
Engine = eq.Engine

from mcp import ClientSession, StdioServerParameters, types  # noqa: E402
from mcp.client.stdio import stdio_client  # noqa: E402

RECORD_BOUND_S = 30.0
TOOL_CHOICE = c43.TOOL_CHOICE
VIEWS = ("drawingView", "cameraView")

#: D-23: what the spec states for each row before it runs. `command`: the one document command; `params`: values the
#: parameters must have; `keys`: parameters that must be present; `label`: the one undo step's label.
EXPECTED = {
    "contour: anchor drag": {"spec": "02 §7.1.2 line 450; §13.3 (Contour Editor); design note 3 §2; label line 463",
                             "command": "draw.point.move", "params": {"snap": False}, "keys": ["points", "delta"],
                             "label": "Contour Editing"},
    "contour: segment drag": {"spec": "02 §7.1.2 line 451; design note 3 §2", "command": "draw.segment.move", "params": {},
                              "keys": ["strokeId", "segment", "t", "delta"], "label": "Contour Editing"},
    "contour: insert (temporary select)": {"spec": "02 §7.1.2 line 452; design note 3 §2", "command": "draw.point.add",
                                           "params": {}, "keys": ["strokeId", "segment", "t"], "label": "Contour Editing"},
    "contour: reset (alternate click)": {"spec": "02 §7.1.2 line 453; SI-186 (D-75 item 4)", "command": "draw.point.set",
                                         "params": {"handleMode": "reset"}, "keys": ["strokeId", "anchor"],
                                         "label": "Contour Editing"},
    "perspective: corner": {"spec": "02 §7.1.3 lines 468-469; SI-188; label line 473", "command": "draw.envelope.apply",
                            "params": {"mapping": "auto"}, "keys": ["corners"], "label": "Perspective Tool"},
    "perspective: alternate side": {"spec": "02 §7.1.3 line 470; SI-188", "command": "draw.envelope.apply",
                                    "params": {"mapping": "auto"}, "keys": ["corners"], "label": "Perspective Tool"},
    "reposition: offset": {"spec": "02 §7.1.4 line 478; SI-189; label line 481", "command": "draw.repositionAll.apply",
                           "params": {}, "keys": ["translate"], "label": "Reposition All Drawings"},
    "reposition: rotation": {"spec": "02 §7.1.4 line 479; SI-141 item 3", "command": "draw.repositionAll.apply",
                             "params": {}, "keys": ["rotate", "pivot"], "label": "Reposition All Drawings"},
    "reposition: alternate corner": {"spec": "02 §7.1.4 line 480; SI-189 (D-75 item 7)",
                                     "command": "draw.repositionAll.apply", "params": {}, "keys": ["corners"],
                                     "label": "Reposition All Drawings"},
    "text: click": {"spec": "02 §7.6 line 669; §13.3 line 1229; SI-191; label 02 line 1066",
                    "command": "draw.text.add", "params": {"text": ""}, "keys": ["position", "format"],
                    "label": "New Text Object"},
}


def problems(expect: dict, path: str, cmds: list, labels: list) -> list:
    out = []
    if len(cmds) != 1:
        out.append(f"{path}: {len(cmds)} commands, the spec states one ({expect['command']})")
    if cmds:
        c, params = cmds[0], cmds[0].get("params") or {}
        if c.get("commandId") != expect["command"]:
            out.append(f"{path}: command {c.get('commandId')}, the spec states {expect['command']}")
        for k, v in expect["params"].items():
            if params.get(k) != v:
                out.append(f"{path}: parameter {k} = {params.get(k)!r}, the spec states {v!r}")
        for k in expect["keys"]:
            if k not in params:
                out.append(f"{path}: parameter {k} missing")
    want = [expect["label"]] if expect["label"] else []
    if labels != want:
        out.append(f"{path}: undo steps {labels}, the spec states {want}")
    return out


async def canvas_state(engine: Engine, view: str) -> dict:
    res = (await engine.tool("inspect_ui", {"target": "window"})).get("result") or {}
    node = c42.find(res.get("root") or {}, lambda n: n.get("id") == f"{view}.canvas") or {}
    try:
        return json.loads(node.get("value") or "{}")
    except json.JSONDecodeError:
        return {}


def os_drag(pid: int, pts: list, mods: list, times: dict) -> None:
    """A real drag with modifier keys held (Windows)."""
    import ctypes
    u = ctypes.windll.user32
    vks = [{"alt": 0x12, "shift": 0x10, "ctrl": 0x11}[m] for m in mods]
    for vk in vks:
        u.keybd_event(vk, 0, 0, 0)
    try:
        e6.os_drag(pid, pts, 0.03 * max(1, len(pts) - 1), 0.05, times)
    finally:
        for vk in reversed(vks):
            u.keybd_event(vk, 0, 2, 0)


def os_type(text: str) -> None:
    """Types through the operating system's keyboard (Unicode key events into the focused control; Windows)."""
    import ctypes
    from ctypes import wintypes

    class KEYBDINPUT(ctypes.Structure):
        _fields_ = [("wVk", wintypes.WORD), ("wScan", wintypes.WORD), ("dwFlags", wintypes.DWORD),
                    ("time", wintypes.DWORD), ("dwExtraInfo", ctypes.POINTER(ctypes.c_ulong))]

    class INPUT(ctypes.Structure):
        class _U(ctypes.Union):
            _fields_ = [("ki", KEYBDINPUT), ("pad", ctypes.c_byte * 32)]
        _anonymous_ = ("u",)
        _fields_ = [("type", wintypes.DWORD), ("u", _U)]

    for ch in text:
        for flags in (0x0004, 0x0004 | 0x0002):
            i = INPUT(type=1)
            i.ki = KEYBDINPUT(0, ord(ch), flags, 0, None)
            ctypes.windll.user32.SendInput(1, ctypes.byref(i), ctypes.sizeof(INPUT))
        time.sleep(0.02)


async def main(app: Engine, start_heads, run, out: Path, work: Path, os_input: bool) -> list:
    rc = app.run
    await rc("project.create", {"name": "ToolEquivalence", "onUnsaved": "fail"})
    bob = ((await app.result("element.create", {"type": "drawing", "name": "Bob"})).get("elementIds") or [None])[0]
    await rc("element.drawing.create", {"elementId": bob, "name": "1", "exposeFrom": 1, "exposeTo": 24})
    await rc("element.drawing.create", {"elementId": bob, "name": "2", "exposeFrom": 25, "exposeTo": 48})
    await rc("app.context.set", {"elementId": bob, "frame": 1})
    sq = await app.result("draw.rectangle.add", {"from": {"x": -60, "y": -40}, "to": {"x": 60, "y": 40}})
    sid = (sq.get("strokeIds") or [None])[0]
    # A second drawing of other bounds (Reposition All maps both; SI-189).
    await rc("app.context.set", {"elementId": bob, "frame": 25})
    await rc("draw.ellipse.add", {"from": {"x": -30, "y": -70}, "to": {"x": 90, "y": 10}})
    await rc("app.context.set", {"elementId": bob, "frame": 1})
    saved = work / "tool-equivalence.animpkg"
    env = await eq.job(app, await rc("project.saveAs", {"path": str(saved), "onExists": "overwrite"}))
    run.check(env.get("status") == "ok", "the application saves the fixture project", env.get("error"))
    sizes = {}
    for view in VIEWS:
        await rc("window.panel.set", {"panelId": view, "visible": True})
    await asyncio.sleep(1.0)
    listed = await app.result("view.list", {})
    views = listed.get("views") or [] if isinstance(listed, dict) else listed
    for view in VIEWS:
        sizes[view] = next((v.get("size") for v in views if v.get("viewId") == view), None) or {}
    h2, h3 = await start_heads(sizes)
    for engine, copy in ((h2, work / "engine2.animpkg"), (h3, work / "engine3.animpkg")):
        shutil.copyfile(saved, copy)
        await eq.open_project(engine, copy)
    listed = await h2.result("view.list", {})
    hviews = listed.get("views") or [] if isinstance(listed, dict) else listed
    same = {view: bool(sizes[view]) and next((v.get("size") for v in hviews if v.get("viewId") == view), None) == sizes[view]
            for view in VIEWS}
    run.check(all(same.values()), "D-32: engine 2's views have the window's sizes", {"window": sizes, "same": same})
    engines = (app, h2, h3)

    async def mapping(view: str) -> list:
        return (await app.result("draw.viewMapping.get", {"view": view})).get("drawingToView") or [1, 0, 0, 1, 0, 0]

    async def box() -> dict:
        return (await app.result("draw.selection.get", {})).get("bounds") or {}

    def du(*pts):
        async def f(_b):
            return list(pts)
        return f

    def corner(dx, dy, out=0.0):
        async def f(b):
            x, y = b["x"] + b["width"] + out, b["y"] + b["height"] + out
            return [(x, y), (x + dx / 2, y + dy / 2), (x + dx, y + dy)]
        return f

    def side(dx, dy):
        async def f(b):
            x, y = b["x"] + b["width"] / 2, b["y"] + b["height"]
            return [(x, y), (x + dx / 2, y + dy / 2), (x + dx, y + dy)]
        return f

    # name, tool, points in drawing units (a function of the selection's box), modifiers, anchors selected before.
    rows = [
        ("contour: anchor drag", "contourEditor", du((-60, -40), (-50, -36), (-40, -32)), [], []),
        ("contour: segment drag", "contourEditor", du((0, -40), (0, -30), (0, -20)), [], []),
        ("contour: insert (temporary select)", "contourEditor", du((20, 40), (20, 40)), ["ctrl"], []),
        ("contour: reset (alternate click)", "contourEditor", du((60, 40), (60, 40)), ["alt"], []),
        ("perspective: corner", "perspective", corner(-20, 0), [], []),
        ("perspective: alternate side", "perspective", side(15, 4), ["alt"], []),
        ("reposition: offset", "repositionAll", du((-20, -10), (0, 0), (20, 10)), [], []),
        ("reposition: rotation", "repositionAll", corner(-30, 20, out=6.0), [], []),
        ("reposition: alternate corner", "repositionAll", corner(8, 6), ["alt"], []),
        ("text: click", "text", du((-100, 55), (-100, 55)), [], []),
    ]
    missing = [name for name, *_ in rows if name not in EXPECTED]
    run.check(not missing, "D-23: every row states the spec's expectation before it runs", missing)
    if missing:
        return []
    only = {x for x in os.environ.get("C44B_EQUIV_ROWS", "").split(",") if x}
    results = []
    for view in VIEWS:
        place, ratio = None, 1.0
        for engine in engines:
            await engine.run("view.active.set", {"viewId": view})
        if os_input:
            await asyncio.sleep(1.0)
            place = await e6.locate_view(app.s, e2e.run_cmd, view)
            if place and sizes[view].get("width"):
                ratio = place[2] / sizes[view]["width"]
            run.check(place is not None, f"the {view} canvas is located in the window (OS input)", place)
        # Calibration (OS input): one Select click at the view's centre; its recorded position corrects the located
        # place (the capture match can be a few pixels off; the hit radii are 4-6 px).
        offset = (0.0, 0.0)
        if os_input and place is not None and sizes[view].get("width"):
            for engine in engines:
                await engine.run("tool.active.set", {"toolId": "select"})
            await asyncio.sleep(0.4)
            want = (sizes[view]["width"] / 2, sizes[view]["height"] / 2)
            before = json.dumps((await canvas_state(app, view)).get("lastGesture"))
            times: dict = {}
            os_drag(e2e.APP_PID["pid"], [(place[0] + want[0] * ratio, place[1] + want[1] * ratio)] * 2, [], times)
            t0 = time.monotonic()
            while time.monotonic() - t0 < 10.0:
                rec = (await canvas_state(app, view)).get("lastGesture")
                if rec is not None and json.dumps(rec) != before and rec.get("events"):
                    ev = rec["events"][0]
                    offset = (ev["x"] - want[0], ev["y"] - want[1])
                    break
                await asyncio.sleep(0.2)
            run.check(abs(offset[0]) < 20 and abs(offset[1]) < 20, f"the {view} calibration click is recorded", offset)
        for name, tool, points, mods, anchors in rows:
            label = f"{name} ({view})"
            if only and name not in only:
                continue
            for engine in engines:
                await engine.run("app.context.set", {"elementId": bob, "frame": 1})
                await engine.run("view.transform.reset", {"viewId": view})
                await engine.run("tool.active.set", {"toolId": tool})
                await engine.run("draw.selection.set", {"by": "ids", "ids": [sid], "mode": "replace"})
                await engine.run("draw.pointSelection.set", {"points": [{"strokeId": sid, "anchor": a} for a in anchors]})
            await asyncio.sleep(0.4)
            before = json.dumps((await canvas_state(app, view)).get("lastGesture"))
            pos = [await engine.position() for engine in engines]
            m = await mapping(view)
            dpts = await points(await box())
            vpts = [c42.apply(m, x, y) for x, y in dpts]
            client = lambda q: (place[0] + (q[0] - offset[0]) * ratio, place[1] + (q[1] - offset[1]) * ratio) if place else q  # noqa: E731
            if os_input and place is not None and eq.on_screen(e2e.APP_PID["pid"], [client(q) for q in vpts]):
                times: dict = {}
                os_drag(e2e.APP_PID["pid"], [client(q) for q in vpts], mods, times)
                how = "os" if "error" not in times else f"os refused: {times['error']}"
            else:
                events = c42.drag(vpts)
                for ev in events:
                    ev["modifiers"] = mods
                await app.tool("pointer", {"view": view, "tool": tool, "events": events})
                how = "agent"
            start = time.monotonic()
            rec = None
            while time.monotonic() - start < RECORD_BOUND_S:
                st = await canvas_state(app, view)
                if st.get("lastGesture") is not None and json.dumps(st.get("lastGesture")) != before:
                    rec = st["lastGesture"]
                    break
                await asyncio.sleep(0.25)
            row = {"name": label, "tool": tool, "input": how, "record": rec}
            if rec is None or not rec.get("events"):
                run.check(False, f"three-way {label}: the window committed and recorded the gesture", row)
                results.append(row)
                continue
            events = [dict(ev) for ev in rec["events"]]
            req2 = {"view": view, "tool": tool, "space": "view", "pointerType": rec.get("pointerType", "mouse"), "events": events}
            if not same[view]:
                to_d = eq.inverse(rec["drawingToView"])
                for ev in events:
                    ev["x"], ev["y"] = c42.apply(to_d, ev["x"], ev["y"])
                req2["space"] = "drawing"
            env2 = await h2.tool("pointer", req2)
            cmds2 = (env2.get("result") or {}).get("commands") or []
            direct = [await h3.run(c["commandId"], c.get("params") or {}) for c in rec.get("commands") or []]
            contents = [await e.result("draw.drawing.get", {}) for e in engines]
            labels = [await e.labels_since(p) for e, p in zip(engines, pos)]
            win_cmds, h2_cmds = c43.normalised(rec.get("commands") or []), c43.normalised(cmds2)
            statuses = [rec.get("status"), env2.get("status"), "ok" if all(d.get("status") == "ok" for d in direct) else "error"]
            cmds_ok, cmds_exact, cmds_diff = c43.replay_agrees(win_cmds, h2_cmds)
            content_ok, content_exact, content_diff = c43.replay_agrees(contents[0], contents[1])
            if same[view]:
                cmds_ok, content_ok = cmds_exact, content_exact
            same_content = content_ok and contents[0] == contents[2]
            same_labels = labels[0] == labels[1] == labels[2]
            row.update({"commands": {"window": win_cmds, "engine2": h2_cmds}, "labels": labels, "statuses": statuses,
                        "sameCommands": cmds_ok, "sameContent": same_content, "sameLabels": same_labels,
                        "replayExact": cmds_exact and content_exact,
                        "replayLargestDifference": max(cmds_diff or 0.0, content_diff or 0.0)})
            expect = EXPECTED[name]
            spec_bad = []
            for path, c, lab in zip(("window", "engine2", "engine3"), (win_cmds, h2_cmds, win_cmds), labels):
                spec_bad += problems(expect, path, c, lab)
            row.update({"expected": expect, "specProblems": spec_bad})
            run.check(not spec_bad, f"three-way {label}: each path does what the spec states ({expect['spec']})", spec_bad)
            run.check(cmds_ok and same_content and same_labels and statuses == ["ok", "ok", "ok"],
                      f"three-way {label} ({how}): the window, the replay and the direct commands agree",
                      {k: row[k] for k in ("sameCommands", "sameContent", "sameLabels", "statuses", "labels", "replayExact")})
            # The Text tool: a click then typing (design note 5 §3): the window types (real keys, or type_text the
            # window's session receives), engine 2 the replayed click and type_text, engine 3 draw.text.set.
            if tool == "text" and statuses[0] == "ok":
                text_id = next(((c.get("envelope") or {}).get("result") or {}).get("textId")
                               for c in (env2.get("result") or {}).get("commands") or [] if c.get("commandId") == "draw.text.add")
                pos_t = [await engine.position() for engine in engines]
                tree = (await app.tool("inspect_ui", {"target": "window"})).get("result") or {}
                row["editorBefore"] = c42.find(tree.get("root") or {}, lambda n: n.get("id") == "canvas.textEditor")
                row["focusBefore"] = (await app.result("app.state.get", {})).get("focus")
                if how == "os":
                    await asyncio.sleep(0.5)
                    os_type("Hi")
                else:
                    await app.tool("type_text", {"text": "Hi"})
                await h2.tool("type_text", {"text": "Hi"})
                d3 = direct[0].get("result") or {}
                await h3.run("draw.text.set", {"drawingId": d3.get("drawingId"), "textId": d3.get("textId"),
                                               "insert": {"at": 0, "text": "Hi"}})
                await asyncio.sleep(1.0)
                texts = []
                for e in engines:
                    d = await e.result("draw.drawing.get", {"flatten": True})
                    texts.append(sorted(n.get("text") for n in d.get("nodes") or [] if "text" in n and "runs" in n))
                tl = [await e.labels_since(p) for e, p in zip(engines, pos_t)]
                tree = (await app.tool("inspect_ui", {"target": "window"})).get("result") or {}
                editor = c42.find(tree.get("root") or {}, lambda n: n.get("id") == "canvas.textEditor")
                row["editor"] = editor
                ok_t = texts[0] == texts[1] == texts[2] and ["Hi"] in [t[-1:] for t in texts] and tl[0] == tl[1] == tl[2] == ["Typing"]
                row["typing"] = {"texts": texts, "labels": tl, "textId": text_id}
                run.check(ok_t, f"three-way {label}: typing gives one \"Typing\" step and the same text (SI-191)", row["typing"])
                labels = [lab + t for lab, t in zip(labels, tl)]
                for e in engines:
                    await e.run("tool.active.set", {"toolId": "select"})
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

            async def start_heads(sizes: dict):
                heads = []
                extra = []
                for view, size in sizes.items():
                    if size.get("width"):
                        extra += ["--view-size", f"{view}={size['width']}x{size['height']}"]
                for k in (2, 3):
                    params = StdioServerParameters(command=headless, args=["--headless", "--mcp-stdio", "--user-dir", str(work / f"user-h{k}"), "--policy", "autonomous", *extra])
                    r, w = await stack.enter_async_context(stdio_client(params))
                    hs = await stack.enter_async_context(ClientSession(r, w, list_roots_callback=roots))
                    await hs.initialize()
                    heads.append(Engine(f"engine{k}", hs))
                return heads[0], heads[1]

            os_input = (os.environ.get("TT_E2E_INPUT") or ("os" if sys.platform == "win32" else "played")) == "os"
            return await main(Engine("window", a), start_heads, run, out, work, os_input)
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
    work = Path(tempfile.mkdtemp(prefix="tt-c44b-equiv-"))
    started = time.monotonic()
    rows: list = []
    try:
        rows = asyncio.run(session_all(app_bin, headless, run, out, work))
    except Exception as ex:  # noqa: BLE001 - reported as a failure with its text
        run.check(False, "the comparison ran", repr(ex))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    manifest = {"kind": "toontown.c44b.gesture-equivalence", "platform": sys.platform, "seconds": round(time.monotonic() - started, 1),
                "rows": len(rows), "input": sorted({r.get("input") for r in rows if r.get("input")}),
                "replayNotExact": [r["name"] for r in rows if r.get("replayExact") is False], "failures": run.failures}
    if rows:
        run.save("rows.json", rows)
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps(manifest, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(run_main())
