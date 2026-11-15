"""M4 C4.3b E6: agent and user coexistence in the running application (11 lines 586-587; busy-arbitration note §5,
rulings D-26, D-27).

Usage: python scripts/c43-e6-coexistence.py <toontown binary> <out dir>

The application runs attached with a project holding the element Bob (a painted rectangle), the Camera view shown and
active, Bob selected, the Transform scene tool current; an agent session is connected over MCP (Streamable HTTP, the
official SDK), with the agent policy `autonomous`.

1. The user drags Bob in the Camera view (from inside his box, away from the pivot at its centre). On Windows this is real OS input (`SendInput`: press, moves over about
   1.5 s, release; ruling D-26). On macOS the runner allows no synthetic OS input to the webview, so the window plays
   the same samples through its own input path (`pointer` routed to the window, C4.2b P1). The receipt states which.
2. While the drag is pressed, the agent calls `anim.transform.set` on Bob. The call waits (the window reported the
   gesture as the modal state) and is answered after the release, not before (E1): the order of the send, the release
   and the answer is asserted, not a duration (D-27).
3. After the release the call has succeeded. The user's drag is one undo step and the agent's follows it.
4. The user's later step: a second drag of Bob.
5. The agent's `edit.undo {ifTop: <its step>}` answers `undo.notTop` naming the user's later step (E2); the history
   shows the user's drag, the agent's call and the user's later step in order, none undone; the agent session's
   activity log records the call and the refused undo.
6. E1 the other way (Windows only: a played gesture lasts about two seconds, under the 10 s `waitForUserMs`): the user
   holds the press past `waitForUserMs`; the agent's call fails `ui.busy` and is answered before the release; the
   user's drag still commits its step.

Receipts in <out dir>: transcript.json, manifest.json (kind toontown.c43.e6), the UI trees, the history and the
activity log. Exit 0 when every check passed.
"""

from __future__ import annotations

import asyncio
import base64
import importlib.util
import json
import os
import shutil
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


e2e = _load("c41e2e", "c41-gui-e2e.py")
c42 = _load("c42e2e", "c42-gui-e2e.py")

#: 11 §6.4: an agent mutation waits up to `waitForUserMs` (default 10 000 ms) during a user gesture.
WAIT_FOR_USER_S = 10.0
#: The long press of the `ui.busy` case holds past `waitForUserMs` with a margin (not a measured quantity).
LONG_HOLD_S = WAIT_FOR_USER_S + 3.0


# ---------------------------------------------------------------------------------------------------------------------
# Windows OS input (SendInput)

def _main_window(pid: int):
    import ctypes
    from ctypes import wintypes
    u = ctypes.windll.user32
    hwnds = []

    def cb(h, _):
        q = wintypes.DWORD()
        u.GetWindowThreadProcessId(h, ctypes.byref(q))
        if q.value == pid and u.IsWindowVisible(h) and u.GetWindow(h, 4) == 0:
            hwnds.append(h)
        return True

    u.EnumWindows(ctypes.WINFUNCTYPE(ctypes.c_bool, wintypes.HWND, wintypes.LPARAM)(cb), 0)
    return hwnds[0] if hwnds else None


def _send_button(down: bool) -> bool:
    """One left button press or release through `SendInput`; whether it was injected."""
    import ctypes
    from ctypes import wintypes

    class MOUSEINPUT(ctypes.Structure):
        _fields_ = [("dx", wintypes.LONG), ("dy", wintypes.LONG), ("mouseData", wintypes.DWORD),
                    ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD), ("dwExtraInfo", ctypes.c_size_t)]

    class INPUT(ctypes.Structure):
        # The union's largest member is MOUSEINPUT, so INPUT is exactly this (SendInput refuses another size).
        _fields_ = [("type", wintypes.DWORD), ("mi", MOUSEINPUT)]

    i = INPUT(type=0, mi=MOUSEINPUT(0, 0, 0, 0x0002 if down else 0x0004, 0, 0))
    return ctypes.windll.user32.SendInput(1, ctypes.byref(i), ctypes.sizeof(INPUT)) == 1


