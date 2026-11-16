"""M4 C4.3b: the Camera view's budgets in the running application (plan §6; rulings D-5, D-27).

Usage: python scripts/c43-camera-measure.py <toontown binary> <out dir> [--backend webgl2|webgpu] [--bounds workstation|ci]
       [--drags N]

Launches the product with `--canvas-metrics` (and `--canvas-backend` when given), drives it over attached mode with the
Camera view active and the Select scene tool, and reads the Camera view canvas's own measurements from its UI-tree value
(desktop/src/canvas/metrics.ts), on two scenes: the sample scene (Bob, a painted rectangle, and Ann, an ellipse
behind him) and a 1,000-element scene (E-C1's fixture: 1,000 drawing elements, an ellipse each).

On each scene:
- frame time p95, idle (6 s, the canvas's last 300 frames);
- with N scene drags (default 30) of a selected element (Windows: real OS drags of about a second; elsewhere agent
  gestures played through the window): frame time p95 during the drags, drag feedback p95 (a pointer sample to the
  frame that draws the preview with it) and release-to-committed p95 (the pointer-up to the frame that draws the
  committed scene), each drag undone afterwards.

The control (ruling D-33): before any project, on the start screen, the webview's own `requestAnimationFrame` cadence
over 300 frames of a loop that does no work (`metrics.cadence`, desktop/src/canvas/CadenceProbe.tsx), with the display
refresh rate. If the control's median is about 16.7 ms the frame time is assessed by plan §6's 17.5 ms bound. If it
paces slower (a display or compositor cap, median above 17 ms), the cap is external and the frame budget is assessed
as D-33 states: work per frame p95 at most 8 ms (the frame callback's start to the end of submission) and at most 1 %
of intervals longer than twice the control's median, idle and during the drags, per back end.

`--bounds workstation` asserts those, 16.7 ms drag feedback, and 50 ms release-to-committed on the sample scene (the
1,000-element scene's release-to-committed is recorded);
`--bounds ci` loose sanity bounds only (D-5: CI runners have no GPU). The receipt is `measurements.json` (kind
`toontown.c43.camera-measure`) with every number, the bounds and the machine.

C43B-R1S-08: the work per frame during the drags is that of the frames drawn while a gesture was in progress
(`gestureWorkMs`), the idle work that of the frames with none (`idleWorkMs`); the idle phase lasts until the canvas's
ring holds 300 idle frames of this scene. An incomplete measurement fails (scripts/c43_measure_assess.py, tested by
scripts/test_c43_measure_assess.py): the requested back end with no fallback, 1,002 drawings resident in the large
scene, every drag committed, and full rings of idle and drag frames and of drag feedback samples.
"""

from __future__ import annotations

import argparse
import asyncio
import importlib.util
import json
import platform
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


measure42 = _load("c42measure", "c42-canvas-measure.py")
e6 = _load("c43e6", "c43-e6-coexistence.py")
cam = _load("c43camera", "c43-camera-e2e.py")
assess_mod = _load("c43assess", "c43_measure_assess.py")
e2e, c42 = e6.e2e, e6.c42

BOUNDS = {
    "workstation": {"frameMs": 17.5, "dragFrameMs": 17.5, "feedbackMs": 16.7, "commitMs": 50.0, "minSamples": 30},
    "ci": {"frameMs": 1000.0, "dragFrameMs": 1000.0, "feedbackMs": 1000.0, "commitMs": 5000.0, "minSamples": 1},
}


p95 = assess_mod.p95


async def metrics(session) -> dict:
    return (await cam.canvas_summary(session)).get("metrics") or {}


