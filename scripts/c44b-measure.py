"""M4 C4.4b budgets (plan §5.5; rulings D-33, D-45, D-75 items 2 and 6): the four tools' workstation receipts.

Usage: python scripts/c44b-measure.py <toontown binary> <out dir> [--backend webgl2|webgpu] [--drags N]

Windows (real OS input), release build, `--canvas-metrics`. On a small drawing (a 120 x 120 du rectangle), N OS drags
(default 30) per row, each undone: the Contour Editor's anchor drag, Perspective's corner drag, Reposition All's offset;
then 30 keystrokes typed into a Text tool session. From the canvas's metrics (`desktop/src/canvas/metrics.ts`):

| Quantity | Bound | Population |
|---|---|---|
| drag feedback (input to ink) | <= 16.7 ms p95 | `ink` of the drags (design notes 3 §9, 4 §10) |
| release to committed, small drawing | <= 50 ms p95 | `commits` of the drags |
| work per frame during drags | <= 8 ms p95 | `gestureWork` (C43B-R1S-08; an incomplete measurement fails) |
| keystroke to glyph | <= 33 ms p95 | `keyToGlyphMs` (design note 5 §8) |
| release to committed, 50,000 points | reported, not asserted (D-75 items 2, 6; M11 C11.1) | a stroke of 50,000 points on its own layer |

Receipts: `measure.json` and `manifest.json` (kind `toontown.c44b.measure`); the committed receipt is their summary
(D-42). Exit 0 when every asserted bound holds.
"""

from __future__ import annotations

import asyncio
import importlib.util
import json
import math
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


meas = _load("c42measure", "c42-canvas-measure.py")
e6 = _load("c43e6", "c43-e6-coexistence.py")
eq = _load("c44bequiv", "c44b-gesture-equivalence.py")
e2e, c42 = e6.e2e, e6.c42
rc = e2e.run_cmd

BOUNDS = {"inkMs": 16.7, "commitSmallMs": 50.0, "gestureWorkMs": 8.0, "keyToGlyphMs": 33.0}


def p95(values: list) -> float | None:
    s = sorted(values)
    return s[max(0, math.ceil(len(s) * 0.95) - 1)] if s else None


