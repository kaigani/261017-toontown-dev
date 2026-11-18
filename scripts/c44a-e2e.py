#!/usr/bin/env python3
"""M4 C4.4a end-to-end checks through the real window (reviews/M4-C4.4-plan.md §4.1, §4.2, §4.6, §4.8, §4.9).

Usage: python scripts/c44a-e2e.py <app> <out>   (Windows: real input; the dist served on 1420 for a release build)

1. Pen (§4.1, 02 line 376): a pen edit through the window (the Maximum size slider, real keys) changes the next Brush
   stroke's width and leaves the existing stroke as it was.
2. Tool options persist (§4.2, 02 lines 1111-1113): Draw on Top Layer set through the Properties panel (a real click)
   survives a restart, and the gesture driver reads it (a Brush stroke after it merges into the top layer, 02 §8.10).
3. Toolbar (§4.8): a toolbar customised in the sheet (real clicks) survives a restart; Restore Default returns the
   items of `toolbar-default.json`.
4. Camera list (§4.9): a second camera created by an agent appears in the list; choosing it (real keys) makes it
   current and changes the Camera view's framing.
5. Activity export (§4.6): Export… writes the log, read back entry for entry.
6. Agent server (§4.6): switched off in Preferences › Agent access (real clicks, OK) the discovery file loses its
   endpoint; switched on again it returns (this script's own MCP session ends with it; the checks go on through the
   page and a new session).
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
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


d39 = _load("c43d39", "c43-d39-bindings.py")
e2e = d39.e2e
e6 = d39.e6


def find_key(v, key):
    if isinstance(v, dict):
        if key in v:
            return v[key]
        for x in v.values():
            r = find_key(x, key)
            if r is not None:
                return r
    if isinstance(v, list):
        for x in v:
            r = find_key(x, key)
            if r is not None:
                return r
    return None


def strokes_of(d):
    out = []

    def walk(ns):
        for n in ns or []:
            out.extend(n.get("strokes") or [])
            walk(n.get("children"))
    walk(d.get("nodes"))
    return out


async def shown(t, ident: str, bound_s: float = 10.0) -> None:
    """Waits until a control is rendered and visible (a wait for a state, ruling D-45; the bound guards a hang)."""
    start = time.monotonic()
    while time.monotonic() - start < bound_s:
        r = t.rect(ident)
        if r and not r.get("hidden"):
            return
        await asyncio.sleep(0.2)
    raise RuntimeError(f"control absent or hidden after {bound_s} s: {ident}")


async def connect(stack: contextlib.AsyncExitStack, discovery: Path, work: Path):
    d = json.loads(discovery.read_text(encoding="utf-8"))
    client = await stack.enter_async_context(
        e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {d['token']}"}, timeout=e2e.httpx2.Timeout(60.0, read=300.0)))
    streams = await stack.enter_async_context(e2e.streamable_http_client(d["endpoint"], http_client=client))
    from mcp import types

    async def roots(_ctx):
        return types.ListRootsResult(roots=[types.Root(uri=work.resolve().as_uri(), name="work")])
    s = await stack.enter_async_context(e2e.ClientSession(streams[0], streams[1], list_roots_callback=roots))
    await s.initialize()
    return s


async def first_run(app: str, run, out: Path, work: Path, user: Path) -> dict:
    discovery = work / "discovery.json"
    proc = e2e.launch(app, user, discovery)
    facts: dict = {}
    try:
        e2e.wait_discovery(discovery, proc)
        async with contextlib.AsyncExitStack() as stack:
            s = await connect(stack, discovery, work)
            await e2e.gui_tree(s, run, lambda r: e2e.by_id(r, "startScreen") is not None, "the GUI is up")
            t = d39.Table(s, run, out, work)

            async def cmd(n, p=None):
                return await t.cmd(n, p or {})
            await cmd("project.create", {"name": "E2E", "onUnsaved": "fail"})
            await cmd("project.saveAs", {"path": str(work / "E2E.animpkg"), "onExists": "overwrite"})
            bob = ((await cmd("element.create", {"type": "drawing", "name": "Bob"})).get("elementIds") or [None])[0]
            await cmd("element.drawing.create", {"elementId": bob, "name": "1", "exposeFrom": 1, "exposeTo": 24})
            await cmd("app.context.set", {"elementId": bob, "frame": 1})
            for p in ("drawingView", "cameraView"):
                await cmd("window.panel.set", {"panelId": p, "visible": True})
            await asyncio.sleep(1.0)
            t.cdp = d39.Cdp(d39.CDP_PORT)
            t.cdp.record()

            # 1. Pen.
            await cmd("tool.active.set", {"toolId": "brush"})
            await cmd("draw.stroke.add", {"kind": "brush", "points": [{"x": -150, "y": 0}, {"x": -60, "y": 40}], "layer": "new"})
            before = strokes_of(await cmd("draw.drawing.get", {}))
            first = before[-1]
            await cmd("window.panel.set", {"panelId": "pen", "visible": True, "focus": True})
            await asyncio.sleep(1.0)
            d39.front()
            t.cdp.js("document.querySelector('[data-tt-id=\"pen.maxSize\"]').focus()")
            for _ in range(4):
                d39.key(0x27)
            await asyncio.sleep(0.8)
            pens = await cmd("tool.pen.list", {})
            current = pens.get("currentBrushPenId")
            size = next((p.get("maxSize") for p in pens.get("pens") or [] if p.get("penId") == current), None)
            await cmd("draw.stroke.add", {"kind": "brush", "points": [{"x": 60, "y": 0}, {"x": 150, "y": 40}], "layer": "new"})
            after = strokes_of(await cmd("draw.drawing.get", {}))
            old_now = next((x for x in after if x.get("id") == first.get("id")), {})
            new = after[-1]
            ok = (find_key(new, "maxSize") == size and find_key(old_now, "maxSize") == find_key(first, "maxSize")
                  and size != find_key(first, "maxSize"))
            run.check(ok, "C4.4a pen: a pen edit through the window changes the next Brush stroke's width, not the existing one (02 line 376)",
                      {"penMaxSize": size, "first": find_key(first, "maxSize"), "firstAfter": find_key(old_now, "maxSize"), "next": find_key(new, "maxSize")})

            # 2. Tool options: Draw on Top Layer through the Properties panel.
            await cmd("window.panel.set", {"panelId": "properties", "visible": True, "focus": True})
            await asyncio.sleep(1.0)
            was = (await cmd("tool.settings.get", {})).get("drawTopLayer")
            await shown(t, "toolOptions.drawTopLayer")
            t.click("toolOptions.drawTopLayer")
            await asyncio.sleep(0.8)
            now = (await cmd("tool.settings.get", {})).get("drawTopLayer")
            if now is not True:
                await shown(t, "toolOptions.drawTopLayer")
                t.click("toolOptions.drawTopLayer")
                await asyncio.sleep(0.8)
                now = (await cmd("tool.settings.get", {})).get("drawTopLayer")
            layers_before = len((await cmd("draw.drawing.get", {})).get("nodes") or [])
            # A real Brush drag in the Drawing view: the gesture driver reads the option (02 §8.10).
            await cmd("tool.active.set", {"toolId": "brush"})
            await cmd("window.panel.set", {"panelId": "drawingView", "visible": True, "focus": True})
            await cmd("view.active.set", {"viewId": "drawingView"})
            await asyncio.sleep(0.8)
            r = t.rect("drawingView.canvas")
            cx, cy = r["x"] * r["dpr"], r["y"] * r["dpr"]
            times: dict = {}
            e6.os_drag(e2e.APP_PID["pid"], [(cx - 60, cy + 40), (cx - 20, cy + 60), (cx + 20, cy + 40)], 0.4, 0.1, times)
            await asyncio.sleep(1.5)
            layers_after = len((await cmd("draw.drawing.get", {})).get("nodes") or [])
            run.check(now is True and layers_after == layers_before,
                      "C4.4a options: Draw on Top Layer set through the window; the next Brush stroke goes on the top layer (02 §8.10)",
                      {"was": was, "now": now, "layersBefore": layers_before, "layersAfter": layers_after})
            facts["drawTopLayer"] = now

            # 3. Toolbar: the sheet's Apply with the first item removed.
            d39.choose_menu(["Window", "Customize Toolbar"])
            await asyncio.sleep(1.2)
            items0 = next((x.get("items") for x in await cmd("window.toolbar.list", {}) if x.get("toolbarId") == "main"), [])
            await shown(t, "toolbarSheet.item.0")
            t.click("toolbarSheet.item.0")
            await shown(t, "toolbarSheet.remove")
            t.click("toolbarSheet.remove")
            await asyncio.sleep(0.3)
            await shown(t, "toolbarSheet.submit")
            t.click("toolbarSheet.submit")
            await asyncio.sleep(1.0)
            items1 = next((x.get("items") for x in await cmd("window.toolbar.list", {}) if x.get("toolbarId") == "main"), [])
            run.check(items1 == items0[1:], "C4.4a toolbar: the sheet's Apply customises the main toolbar", {"before": items0, "after": items1})
            # Stored with the workspace, as at quit with autosave on (agents cannot quit; the C4.1 e2e does the same).
            await cmd("workspace.save", {})
            facts["toolbar"] = items1
            facts["toolbarDefault"] = items0

            # 4. Camera list: an agent's second camera, chosen with real keys.
            cams0 = (await cmd("anim.camera.list", {})).get("cameras") or []
            await cmd("anim.camera.create", {"name": "Wide", "fov": 80})
            # The scene's first camera current again, so the person's choice is the change.
            if cams0:
                await cmd("anim.camera.current.set", {"cameraId": cams0[0].get("id")})
            # Creating the camera made it the current element; the mapping is the drawing's (Bob's).
            await cmd("app.context.set", {"elementId": bob, "frame": 1})
            await cmd("window.panel.set", {"panelId": "cameraView", "visible": True})
            await asyncio.sleep(1.0)
            options = t.cdp.js("[...document.querySelectorAll('[data-tt-id=\"mainToolbar.cameraList\"] option')].map(o => o.textContent)")
            shot0 = await cmd("draw.viewMapping.get", {"view": "cameraView"})
            cams = (await cmd("anim.camera.list", {})).get("cameras") or []
            wide = next((c for c in cams if c.get("name") == "Wide"), {})
            moves = t.cdp.js("(() => { const s = document.querySelector('[data-tt-id=\"mainToolbar.cameraList\"]');"
                             " return [...s.options].findIndex(o => o.value === %s) - s.selectedIndex; })()" % json.dumps(wide.get("id")))
            await shown(t, "mainToolbar.cameraList")
            t.click("mainToolbar.cameraList")
            await asyncio.sleep(0.3)
            for _ in range(abs(moves or 0)):
                d39.key(0x28 if (moves or 0) > 0 else 0x26)
            d39.key(0x0D)
            await asyncio.sleep(1.5)
            current = (await cmd("anim.camera.current.get", {})).get("cameraId")
            shot1 = await cmd("draw.viewMapping.get", {"view": "cameraView"})
            framing = json.dumps(shot0, sort_keys=True) != json.dumps(shot1, sort_keys=True)
            facts["shotKeys"] = sorted(shot0) if isinstance(shot0, dict) else str(type(shot0))
            run.check("Wide" in (options or []) and current == wide.get("id") and len(cams) == len(cams0) + 1 and framing,
                      "C4.4a camera list: an agent's second camera appears; choosing it makes it current and changes the Camera view",
                      {"options": options, "current": current, "wide": wide.get("id"), "framingChanged": framing,
                       "shotKeys": facts.get("shotKeys"), "cameras0": cams0})

            # 5. Activity export, read back.
            await cmd("window.panel.set", {"panelId": "activity", "visible": True, "focus": True})
            await asyncio.sleep(1.0)
            target = work / "activity.jsonl"
            await shown(t, "activity.export")
            t.click("activity.export")
            await asyncio.sleep(1.8)
            d39.type_text(str(target))
            d39.key(0x0D)
            await asyncio.sleep(1.5)
            lines = [json.loads(x) for x in target.read_text(encoding="utf-8").splitlines() if x.strip()] if target.exists() else []
            log = await cmd("agent.activity.list", {"limit": 10000})
            seqs = [e.get("seq") for e in lines]
            run.check(bool(lines) and seqs == [e.get("seq") for e in log][:len(seqs)],
                      "C4.4a activity export: Export… writes the log, read back entry for entry", {"entries": len(lines)})
            await cmd("project.save", {})
            await asyncio.sleep(1.0)

            # 6. The server switched off and on from Preferences (this session ends with the first OK).
            def endpoint():
                with contextlib.suppress(Exception):
                    return json.loads(discovery.read_text(encoding="utf-8")).get("endpoint")
                return None
            on_before = endpoint()

            async def toggle_server():
                d39.choose_menu(["File", "Preferences"])
                await asyncio.sleep(0.8)
                await shown(t, "preferences.category.agent")
                t.click("preferences.category.agent")
                await asyncio.sleep(0.6)
                await shown(t, "agentAccess.server")
                t.click("agentAccess.server")
                await asyncio.sleep(0.3)
                await shown(t, "preferences.submit")
                t.click("preferences.submit")
                await asyncio.sleep(2.0)
            # The preference first on (the server runs from the launch flag), from the page.
            t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command', {json: JSON.stringify({command: 'prefs.set', params:"
                     " {values: {'agent.server.enabled': true}}})}).then(() => true)")
            await asyncio.sleep(0.8)
            await toggle_server()
            off = endpoint()
            await toggle_server()
            on_again = endpoint()
            run.check(bool(on_before) and off is None and bool(on_again),
                      "C4.4a agent server: off from Preferences clears the discovery file's endpoint; on again it returns",
                      {"before": on_before, "off": off, "onAgain": on_again})
    finally:
        with contextlib.suppress(Exception):
            e2e.stop(proc)
    return facts


async def second_run(app: str, run, out: Path, work: Path, user: Path, facts: dict) -> None:
    discovery = work / "discovery-2.json"
    proc = e2e.launch(app, user, discovery)
    try:
        e2e.wait_discovery(discovery, proc)
        async with contextlib.AsyncExitStack() as stack:
            s = await connect(stack, discovery, work)
            await e2e.gui_tree(s, run, lambda r: e2e.by_id(r, "startScreen") is not None or e2e.by_id(r, "app") is not None, "the GUI is up again")
            t = d39.Table(s, run, out, work)
            await t.cmd("project.open", {"path": str(work / "E2E.animpkg"), "onUnsaved": "fail"})
            settings = await t.cmd("tool.settings.get", {})
            run.check(settings.get("drawTopLayer") == facts.get("drawTopLayer"),
                      "C4.4a options: Draw on Top Layer persisted across a restart (02 lines 1111-1113)", settings)
            items = next((x.get("items") for x in await t.cmd("window.toolbar.list", {}) if x.get("toolbarId") == "main"), [])
            run.check(items == facts.get("toolbar"), "C4.4a toolbar: the customised toolbar persisted across a restart",
                      {"expected": facts.get("toolbar"), "got": items})
            await asyncio.sleep(1.0)
            t.cdp = d39.Cdp(d39.CDP_PORT)
            d39.choose_menu(["Window", "Customize Toolbar"])
            await asyncio.sleep(1.2)
            await shown(t, "toolbarSheet.restore")
            t.click("toolbarSheet.restore")
            await asyncio.sleep(1.0)
            default = json.loads((ROOT / "crates/executor/src/toolbar-default.json").read_text(encoding="utf-8"))
            want = default.get("items") if isinstance(default, dict) else default
            items = next((x.get("items") for x in await t.cmd("window.toolbar.list", {}) if x.get("toolbarId") == "main"), [])
            run.check(items == want, "C4.4a toolbar: Restore Default returns toolbar-default.json's items", {"expected": want, "got": items})
    finally:
        with contextlib.suppress(Exception):
            e2e.stop(proc)


def main() -> int:
    e6.dpi_aware()
    ap = argparse.ArgumentParser()
    ap.add_argument("app")
    ap.add_argument("out")
    a = ap.parse_args()
    if sys.platform != "win32":
        print("the C4.4a e2e drives the real window through Windows input")
        return 0
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    work = Path(tempfile.mkdtemp(prefix="tt-c44a-"))
    user = work / "user"
    (user / "Settings").mkdir(parents=True)
    seed = {"agent.policy.defaults": {"policy": "autonomous", "maxJobs": 2, "maxBatchSteps": 1000, "maxInlineImagePx": 2048,
                                      "maxInlineBytes": 52428800, "askInApp": True, "letAgentMoveView": True, "highlightAgentChanges": True}}
    (user / "Settings" / "preferences.json").write_text(json.dumps({"format": "toontown-settings", "kind": "preferences",
                                                                     "formatVersion": "1.0", "data": seed}), encoding="utf-8")
    os.environ["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = f"--remote-debugging-port={d39.CDP_PORT}"
    os.environ["WEBVIEW2_USER_DATA_FOLDER"] = str(work / "webview2")
    started = time.monotonic()
    try:
        facts = asyncio.run(first_run(a.app, run, out, work, user))
        asyncio.run(second_run(a.app, run, out, work, user, facts))
    except Exception as ex:  # noqa: BLE001
        import traceback
        traceback.print_exception(ex)
        run.check(False, "the C4.4a e2e ran", repr(ex))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    manifest = {"kind": "toontown.c44a.e2e", "seconds": round(time.monotonic() - started, 1), "failures": run.failures}
    run.save("manifest.json", manifest)
    print(json.dumps(manifest, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