async def scene_measure(session, run, label: str, element: str, drags: int, os_input: bool, control_ms: float | None) -> dict:
    """Idle frames, then `drags` scene drags of `element` (selected) with the Select tool; each drag undone."""
    rc = e2e.run_cmd
    await rc(session, "view.active.set", {"viewId": "cameraView"})
    await rc(session, "view.transform.reset", {"viewId": "cameraView"})
    await rc(session, "app.context.set", {"elementId": element, "frame": 1})
    await rc(session, "edit.selection.set", {"context": "staging", "ids": [element]})
    await rc(session, "anim.tool.set", {"tool": "select"})
    # Long enough for the ring to hold only this scene's idle frames: 300 at the control's cadence, with a margin.
    await asyncio.sleep(max(6.0, 1.3 * assess_mod.FULL * (control_ms or 16.7) / 1000.0))
    idle = await metrics(session)
    out: dict = {"idle": idle.get("frameMs"), "idleWork": idle.get("idleWorkMs"), "idleIntervals": idle.get("frameIntervals") or []}
    place = await e6.locate_view(session, rc, "cameraView") if os_input else None
    out["input"] = "os" if place else "agent"
    committed = 0
    feedback_before = (await metrics(session)).get("inkMs") or {}
    for k in range(drags):
        box = await cam.box_of(session, "cameraView")
        if not box:
            run.check(False, f"{label}: the selection's box is known", None)
            break
        cx, cy = (box[0] + box[2]) / 2 + (box[2] - box[0]) / 4, (box[1] + box[3]) / 2
        pts = [(cx + i * 2.0, cy + (i % 10) * 0.5) for i in range(30)]
        pos0 = ((await rc(session, "edit.history.get", {"limit": 1000})).get("result") or {}).get("position", 0)
        if place:
            times: dict = {}
            client = [(place[0] + x, place[1] + y) for x, y in pts]
            th = threading.Thread(target=e6.os_drag, args=(e2e.APP_PID["pid"], client, 0.9, 0.05, times))
            th.start()
            while th.is_alive():
                await asyncio.sleep(0.05)
            if "error" in times:
                run.check(False, f"{label}: an OS drag lands in the Camera view", times["error"])
                break
        else:
            await e2e.call(session, "pointer", {"view": "cameraView", "tool": "select", "events": c42.drag(pts)})
        await asyncio.sleep(0.7)
        after = ((await rc(session, "edit.history.get", {"limit": 1000})).get("result") or {}).get("position", 0)
        if after == pos0 + 1:
            committed += 1
            await rc(session, "edit.undo", {})
            await asyncio.sleep(0.4)
    await asyncio.sleep(1.0)
    m = await metrics(session)
    # The canvas keeps its last commits in a ring: this scene's are its last ones, one per drag that committed.
    all_commits = m.get("commits") or []
    mine = all_commits[-committed:] if committed else []
    out.update({"dragFrames": m.get("gestureFrameMs"), "dragWork": m.get("gestureWorkMs"), "dragIntervals": m.get("gestureIntervals") or [], "feedback": m.get("inkMs"), "feedbackBefore": feedback_before,
                "commits": {"n": len(mine), "p95": p95(mine), "max": max(mine) if mine else None, "samples": mine},
                "commitParts": (m.get("commitParts") or [])[-5:], "drags": drags})
    return out


