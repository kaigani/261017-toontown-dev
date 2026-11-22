"""Ruling D-50 (C4.3b closure): the preview lifecycle under real input, made checkable with the dev-only delays.

Usage: python scripts/c43-preview-lifecycle.py <toontown binary built with --features test-hooks> <toontown-headless>
       <out dir> [--commit-delay MS] [--scene-delay MS] [--states abcdef]

The window is built with the `test-hooks` feature (`cargo build --release -p tt-desktop --features test-hooks`) and
launched with `TT_TEST_COMMIT_DELAY_MS` (a gesture commit's answer reaches the window that much later; the commit itself
is made at once) and, for state (b), `TT_TEST_SCENE_DELAY_MS` too (a scene answer reaches the window that much later).
Defaults: commit 2500 ms, scene 1800 ms. Real OS input in the Drawing view (Windows). Six states (D-50 item 1), each ending with the window's drawing
exactly equal to the drawing of a headless engine that opened a copy of the same saved project and received the window's
recorded commands directly (ruling D-32; exact equality, ruling D-4):

(a) a slow commit keeps the preview (the Select tool's) or the overlay (a drawing tool's stroke) on every sample until
    the answer, and it is gone once the committed frame is drawn: Select move, Select corner scale, Brush, Rectangle;
(b) the C43B-R3S-02 trace: a scene asked for just before the press (built at the revision before the commit) arrives
    after the commit was made and before its answer, and does not clear the preview;
(c) undo by keyboard (a real Ctrl+Z) right after the release;
(d) a second gesture pressed before the first's answer: both commit, nothing is left drawn;
(e) a refusal (the drawing locked) clears the overlay at its answer, the drawing unchanged;
(f) the window switched to the Camera view and back while the answer is pending: the overlay is not drawn
    in the Camera view and is drawn again back in the Drawing view (D-53).

Output in <out dir>: `rows.json`, `transcript.json`, `manifest.json` (kind `toontown.c43b.preview-lifecycle`); the
committed receipt is their summary (D-42). Exit 0 when every row holds.
"""

from __future__ import annotations

import argparse
import asyncio
import contextlib
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


eq = _load("c42equivpl", "c42-gesture-equivalence.py")
e6 = _load("c43e6pl", "c43-e6-coexistence.py")
d39 = _load("c43d39pl", "c43-d39-bindings.py")
c42, e2e = eq.c42, eq.e2e
Engine = eq.Engine

from mcp import ClientSession, StdioServerParameters, types  # noqa: E402
from mcp.client.stdio import stdio_client  # noqa: E402

VIEW = "drawingView"


def pending(st: dict) -> bool:
    """A gesture's preview (Select) or overlay (a drawing tool's stroke) is drawn."""
    return bool(st.get("preview")) or st.get("overlay") is True


def os_click_client(x: float, y: float) -> None:
    """A real left click at a client point (CSS pixels times the device ratio) of the application's main window, which
    must own the foreground (D-49 item 5)."""
    import ctypes
    from ctypes import wintypes
    u = ctypes.windll.user32
    h = e6._main_window(e2e.APP_PID["pid"])
    if h is None or not c42.ensure_foreground(h):
        raise RuntimeError("the application window is not in the foreground: no input is sent (D-49 item 5)")
    origin = wintypes.POINT(0, 0)
    u.ClientToScreen(h, ctypes.byref(origin))
    u.SetCursorPos(int(origin.x + x), int(origin.y + y))
    time.sleep(0.05)
    e6._send_button(True)
    time.sleep(0.03)
    e6._send_button(False)


def ctrl_z() -> None:
    import ctypes
    u = ctypes.windll.user32
    u.keybd_event(0x11, 0, 0, 0)
    u.keybd_event(0x5A, 0, 0, 0)
    time.sleep(0.03)
    u.keybd_event(0x5A, 0, 2, 0)
    u.keybd_event(0x11, 0, 2, 0)


