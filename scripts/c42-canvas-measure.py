"""M4 C4.2b: the interactive canvas's GPU and webview quantities in the running application (plan §6; ruling D-5).

Usage: python scripts/c42-canvas-measure.py <toontown binary> <out dir> [--backend webgl2|webgpu] [--bounds workstation|ci]

Launches the product with `--canvas-metrics` (and `--canvas-backend` when given), drives it over attached mode, and
reads the canvas's own measurements from its UI-tree value (desktop/src/canvas/metrics.ts):

- frame time p95, idle and during a gesture, on the small drawing (the e2e fixtures) and the large one (50,000 points
  of brush strokes built through commands);
- input-to-ink p95 (OS input only: Windows);
- release-to-committed p95, small and large (Windows: OS drags; elsewhere: agent gestures routed through the window);
- the large drawing's load (transfer, decode and upload) and the back end's resident bytes.

`--bounds workstation` asserts plan §6's numbers (17.5 ms frame p95, 16.7 ms input-to-ink, 50 ms and 150 ms
release-to-committed); `--bounds ci` asserts loose sanity bounds only (D-5: CI runners have no GPU). The receipt is
`measurements.json` (kind `toontown.c42.canvas-measure`) with every number, the bounds and the machine.
"""

from __future__ import annotations

import argparse
import asyncio
import hashlib
import importlib.util
import json
import platform
import shutil
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location("c41e2e", ROOT / "scripts" / "c41-gui-e2e.py")
e2e = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(e2e)
_spec42 = importlib.util.spec_from_file_location("c42e2e", ROOT / "scripts" / "c42-gui-e2e.py")
c42 = importlib.util.module_from_spec(_spec42)
_spec42.loader.exec_module(c42)

BOUNDS = {
    "workstation": {"frameMs": 17.5, "gestureFrameMs": 17.5, "inkMs": 16.7, "commitSmallMs": 50.0, "commitLargeMs": 150.0},
    "ci": {"frameMs": 1000.0, "gestureFrameMs": 1000.0, "inkMs": 1000.0, "commitSmallMs": 5000.0, "commitLargeMs": 10000.0},
}


def launch(app: str, user: Path, discovery: Path, backend: str | None):
    if discovery.exists():
        discovery.unlink()
    log = open(discovery.with_suffix(".log"), "ab")
    args = [app, "--user-dir", str(user), "--mcp-http", "127.0.0.1:0", "--mcp-discovery", str(discovery), "--canvas-metrics"]
    if backend:
        args += ["--canvas-backend", backend]
    import subprocess
    proc = subprocess.Popen(args, stdout=log, stderr=subprocess.STDOUT)
    e2e.APP_PID["pid"] = proc.pid
    return proc


async def metrics_now(session) -> dict:
    st = await c42.canvas_state(session)
    return st


async def wait_metrics(session, pred, bound=60.0) -> dict:
    start = time.monotonic()
    last = {}
    while time.monotonic() - start < bound:
        last = await metrics_now(session)
        if pred(last):
            return last
        await asyncio.sleep(0.5)
    return last


