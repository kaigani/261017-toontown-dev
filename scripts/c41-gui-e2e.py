"""M4 C4.1: drives the real desktop application on a CI runner (or a workstation) through attached mode.

Usage: python scripts/c41-gui-e2e.py <toontown binary> <out dir>

What it does (reviews/M4-C4.1-plan.md §8.3):
1. launches the product build with a fresh user folder (`--user-dir`), attached mode on a free loopback port
   (`--mcp-http 127.0.0.1:0`) and a discovery file (`--mcp-discovery`); the user folder is seeded with an agent policy
   default of `autonomous` (a user preference agents cannot set themselves), so the session may also change
   preferences and key bindings;
2. connects the official Python SDK client over Streamable HTTP with the token of the discovery file;
3. drives the shell with the spec's tools (`inspect_ui`, `key`, `run_command`, `get_state`) and checks after each step
   that the rendered GUI (the UI tree the front end reported) reflects the change;
4. checks user priority end to end: an open preferences dialog makes an agent change wait and fail with `ui.busy`;
5. restarts the application over the same user folder and reads the persisted preferences, key binding, workspace and
   window geometry back; then sets a corrupt settings file aside;
6. writes a JSON transcript, the UI trees and best-effort OS screen captures under <out dir>, with a manifest.

Exit status 0 when every check passed, 1 otherwise (each failed check is listed).
"""

from __future__ import annotations

import asyncio
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import httpx2
from mcp import ClientSession
from mcp.client.streamable_http import streamable_http_client

APP_START_BOUND_S = 120
APP_PID: dict = {"pid": None}  # first webview start on a cold CI runner; generous on purpose
TREE_BOUND_S = 60
GEOMETRY = {"x": 40, "y": 30, "width": 860, "height": 600}


class Run:
    def __init__(self, out: Path):
        self.out = out
        self.transcript: list[dict] = []
        self.failures: list[str] = []
        self.shots: list[str] = []
        self.geometry: dict = {}

    def check(self, ok: bool, what: str, detail: object = None) -> bool:
        self.transcript.append({"check": what, "ok": bool(ok), "detail": detail})
        if not ok:
            self.failures.append(f"{what}: {json.dumps(detail, default=str)[:600]}")
        print(("ok   " if ok else "FAIL ") + what, flush=True)
        return ok

    def save(self, name: str, value: object) -> None:
        (self.out / name).write_text(json.dumps(value, indent=2, default=str), encoding="utf-8")

    def shot(self, name: str, pid: int | None = None) -> None:
        """Best-effort capture of the application window (not gated: runners may refuse screen capture). On Windows
        only the visible frame of the window of `pid` is captured (DWM extended frame bounds, without the invisible
        resize border), so no other window of a workstation appears in the receipts; on macOS
        (CI runners only) the screen."""
        path = self.out / f"{name}.png"
        try:
            if sys.platform == "win32" and pid is not None:
                ps = (
                    "Add-Type -AssemblyName System.Drawing;"
                    "Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;"
                    "public struct R{public int L,T,Ri,B;}"
                    "public static class W{[DllImport(\"user32.dll\")]public static extern bool GetWindowRect(IntPtr h,out R r);"
                    "[DllImport(\"user32.dll\")]public static extern bool SetForegroundWindow(IntPtr h);"
                    "[DllImport(\"dwmapi.dll\")]public static extern int DwmGetWindowAttribute(IntPtr h,int a,out R r,int n);}';"
                    f"$p=Get-Process -Id {pid};$h=$p.MainWindowHandle;[W]::SetForegroundWindow($h)|Out-Null;Start-Sleep -Milliseconds 300;"
                    "$r=New-Object R;if([W]::DwmGetWindowAttribute($h,9,[ref]$r,16) -ne 0){[W]::GetWindowRect($h,[ref]$r)|Out-Null};$w=$r.Ri-$r.L;$t=$r.B-$r.T;"
                    "$i=New-Object System.Drawing.Bitmap $w,$t;$g=[System.Drawing.Graphics]::FromImage($i);"
                    "$g.CopyFromScreen($r.L,$r.T,0,0,$i.Size);"
                    f"$i.Save('{path}');"
                )
                subprocess.run(["powershell", "-NoProfile", "-Command", ps], timeout=30, check=False, capture_output=True)
            elif sys.platform == "darwin" and os.environ.get("CI"):
                subprocess.run(["screencapture", "-x", str(path)], timeout=30, check=False, capture_output=True)
            if path.exists() and path.stat().st_size > 0:
                self.shots.append(path.name)
                self.transcript.append({"screenshot": path.name})
            else:
                self.transcript.append({"screenshot": path.name, "captured": False})
        except Exception as e:  # noqa: BLE001 - evidence only
            self.transcript.append({"screenshot": path.name, "captured": False, "error": str(e)})