class Lifecycle:
    def __init__(self, app: Engine, head: Engine, run, work: Path, commit_ms: int, scene_ms: int = 0):
        self.app, self.head, self.run, self.work, self.commit_ms = app, head, run, work, commit_ms
        self.scene_ms = scene_ms
        self.rows: list = []
        self.bob = None
        self.black = None
        self.n = 0
        self.offset = None
        self.ratio = 1.0
        self.cdp = None

    def click_dom(self, ident: str) -> None:
        """A real OS click on the page's element `ident` (its centre from the webview's debugging port)."""
        if self.cdp is None:
            self.cdp = d39.Cdp(d39.CDP_PORT)
        r = self.cdp.js("(() => { const e = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']');"
                        " if (!e) return null; const b = e.getBoundingClientRect();"
                        " return {x: b.x + b.width / 2, y: b.y + b.height / 2, dpr: window.devicePixelRatio || 1}; })()"
                        % json.dumps(ident))
        if not r:
            raise RuntimeError(f"no element {ident}")
        os_click_client(r["x"] * r["dpr"], r["y"] * r["dpr"])

    async def cmd(self, n: str, p: dict | None = None) -> dict:
        env = await eq.job(self.app, await self.app.run(n, p or {}))
        if env.get("status") != "ok":
            raise RuntimeError(f"{n}: {json.dumps(env)[:400]}")
        return env.get("result") or {}

    async def both(self, n: str, p: dict | None = None) -> None:
        """The same command on the window and on the headless engine (the starting state of a row)."""
        await self.cmd(n, p)
        env = await self.head.run(n, p or {})
        if env.get("status") != "ok":
            raise RuntimeError(f"headless {n}: {json.dumps(env)[:400]}")

    async def state(self) -> dict:
        return await c42.canvas_state(self.app.s)

    async def fixture(self) -> None:
        await self.cmd("project.create", {"name": "Lifecycle", "onUnsaved": "fail"})
        await self.cmd("workspace.select", {"name": "Drawing"})
        found = await self.cmd("doc.object.find", {"type": "swatch", "name": "Colour 2"})
        items = found.get("items") or found.get("matches") or []
        self.black = next((it.get("id") or it.get("key") or it.get("colourId") for it in items if isinstance(it, dict)), None)
        self.bob = ((await self.cmd("element.create", {"type": "drawing", "name": "Bob"})).get("elementIds") or [None])[0]
        await self.cmd("element.drawing.create", {"elementId": self.bob, "name": "1", "exposeFrom": 1, "exposeTo": 24})
        await self.cmd("app.context.set", {"elementId": self.bob, "frame": 1})
        await self.cmd("colour.current.set", {"colourId": self.black})
        await self.cmd("draw.rectangle.add", {"from": {"x": -120, "y": -90}, "to": {"x": 120, "y": 90}})
        await self.cmd("window.panel.set", {"panelId": "cameraView", "visible": True})
        await self.cmd("view.active.set", {"viewId": VIEW})
        await c42.wait_canvas(self.app.s, lambda s_: bool(s_.get("drawings")))
        await asyncio.sleep(1.0)
        wpng = c42.png_of(await self.app.run("inspect.screenshot", {"target": "window"}))
        vpng = c42.png_of(await self.app.run("inspect.screenshot", {"target": VIEW}))
        if wpng and vpng:
            self.offset = eq.canvas_offset(wpng, vpng)
            self.ratio = c42.png_rgba(vpng)[0] / (((await self.app.mapping()).get("viewSize") or {}).get("width") or 1)
        self.run.check(self.offset is not None, "the Drawing view's canvas is located in the window", self.offset)
        if self.offset is None:
            raise RuntimeError("the Drawing view's canvas is not found in the window capture")

    async def start_row(self, tool: str, select_all: bool = False) -> None:
        """The window's project saved and opened by the headless engine: both start the row from the same document."""
        self.n += 1
        saved = self.work / f"row-{self.n}.animpkg"
        await self.cmd("project.saveAs", {"path": str(saved), "onExists": "overwrite"})
        copy = self.work / f"head-{self.n}.animpkg"
        shutil.copyfile(saved, copy)
        # The previous row's copy saved first: the headless agent may not discard unsaved changes (scope `app`).
        await eq.job(self.head, await self.head.run("project.save", {}))
        env = await eq.job(self.head, await self.head.run("project.open", {"path": str(copy), "onUnsaved": "fail"}))
        if env.get("status") != "ok":
            raise RuntimeError(f"headless project.open: {json.dumps(env)[:400]}")
        for n, p in (("app.context.set", {"elementId": self.bob, "frame": 1}), ("colour.current.set", {"colourId": self.black}),
                     ("view.transform.reset", {"viewId": VIEW}), ("tool.active.set", {"toolId": tool}),
                     ("draw.selection.set", {"by": "all"} if select_all else {"by": "none"})):
            await self.both(n, p)
        await asyncio.sleep(0.5)

    async def press(self, pts_drawing: list, during=None, prepare=None) -> dict:
        """A real OS drag through drawing points (0.4 s of movement, the press held 0.2 s more). `prepare` runs just
        before the drag starts (its return time is `times["prepared"]`); `during` runs once, 0.25 s after the press,
        while the button is still down. Returns at the release."""
        m = (await self.app.mapping()).get("drawingToView") or [1, 0, 0, 1, 0, 0]
        client = [(self.offset[0] + v[0] * self.ratio, self.offset[1] + v[1] * self.ratio)
                  for v in (c42.apply(m, x, y) for x, y in pts_drawing)]
        before = json.dumps((await self.state()).get("lastGesture"))
        times: dict = {}
        if prepare:
            await prepare()
            times["prepared"] = time.monotonic()
        th = threading.Thread(target=e6.os_drag, args=(e2e.APP_PID["pid"], client, 0.4, 0.2, times))
        th.start()
        fired = False
        while th.is_alive():
            if during and not fired and "press" in times and time.monotonic() - times["press"] > 0.25:
                fired = True
                await during()
            await asyncio.sleep(0.01)
        th.join()
        if "error" in times:
            raise RuntimeError(f"the OS drag: {times['error']}")
        return {"times": times, "before": before}

    async def settle(self, pressed: dict, extra_s: float = 1.5) -> dict:
        """Samples the canvas from the release until the gesture's record changes (its answer) and `extra_s` more."""
        rel, before = pressed["times"]["release"], pressed["before"]
        samples, record, answered_at = [], None, None
        end = time.monotonic() + self.commit_ms / 1000 + 10
        while time.monotonic() < end:
            st = await self.state()
            t = round(time.monotonic() - rel, 3)
            raw = st.get("lastGesture")
            if answered_at is None and raw is not None and json.dumps(raw) != before:
                answered_at, record = t, raw
            samples.append((t, pending(st)))
            if answered_at is not None and t >= answered_at + extra_s:
                break
            await asyncio.sleep(0.05)
        return {"samples": samples, "record": record, "answeredAt": answered_at}

    async def drag(self, pts_drawing: list, during=None, after_release=None, prepare=None) -> dict:
        pressed = await self.press(pts_drawing, during, prepare)
        if after_release:
            await after_release()
        r = await self.settle(pressed)
        r["times"] = pressed["times"]
        return r

    async def same_as_direct(self, records: list, extra: list | None = None) -> tuple:
        """D-32: the window's drawing against the headless engine's after the window's commands sent directly."""
        statuses = []
        for rec in records:
            for c in (rec or {}).get("commands") or []:
                statuses.append((await self.head.run(c["commandId"], c.get("params") or {})).get("status"))
        for n, p in extra or []:
            statuses.append((await self.head.run(n, p)).get("status"))
        win = await self.app.result("draw.drawing.get", {})
        head = await self.head.result("draw.drawing.get", {})
        same = eq.close(win, head) and bool(statuses) and all(s == "ok" for s in statuses)
        return same, {"statuses": statuses, **({"window": win, "direct": head} if win != head else {})}

    def check(self, name: str, ok: bool, detail) -> None:
        self.rows.append({"name": name, "ok": bool(ok), "detail": detail})
        self.run.check(bool(ok), f"D-50 {name}", detail)

    def held_while_pending(self, r: dict) -> tuple:
        """Drawn on every sample from the release until the answer, the answer at least 0.8 x the delay after the
        release, and gone on every sample from 1 s after it (the committed frame drawn)."""
        a = r["answeredAt"]
        if a is None:
            return False, {"answeredAt": None, "samples": r["samples"]}
        held = [p for t, p in r["samples"] if t < a]
        after = [p for t, p in r["samples"] if t >= a + 1.0]
        late = a >= 0.8 * self.commit_ms / 1000
        ok = late and bool(held) and all(held) and bool(after) and not any(after)
        return ok, {"answeredAt": a, "heldSamples": len(held), "heldAll": all(held), "afterSamples": len(after),
                    "afterNone": not any(after), "status": (r["record"] or {}).get("status")}

    # ----- the states ---------------------------------------------------------------------------------------------

    async def state_a(self) -> None:
        cases = [
            ("(a) Select move", "select", False, [(-60, 40), (-30, 50), (0, 60)]),
            ("(a) Select corner scale", "select", True, [(120, 90), (130, 96), (140, 102)]),
            ("(a) Brush", "brush", False, [(-80, -40), (-40, -20), (0, -30), (40, -10)]),
            ("(a) Rectangle", "rectangle", False, [(-50, -60), (-20, -40), (10, -20)]),
        ]
        for name, tool, select_all, pts in cases:
            try:
                await self.start_row(tool, select_all)
                r = await self.drag(pts)
                ok, detail = self.held_while_pending(r)
                self.check(f"{name}: drawn until the committed frame (answer {self.commit_ms} ms late)",
                           ok and detail["status"] == "ok", detail)
                same, diff = await self.same_as_direct([r["record"]])
                self.check(f"{name}: the drawing equals the direct commands' (D-32)", same, diff)
            except Exception as ex:  # noqa: BLE001
                self.check(f"{name}: ran", False, repr(ex))

    async def state_b(self) -> None:
        name = "(b) a scene answer in flight before the commit arrives first"
        try:
            await self.start_row("select")
            grid = bool(((await self.cmd("drawAid.state.get", {})).get("grid") or {}).get("visible"))

            async def ask_scene():
                # A change the canvas answers with a new scene request (the grid), made just before the press: that
                # scene is built at once, at the revision before the commit, and delivered TT_TEST_SCENE_DELAY_MS
                # later, after the commit was made at the release but before the commit's answer (delayed
                # TT_TEST_COMMIT_DELAY_MS after the release). An agent's command made during the drag would wait for
                # the gesture's end (the user's gesture holds the kernel), so it could not stage this order.
                await self.cmd("drawAid.grid.set", {"visible": not grid})
            r = await self.drag([(-60, 40), (-30, 50), (0, 60)], prepare=ask_scene)
            await self.head.run("drawAid.grid.set", {"visible": not grid})
            t = r["times"]
            # The scene was asked for at most at `prepared`; it arrives scene_ms after it was built.
            arrive = t["prepared"] + self.scene_ms / 1000
            staged = t["release"] < arrive < t["release"] + self.commit_ms / 1000
            ok, detail = self.held_while_pending(r)
            detail.update({"releaseAfterSceneAsked": round(t["release"] - t["prepared"], 3), "sceneDelayMs": self.scene_ms,
                           "sceneArrivesAfterRelease": round(arrive - t["release"], 3), "staged": staged})
            self.check(f"{name}: the trace is staged (scene built before the commit, delivered between the commit and "
                       "its answer) and the preview stays until the committed frame",
                       ok and staged and detail["status"] == "ok", detail)
            same, diff = await self.same_as_direct([r["record"]])
            self.check(f"{name}: the drawing equals the direct commands' (D-32)", same, diff)
            await self.both("drawAid.grid.set", {"visible": grid})
        except Exception as ex:  # noqa: BLE001
            self.check(f"{name}: ran", False, repr(ex))

    async def state_c(self) -> None:
        name = "(c) undo by keyboard right after the release"
        try:
            await self.start_row("brush")

            async def undo_key():
                await asyncio.sleep(0.05)
                ctrl_z()
            r = await self.drag([(-80, 50), (-40, 60), (0, 55)], after_release=undo_key)
            st = await self.state()
            self.check(f"{name}: answered, and nothing is left drawn once settled",
                       (r["record"] or {}).get("status") == "ok" and not pending(st),
                       {"answeredAt": r["answeredAt"], "status": (r["record"] or {}).get("status"), "pending": pending(st)})
            same, diff = await self.same_as_direct([r["record"]], extra=[("edit.undo", {})])
            self.check(f"{name}: the drawing equals the direct commands then undo (D-32)", same, diff)
        except Exception as ex:  # noqa: BLE001
            self.check(f"{name}: ran", False, repr(ex))

    async def state_d(self) -> None:
        name = "(d) a second gesture before the first settles"
        try:
            await self.start_row("brush")
            p1 = await self.press([(-90, -70), (-60, -60), (-30, -65)])
            p2 = await self.press([(30, -70), (60, -60), (90, -65)])
            early = round(p2["times"]["press"] - p1["times"]["release"], 3)
            # The window shows its latest record: the first gesture's appears at its answer (the delay after the first
            # release), and is replaced by the second's at the second answer.
            r1 = await self.settle(p1, extra_s=0)
            r2 = await self.settle({"times": p2["times"], "before": json.dumps(r1["record"])})
            st = await self.state()
            ok = (early < self.commit_ms / 1000 and all((r["record"] or {}).get("status") == "ok" for r in (r1, r2))
                  and not pending(st))
            self.check(f"{name}: the second press came before the first answer, both committed, nothing left drawn", ok,
                       {"secondPressAfterFirstRelease": early, "firstAnsweredAt": r1["answeredAt"],
                        "secondAnsweredAt": r2["answeredAt"], "pending": pending(st)})
            same, diff = await self.same_as_direct([r1["record"], r2["record"]])
            self.check(f"{name}: the drawing equals both gestures' direct commands (D-32)", same, diff)
        except Exception as ex:  # noqa: BLE001
            self.check(f"{name}: ran", False, repr(ex))

    async def state_e(self) -> None:
        name = "(e) a refusal clears at once"
        try:
            await self.start_row("brush")
            await self.both("draw.lock.set", {"locked": True})
            before = await self.app.result("draw.drawing.get", {})
            r = await self.drag([(-80, 20), (-40, 30), (0, 25)])
            a = r["answeredAt"]
            status = (r["record"] or {}).get("status")
            late = [p for t, p in r["samples"] if a is not None and t >= a + 0.2]
            self.check(f"{name}: refused, and nothing is drawn from just after the answer",
                       status == "error" and bool(late) and not any(late),
                       {"answeredAt": a, "status": status, "code": (r["record"] or {}).get("code"),
                        "drawnAfterAnswer": [t for t, p in r["samples"] if a is not None and t >= a and p]})
            after = await self.app.result("draw.drawing.get", {})
            self.check(f"{name}: the drawing is unchanged", before == after, None)
            await self.both("draw.lock.set", {"locked": False})
        except Exception as ex:  # noqa: BLE001
            self.check(f"{name}: ran", False, repr(ex))

    async def state_f(self) -> None:
        name = "(f) the window switched to the Camera view and back while pending"
        try:
            await self.start_row("brush")
            seen: list = []

            async def switch():
                # Ruling D-53 (C43B-R4-01): real OS clicks on the window's type buttons show the Camera view, then the
                # Drawing view again, both before the delayed answer (an agent's `view.type.set` would wait for the
                # user's gesture to end). Each time the canvas is sampled once its scene shows the new type.
                for view_type in ("camera", "drawing"):
                    self.click_dom(f"{VIEW}.type.{view_type}")
                    st = await c42.wait_canvas(self.app.s, lambda s_, v=view_type: s_.get("viewType") == v, 5)
                    await asyncio.sleep(0.15)
                    st = await self.state()
                    seen.append({"type": view_type, "viewType": st.get("viewType"), "overlay": st.get("overlay"),
                                 "preview": bool(st.get("preview")), "answered": json.dumps(st.get("lastGesture")) != before[0]})
            before = [json.dumps((await self.state()).get("lastGesture"))]
            r = await self.drag([(-80, -80), (-40, -70), (0, -75)], after_release=switch)
            cam = next((x for x in seen if x["type"] == "camera"), {})
            back = next((x for x in seen if x["type"] == "drawing"), {})
            # In the Camera view: not drawn (never with another view's mapping); back in its own view before the
            # answer: drawn again; after the committed frame: gone.
            ok_cam = cam.get("viewType") == "camera" and cam.get("overlay") is False and not cam.get("preview") and not cam.get("answered")
            ok_back = back.get("viewType") == "drawing" and back.get("overlay") is True and not back.get("answered")
            self.check(f"{name}: the pending overlay is not drawn in the Camera view, and is drawn again back in the "
                       "Drawing view, both before the answer (D-53)", ok_cam and ok_back, {"samples": seen, "answeredAt": r["answeredAt"]})
            st = await self.state()
            self.check(f"{name}: the commit answered, nothing left drawn",
                       (r["record"] or {}).get("status") == "ok" and not pending(st),
                       {"answeredAt": r["answeredAt"], "pending": pending(st)})
            same, diff = await self.same_as_direct([r["record"]])
            self.check(f"{name}: the drawing equals the direct commands' (D-32)", same, diff)
        except Exception as ex:  # noqa: BLE001
            self.check(f"{name}: ran", False, repr(ex))