async def measure(url, token, run, out: Path, bounds: dict, record: dict) -> None:
    client = e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {token}"}, timeout=e2e.httpx2.Timeout(60.0, read=300.0))
    async with client:
        async with e2e.streamable_http_client(url, http_client=client) as streams:
            async with e2e.ClientSession(streams[0], streams[1]) as session:
                await session.initialize()
                await e2e.gui_tree(session, run, lambda r: e2e.by_id(r, "startScreen") is not None, "the GUI is up")
                rc = e2e.run_cmd
                await rc(session, "project.create", {"name": "Measure", "onUnsaved": "fail"})
                await rc(session, "workspace.select", {"name": "Drawing"})
                el = (await rc(session, "element.create", {"type": "drawing", "name": "Ink"})).get("result") or {}
                eid = (el.get("elementIds") or [None])[0]
                small = (await rc(session, "element.drawing.create", {"elementId": eid, "name": "small", "exposeFrom": 1, "exposeTo": 1})).get("result") or {}
                large = (await rc(session, "element.drawing.create", {"elementId": eid, "name": "large", "exposeFrom": 2, "exposeTo": 2})).get("result") or {}
                await rc(session, "app.context.set", {"elementId": eid, "frame": 1})
                found = (await rc(session, "doc.object.find", {"type": "swatch", "name": "Colour 2"})).get("result") or {}
                items = found.get("items") or found.get("matches") or []
                black = next((it.get("id") or it.get("key") for it in items if isinstance(it, dict)), None)
                await rc(session, "colour.current.set", {"colourId": black})
                for a, b in [((-150, -60), (-30, 60)), ((30, -60), (150, 60))]:
                    await rc(session, "draw.rectangle.add", {"from": {"x": a[0], "y": a[1]}, "to": {"x": b[0], "y": b[1]}})
                await rc(session, "colour.paint.apply", {"points": [{"x": -90, "y": 0}]})
                st = await wait_metrics(session, lambda s: any(d.get("id") == small.get("drawingId") for d in s.get("drawings", [])))
                record["backend"] = st.get("backend")
                record["fallback"] = st.get("fallback")
                m = ((await rc(session, "draw.viewMapping.get", {"view": "drawingView"})).get("result") or {}).get("drawingToView")
                v = lambda x, y: c42.apply(m, x, y)  # noqa: E731

                # Idle frames on the small drawing.
                await asyncio.sleep(6.0)
                st = await metrics_now(session)
                record["small"] = {"idle": (st.get("metrics") or {}).get("frameMs")}

                async def gesture(label: str):
                    """Five brush gestures of about two seconds each (OS input on Windows, agent gestures elsewhere):
                    the metrics after them, with `commitMs` taken from these gestures' own commits."""
                    await rc(session, "tool.active.set", {"toolId": "brush"})
                    before = len(((await metrics_now(session)).get("metrics") or {}).get("commits") or [])
                    for g in range(5):
                        if sys.platform == "win32" and record.get("canvasAt"):
                            x0, y0 = record["canvasAt"]
                            pts = [(x0 + 100 + i * 4, y0 + 120 + g * 40 + (i % 20) * 2) for i in range(120)]
                            await asyncio.get_running_loop().run_in_executor(None, c42.os_drag, e2e.APP_PID["pid"], pts)
                        else:
                            events = c42.drag([v(-200 + i * 3, 150 - g * 30 + (i % 20) * 2) for i in range(120)])
                            await e2e.call(session, "pointer", {"view": "drawingView", "tool": "brush", "events": events})
                        await asyncio.sleep(1.0)
                    await asyncio.sleep(1.0)
                    m = (await metrics_now(session)).get("metrics") or {}
                    mine = (m.get("commits") or [])[before:]
                    s = sorted(mine)
                    m["commitMs"] = {"n": len(s), "p50": s[(len(s) - 1) // 2] if s else None,
                                     "p95": s[max(0, -(-len(s) * 95 // 100) - 1)] if s else None, "max": s[-1] if s else None,
                                     "samples": mine}
                    return m

                # The canvas's place in the window (Windows OS input): where the view capture sits in the window's.
                if sys.platform == "win32":
                    import base64
                    shots = {}
                    for tgt in ("window", "drawingView"):
                        r = (await rc(session, "inspect.screenshot", {"target": tgt})).get("result") or {}
                        if r.get("png"):
                            shots[tgt] = c42.png_rgba(base64.b64decode(r["png"]))
                    if len(shots) == 2:
                        W, H, cw, wrows = shots["window"]
                        w, h, cv, vrows = shots["drawingView"]
                        first = [vrows[0][i:i + 3] for i in range(0, w * cv, cv)]
                        for y in range(0, H - h + 1):
                            hit = next((x for x in range(0, W - w + 1)
                                        if all(wrows[y][(x + i) * cw:(x + i) * cw + 3] == first[i] for i in range(0, w, max(1, w // 16)))), None)
                            if hit is not None:
                                record["canvasAt"] = (hit, y)
                                break
                g = await gesture("small")
                record["small"].update({"gesture": g.get("gestureFrameMs"), "ink": g.get("inkMs"), "commit": g.get("commitMs")})

                # The long stroke (D-13): one 10,000-sample Brush gesture through the window (an agent gesture, played
                # through the window's input queue), release-to-committed from the end of its playback.
                before = len(((await metrics_now(session)).get("metrics") or {}).get("commits") or [])
                long_events = c42.drag([v(-500 + i * 0.1, -300 + 40 * ((i // 50) % 2) + (i % 50) * 0.8) for i in range(10_000)])
                env = await e2e.call(session, "pointer", {"view": "drawingView", "tool": "brush", "events": long_events})
                await asyncio.sleep(2.0)
                commits = ((await metrics_now(session)).get("metrics") or {}).get("commits") or []
                parts = ((await metrics_now(session)).get("metrics") or {}).get("commitParts") or []
                record["longStroke"] = {"samples": 10_000, "status": env.get("status"), "commitMs": commits[before:],
                                        "parts": parts[-1:] if commits[before:] else []}
                # The large drawing: 50,000 points of brush strokes, built through commands, then shown.
                await rc(session, "app.context.set", {"elementId": eid, "frame": 2})
                steps = []
                for k in range(100):
                    pts = [{"x": -600 + i * 2.4, "y": -400 + k * 8 + ((i * 0.3) % 6), "pressure": 0.6} for i in range(500)]
                    steps.append({"id": "draw.stroke.add", "params": {"kind": "brush", "points": pts, "fit": False, "layer": "top"}})
                t0 = time.monotonic()
                for i in range(0, 100, 10):
                    env = await e2e.call(session, "batch", {"steps": steps[i:i + 10], "label": "large"})
                    if env.get("status") not in ("ok", None):
                        run.check(False, "the large drawing builds", env.get("error"))
                record["largeBuildS"] = round(time.monotonic() - t0, 1)
                # The revision without the content (draw.drawing.get would answer 50,000 points as JSON).
                fr = (await rc(session, "doc.object.find", {"type": "drawing", "name": "large"})).get("result") or {}
                rev = next((it.get("rev") for it in (fr.get("items") or fr.get("matches") or []) if isinstance(it, dict)), None)
                print("large drawing built", record["largeBuildS"], "s, rev", rev, flush=True)
                st = await wait_metrics(session, lambda s: any(d.get("id") == large.get("drawingId") and d.get("rev") == rev for d in s.get("drawings", [])), bound=120)
                loads = [l for l in ((st.get("metrics") or {}).get("loads") or []) if l.get("key") == large.get("drawingId")]
                await asyncio.sleep(6.0)
                st = await metrics_now(session)
                record["large"] = {"idle": (st.get("metrics") or {}).get("frameMs"), "load": loads[-1] if loads else None,
                                   "residentBytes": st.get("residentBytes")}
                g = await gesture("large")
                record["large"].update({"gesture": g.get("gestureFrameMs"), "commit": g.get("commitMs")})

                # The long stroke on the large drawing (D-13; C42B-R1-04): the reviewer's 9,998-event Brush gesture
                # through the window (an agent gesture played by the window), `--long-samples` times, each undone so
                # every sample commits to the same 50,000-point drawing; release-to-committed is the canvas's own
                # (release to the frame showing every drawing at its new revision).
                m_large = ((await rc(session, "draw.viewMapping.get", {"view": "drawingView"})).get("result") or {}).get("drawingToView") or m
                stroke = c42.drag([c42.apply(m_large, -180 + i * 240 / 9997, -140) for i in range(9_998)])
                long_commits = []
                for _ in range(int(record.get("longSamples") or 5)):
                    before = len(((await metrics_now(session)).get("metrics") or {}).get("commits") or [])
                    env = await e2e.call(session, "pointer", {"view": "drawingView", "tool": "brush", "events": stroke})
                    t1 = time.monotonic()
                    got = []
                    while time.monotonic() - t1 < 20:
                        got = (((await metrics_now(session)).get("metrics") or {}).get("commits") or [])[before:]
                        if got:
                            break
                        await asyncio.sleep(0.25)
                    if env.get("status") == "ok" and got:
                        long_commits.append(got[0])
                    await rc(session, "edit.undo", {})
                    await asyncio.sleep(0.75)
                ordered = sorted(long_commits)
                record["largeLongStroke"] = {
                    "events": 9_998, "samples": len(long_commits), "commitMs": long_commits,
                    "p50": ordered[len(ordered) // 2] if ordered else None,
                    "p95": ordered[max(0, -(-len(ordered) * 95 // 100) - 1)] if ordered else None,
                    "max": ordered[-1] if ordered else None,
                }

                def p95(x):
                    return (x or {}).get("p95")

                if record.get("ruleSet"):
                    # The product's own back-end measurement (ADR-0007 rule set step 1, SI-136): it runs once the canvas
                    # has been quiet for 30 s, so nothing is called for 40 s; then the stored cells and selection are read.
                    await asyncio.sleep(40.0)
                    t0 = time.monotonic()
                    while time.monotonic() - t0 < 180:
                        v = ((await rc(session, "prefs.get", {"keys": ["display.canvasBackendMeasured"]})).get("result") or {}).get("values", {})
                        meas = v.get("display.canvasBackendMeasured") or {}
                        if meas.get("fingerprint"):
                            record["ruleSet"] = {"selected": meas.get("selected"), "cells": json.loads(meas.get("cells") or "{}"),
                                                 # A digest only: receipts name the machine class, never models.
                                                 "fingerprintSha256": hashlib.sha256(str(meas.get("fingerprint")).encode()).hexdigest()[:16]}
                            break
                        await asyncio.sleep(5.0)
                    run.check(isinstance(record.get("ruleSet"), dict), "the product measured the back ends on the rule set's scenes", record.get("ruleSet"))

                if record.get("experiments"):
                    # E-C2: sustained edits on the large drawing, each re-tessellated and uploaded (ADR-0007, plan §7):
                    # no frame over 33 ms.
                    for k in range(20):
                        pts = [{"x": -500 + i * 4, "y": 300 + k * 3 + (i % 5), "pressure": 0.7} for i in range(50)]
                        await rc(session, "draw.stroke.add", {"kind": "brush", "points": pts, "fit": False, "layer": "top"})
                        await asyncio.sleep(0.15)
                    await asyncio.sleep(1.0)
                    mm = (await metrics_now(session)).get("metrics") or {}
                    reloads = [l for l in mm.get("loads") or [] if l.get("key") == large.get("drawingId")]
                    record["E-C2"] = {"frameMs": mm.get("frameMs"), "reloads": reloads[-10:]}
                    run.check((mm.get("frameMs") or {}).get("max", 1e9) <= 33.0 if bounds is BOUNDS["workstation"] else True,
                              "E-C2: no frame over 33 ms during sustained edits of the large drawing", mm.get("frameMs"))
                    # E-C1: a groups1k-like scene: 1,000 drawing elements shown by the auto light table; the view and the
                    # composition change every frame (a pan and a scene pull per step) for five seconds.
                    for b in range(10):
                        steps = []
                        for i in range(100):
                            n = b * 100 + i
                            steps += [
                                {"id": "element.create", "params": {"type": "drawing", "name": f"G{n}"}, "as": f"e{i}"},
                                {"id": "element.drawing.create", "params": {"elementId": f"$e{i}.result.elementIds.0", "name": "1", "exposeFrom": 1, "exposeTo": 1}, "as": f"d{i}"},
                                {"id": "draw.ellipse.add", "params": {"drawingId": f"$d{i}.result.drawingId", "from": {"x": -600 + (n % 40) * 30, "y": -400 + (n // 40) * 30}, "to": {"x": -580 + (n % 40) * 30, "y": -380 + (n // 40) * 30}}},
                            ]
                        env = await e2e.call(session, "batch", {"steps": steps, "label": "groups"})
                        if env.get("status") not in ("ok", None):
                            run.check(False, "E-C1: the 1,000 elements build", env.get("error"))
                            break
                    await rc(session, "app.context.set", {"elementId": eid, "frame": 1})
                    await rc(session, "drawAid.autoLightTable.set", {"enabled": True})
                    st = await wait_metrics(session, lambda s: len(s.get("drawings") or []) >= 1000, bound=180)
                    record["E-C1"] = {"resident": len(st.get("drawings") or [])}
                    t0 = time.monotonic()
                    i = 0
                    while time.monotonic() - t0 < 5.0:
                        await rc(session, "view.pan.set", {"viewId": "drawingView", "by": {"x": 1 if i % 2 else -1, "y": 0}})
                        await rc(session, "drawAid.lightTableShade.set", {"auto": i % 2 == 0})
                        i += 1
                    await asyncio.sleep(0.6)
                    mm = (await metrics_now(session)).get("metrics") or {}
                    record["E-C1"].update({"steps": i, "frameMs": mm.get("frameMs"), "sceneMs": mm.get("sceneMs")})
                    if bounds is BOUNDS["workstation"]:
                        run.check(p95(mm.get("sceneMs")) is not None and p95(mm.get("sceneMs")) <= 16.7,
                                  "E-C1: a scene pull is delivered within one frame (p95)", mm.get("sceneMs"))
                        run.check(p95(mm.get("frameMs")) is not None and p95(mm.get("frameMs")) <= 17.5,
                                  "E-C1: >= 57 fps while the 1,000-drawing scene changes every frame", mm.get("frameMs"))

                checks = [
                    ("frame time p95, idle, small", p95(record["small"]["idle"]), bounds["frameMs"]),
                    ("frame time p95, idle, large", p95(record["large"]["idle"]), bounds["frameMs"]),
                    ("frame time p95, during a gesture, small", p95(record["small"]["gesture"]), bounds["gestureFrameMs"]),
                    ("frame time p95, during a gesture, large", p95(record["large"]["gesture"]), bounds["gestureFrameMs"]),
                    ("release-to-committed p95, small", p95(record["small"]["commit"]), bounds["commitSmallMs"]),
                    ("release-to-committed p95, large", p95(record["large"]["commit"]), bounds["commitLargeMs"]),
                ]
                if sys.platform == "win32":
                    checks.append(("input-to-ink p95, small", p95(record["small"]["ink"]), bounds["inkMs"]))
                long_commit = (record.get("longStroke") or {}).get("commitMs") or []
                checks.append(("release-to-committed of a 10,000-sample stroke", long_commit[-1] if long_commit else None, bounds["commitLargeMs"]))
                checks.append(("release-to-committed p95 of a 9,998-event stroke on the large drawing",
                               (record.get("largeLongStroke") or {}).get("p95"), bounds["commitLargeMs"]))
                for name, value, bound in checks:
                    run.check(value is not None and value <= bound, f"{name} <= {bound} ms", value)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("app")
    ap.add_argument("out")
    ap.add_argument("--backend", choices=["webgl2", "webgpu"])
    ap.add_argument("--bounds", choices=["workstation", "ci"], default="ci")
    ap.add_argument("--experiments", action="store_true", help="also run E-C1 and E-C2 (ADR-0007)")
    ap.add_argument("--rule-set", action="store_true", help="also wait for the product's back-end measurement (SI-136)")
    ap.add_argument("--long-samples", type=int, default=None,
                    help="9,998-event strokes on the large drawing (default: 30 with workstation bounds, 5 on CI)")
    a = ap.parse_args()
    if a.long_samples is None:
        a.long_samples = 30 if a.bounds == "workstation" else 5
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    user = Path(tempfile.mkdtemp(prefix="tt-c42-measure-"))
    (user / "Settings").mkdir(parents=True)
    seed = {"agent.policy.defaults": {"policy": "autonomous", "maxJobs": 2, "maxBatchSteps": 1000, "maxInlineImagePx": 2048,
                                      "maxInlineBytes": 52428800, "askInApp": True, "letAgentMoveView": True, "highlightAgentChanges": True}}
    (user / "Settings" / "preferences.json").write_text(json.dumps({"format": "toontown-settings", "kind": "preferences", "formatVersion": "1.0", "data": seed}), encoding="utf-8")
    # The machine class only (OS and architecture): receipts never carry hardware model strings.
    record: dict = {"kind": "toontown.c42.canvas-measure", "platform": sys.platform, "machine": platform.system(),
                    "architecture": platform.architecture()[0], "boundsKind": a.bounds, "bounds": BOUNDS[a.bounds], "requestedBackend": a.backend,
                    "experiments": a.experiments, "ruleSet": a.rule_set, "longSamples": a.long_samples}
    discovery = out / "discovery.json"
    try:
        proc = launch(a.app, user, discovery, a.backend)
        try:
            d = e2e.wait_discovery(discovery, proc)
            asyncio.run(measure(d["endpoint"], d["token"], run, out, BOUNDS[a.bounds], record))
        finally:
            e2e.stop(proc)
    except Exception as ex:  # noqa: BLE001
        run.check(False, "the measurement ran", repr(ex))
    finally:
        shutil.rmtree(user, ignore_errors=True)
    record["failures"] = run.failures
    record.pop("canvasAt", None)
    run.save("measurements.json", record)
    print(json.dumps(record, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
