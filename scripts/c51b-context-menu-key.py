"""M5 C5.1b round 2, ruling E-52: an agent's Shift+F10 obeys the rules of every other key, on a real MCP stream.

Usage: python scripts/c51b-context-menu-key.py <toontown binary> <out dir>

The `key` tool's `shift+F10` (11 §7.6; E-48) opens a window control's context menu. It takes the path of every key: the
`input` scope, the text session, the coexistence checks (11 lines 237-241: a dialog the person opened answers
`ui.modalOpen`; a text field being edited keeps its Shift+F10, the shared text-field rule: `ui.busy`), and then the
window opens the menu and answers what it did (C51B-R2S-02): `handled: true` with the menu, `handled: false` when the
control offers none, `view.notFound` for no such control, `arg.invalid` for a `view` that is not a string. The window is
read through the webview's debugging port and driven with real OS input where a person's action is part of the case (a
dialog opened from the menu bar, the rename editor opened by a double click), as `c43-d39-bindings.py` does.

Rows: the passing case (the cell context of the tracks), a control without a menu (the toolbar's Add), an unknown
control, a stale row id (an element deleted), a non-string `view`, a dialog open (nothing focused or opened), the rename
editor open (its text kept), and whose action an item is: the agent runs an opened item's reference (`run_command`,
the agent's origin and scopes: an agent step), a person's real click on an item of the same menu is a user step.

Output: `rows.json`, `transcript.json`, `manifest.json` (kind `toontown.c51b.context-menu-key`). Exit 0 when every row
holds. Windows workstation (real input).
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
    spec.loader.exec_module(mod)
    return mod


d39 = _load("c43d39", "c43-d39-bindings.py")
e2e = d39.e2e


async def until(pred, bound: float, step: float = 0.1):
    """Waits for a state (D-45); the bound only guards a hang. Returns the seconds it took, or None."""
    start = time.monotonic()
    while time.monotonic() - start < bound:
        if pred():
            return round(time.monotonic() - start, 2)
        await asyncio.sleep(step)
    return None


async def check(app: str, run, out: Path, work: Path) -> list:
    rows: list = []

    def row(name: str, ok: bool, detail) -> None:
        rows.append({"name": name, "ok": bool(ok), "detail": detail})
        run.check(bool(ok), f"E-52 {name}", detail)
        run.save("rows.json", rows)

    user = work / "user"
    (user / "Settings").mkdir(parents=True)
    seed = {"agent.policy.defaults": {"policy": "autonomous", "maxJobs": 2, "maxBatchSteps": 1000,
                                      "maxInlineImagePx": 2048, "maxInlineBytes": 52428800, "askInApp": True,
                                      "letAgentMoveView": True, "highlightAgentChanges": True}}
    (user / "Settings" / "preferences.json").write_text(json.dumps(
        {"format": "toontown-settings", "kind": "preferences", "formatVersion": "1.0", "data": seed}), encoding="utf-8")
    os.environ["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = f"--remote-debugging-port={d39.CDP_PORT}"
    os.environ["WEBVIEW2_USER_DATA_FOLDER"] = str(work / "webview2")
    discovery = work / "discovery.json"
    proc = e2e.launch(app, user, discovery)
    try:
        d = e2e.wait_discovery(discovery, proc)
        async with contextlib.AsyncExitStack() as stack:
            client = await stack.enter_async_context(
                e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {d['token']}"},
                                       timeout=e2e.httpx2.Timeout(60.0, read=300.0)))
            streams = await stack.enter_async_context(e2e.streamable_http_client(d["endpoint"], http_client=client))
            from mcp import types

            async def roots(_ctx):
                return types.ListRootsResult(roots=[types.Root(uri=work.resolve().as_uri(), name="work")])
            s = await stack.enter_async_context(e2e.ClientSession(streams[0], streams[1], list_roots_callback=roots))
            await s.initialize()
            await e2e.gui_tree(s, run, lambda r: e2e.by_id(r, "startScreen") is not None, "the GUI is up")
            t = d39.Table(s, run, out, work)
            t.cdp = d39.Cdp(d39.CDP_PORT)
            cdp = t.cdp

            def js(expr: str):
                return cdp.js(expr)

            def present(ident: str) -> bool:
                return bool(js("!!document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']')" % json.dumps(ident)))

            def focused() -> str | None:
                return js("document.activeElement?.dataset?.ttId ?? null")

            def menu_open() -> str | None:
                return js("document.querySelector('[data-tt-role=\"menu\"]')?.dataset.ttId ?? null")

            async def press(view=None):
                args = {"keys": "shift+F10"}
                if view is not None:
                    args["view"] = view
                return await e2e.call(s, "key", args)

            def close_menu():
                if menu_open():
                    d39.front()
                    d39.key(0x1B)

            # A project with a drawing exposed on 1-12 and a peg, in a shown Timeline.
            await t.cmd("project.create", {"name": "Context menu key", "onUnsaved": "fail"})
            a = (await t.cmd("element.create", {"type": "drawing", "name": "TlA"}))["elementIds"][0]
            await t.cmd("element.drawing.create", {"elementId": a, "name": "1", "exposeFrom": 1, "exposeTo": 12})
            await t.cmd("window.panel.set", {"panelId": "timeline", "visible": True, "focus": True})
            await t.cmd("xsheet.currentCell.set", {"elementId": a, "frame": 3})
            await until(lambda: present(f"timeline.row.{a}"), 15.0)
            await asyncio.sleep(0.5)

            # 1. The passing case: the tracks' cell context at the current cell.
            env = await press("timeline.tracks")
            res = env.get("result") or {}
            row("1. the passing case: the cell context opens and the answer names it",
                env.get("status") == "ok" and res.get("handled") is True and res.get("menuId") == "timeline.cellMenu"
                and menu_open() == "timeline.cellMenu", {"answer": env, "menu": menu_open()})

            # 8. Whose action an item is: the agent runs an opened item's reference itself (its own origin and scopes),
            # and a person's real click on an item is the person's.
            node = e2e.by_id(await e2e.tree(s), "timeline.cellMenu.timeline.cell.addKey") or {}
            ref = node.get("command") or {}
            before = await t.position()
            ran = await e2e.run_cmd(s, ref.get("commandId") or "", ref.get("params") or {})
            agent_steps = await t.steps_since(before)
            close_menu()
            await asyncio.sleep(0.3)
            env = await press("timeline.tracks")
            await until(lambda: present("timeline.cellMenu.timeline.cell.removeKey"), 5.0)
            before = await t.position()
            how = t.click("timeline.cellMenu.timeline.cell.removeKey")
            await asyncio.sleep(1.0)
            person_steps = await t.steps_since(before)
            row("8. an item is the action of whoever activates it: the agent's run of its reference is an agent step, "
                "a person's click a user step",
                ran.get("status") == "ok" and [src for _, src in agent_steps] == ["agent"]
                and [src for _, src in person_steps] == ["user"],
                {"reference": ref, "agentSteps": agent_steps, "personClick": how, "personSteps": person_steps})
            close_menu()

            # 2. A control without a context menu: the toolbar's Add.
            await asyncio.sleep(0.3)
            focus_before = focused()
            env = await press("timeline.add")
            res = env.get("result") or {}
            row("2. a control without a context menu: handled false, nothing opened, the focus back",
                env.get("status") == "ok" and res.get("handled") is False and res.get("reason") == "noContextMenu"
                and menu_open() is None and focused() == focus_before,
                {"answer": env, "focusBefore": focus_before, "focusAfter": focused()})

            # 3. An unknown control.
            env = await press("timeline.noSuchControl")
            row("3. an unknown control: view.notFound, nothing opened",
                (env.get("error") or {}).get("code") == "view.notFound" and menu_open() is None, {"answer": env})

            # 4. A stale row id: an element deleted.
            gone = (await t.cmd("element.create", {"type": "drawing", "name": "Gone"}))["elementIds"][0]
            await t.cmd("element.delete", {"elementIds": [gone]})
            await until(lambda: not present(f"timeline.row.{gone}"), 10.0)
            env = await press(f"timeline.row.{gone}")
            row("4. a stale row id: view.notFound, nothing opened",
                (env.get("error") or {}).get("code") == "view.notFound" and menu_open() is None, {"answer": env})

            # 5. A view that is not a string.
            env = await press(42)
            row("5. a non-string view: arg.invalid, nothing opened",
                (env.get("error") or {}).get("code") == "arg.invalid" and menu_open() is None, {"answer": env})

            # 6. A dialog the person opened (Element › New Elements, real input): ui.modalOpen, no focus moved.
            d39.choose_menu(["Element", "New Elements"])
            opened = await until(lambda: present("timelineNewElements"), 10.0)
            await asyncio.sleep(0.5)
            focus_before = focused()
            env = await press("timeline.tracks")
            row("6. a dialog open: ui.modalOpen, no menu, the focus unmoved",
                opened is not None and (env.get("error") or {}).get("code") == "ui.modalOpen" and menu_open() is None
                and focused() == focus_before, {"answer": env, "focusBefore": focus_before, "focusAfter": focused()})
            d39.front()
            d39.key(0x1B)
            await until(lambda: not present("timelineNewElements"), 10.0)

            # 7. The rename editor open (a real double click on the row's name): ui.busy, the text kept, no menu.
            r = t.rect(f"timeline.row.{a}.label")
            h = d39.front()
            origin = d39.wintypes.POINT(0, 0)
            d39.u32.ClientToScreen(h, d39.ctypes.byref(origin))
            d39.u32.SetCursorPos(int(origin.x + r["x"] * r["dpr"]), int(origin.y + r["y"] * r["dpr"]))
            time.sleep(0.15)
            for _ in range(2):
                d39.e6._send_button(True)
                time.sleep(0.02)
                d39.e6._send_button(False)
                time.sleep(0.05)
            editor = f"timeline.row.{a}.name"
            editing = await until(lambda: focused() == editor, 5.0)
            text_before = js("document.activeElement?.value ?? null")
            env = await press("timeline.tracks")
            row("7. the rename editor open: ui.busy (the field keeps its Shift+F10), its text kept, no menu",
                editing is not None and (env.get("error") or {}).get("code") == "ui.busy" and menu_open() is None
                and focused() == editor and js("document.activeElement?.value ?? null") == text_before,
                {"answer": env, "editing": editing, "focus": focused()})
            d39.key(0x1B)
    finally:
        e2e.stop(proc)
    return rows


def main() -> int:
    d39.e6.dpi_aware()
    ap = argparse.ArgumentParser()
    ap.add_argument("app")
    ap.add_argument("out")
    a = ap.parse_args()
    if sys.platform != "win32":
        print("The context-menu key check drives the real window through Windows input; run it on Windows.")
        return 0
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    work = Path(tempfile.mkdtemp(prefix="tt-c51b-context-key-"))
    started = time.monotonic()
    rows: list = []
    try:
        rows = asyncio.run(check(a.app, run, out, work))
    except Exception as ex:  # noqa: BLE001 - reported as a failure with its text
        import traceback
        traceback.print_exception(ex)
        run.check(False, "the check ran", repr(ex))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    manifest = {"kind": "toontown.c51b.context-menu-key", "seconds": round(time.monotonic() - started, 1),
                "rows": len(rows), "failed": [r.get("name") for r in rows if not r.get("ok")], "failures": run.failures}
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps(manifest, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