def os_escape(pid: int | None) -> dict:
    """Presses Escape in the application window through the operating system, as a user would (no command exists to
    close a user dialog). Windows: SendKeys to the foreground window of `pid`; macOS: System Events."""
    if pid is None:
        return {"ok": False, "error": "no pid"}
    if sys.platform == "win32":
        ps = (
            "Add-Type -AssemblyName System.Windows.Forms;"
            "Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;"
            "public static class F{[DllImport(\"user32.dll\")]public static extern bool SetForegroundWindow(IntPtr h);}';"
            f"$p=Get-Process -Id {pid};[F]::SetForegroundWindow($p.MainWindowHandle)|Out-Null;Start-Sleep -Milliseconds 300;"
            "[System.Windows.Forms.SendKeys]::SendWait('{ESC}')"
        )
        r = subprocess.run(["powershell", "-NoProfile", "-Command", ps], timeout=30, check=False, capture_output=True, text=True)
    elif sys.platform == "darwin":
        script = (
            f'tell application "System Events" to set frontmost of (first process whose unix id is {pid}) to true\n'
            "delay 0.3\n"
            'tell application "System Events" to key code 53'
        )
        r = subprocess.run(["osascript", "-e", script], timeout=30, check=False, capture_output=True, text=True)
    else:
        return {"ok": False, "error": f"unsupported platform {sys.platform}"}
    return {"ok": r.returncode == 0, "stderr": r.stderr[-400:]}


async def last_activity(session: ClientSession, tool: str) -> dict:
    """The newest activity-log entry of `tool` (the command a `key` call resolved to is recorded there)."""
    entries = (await run_cmd(session, "agent.activity.list", {"limit": 50})).get("result") or []
    if isinstance(entries, dict):
        entries = entries.get("entries") or []
    mine = [e for e in entries if e.get("tool") == tool]
    return max(mine, key=lambda e: e.get("seq", 0)) if mine else {}


def launch(app: str, user: Path, discovery: Path) -> subprocess.Popen:
    if discovery.exists():
        discovery.unlink()
    log = open(discovery.with_suffix(".log"), "ab")
    proc = subprocess.Popen(
        [app, "--user-dir", str(user), "--mcp-http", "127.0.0.1:0", "--mcp-discovery", str(discovery)],
        stdout=log,
        stderr=subprocess.STDOUT,
    )
    APP_PID["pid"] = proc.pid
    return proc


def wait_discovery(discovery: Path, proc: subprocess.Popen) -> dict:
    start = time.monotonic()
    while time.monotonic() - start < APP_START_BOUND_S:
        if proc.poll() is not None:
            raise RuntimeError(f"the application exited with {proc.returncode}")
        if discovery.exists():
            try:
                d = json.loads(discovery.read_text(encoding="utf-8"))
                if d.get("endpoint") and d.get("token"):
                    return d
            except (OSError, json.JSONDecodeError):
                pass
        time.sleep(0.25)
    raise RuntimeError("no discovery file within the bound")


def stop(proc: subprocess.Popen) -> None:
    proc.terminate()
    try:
        proc.wait(timeout=20)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=20)


def tool_result(r) -> dict:
    """The structured content of a tool result (camelCase in SDK 1.x, snake_case in 2.x)."""
    return getattr(r, "structuredContent", None) or getattr(r, "structured_content", None) or {}


async def call(session: ClientSession, tool: str, args: dict) -> dict:
    r = await session.call_tool(tool, args)
    return tool_result(r)


async def run_cmd(session: ClientSession, cid: str, params: dict | None = None) -> dict:
    return await call(session, "run_command", {"id": cid, "params": params or {}})


async def tree(session: ClientSession, target: str = "window") -> dict:
    return await call(session, "inspect_ui", {"target": target})


def find(node: dict, pred) -> dict | None:
    if pred(node):
        return node
    for c in node.get("children", []) or []:
        f = find(c, pred)
        if f:
            return f
    return None


def by_id(node: dict, ident: str) -> dict | None:
    return find(node, lambda n: n.get("id") == ident)


