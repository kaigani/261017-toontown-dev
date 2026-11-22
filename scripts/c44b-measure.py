"""M4 C4.4b budgets (plan §5.5; rulings D-33, D-45, D-75 items 2 and 6): the four tools' workstation receipts.

Usage: python scripts/c44b-measure.py <toontown binary> <out dir> [--backend webgl2|webgpu] [--drags N] [--text-only|--large-only]

Windows (real OS input), release build, `--canvas-metrics`. On a small drawing (a 120 x 120 du rectangle), N OS drags
(default 30) per row, each undone: the Contour Editor's anchor drag, Perspective's corner drag, Reposition All's offset;
then 30 keystrokes typed into a Text tool session. From the canvas's metrics (`desktop/src/canvas/metrics.ts`):

| Quantity | Bound | Population |
|---|---|---|
| drag feedback (input to ink) | <= 16.7 ms p95 | `ink` of the drags (design notes 3 §9, 4 §10) |
| release to committed, small drawing | <= 50 ms p95 | `commits` of the drags |
| work per frame during drags | <= 8 ms p95 | `gestureWork` (C43B-R1S-08; an incomplete measurement fails) |
| keystroke to glyph | <= 2 control frames p95 (D-76 item 2; the control median of the run, D-33) | `keyToGlyphMs` (design note 5 §8) |
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

#: `--text-only`: the keystroke row alone (a diagnosis run; not a receipt).
ONLY = {"text": False, "large": False}
#: Commands a gesture can send that make no undo step (selection, point selection, tool).
UI_ONLY = {"draw.selection.set", "draw.pointSelection.set", "tool.active.set"}
#: Drags per row of the 50,000-point case (each commit and undo reloads a 17 MB payload).
LARGE_DRAGS = 3
BOUNDS = {"inkMs": 16.7, "commitSmallMs": 50.0, "gestureWorkMs": 8.0, "keyToGlyphFrames": 2}
cam = _load("c43cammeasure", "c43-camera-measure.py")


def added(before: list, after: list) -> list:
    """The values a ring gained between two reads: `after` less the longest suffix of `before` it starts with (the
    canvas's rings keep the last 50 or 300 values, so a full ring drops old values as it gains new ones)."""
    for shift in range(len(before) + 1):
        tail = before[shift:]
        if after[:len(tail)] == tail:
            return after[len(tail):]
    return list(after)


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
                # The control (D-33, D-76 item 2): the webview's cadence on the start screen, 300 frames of a loop doing
                # no work (`metrics.cadence`); its median is the display frame the keystroke bound counts in.
                await asyncio.sleep(7.0)
                control: dict = {}
                for _ in range(40):
                    tree = (await e2e.tree(s)).get("result") or {}
                    node = c42.find(tree.get("root") or {}, lambda n: n.get("id") == "metrics.cadence") or {}
                    try:
                        control = json.loads(node.get("value") or "{}")
                    except json.JSONDecodeError:
                        control = {}
                    if (control.get("frameMs") or {}).get("n", 0) >= 300:
                        break
                    await asyncio.sleep(0.5)
                record["control"] = control
                record["displayRefreshHz"] = cam.refresh_rate()
                run.check((control.get("frameMs") or {}).get("n", 0) >= 300 and control.get("startScreen") is True,
                          "the control: 300 frames of the start screen's cadence (D-33)", control)

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
                # Black ink (the default palette's Colour 2): the captures' ink mask needs a line colour.
                found = await res("doc.object.find", {"type": "swatch", "name": "Colour 2"})
                items = found.get("items") or found.get("matches") or []
                black = next((it.get("id") or it.get("key") for it in items if isinstance(it, dict)), None)
                if black:
                    await rc(s, "colour.current.set", {"colourId": black})
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

                async def resident() -> str:
                    return json.dumps((await c42.canvas_state(s)).get("drawings"))

                async def row(name: str, tool: str, dpts: list, n: int, select=True, into="rows") -> dict:
                    await rc(s, "tool.active.set", {"toolId": tool})
                    if select:
                        await rc(s, "draw.selection.set", {"by": "ids", "ids": [sid], "mode": "replace"})
                    await asyncio.sleep(0.8)
                    errors = []
                    commits: list = []
                    ink: list = []
                    work: list = []
                    notes: list = []
                    for _ in range(n):
                        # Each drag's own values: the rings read before and after it (an undo commits no drag).
                        m0 = await metrics()
                        t = await os_drag(dpts)
                        if "error" in t:
                            errors.append(t["error"])
                        # The commit drawn: a wait for the state (D-45), bounded for a drag that commits nothing.
                        t1 = time.monotonic()
                        m1 = await metrics()
                        bound = 15.0 if into == "rows" else 90.0
                        while not added(m0.get("commits") or [], m1.get("commits") or []) and time.monotonic() - t1 < bound:
                            await asyncio.sleep(0.1)
                            m1 = await metrics()
                        got = added(m0.get("commits") or [], m1.get("commits") or [])
                        st = await c42.canvas_state(s)
                        notes.append({"commits": got, "status": (st.get("lastGesture") or {}).get("status"),
                                      "code": (st.get("lastGesture") or {}).get("code"),
                                      "commands": [c.get("commandId") for c in (st.get("lastGesture") or {}).get("commands") or []],
                                      "toolBox": st.get("toolBox"), "failures": st.get("failures"), "times": t})
                        cmds = [c.get("commandId") for c in (st.get("lastGesture") or {}).get("commands") or []]
                        committed = (st.get("lastGesture") or {}).get("status") == "ok" and any(c not in UI_ONLY for c in cmds)
                        # Release to committed counts the drags that committed an edit (a selection click draws no commit).
                        if committed:
                            commits += got
                        ink += added(m0.get("ink") or [], m1.get("ink") or [])
                        work += added(m0.get("gestureWork") or [], m1.get("gestureWork") or [])
                        # The drag's own step undone, and only when it made one (D-76 item 3: an undo after a drag that
                        # committed nothing undid the stroke, then the drawing, then the element of the earlier steps).
                        if not committed:
                            await asyncio.sleep(0.3)
                            continue
                        drawn = await resident()
                        await rc(s, "edit.undo", {})
                        t2 = time.monotonic()
                        while await resident() == drawn and time.monotonic() - t2 < bound:
                            await asyncio.sleep(0.1)
                        await asyncio.sleep(0.3)
                    out_ = {"name": name, "drags": n, "errors": errors, "commitP95": p95(commits), "commits": len(commits),
                            "commitsMs": commits, "inkP95": p95(ink), "inkSamples": len(ink), "gestureWorkP95": p95(work),
                            "workFrames": len(work), **({"notes": notes} if into == "large" else {})}
                    record.setdefault(into, []).append(out_)
                    return out_

                rows = [] if ONLY["text"] or ONLY["large"] else [
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
                await rc(s, "tool.active.set", {"toolId": "text"} if not ONLY["large"] else {"toolId": "select"})
                await asyncio.sleep(0.5)
                await os_drag([(-120, -50), (-120, -50)])
                await asyncio.sleep(1.5)
                for ch in "" if ONLY["large"] else "the quick brown fox jumps over":
                    eq.os_type(ch)
                    await asyncio.sleep(0.25)
                await asyncio.sleep(1.5)
                mk = await metrics()
                k = mk.get("keyToGlyphMs") or {}
                record["keyToGlyph"] = k
                record["keyParts"] = {"stages": mk.get("keyStagesMs"), "sceneMs": mk.get("sceneMs"), "loads": mk.get("loads")}
                # D-76 item 2: the glyph within two display frames of the key, p95, a frame being this run's control median.
                frame = (record.get("control", {}).get("frameMs") or {}).get("p50")
                bound = BOUNDS["keyToGlyphFrames"] * frame if frame else None
                record["keyToGlyphBound"] = {"frames": BOUNDS["keyToGlyphFrames"], "controlMedianMs": frame, "boundMs": bound,
                                             "marginMs": round(bound - k["p95"], 2) if bound and k.get("p95") is not None else None}
                run.check(ONLY["large"] or (k.get("n", 0) >= 25 and k.get("p95") is not None and bound is not None and k["p95"] <= bound),
                          f"keystroke to glyph {k.get('p95')} ms p95 <= two control frames ({bound} ms; {k.get('n')} keys)",
                          {**k, **record["keyToGlyphBound"]})
                await rc(s, "tool.active.set", {"toolId": "select"})
                if ONLY["text"]:
                    return

                # The 50,000-point case: one stroke on its own layer (reported, not asserted: D-75 items 2 and 6).
                await rc(s, "app.context.set", {"elementId": eid, "frame": 2})
                # The whole 400 x 160 du stroke in the Drawing view, so every drag lands on it.
                await rc(s, "view.zoom.set", {"viewId": "drawingView", "zoom": 0.5})
                await rc(s, "view.pan.set", {"viewId": "drawingView", "centre": {"x": 0, "y": 0}})
                pts = [{"x": -200 + 400 * (i / 50000), "y": 80 * math.sin(i / 300.0)} for i in range(50000)]
                big = await res("draw.stroke.add", {"kind": "pencil", "points": pts, "layer": "new", "fit": False})
                bid = (big.get("strokeIds") or [None])[0]
                run.check(bid is not None, "the 50,000-point stroke is added (straight segments, no fitting)", big)
                if bid is None:
                    return
                t3 = time.monotonic()
                while time.monotonic() - t3 < 30.0 and not any(d.get("bucket") is not None for d in json.loads(await resident()) or []):
                    await asyncio.sleep(0.2)
                await asyncio.sleep(1.0)
                large = []
                for name, tool, dpts in (("Contour Editor anchor drag (50,000 points)", "contourEditor", [(-200, 0), (-190, 10), (-180, 20)]),
                                         ("Perspective corner drag (50,000 points)", "perspective", [(200, 80), (190, 80), (180, 80)]),
                                         ("Reposition All offset (50,000 points)", "repositionAll", [(0, 0), (10, 10), (20, 20)])):
                    await rc(s, "draw.selection.set", {"by": "ids", "ids": [bid], "mode": "replace"})
                    sid_saved = sid
                    sid = bid or sid
                    r = await row(name, tool, dpts, LARGE_DRAGS, into="large")
                    sid = sid_saved
                    large.append(r)
                run.check(True, "the 50,000-point case is reported, not asserted (D-75 items 2 and 6; M11 C11.1)", large)


def main() -> int:
    e6.dpi_aware()
    args = sys.argv[1:]
    backend, drags = None, 30
    if "--backend" in args:
        i = args.index("--backend")
        backend = args[i + 1]
        del args[i:i + 2]
    if "--large-only" in args:
        ONLY["large"] = True
        args.remove("--large-only")
    if "--text-only" in args:
        ONLY["text"] = True
        args.remove("--text-only")
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
        # The OS input finds the window by the process.
        e2e.APP_PID["pid"] = proc.pid
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