def os_drag(pid: int, pts: list, seconds: float, hold_s: float, times: dict) -> None:
    """A real left drag through client points of the main window over `seconds`, the press held `hold_s` more before
    the release (Windows; moves the real cursor). Records the press and release times (monotonic) in `times`."""
    import ctypes
    from ctypes import wintypes
    u = ctypes.windll.user32
    h = _main_window(pid)
    if h is None:
        times["error"] = "no window"
        return
    # A workstation's foreground lock refuses SetForegroundWindow to a background process; a released Alt key lifts it
    # (only when another window is in front: in our own window it would open the menu bar).
    if u.GetForegroundWindow() != h:
        u.keybd_event(0x12, 0, 0, 0)
        u.keybd_event(0x12, 0, 2, 0)
    u.SetForegroundWindow(h)
    origin = wintypes.POINT(0, 0)
    u.ClientToScreen(h, ctypes.byref(origin))
    time.sleep(0.3)
    sx, sy = origin.x + int(pts[0][0]), origin.y + int(pts[0][1])
    u.SetCursorPos(sx, sy)
    time.sleep(0.05)
    # No input unless the press lands in the application: its window in front and under the cursor.
    # (The webview's own window belongs to its browser process; its root is the application's window.)
    u.WindowFromPoint.restype = wintypes.HWND
    u.GetAncestor.restype = wintypes.HWND
    root = u.GetAncestor(u.WindowFromPoint(wintypes.POINT(sx, sy)), 2)
    if u.GetForegroundWindow() != h or root != h:
        times["error"] = f"the application is not in front at the press (foreground {u.GetForegroundWindow()}, window {h}, under {root}, client points {pts})"
        return
    if not _send_button(True):
        times["error"] = f"SendInput refused the press (error {ctypes.GetLastError()})"
        return
    times["press"] = time.monotonic()
    step = seconds / max(1, len(pts) - 1)
    for x, y in pts[1:]:
        time.sleep(step)
        u.SetCursorPos(origin.x + int(x), origin.y + int(y))
    time.sleep(hold_s)
    times["release"] = time.monotonic()
    if not _send_button(False):
        times["error"] = "SendInput refused the release"


# ---------------------------------------------------------------------------------------------------------------------