async def gui_tree(session: ClientSession, run: Run, pred, what: str) -> dict:
    """The rendered tree once `pred(root)` holds (the GUI catches up asynchronously), bounded."""
    start = time.monotonic()
    last: dict = {}
    while time.monotonic() - start < TREE_BOUND_S:
        env = await tree(session)
        last = env.get("result") or {}
        root = last.get("root") or {}
        if last.get("source") == "gui" and pred(root):
            return root
        await asyncio.sleep(0.25)
    run.check(False, what, {"last": last})
    return last.get("root") or {}


async def first_session(url: str, token: str, run: Run) -> None:
    client = httpx2.AsyncClient(headers={"Authorization": f"Bearer {token}"}, timeout=httpx2.Timeout(60.0, read=300.0))
    async with client:
        async with streamable_http_client(url, http_client=client) as streams:
            read, write = streams[0], streams[1]
            async with ClientSession(read, write) as session:
                init = await session.initialize()
                info = getattr(init, "serverInfo", None) or getattr(init, "server_info", None)
                run.check(info is not None, "initialize over Streamable HTTP", info.model_dump() if info else None)

                # The front end reports its UI tree: panels, start screen, status bar, main toolbar.
                root = await gui_tree(session, run, lambda r: by_id(r, "startScreen") is not None, "the GUI shows the start screen")
                run.save("tree-01-start.json", root)
                run.shot("01-start-screen", APP_PID["pid"])
                run.check(by_id(root, "startScreen") is not None, "start screen in the rendered tree")
                run.check(by_id(root, "statusBar") is not None, "status bar in the rendered tree")
                run.check(by_id(root, "mainToolbar") is not None, "main toolbar in the rendered tree")
                panels = (await run_cmd(session, "window.panel.list")).get("result") or []
                shown = [p["panelId"] for p in panels if p.get("visible")]
                missing = [p for p in shown if by_id(root, p) is None and by_id(root, f"tab.{p}") is None]
                run.check(not missing, "every visible panel is rendered", {"visible": shown, "missing": missing})

                # Agent commands change the GUI: the History panel appears, the status bar hides and returns.
                env = await run_cmd(session, "window.panel.set", {"panelId": "history", "visible": True})
                run.check(env.get("status") == "ok", "window.panel.set history", env.get("error"))
                root = await gui_tree(session, run, lambda r: by_id(r, "tab.history") is not None, "the History tab appears")
                env = await run_cmd(session, "ui.statusBar.set", {"visible": False})
                root = await gui_tree(session, run, lambda r: by_id(r, "statusBar") is None, "the status bar hides")
                env = await run_cmd(session, "ui.statusBar.set", {"visible": True})
                root = await gui_tree(session, run, lambda r: by_id(r, "statusBar") is not None, "the status bar returns")

                # A project: the start screen closes, the window title names the project.
                env = await run_cmd(session, "project.create", {"name": "Bounce", "onUnsaved": "fail"})
                run.check(env.get("status") == "ok", "project.create", env.get("error"))
                root = await gui_tree(session, run, lambda r: by_id(r, "startScreen") is None, "the start screen closes")
                run.check("Bounce" in str(root.get("label", "")), "the window title names the project", root.get("label"))
                run.save("tree-02-project.json", root)
                run.shot("02-project", APP_PID["pid"])

                # Workspaces: selecting Drawing makes the Drawing view central.
                env = await run_cmd(session, "workspace.select", {"name": "Drawing"})
                run.check(env.get("status") == "ok", "workspace.select Drawing", env.get("error"))
                root = await gui_tree(session, run, lambda r: by_id(r, "tab.drawingView") is not None, "the Drawing view is shown")
                state = (await call(session, "get_state", {})).get("result") or {}
                run.check(state.get("mode") == "drawing", "drawing mode follows the active view", state.get("activeView"))

                # Preferences, a key binding, a user workspace and the window geometry (persisted, read back later).
                env = await run_cmd(session, "prefs.set", {"values": {"interface.theme": "dark", "files.backups.count": 7}})
                run.check(env.get("status") == "ok", "prefs.set", env.get("error"))
                env = await run_cmd(session, "keymap.set", {"commandId": "project.save", "keys": "F2"})
                run.check(env.get("status") == "ok", "keymap.set F2", env.get("error"))
                # The key tool resolves through the kernel's keymap and runs the bound command: F2 runs project.save,
                # which refuses an unsaved project without a path (the typed refusal is its effect).
                env = await call(session, "key", {"keys": "F2"})
                act = await last_activity(session, "key")
                run.check(act.get("commandId") == "project.save" and (env.get("error") or {}).get("code") == "project.pathRequired",
                          "the key tool runs the command bound to F2 (project.save, refused without a path)", {"env": env, "activity": act})
                env = await run_cmd(session, "keymap.set", {"commandId": "ui.statusBar.set", "params": {"visible": False}, "keys": "F3"})
                run.check(env.get("status") == "ok", "keymap.set F3 (hide the status bar)", env.get("error"))
                env = await run_cmd(session, "workspace.create", {"name": "E2E"})
                run.check(env.get("status") == "ok", "workspace.create E2E", env.get("error"))
                # A geometry that fits small runner displays (the Windows runner's is about 1024 x 768: the OS clamps a
                # larger window and the kernel then stores the real size, as it should).
                env = await run_cmd(session, "window.geometry.set", GEOMETRY)
                run.check(env.get("status") == "ok", "window.geometry.set", env.get("error"))

                # Typed refusals reach the agent with their codes.
                env = await run_cmd(session, "prefs.set", {"values": {"files.backups.count": 99}})
                run.check((env.get("error") or {}).get("code") == "prefs.outOfRange", "prefs.outOfRange", env)
                env = await run_cmd(session, "keymap.set", {"commandId": "project.save", "keys": "Escape"})
                run.check((env.get("error") or {}).get("code") == "keymap.reservedKey", "keymap.reservedKey", env)

                # User priority: an open preferences dialog makes an agent change wait, then fail with ui.busy.
                env = await run_cmd(session, "ui.preferences.open", {"category": "interface"})
                root = await gui_tree(session, run, lambda r: by_id(r, "preferences") is not None, "the preferences dialog opens")
                run.shot("03-preferences", APP_PID["pid"])
                state = (await call(session, "get_state", {})).get("result") or {}
                run.check((state.get("modal") or {}).get("kind") == "dialog", "app.state.get reports the modal dialog", state.get("modal"))
                t0 = time.monotonic()
                env = await run_cmd(session, "doc.object.patch", {"targets": ["@current"], "props": {"meta": {"e2e": True}}})
                waited = time.monotonic() - t0
                run.check((env.get("error") or {}).get("code") == "ui.busy", "an agent change waits for the user and fails with ui.busy", {"env": env, "waitedS": waited})
                env = await run_cmd(session, "window.panel.set", {"panelId": "colour", "visible": True})
                run.check((env.get("error") or {}).get("code") == "ui.modalOpen", "a focus change under a user dialog is ui.modalOpen", env)

                # The user closes Preferences (Escape through the OS): the GUI drops the dialog, the kernel sees the
                # modal state end, a shortcut resolves again and an agent action is accepted (C41-R1-01).
                pressed = os_escape(APP_PID["pid"])
                run.check(pressed.get("ok") is True, "Escape pressed in the application window (OS input)", pressed)
                root = await gui_tree(session, run, lambda r: by_id(r, "preferences") is None, "Escape closes Preferences in the GUI")
                state = (await call(session, "get_state", {})).get("result") or {}
                run.check(state.get("modal") is None, "the kernel sees the dialog closed (modal: null)", state.get("modal"))
                env = await call(session, "key", {"keys": "F3"})
                act = await last_activity(session, "key")
                run.check(env.get("status") == "ok" and act.get("commandId") == "ui.statusBar.set",
                          "after the dialog closed, F3 resolves to ui.statusBar.set and runs", {"env": env, "activity": act})
                root = await gui_tree(session, run, lambda r: by_id(r, "statusBar") is None, "the F3 shortcut hides the status bar in the GUI")
                env = await run_cmd(session, "ui.statusBar.set", {"visible": True})
                await gui_tree(session, run, lambda r: by_id(r, "statusBar") is not None, "the status bar returns")
                env = await run_cmd(session, "doc.object.patch", {"targets": ["@current"], "props": {"meta": {"e2e": True}}})
                run.check(env.get("status") == "ok", "after the dialog closed, an agent action is accepted", env)
                run.save("activity.json", (await run_cmd(session, "agent.activity.list", {"limit": 100})).get("result"))
                # The geometry the kernel holds before the restart (the window's real size, reported by the host).
                run.geometry = (await run_cmd(session, "window.geometry.get")).get("result") or {}
                size = (run.geometry.get("width"), run.geometry.get("height"))
                run.check(size == (GEOMETRY["width"], GEOMETRY["height"]), "the window took the requested size", run.geometry)