async def measure(url: str, token: str, run, out: Path, drags: int, record: dict) -> None:
    client = e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {token}"}, timeout=e2e.httpx2.Timeout(60.0, read=600.0))
    async with client:
        async with e2e.streamable_http_client(url, http_client=client) as streams:
            async with e2e.ClientSession(streams[0], streams[1]) as s:
                await s.initialize()
                await e2e.gui_tree(s, run, lambda r: e2e.by_id(r, "startScreen") is not None, "the GUI is up")

                async def res(cid, params=None):
                    return (await rc(s, cid, params or {})).get("result") or {}

                async def metrics() -> dict:
                    return ((await c42.canvas_state(s)).get("metrics")) or {}

                await rc(s, "project.create", {"name": "Measure", "onUnsaved": "fail"})
                await rc(s, "workspace.select", {"name": "Drawing"})
                eid = ((await res("element.create", {"type": "drawing", "name": "Ink"})).get("elementIds") or [None])[0]
                await rc(s, "element.drawing.create", {"elementId": eid, "name": "small", "exposeFrom": 1, "exposeTo": 1})
                await rc(s, "element.drawing.create", {"elementId": eid, "name": "large", "exposeFrom": 2, "exposeTo": 2})
                await rc(s, "app.context.set", {"elementId": eid, "frame": 1})
                sq = await res("draw.rectangle.add", {"from": {"x": 30, "y": -60}, "to": {"x": 150, "y": 60}})
                sid = (sq.get("strokeIds") or [None])[0]
                await rc(s, "view.active.set", {"viewId": "drawingView"})
                await asyncio.sleep(2.0)
                place = None
                t0 = time.monotonic()
                while time.monotonic() - t0 < 30 and not place:
                    place = await e6.locate_view(s, rc, "drawingView")
                    if not place:
                        await asyncio.sleep(0.5)
                run.check(place is not None, "the Drawing view's canvas is located (OS input)", place)
                if not place:
                    return
                listed = await res("view.list", {})
                views = listed.get("views") or [] if isinstance(listed, dict) else listed
                logical = next((v.get("size") for v in views if v.get("viewId") == "drawingView"), None) or {}
                ratio = place[2] / logical["width"] if logical.get("width") else 1.0

                async def os_drag(dpts: list, mods=()):
                    m = (await res("draw.viewMapping.get", {"view": "drawingView"})).get("drawingToView") or [1, 0, 0, 1, 0, 0]
                    pts = []
                    # Intermediate samples so the drag has frames of feedback (about 25 samples over 0.5 s).
                    for (x0, y0), (x1, y1) in zip(dpts, dpts[1:]):
                        for k in range(12):
                            pts.append((x0 + (x1 - x0) * k / 12, y0 + (y1 - y0) * k / 12))
                    pts.append(dpts[-1])
                    vpts = [c42.apply(m, x, y) for x, y in pts]
                    client_pts = [(place[0] + x * ratio, place[1] + y * ratio) for x, y in vpts]
                    times: dict = {}
                    await asyncio.get_running_loop().run_in_executor(None, eq.os_drag, e2e.APP_PID["pid"], client_pts, list(mods), times)
                    return times

                async def row(name: str, tool: str, dpts: list, n: int, select=True) -> dict:
                    await rc(s, "tool.active.set", {"toolId": tool})
                    if select:
                        await rc(s, "draw.selection.set", {"by": "ids", "ids": [sid], "mode": "replace"})
                    await asyncio.sleep(0.8)
                    m0 = await metrics()
                    c0, i0, w0 = len(m0.get("commits") or []), len(m0.get("ink") or []), len(m0.get("gestureWork") or [])
                    errors = []
                    for _ in range(n):
                        t = await os_drag(dpts)
                        if "error" in t:
                            errors.append(t["error"])
                        await asyncio.sleep(0.7)
                        await rc(s, "edit.undo", {})
                        await asyncio.sleep(0.3)
                    await asyncio.sleep(1.2)
                    m1 = await metrics()
                    # The rings keep the last 50 commits and 300 samples: the counts since the row began.
                    commits = (m1.get("commits") or [])[c0:] if len(m1.get("commits") or []) > c0 else (m1.get("commits") or [])[-n:]
                    ink = (m1.get("ink") or [])[i0:] if len(m1.get("ink") or []) > i0 else (m1.get("ink") or [])
                    work = (m1.get("gestureWork") or [])[w0:] if len(m1.get("gestureWork") or []) > w0 else (m1.get("gestureWork") or [])
                    out_ = {"name": name, "drags": n, "errors": errors, "commitP95": p95(commits), "commits": len(commits),
                            "inkP95": p95(ink), "inkSamples": len(ink), "gestureWorkP95": p95(work), "workFrames": len(work)}
                    record.setdefault("rows", []).append(out_)
                    return out_

                rows = [
                    await row("Contour Editor anchor drag", "contourEditor", [(30, -60), (45, -50), (60, -40)], drags),
                    await row("Perspective corner drag", "perspective", [(150, 60), (135, 60), (120, 60)], drags),
                    await row("Reposition All offset", "repositionAll", [(70, -20), (85, -10), (100, 0)], drags, select=False),
                ]
                for r in rows:
                    complete = r["commits"] >= drags and r["inkSamples"] > 0 and r["workFrames"] > 0 and not r["errors"]
                    run.check(complete, f"{r['name']}: a complete measurement ({drags} drags)", r)
                    run.check(complete and r["inkP95"] is not None and r["inkP95"] <= BOUNDS["inkMs"],
                              f"{r['name']}: drag feedback {r['inkP95']} ms p95 <= {BOUNDS['inkMs']} ms", r)
                    run.check(complete and r["commitP95"] is not None and r["commitP95"] <= BOUNDS["commitSmallMs"],
                              f"{r['name']}: release to committed {r['commitP95']} ms p95 <= {BOUNDS['commitSmallMs']} ms", r)
                    run.check(complete and r["gestureWorkP95"] is not None and r["gestureWorkP95"] <= BOUNDS["gestureWorkMs"],
                              f"{r['name']}: work per frame during the drags {r['gestureWorkP95']} ms p95 <= {BOUNDS['gestureWorkMs']} ms", r)

                # Keystroke to glyph: a Text tool session opened by a real click, 30 real keystrokes.
                await rc(s, "tool.active.set", {"toolId": "text"})
                await asyncio.sleep(0.5)
                await os_drag([(-200, -120), (-200, -120)])
                await asyncio.sleep(1.5)
                for ch in "the quick brown fox jumps over":
                    eq.os_type(ch)
                    await asyncio.sleep(0.25)
                await asyncio.sleep(1.5)
                k = (await metrics()).get("keyToGlyphMs") or {}
                record["keyToGlyph"] = k
                run.check(k.get("n", 0) >= 25 and k.get("p95") is not None and k["p95"] <= BOUNDS["keyToGlyphMs"],
                          f"keystroke to glyph {k.get('p95')} ms p95 <= {BOUNDS['keyToGlyphMs']} ms ({k.get('n')} keys)", k)
                await rc(s, "tool.active.set", {"toolId": "select"})

                # The 50,000-point case: one stroke on its own layer (reported, not asserted: D-75 items 2 and 6).
                await rc(s, "app.context.set", {"elementId": eid, "frame": 2})
                pts = [{"x": -200 + 400 * (i / 50000), "y": 80 * math.sin(i / 300.0)} for i in range(50000)]
                big = await res("draw.stroke.add", {"kind": "pencil", "points": pts, "layer": "new", "smoothing": 0})
                bid = (big.get("strokeIds") or [None])[0]
                await asyncio.sleep(3.0)
                large = []
                for name, tool, dpts in (("Contour Editor anchor drag (50,000 points)", "contourEditor", [(-200, 0), (-190, 10), (-180, 20)]),
                                         ("Perspective corner drag (50,000 points)", "perspective", [(200, 80), (190, 80), (180, 80)]),
                                         ("Reposition All offset (50,000 points)", "repositionAll", [(0, 0), (10, 10), (20, 20)])):
                    await rc(s, "draw.selection.set", {"by": "ids", "ids": [bid] if bid else [], "mode": "replace"})
                    sid_saved = sid
                    sid = bid or sid
                    r = await row(name, tool, dpts, 10)
                    sid = sid_saved
                    large.append(r)
                record["large"] = large
                run.check(True, "the 50,000-point case is reported, not asserted (D-75 items 2 and 6; M11 C11.1)", large)


