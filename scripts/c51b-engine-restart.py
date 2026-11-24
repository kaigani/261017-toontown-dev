"""M5 C5.1b, ruling E-29 item 3: the watchdog's live restart check, through the real window (Windows workstation).

Usage: python scripts/c51b-engine-restart.py <toontown built with --features test-hooks> <out dir>

The engine is stopped in the running application by the dev-only trigger of `desktop/src-tauri/src/degrade_hook.rs`
(`TT_TEST_DEGRADE_TRIGGER`; a `test-hooks` build only): the next document commit unwinds at the commit boundary and the
engine marks itself degraded. The check, with real OS input on the window (the page read through the webview's
debugging port, the kernel through the attached MCP interface, as `c43-d39-bindings.py`):

1. a project with an element is autosaved (the autosave interval set to its minimum, one minute);
2. the trigger file is made and an agent's change answers `internal` (reason `engineStopped`, the hint naming
   `project.recovery.list`);
3. the window marks the engine not responding at once: the banner shows within one heartbeat period and a margin
   (`watchdog::PERIOD`, 5 s, plus 2 s), not after the three missed heartbeats a busy engine needs (about 15 s);
4. the restart question (D-44's `engine.restartConfirm`, both choices): a real click on Restart the engine, then Cancel
   leaves the engine stopped and the banner up; again, then Restart restarts it (the banner goes, the engine answers);
5. Recover: the start screen offers the autosave and a real click on its Recover opens it: the element made before the
   autosave is there, the change the engine died on is not.

Ruling E-50 item 3 (steps 3c, 3d, 6): with the banner up, a real drag of a peg's bar in the Timeline and a real click on
a Timeline context-menu item change nothing, each shows a refusal on the status line, and neither hangs the window (the
page answers at once afterwards); after Restart and Recover the Timeline shows the recovered rows and a real drag of the
bar commits.

Output: `rows.json`, `transcript.json`, `manifest.json` (kind `toontown.c51b.engine-restart`). Exit 0 when every step
holds. Also a step of the manual script (reviews/manual/M5/).
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
WATCHDOG_PERIOD_S = 5.0
MARGIN_S = 2.0


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


d39 = _load("c43d39", "c43-d39-bindings.py")
e2e = d39.e2e


def present(cdp, ident: str) -> bool:
    return bool(cdp.js("!!document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']')" % json.dumps(ident)))


async def until(pred, bound: float, step: float = 0.1):
    """Waits for a state (D-45); the bound only guards a hang. Returns the seconds it took, or None."""
    start = time.monotonic()
    while time.monotonic() - start < bound:
        if pred():
            return round(time.monotonic() - start, 2)
        await asyncio.sleep(step)
    return None


def client_of(cdp, ident: str, x: float, y_of: str | None = None) -> tuple[float, float] | None:
    """A point in the window's client area (device pixels): `x` CSS pixels in from the left of `ident`, at the vertical
    centre of `y_of` (default `ident`)."""
    return cdp.js(
        "(() => { const box = id => document.querySelector('[data-tt-id=' + JSON.stringify(id) + ']')"
        "?.getBoundingClientRect(); const a = box(%s), b = box(%s); if (!a || !b) return null; const d = devicePixelRatio;"
        " return [(a.x + %s) * d, (b.y + b.height / 2) * d]; })()"
        % (json.dumps(ident), json.dumps(y_of or ident), json.dumps(x)))


def status_line(cdp) -> str:
    return cdp.js("document.querySelector('[data-tt-id=\"status.line\"]')?.textContent ?? ''") or ""


async def check(app: str, run, out: Path, work: Path) -> list:
    rows: list = []

    def row(name: str, ok: bool, detail) -> None:
        rows.append({"name": name, "ok": bool(ok), "detail": detail})
        run.check(bool(ok), f"E-29 item 3 {name}", detail)
        run.save("rows.json", rows)

    user = work / "user"
    (user / "Settings").mkdir(parents=True)
    seed = {"agent.policy.defaults": {"policy": "autonomous", "maxJobs": 2, "maxBatchSteps": 1000, "maxInlineImagePx": 2048,
                                      "maxInlineBytes": 52428800, "askInApp": True, "letAgentMoveView": True,
                                      "highlightAgentChanges": True},
            "files.autosave.enabled": True, "files.autosave.intervalMinutes": 1}
    (user / "Settings" / "preferences.json").write_text(json.dumps(
        {"format": "toontown-settings", "kind": "preferences", "formatVersion": "1.0", "data": seed}), encoding="utf-8")
    trigger = work / "degrade.trigger"
    os.environ["TT_TEST_DEGRADE_TRIGGER"] = str(trigger)
    os.environ["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = f"--remote-debugging-port={d39.CDP_PORT}"
    os.environ["WEBVIEW2_USER_DATA_FOLDER"] = str(work / "webview2")
    discovery = work / "discovery.json"
    proc = e2e.launch(app, user, discovery)

    @contextlib.asynccontextmanager
    async def session(d):
        """An attached MCP session on the endpoint the discovery file names (the host serves it anew with a new
        engine)."""
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
            yield s

    try:
        d = e2e.wait_discovery(discovery, proc)
        t = None
        cdp = None
        asked_again = None
        try:
            async with session(d) as s:
                await e2e.gui_tree(s, run, lambda r: e2e.by_id(r, "startScreen") is not None, "the GUI is up")
                t = d39.Table(s, run, out, work)
                # 1. A project with an element, autosaved.
                await t.cmd("project.create", {"name": "Restart", "onUnsaved": "fail"})
                await t.cmd("project.saveAs", {"path": str(work / "Restart.animpkg"), "onExists": "overwrite"})
                await t.cmd("element.create", {"type": "drawing", "name": "Before"})
                # E-50 item 3: a peg with a bar, in a shown Timeline (made before the autosave: Recover brings it back).
                peg = (await t.cmd("element.create", {"type": "peg", "name": "BarPeg"}))["elementIds"][0]
                await t.cmd("element.timing.set", {"elementIds": [peg], "start": 1, "duration": 24})
                await t.cmd("window.panel.set", {"panelId": "timeline", "visible": True, "focus": True})
                await t.cmd("timeline.view.set", {"zoomLevel": 6, "firstFrame": 1})
                cell = d39.TL_LAYOUT["zoomLevels"][6]["cellWidth"]
                saved = await until(lambda: any((user / "Recovery").rglob("*.json")) if (user / "Recovery").exists() else False,
                                    150.0, 1.0)
                row("1. an autosave of the modified project", saved is not None, {"seconds": saved})
                # The page's debugging connection is opened now: the script's socket gives up after 60 s without a message,
                # and the autosave wait is longer.
                t.cdp = d39.Cdp(d39.CDP_PORT)
                cdp = t.cdp

                # 2. The engine stops at the next commit.
                trigger.write_text("stop", encoding="utf-8")
                died = await e2e.run_cmd(s, "element.create", {"type": "drawing", "name": "Dies"})
                stopped_at = time.monotonic()
                later = await e2e.run_cmd(s, "doc.state.get", {})
                err = later.get("error") or {}
                row("2. the engine stops: internal, reason engineStopped, the hint names project.recovery.list",
                    (died.get("error") or {}).get("code") == "internal" and err.get("code") == "internal"
                    and (err.get("details") or {}).get("reason") == "engineStopped"
                    and "project.recovery.list" in (err.get("hint") or "") and not trigger.exists(),
                    {"died": died.get("error"), "later": err})

                # 3. The banner at once: within one heartbeat period and a margin.
                shown = await until(lambda: present(cdp, "engineNotResponding"), 30.0)
                since = None if shown is None else round(time.monotonic() - stopped_at, 2)
                row(f"3. the window marks the engine not responding at once (within {WATCHDOG_PERIOD_S + MARGIN_S} s)",
                    since is not None and since <= WATCHDOG_PERIOD_S + MARGIN_S, {"secondsAfterTheStop": since})
                # Nothing covers the banner's Restart (the reads that fail meanwhile once did): a real click reaches it.
                await asyncio.sleep(1.0)
                box = t.rect("engine.restart")
                row("3b. the banner's Restart the engine is in view and reachable by a click (nothing covers it)",
                    bool(box) and not box.get("hidden") and box.get("reachable") is True, {"rect": box})

                # 3c (E-50 item 3). A real drag of the peg's bar while the engine is stopped: nothing changes, the status
                # line shows a refusal, and the page still answers.
                before_line = status_line(cdp)
                pts = [client_of(cdp, "timeline.tracks", (f - 0.5) * cell, f"timeline.row.{peg}") for f in (10, 11, 14)]
                times: dict = {}
                if all(pts):
                    d39.front()
                    d39.e6.os_drag(proc.pid, pts, 0.3, 0.05, times)
                refused = await until(lambda: status_line(cdp) not in ("", before_line), 10.0)
                t0 = time.monotonic()
                answered = cdp.js("1 + 1") == 2
                row("3c. a real bar drag in the Timeline while the engine is stopped: nothing changes, a refusal shows, "
                    "the window answers", all(pts) and refused is not None and answered and time.monotonic() - t0 < 2.0
                    and present(cdp, "engineNotResponding"),
                    {"points": pts, "drag": times, "statusLine": status_line(cdp), "refusalAfter": refused,
                     "pageAnswers": answered})

                # 3d (E-50 item 3). A real click on a Timeline context-menu item (the row's Lock) while the engine is
                # stopped: nothing changes, a refusal shows (on the item or the status line), the window answers.
                before_line = status_line(cdp)
                at = client_of(cdp, f"timeline.row.{peg}", 60)
                item = "timeline.rowMenu.timeline.row.lock"
                if at:
                    h = d39.front()
                    origin = d39.wintypes.POINT(0, 0)
                    d39.u32.ClientToScreen(h, d39.ctypes.byref(origin))
                    d39.right_click_screen(origin.x + at[0], origin.y + at[1])
                opened = await until(lambda: present(cdp, item), 5.0)
                facts = cdp.js("(() => { const e = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']');"
                               " return e ? {disabled: e.getAttribute('aria-disabled'), title: e.title,"
                               " reason: e.querySelector('.context-menu-reason')?.textContent ?? null} : null; })()"
                               % json.dumps(item))
                if opened is not None:
                    t.click(item)
                refused = await until(lambda: status_line(cdp) not in ("", before_line), 5.0)
                answered = cdp.js("1 + 1") == 2
                shown_reason = bool(facts and facts.get("disabled") == "true" and (facts.get("reason") or facts.get("title")))
                row("3d. a real click on a Timeline context-menu item while the engine is stopped: nothing changes, a "
                    "refusal shows, the window answers",
                    opened is not None and (refused is not None or shown_reason) and answered
                    and present(cdp, "engineNotResponding"),
                    {"opened": opened, "item": facts, "statusLine": status_line(cdp), "refusalAfter": refused,
                     "pageAnswers": answered})
                d39.key(0x1B)
                await asyncio.sleep(0.3)

                # 4. The restart question: Cancel, then Restart.
                async def ask():
                    t.click("engine.restart")
                    return await until(lambda: present(cdp, "message.engine.restartConfirm"), 10.0)
                asked = await ask()
                t.click("message.cancel")
                await asyncio.sleep(1.0)
                still = await e2e.run_cmd(s, "doc.state.get", {})
                row("4a. the restart question, Cancel: the engine stays stopped and the banner stays",
                    asked is not None and present(cdp, "engineNotResponding")
                    and (still.get("error") or {}).get("code") == "internal",
                    {"asked": asked, "banner": present(cdp, "engineNotResponding"), "engine": still.get("error")})
                asked_again = await ask()
                t.click("message.restart")
        except (Exception, BaseExceptionGroup) as ex:  # noqa: BLE001 - the old engine's session ends with the restart
            if t is None or cdp is None:
                raise
            print(f"note: the first session ended with the restart ({type(ex).__name__})", flush=True)
        gone = await until(lambda: not present(cdp, "engineNotResponding"), 30.0)
        # The host serves attached mode anew with the new engine and rewrites the discovery file.
        d2 = None
        for _ in range(150):
            with contextlib.suppress(Exception):
                d2 = json.loads(discovery.read_text(encoding="utf-8"))
                if d2.get("endpoint") and (d2.get("endpoint"), d2.get("token")) != (d.get("endpoint"), d.get("token")):
                    break
            d2 = None
            await asyncio.sleep(0.2)
        async with session(d2 or d) as s:
            alive = None
            for _ in range(60):
                alive = await e2e.run_cmd(s, "app.state.get", {})
                if alive.get("status") == "ok":
                    break
                await asyncio.sleep(0.5)
            row("4b. the restart question, Restart: the banner goes and the new engine answers",
                asked_again is not None and gone is not None and (alive or {}).get("status") == "ok",
                {"asked": asked_again, "bannerGoneAfter": gone, "newEndpoint": d2 is not None, "engine": (alive or {}).get("status"),
                 "error": (alive or {}).get("error")})
            # 5. Recover the autosave from the start screen.
            entry = None
            for _ in range(100):
                entry = cdp.js("document.querySelector('[data-tt-id^=\"start.recover.\"]')?.dataset.ttId ?? null")
                if entry:
                    break
                await asyncio.sleep(0.2)
            if entry:
                t.click(entry)
            names = []
            for _ in range(100):
                listed = (await e2e.run_cmd(s, "element.list", {})).get("result") or {}
                names = sorted(e.get("name") for e in listed.get("elements") or [])
                if "Before" in names:
                    break
                await asyncio.sleep(0.2)
            row("5. Recover: the start screen offers the autosave; it opens with the work before it, not the change the "
                "engine died on", entry is not None and "Before" in names and "Dies" not in names,
                {"entry": entry, "elements": names})

            # 6 (E-50 item 3). The Timeline shows the recovered rows, and a real drag of the peg's bar commits.
            listed = (await e2e.run_cmd(s, "element.list", {})).get("result") or {}
            ids = {e.get("name"): e.get("id") for e in listed.get("elements") or []}
            peg2 = ids.get("BarPeg")
            await e2e.run_cmd(s, "window.panel.set", {"panelId": "timeline", "visible": True, "focus": True})
            await e2e.run_cmd(s, "timeline.view.set", {"zoomLevel": 6, "firstFrame": 1})
            rows_shown = await until(lambda: bool(peg2) and present(cdp, f"timeline.row.{peg2}")
                                     and present(cdp, f"timeline.row.{ids.get('Before')}"), 15.0)
            await asyncio.sleep(0.5)

            async def start_of():
                got = (await e2e.run_cmd(s, "element.get", {"elementId": peg2})).get("result") or {}
                return got.get("start")
            start_before = await start_of() if peg2 else None
            pts = [client_of(cdp, "timeline.tracks", (f - 0.5) * cell, f"timeline.row.{peg2}") for f in (10, 11, 14)]
            times = {}
            if rows_shown is not None and all(pts):
                d39.front()
                d39.e6.os_drag(proc.pid, pts, 0.3, 0.05, times)
            moved = None
            for _ in range(50):
                moved = await start_of() if peg2 else None
                if moved is not None and moved != start_before:
                    break
                await asyncio.sleep(0.2)
            row("6. after Restart and Recover the Timeline shows the recovered rows and a real bar drag commits",
                rows_shown is not None and start_before == 1 and moved == 5,
                {"rowsShownAfter": rows_shown, "startBefore": start_before, "startAfter": moved, "drag": times})
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
        print("The live restart check drives the real window through Windows input; run it on the Windows workstation.")
        return 0
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    work = Path(tempfile.mkdtemp(prefix="tt-c51b-restart-"))
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
    manifest = {"kind": "toontown.c51b.engine-restart", "seconds": round(time.monotonic() - started, 1), "rows": len(rows),
                "failed": [r.get("name") for r in rows if not r.get("ok")], "failures": run.failures}
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps(manifest, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