async def second_session(url: str, token: str, run: Run) -> None:
    client = httpx2.AsyncClient(headers={"Authorization": f"Bearer {token}"}, timeout=httpx2.Timeout(60.0, read=300.0))
    async with client:
        async with streamable_http_client(url, http_client=client) as streams:
            async with ClientSession(streams[0], streams[1]) as session:
                await session.initialize()
                env = await run_cmd(session, "prefs.get", {"keys": ["interface.theme", "files.backups.count"]})
                run.check((env.get("result") or {}).get("values") == {"interface.theme": "dark", "files.backups.count": 7}, "preferences persisted across a restart", env)
                env = await run_cmd(session, "keymap.get", {"keys": "F2"})
                run.check(((env.get("result") or {}).get("effectiveCommand") or {}).get("commandId") == "project.save", "the key binding persisted", env)
                names = [w["name"] for w in (await run_cmd(session, "workspace.list")).get("result") or []]
                run.check("E2E" in names, "the user workspace persisted", names)
                g = (await run_cmd(session, "window.geometry.get")).get("result") or {}
                before = {k: run.geometry.get(k) for k in ("x", "y", "width", "height")}
                run.check({k: g.get(k) for k in before} == before, "the window geometry persisted", {"before": before, "after": g})
                root = await gui_tree(session, run, lambda r: by_id(r, "startScreen") is not None, "the restarted GUI shows the start screen")
                run.save("tree-04-restart.json", root)
                run.shot("04-restart", APP_PID["pid"])


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    app, out = sys.argv[1], Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    run = Run(out)
    user = Path(tempfile.mkdtemp(prefix="tt-c41-e2e-"))
    settings = user / "Settings"
    settings.mkdir(parents=True)
    seed = {"agent.policy.defaults": {"policy": "autonomous", "maxJobs": 2, "maxBatchSteps": 1000, "maxInlineImagePx": 2048,
                                      "maxInlineBytes": 52428800, "askInApp": True, "letAgentMoveView": True, "highlightAgentChanges": True}}
    (settings / "preferences.json").write_text(json.dumps({"format": "toontown-settings", "kind": "preferences", "formatVersion": "1.0", "data": seed}), encoding="utf-8")
    discovery = out / "discovery.json"
    started = time.monotonic()
    try:
        proc = launch(app, user, discovery)
        try:
            d = wait_discovery(discovery, proc)
            run.check(True, "the application started and serves attached mode", {"endpoint": d["endpoint"], "startS": round(time.monotonic() - started, 1)})
            asyncio.run(first_session(d["endpoint"], d["token"], run))
        finally:
            stop(proc)
        proc = launch(app, user, discovery)
        try:
            d = wait_discovery(discovery, proc)
            asyncio.run(second_session(d["endpoint"], d["token"], run))
        finally:
            stop(proc)
        # A corrupt settings file is set aside and its defaults apply (01 §11).
        (settings / "keymap.json").write_text("{broken", encoding="utf-8")
        proc = launch(app, user, discovery)
        try:
            d = wait_discovery(discovery, proc)

            async def corrupt() -> None:
                client = httpx2.AsyncClient(headers={"Authorization": f"Bearer {d['token']}"}, timeout=60.0)
                async with client:
                    async with streamable_http_client(d["endpoint"], http_client=client) as streams:
                        async with ClientSession(streams[0], streams[1]) as session:
                            await session.initialize()
                            env = await run_cmd(session, "keymap.get", {"keys": "F2"})
                            run.check((env.get("result") or {}).get("effectiveCommand") is None, "a corrupt keymap file falls back to the defaults", env)

            asyncio.run(corrupt())
            run.check(any(p.name.startswith("keymap.corrupt-") for p in settings.iterdir()), "the corrupt file was kept aside", [p.name for p in settings.iterdir()])
        finally:
            stop(proc)
    except Exception as e:  # noqa: BLE001 - reported as a failed check
        run.check(False, "the run completed", repr(e))
    finally:
        manifest = {
            "kind": "toontown.c41.gui-e2e",
            "os": platform.platform(),
            "app": Path(app).name,
            "durationS": round(time.monotonic() - started, 1),
            "checks": sum(1 for t in run.transcript if "check" in t),
            "failed": run.failures,
            "screenshots": run.shots,
            "files": sorted(p.name for p in out.iterdir()),
        }
        # The evidence keeps the discovery file's shape but never its bearer token (secrets stay out of artifacts).
        if discovery.exists():
            try:
                disc = json.loads(discovery.read_text(encoding="utf-8"))
                disc["token"] = "<redacted>"
                discovery.write_text(json.dumps(disc, indent=2), encoding="utf-8")
            except (OSError, ValueError):
                discovery.unlink(missing_ok=True)
        run.save("transcript.json", run.transcript)
        run.save("manifest.json", manifest)
        shutil.rmtree(user, ignore_errors=True)
    print(json.dumps({"failed": run.failures}, indent=2))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