async def measure(url, token, run, out: Path, bounds: dict, record: dict, drags: int, os_input: bool) -> None:
    from contextlib import AsyncExitStack

    async with AsyncExitStack() as stack:
        client = await stack.enter_async_context(
            e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {token}"}, timeout=e2e.httpx2.Timeout(60.0, read=300.0)))
        streams = await stack.enter_async_context(e2e.streamable_http_client(url, http_client=client))
        session = await stack.enter_async_context(e2e.ClientSession(streams[0], streams[1]))
        await session.initialize()
        rc = e2e.run_cmd
        await e2e.gui_tree(session, run, lambda r: e2e.by_id(r, "startScreen") is not None, "the GUI is up")
        # The control (D-33): the webview's cadence on the start screen, no project, 300 frames of a loop doing no work.
        await asyncio.sleep(7.0)
        control: dict = {}
        for _ in range(20):
            res = (await e2e.tree(session)).get("result") or {}
            node = c42.find(res.get("root") or {}, lambda n: n.get("id") == "metrics.cadence") or {}
            try:
                control = json.loads(node.get("value") or "{}")
            except json.JSONDecodeError:
                control = {}
            if (control.get("frameMs") or {}).get("n", 0) >= 300:
                break
            await asyncio.sleep(0.5)
        record["control"] = control
        record["displayRefreshHz"] = refresh_rate()
        # The control's completeness is assessed with the rest (assess_mod.assess).
        await rc(session, "project.create", {"name": "CameraMeasure", "onUnsaved": "fail"})
        found = (await rc(session, "doc.object.find", {"type": "swatch", "name": "Colour 2"})).get("result") or {}
        items = found.get("items") or found.get("matches") or []
        black = next((it.get("id") or it.get("key") or it.get("colourId") for it in items if isinstance(it, dict)), None)
        ids = {}
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
        st: dict = {}
        start = time.monotonic()
        while time.monotonic() - start < 60:
            st = await cam.canvas_summary(session)
            if len(st.get("drawings") or []) >= 2 and st.get("metrics"):
                break
            await asyncio.sleep(0.5)
        record["backend"], record["fallback"] = st.get("backend"), st.get("fallback")
        control_ms = (control.get("frameMs") or {}).get("p50")
        record["sample"] = await scene_measure(session, run, "the sample scene", ids["Bob"], drags, os_input, control_ms)

        # The 1,000-element scene (E-C1's fixture), built in batches of 100 elements.
        for g in range(10):
            steps = []
            for i in range(100):
                n = g * 100 + i
                steps += [
                    {"id": "element.create", "params": {"type": "drawing", "name": f"G{n}"}, "as": f"e{i}"},
                    {"id": "element.drawing.create", "params": {"elementId": f"$e{i}.result.elementIds.0", "name": "1", "exposeFrom": 1, "exposeTo": 1}, "as": f"d{i}"},
                    {"id": "draw.ellipse.add", "params": {"drawingId": f"$d{i}.result.drawingId", "from": {"x": -600 + (n % 40) * 30, "y": -400 + (n // 40) * 30}, "to": {"x": -580 + (n % 40) * 30, "y": -380 + (n // 40) * 30}}},
                ]
            env = await e2e.call(session, "batch", {"steps": steps, "label": "groups"})
            if env.get("status") not in ("ok", None):
                run.check(False, "the 1,000 elements build", env.get("error"))
                break
        start = time.monotonic()
        while time.monotonic() - start < 180:
            st = await cam.canvas_summary(session)
            if len(st.get("drawings") or []) >= assess_mod.RESIDENT_1K:
                break
            await asyncio.sleep(0.5)
        record["groups1k"] = {"resident": len(st.get("drawings") or [])}
        record["groups1k"].update(await scene_measure(session, run, "the 1,000-element scene", ids["Bob"], drags, os_input, control_ms))
        for ok, name, detail in assess_mod.assess(record, bounds, drags):
            run.check(ok, name, detail)


def refresh_rate() -> list | None:
    """The displays' refresh rates as the OS reports them (Windows: Win32_VideoController), for the receipt."""
    if sys.platform != "win32":
        return None
    import subprocess
    r = subprocess.run(["powershell", "-NoProfile", "-Command",
                        "Get-CimInstance Win32_VideoController | ForEach-Object { $_.CurrentRefreshRate }"],
                       capture_output=True, text=True, timeout=60, check=False)
    return [int(x) for x in r.stdout.split() if x.strip().isdigit()]


def main() -> int:
    e6.dpi_aware()
    ap = argparse.ArgumentParser()
    ap.add_argument("app")
    ap.add_argument("out")
    ap.add_argument("--backend", choices=["webgl2", "webgpu"])
    ap.add_argument("--bounds", choices=["workstation", "ci"], default="ci")
    ap.add_argument("--drags", type=int, default=30)
    a = ap.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    os_input = sys.platform == "win32"
    user = Path(tempfile.mkdtemp(prefix="tt-c43-measure-"))
    (user / "Settings").mkdir(parents=True)
    seed = {"agent.policy.defaults": {"policy": "autonomous", "maxJobs": 2, "maxBatchSteps": 1000, "maxInlineImagePx": 2048,
                                      "maxInlineBytes": 52428800, "askInApp": True, "letAgentMoveView": True, "highlightAgentChanges": True}}
    (user / "Settings" / "preferences.json").write_text(json.dumps({"format": "toontown-settings", "kind": "preferences", "formatVersion": "1.0", "data": seed}), encoding="utf-8")
    discovery = out / "discovery.json"
    record: dict = {"requestedBackend": a.backend, "bounds": a.bounds}
    started = time.monotonic()
    try:
        proc = measure42.launch(a.app, user, discovery, a.backend)
        # The OS input helpers read the process id from their own module.
        e2e.APP_PID["pid"] = proc.pid
        try:
            d = e2e.wait_discovery(discovery, proc)
            asyncio.run(measure(d["endpoint"], d["token"], run, out, BOUNDS[a.bounds], record, a.drags, os_input))
        finally:
            e2e.stop(proc)
    except Exception as ex:  # noqa: BLE001 - reported as a failure with its text
        run.check(False, "the measurement ran", repr(ex))
    finally:
        shutil.rmtree(user, ignore_errors=True)
        if discovery.exists():
            discovery.unlink()
    receipt = {"kind": "toontown.c43.camera-measure", "machine": {"platform": sys.platform, "machine": platform.machine(),
               "processor": platform.processor(), "release": platform.release()},
               "seconds": round(time.monotonic() - started, 1), "record": record, "bounds": BOUNDS[a.bounds],
               "failures": run.failures}
    run.save("measurements.json", receipt)
    run.save("transcript.json", run.transcript)
    print(json.dumps({k: receipt[k] for k in ("kind", "seconds", "failures")}, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