def main() -> int:
    e6.dpi_aware()
    args = sys.argv[1:]
    backend, drags = None, 30
    if "--backend" in args:
        i = args.index("--backend")
        backend = args[i + 1]
        del args[i:i + 2]
    if "--drags" in args:
        i = args.index("--drags")
        drags = int(args[i + 1])
        del args[i:i + 2]
    if len(args) != 2 or backend not in (None, "webgl2", "webgpu"):
        print(__doc__)
        return 2
    app, out = args[0], Path(args[1])
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    record: dict = {"backend": backend, "drags": drags}
    started = time.monotonic()
    if sys.platform != "win32":
        run.check(True, "the budgets are Windows workstation receipts (real OS input)", None)
    else:
        user = Path(tempfile.mkdtemp(prefix="tt-c44b-measure-"))
        discovery = out / "discovery.json"
        proc = meas.launch(app, user, discovery, backend)
        try:
            d = e2e.wait_discovery(discovery, proc)
            asyncio.run(measure(d["endpoint"], d["token"], run, out, drags, record))
        except Exception as ex:  # noqa: BLE001 - reported as a failure with its text
            run.check(False, "the measurement ran", repr(ex))
        finally:
            e2e.stop(proc)
    manifest = {"kind": "toontown.c44b.measure", "platform": sys.platform, "backend": backend,
                "seconds": round(time.monotonic() - started, 1), "bounds": BOUNDS, "failures": run.failures}
    run.save("measure.json", record)
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps({**manifest, "rows": record.get("rows"), "keyToGlyph": record.get("keyToGlyph")}, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