async def locate_view(session, rc, target: str):
    """Where a view's canvas is in the window's client area: its capture found in the window capture (device pixels)."""
    shots = {}
    for tgt in ("window", target):
        r = (await rc(session, "inspect.screenshot", {"target": tgt})).get("result") or {}
        if not r.get("png"):
            return None
        shots[tgt] = c42.png_rgba(base64.b64decode(r["png"]))
    W, H, cw, wrows = shots["window"]
    w, h, cv, vrows = shots[target]
    if w > W or h > H:
        return None
    first = [vrows[0][i:i + 3] for i in range(0, w * cv, cv)]
    for y in range(0, H - h + 1):
        row = wrows[y]
        for x in range(0, W - w + 1):
            if all(row[(x + i) * cw:(x + i) * cw + 3] == first[i] for i in range(0, w, max(1, w // 16))):
                if all(wrows[y + yy][x * cw:(x + w) * cw][::cw] == vrows[yy][::cv] for yy in range(0, h, max(1, h // 32))):
                    return (x, y, w, h)
    return None


def steps_of(hist: dict, since: int) -> list:
    """The history's done entries after `since`: (stepId, label, source, agent session)."""
    entries = (hist.get("entries") or [])[since:hist.get("position", 0)]
    return [(e.get("stepId"), e.get("label"), e.get("source"), e.get("agentSessionId")) for e in entries]


#: Whose a step is: the user's (real OS input), or, in the played mode, the stand-in's: the second MCP session whose
#: samples the window plays (its steps are agent steps of another session than the agent's under test).
WHO = {"mode": "os", "agent": None}


def is_user(step) -> bool:
    if WHO["mode"] == "os":
        return step[2] == "user"
    return step[2] == "agent" and step[3] is not None and step[3] != WHO["agent"]


async def session_run(url: str, token: str, run, out: Path, mode: str) -> None:
    from contextlib import AsyncExitStack

    async def connect(stack):
        client = await stack.enter_async_context(
            e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {token}"}, timeout=e2e.httpx2.Timeout(60.0, read=300.0)))
        streams = await stack.enter_async_context(e2e.streamable_http_client(url, http_client=client))
        s = await stack.enter_async_context(e2e.ClientSession(streams[0], streams[1]))
        await s.initialize()
        return s

    async with AsyncExitStack() as stack:
        session = await connect(stack)
        # The played mode's stand-in for the user: its own session, so the agent's calls never queue behind it.
        player = await connect(stack) if mode == "played" else None
        WHO["mode"] = mode
        rc = e2e.run_cmd
        await e2e.gui_tree(session, run, lambda r: e2e.by_id(r, "startScreen") is not None, "the GUI shows the start screen")
        env = await rc(session, "project.create", {"name": "Coexistence", "onUnsaved": "fail"})
        run.check(env.get("status") == "ok", "project.create", env.get("error"))
        el = (await rc(session, "element.create", {"type": "drawing", "name": "Bob"})).get("result") or {}
        bob = (el.get("elementIds") or [None])[0]
        await rc(session, "element.drawing.create", {"elementId": bob, "name": "1", "exposeFrom": 1, "exposeTo": 24})
        await rc(session, "app.context.set", {"elementId": bob, "frame": 1})
        await rc(session, "draw.rectangle.add", {"from": {"x": -120, "y": -90}, "to": {"x": 120, "y": 90}})
        await rc(session, "colour.paint.apply", {"points": [{"x": 0, "y": 0}]})
        await rc(session, "window.panel.set", {"panelId": "cameraView", "visible": True})
        await rc(session, "view.active.set", {"viewId": "cameraView"})
        await rc(session, "edit.selection.set", {"context": "staging", "ids": [bob]})
        await rc(session, "anim.tool.set", {"tool": "transform"})
        tree = await e2e.gui_tree(session, run, lambda r: e2e.by_id(r, "cameraView") is not None, "the GUI shows the Camera view")
        run.save("tree-start.json", tree)
        m = ((await rc(session, "draw.viewMapping.get", {"view": "cameraView"})).get("result") or {}).get("drawingToView")
        run.check(isinstance(m, list) and len(m) == 6, "the Camera view's mapping of Bob's drawing", m)
        place = await locate_view(session, rc, "cameraView") if mode == "os" else (0, 0, 0, 0)
        run.check(place is not None, "the Camera view's canvas is found in the window capture", place)
        if not (isinstance(m, list) and place):
            return
        x0, y0 = place[0], place[1]
        listed = (await rc(session, "view.list", {})).get("result") or []
        views = listed.get("views") or [] if isinstance(listed, dict) else listed
        logical = next((v.get("size") for v in views if v.get("viewId") == "cameraView"), None) or {}
        ratio = place[2] / logical["width"] if mode == "os" and logical.get("width") else 1.0
        run.save("place.json", {"place": place, "logical": logical, "ratio": ratio})

        def client(m, px, py):
            v = c42.apply(m, px, py)
            return (x0 + v[0] * ratio, y0 + v[1] * ratio)

        async def history():
            return (await rc(session, "edit.history.get", {"limit": 1000})).get("result") or {}

        async def user_drag(pts_du, seconds, hold_s, during):
            """The user's drag of Bob through drawing points; `during` runs while the press is held. Returns
            the times and what `during` returned."""
            times: dict = {}
            # Bob's drawing units -> Camera view pixels now (Bob moves between the drags).
            m = ((await rc(session, "draw.viewMapping.get", {"view": "cameraView"})).get("result") or {}).get("drawingToView")
            if mode == "os":
                pts = [client(m, *p) for p in pts_du]
                th = threading.Thread(target=os_drag, args=(e2e.APP_PID["pid"], pts, seconds, hold_s, times))
                th.start()
                while "press" not in times and "error" not in times and th.is_alive():
                    await asyncio.sleep(0.02)
                await asyncio.sleep(0.3)
                result = await during(times)
                while th.is_alive():
                    await asyncio.sleep(0.05)
                run.check("error" not in times, "the OS drag lands in the application window", times.get("error"))
            else:
                # The window plays the samples through its own input path: about 1.5 s of frames.
                pts = [c42.apply(m, *p) for p in pts_du]
                n = 1440
                line = [(pts[0][0] + (pts[-1][0] - pts[0][0]) * i / (n - 1), pts[0][1] + (pts[-1][1] - pts[0][1]) * i / (n - 1)) for i in range(n)]
                task = asyncio.create_task(e2e.call(player, "pointer", {"view": "cameraView", "events": c42.drag(line)}))
                # The played gesture's release is its commit: the time its answer arrives.
                task.add_done_callback(lambda _t: times.setdefault("release", time.monotonic()))
                times["press"] = time.monotonic()
                await asyncio.sleep(0.3)
                result = await during(times)
                played = await task
                times["played"] = played.get("status")
            await asyncio.sleep(1.0)
            return times, result

        own = (await rc(session, "agent.activity.list", {"limit": 5})).get("result") or []
        own = own.get("entries") or [] if isinstance(own, dict) else own
        WHO["agent"] = own[0].get("sessionId") if own else None
        run.check(mode == "os" or WHO["agent"] is not None, "the agent session's id is known", WHO)

        # 1-3. The user drags Bob; the agent's change waits for the release, then succeeds.
        pos0 = (await history()).get("position", 0)
        rev0 = (await rc(session, "app.state.get", {})).get("documentRev")

        async def agent_change(times):
            shot = (await rc(session, "inspect.screenshot", {"target": "window"})).get("result") or {}
            if shot.get("png"):
                (out / "while-pressed.png").write_bytes(base64.b64decode(shot["png"]))
            st = (await rc(session, "app.state.get", {})).get("result") or {}
            times["modalSeen"] = st.get("modal")
            times["send"] = time.monotonic()
            env = await rc(session, "anim.transform.set", {"nodeIds": [bob], "target": "static", "position": {"x": 0.5, "y": 0}})
            times["answer"] = time.monotonic()
            return env

        times, env = await user_drag([(60, 40), (100, 60)], 1.5, 0.0, agent_change)
        run.save("e1-times.json", {k: v for k, v in times.items()})
        run.check((times.get("modalSeen") or {}).get("kind") == "gesture",
                  "while the user's drag is pressed the window reports the gesture as the modal state", times.get("modalSeen"))
        run.check((times.get("modalSeen") or {}).get("title") == "transform",
                  "the modal state names the acting tool, the Transform scene tool", times.get("modalSeen"))
        run.check(env.get("status") == "ok", "E1: the agent's change during the user's drag succeeds", env.get("error"))
        # With real input the release is the button's, before the commit: the answer comes after it. A played gesture's
        # release is its commit's answer, a large envelope whose arrival can trail the agent's small answer by
        # milliseconds; there the causal order is the history's (the next checks): sent while playing.
        if mode == "os":
            order = "send" in times and "release" in times and "answer" in times and times["send"] < times["release"] <= times["answer"]
            what = "E1: the agent's change was sent while pressed and answered after the release"
        else:
            order = "send" in times and "release" in times and times["send"] < times["release"]
            what = "E1: the agent's change was sent while the window played the gesture"
        run.check(order, what, {k: times.get(k) for k in ("press", "send", "release", "answer")})
        rev1 = env.get("documentRev")
        run.check(isinstance(rev1, int) and isinstance(rev0, int) and rev1 > rev0, "the agent's change advanced the document", {"before": rev0, "after": rev1})
        hist = await history()
        steps = steps_of(hist, pos0)
        run.check(len(steps) == 2 and is_user(steps[0]) and not is_user(steps[1]),
                  "the user's drag is one undo step and the agent's step follows it", steps)
        agent_step = steps[1][0] if len(steps) == 2 else None

        # 4. The user's later step.
        async def nothing(_times):
            return None

        await user_drag([(60, -40), (60, -70)], 1.0, 0.0, nothing)
        hist = await history()
        steps = steps_of(hist, pos0)
        run.check(len(steps) == 3 and is_user(steps[2]), "the user's later drag is a step after the agent's", steps)

        # 5. The agent's undo ifTop refuses to undo the user's later step (E2).
        if not agent_step:
            run.check(False, "the agent's step is known (E2 not run)", steps)
            return
        env = await rc(session, "edit.undo", {"ifTop": agent_step})
        err = env.get("error") or {}
        top = (err.get("details") or {}).get("top") or {}
        run.check(err.get("code") == "undo.notTop" and top.get("stepId") == (steps[2][0] if len(steps) == 3 else None),
                  "E2: undo ifTop answers undo.notTop naming the user's later step", err)
        hist = await history()
        after = steps_of(hist, pos0)
        run.check(after == steps, "the user's later step survives: the three steps in order, none undone", after)
        run.save("history.json", hist)
        activity_env = await rc(session, "agent.activity.list", {"limit": 100})
        activity = activity_env.get("result")
        run.save("activity.json", activity_env)
        entries = (activity.get("entries") if isinstance(activity, dict) else activity) or []
        change = [e for e in entries if e.get("commandId") == "anim.transform.set"]
        undo = [e for e in entries if e.get("commandId") == "edit.undo"]
        run.check(any(e.get("status") == "ok" for e in change) and any(e.get("status") == "error" for e in undo),
                  "the activity log records the agent's change (ok) and the refused undo (error)", {"change": change, "undo": undo})

        # 6. E1 the other way: the press held past waitForUserMs; the agent's change fails ui.busy before the
        # release; the user's drag still commits.
        if mode == "os":
            pos1 = (await history()).get("position", 0)
            times, env = await user_drag([(-60, -40), (-80, -40)], 0.5, LONG_HOLD_S, agent_change)
            run.save("e1-busy-times.json", times)
            err = env.get("error") or {}
            run.check(err.get("code") == "ui.busy", "E1: held past waitForUserMs, the agent's change fails ui.busy", err)
            order = "send" in times and "answer" in times and "release" in times and times["send"] < times["answer"] < times["release"]
            run.check(order, "E1: the ui.busy answer comes while the user still holds the press",
                      {k: times.get(k) for k in ("press", "send", "answer", "release")})
            steps = steps_of(await history(), pos1)
            run.check(len(steps) == 1 and is_user(steps[0]), "the user's long drag commits its one step", steps)
        run.save("tree-final.json", (await e2e.tree(session)).get("root"))


def dpi_aware() -> None:
    """Client and screen coordinates in physical pixels, as the captures are (Windows; per-monitor aware v2)."""
    if sys.platform == "win32":
        import ctypes
        try:
            ctypes.windll.user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
        except (AttributeError, OSError):
            pass


def main() -> int:
    dpi_aware()
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    app, out = sys.argv[1], Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    # TT_E6_MODE=played runs the macOS path anywhere (a workstation check of it).
    mode = os.environ.get("TT_E6_MODE") or ("os" if sys.platform == "win32" else "played")
    user = Path(tempfile.mkdtemp(prefix="tt-c43-e6-"))
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
            asyncio.run(session_run(d["endpoint"], d["token"], run, out, mode))
        finally:
            e2e.stop(proc)
    except Exception as ex:  # noqa: BLE001 - reported as a failure with its text
        run.check(False, "the run completed", repr(ex))
    finally:
        shutil.rmtree(user, ignore_errors=True)
    manifest = {"kind": "toontown.c43.e6", "platform": sys.platform, "input": "os (SendInput)" if mode == "os" else "played by the window (pointer, C4.2b P1)",
                "waitForUserMs": int(WAIT_FOR_USER_S * 1000), "seconds": round(time.monotonic() - started, 1),
                "failures": run.failures, "checks": len(run.transcript)}
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps(manifest, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