async def session(app_bin: str, headless: str, run, work: Path, commit_ms: int, scene_ms: int, states: str) -> list:
    tag = f"{commit_ms}-{scene_ms}"
    user = work / f"user-{tag}"
    (user / "Settings").mkdir(parents=True)
    seed = {"agent.policy.defaults": {"policy": "autonomous", "maxJobs": 2, "maxBatchSteps": 1000, "maxInlineImagePx": 2048,
                                      "maxInlineBytes": 52428800, "askInApp": True, "letAgentMoveView": True, "highlightAgentChanges": True}}
    (user / "Settings" / "preferences.json").write_text(json.dumps(
        {"format": "toontown-settings", "kind": "preferences", "formatVersion": "1.0", "data": seed}), encoding="utf-8")

    async def roots(_ctx):
        return types.ListRootsResult(roots=[types.Root(uri=work.resolve().as_uri(), name="work")])

    os.environ["TT_TEST_COMMIT_DELAY_MS"] = str(commit_ms)
    # The webview's debugging port, for row (f)'s real clicks on page elements (this run only).
    os.environ["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = f"--remote-debugging-port={d39.CDP_PORT}"
    os.environ["TT_TEST_SCENE_DELAY_MS"] = str(scene_ms)
    discovery = work / f"discovery-{tag}.json"
    proc = e2e.launch(app_bin, user, discovery)
    try:
        d = e2e.wait_discovery(discovery, proc)
        async with contextlib.AsyncExitStack() as stack:
            client = await stack.enter_async_context(
                e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {d['token']}"}, timeout=e2e.httpx2.Timeout(60.0, read=300.0)))
            streams = await stack.enter_async_context(e2e.streamable_http_client(d["endpoint"], http_client=client))
            a = await stack.enter_async_context(ClientSession(streams[0], streams[1], list_roots_callback=roots))
            await a.initialize()
            await e2e.gui_tree(a, run, lambda r: e2e.by_id(r, "startScreen") is not None, f"the GUI is up (delays {tag} ms)")
            params = StdioServerParameters(command=headless, args=["--headless", "--mcp-stdio", "--user-dir", str(work / f"user-h-{tag}"), "--policy", "autonomous"])
            r, w = await stack.enter_async_context(stdio_client(params))
            hs = await stack.enter_async_context(ClientSession(r, w, list_roots_callback=roots))
            await hs.initialize()
            lc = Lifecycle(Engine("window", a), Engine("direct", hs), run, work, commit_ms, scene_ms)
            await lc.fixture()
            for s in states:
                await getattr(lc, f"state_{s}")()
                run.save("rows.json", lc.rows)
            return lc.rows
    finally:
        with contextlib.suppress(Exception):
            e2e.stop(proc)
        for k in ("TT_TEST_COMMIT_DELAY_MS", "TT_TEST_SCENE_DELAY_MS", "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"):
            os.environ.pop(k, None)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("app")
    ap.add_argument("headless")
    ap.add_argument("out")
    ap.add_argument("--commit-delay", type=int, default=2500)
    ap.add_argument("--scene-delay", type=int, default=1800)
    ap.add_argument("--states", default="abcdef")
    a = ap.parse_args()
    if sys.platform != "win32":
        print("D-50's lifecycle check drives the real window through Windows input.")
        return 0
    e6.dpi_aware()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    work = Path(tempfile.mkdtemp(prefix="tt-d50-"))
    started = time.monotonic()
    rows: list = []
    app, head = str(Path(a.app).resolve()), str(Path(a.headless).resolve())
    plain = "".join(s for s in a.states if s != "b")
    try:
        if plain:
            rows += asyncio.run(session(app, head, run, work, a.commit_delay, 0, plain))
        if "b" in a.states:
            rows += asyncio.run(session(app, head, run, work, a.commit_delay, a.scene_delay, "b"))
    except Exception as ex:  # noqa: BLE001
        run.check(False, "the lifecycle check ran", repr(ex))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    manifest = {"kind": "toontown.c43b.preview-lifecycle", "commitDelayMs": a.commit_delay, "sceneDelayMs": a.scene_delay,
                "seconds": round(time.monotonic() - started, 1), "rows": len(rows), "passed": sum(1 for r in rows if r["ok"]),
                "failed": [r["name"] for r in rows if not r["ok"]], "failures": run.failures}
    run.save("rows.json", rows)
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps(manifest, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
