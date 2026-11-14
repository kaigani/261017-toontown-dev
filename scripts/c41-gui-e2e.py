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
        self.splitter_area: str | None = None

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
                # Captures carry no ancillary metadata (round 2 ruling on the image metadata, C41-R2-23).
                strip_png(path)
                self.shots.append(path.name)
                self.transcript.append({"screenshot": path.name})
            else:
                self.transcript.append({"screenshot": path.name, "captured": False})
        except Exception as e:  # noqa: BLE001 - evidence only
            self.transcript.append({"screenshot": path.name, "captured": False, "error": str(e)})


#: PNG chunks kept by `strip_png`: the critical ones and `tRNS` (transparency changes pixels); every other ancillary
#: chunk (colour profile, gamma, text, time, physical size) is metadata and is dropped.
PNG_KEEP = {b"IHDR", b"PLTE", b"tRNS", b"IDAT", b"IEND"}


def strip_png(path: Path) -> bool:
    """Rewrites a PNG without its ancillary metadata chunks; the image data (and so every pixel) is unchanged. Returns
    whether the file was a PNG and was rewritten."""
    data = path.read_bytes()
    sig = b"\x89PNG\r\n\x1a\n"
    if not data.startswith(sig):
        return False
    out, i = [sig], len(sig)
    while i + 8 <= len(data):
        n = int.from_bytes(data[i:i + 4], "big")
        kind = data[i + 4:i + 8]
        chunk = data[i:i + 12 + n]
        if len(chunk) < 12 + n:
            return False
        if kind in PNG_KEEP:
            out.append(chunk)
        i += 12 + n
        if kind == b"IEND":
            break
    path.write_bytes(b"".join(out))
    return True


def os_escape(pid: int | None) -> dict:
    """Presses Escape in the application window through the operating system, as a user would (no command exists to
    close a user dialog). Windows: SendKeys to the foreground window of `pid`; macOS: System Events."""
    if pid is None:
        return {"ok": False, "error": "no pid"}
    if sys.platform == "win32":
        ps = (
            "Add-Type -AssemblyName System.Windows.Forms;"
            "Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;"
            "public static class F{[DllImport(\"user32.dll\")]public static extern bool SetForegroundWindow(IntPtr h);"
            "[DllImport(\"user32.dll\")]public static extern IntPtr GetForegroundWindow();"
            "[DllImport(\"user32.dll\")]public static extern void keybd_event(byte k,byte s,uint f,UIntPtr e);}';"
            # A workstation's foreground lock refuses SetForegroundWindow to a background process; a released Alt key
            # lifts it. Only when another window is in front: in our own window a released Alt would open its menu bar
            # and the Escape would only close that (CI runners have no other foreground window).
            f"$p=Get-Process -Id {pid};$h=$p.MainWindowHandle;"
            "if([F]::GetForegroundWindow() -ne $h){[F]::keybd_event(0x12,0,0,[UIntPtr]::Zero);[F]::keybd_event(0x12,0,2,[UIntPtr]::Zero)};"
            "[F]::SetForegroundWindow($h)|Out-Null;Start-Sleep -Milliseconds 300;"
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


def os_window_size(pid: int | None) -> dict | None:
    """The main window's size (the window titled Toontown...), as the OS reports it; see `os_windows`."""
    rows = os_windows(pid) or []
    main = next((r for r in rows if r["title"].startswith("Toontown")), None)
    return {"width": main["width"], "height": main["height"], "frame": main["frame"]} if main else None


def os_window_size_by_handle(pid: int | None) -> dict | None:
    """The application window's size as the operating system reports it, in logical pixels: Windows, the client area
    (GetClientRect, divided by the window's DPI scale); macOS, the outer frame from System Events (points, title bar
    included). `None` when it cannot be measured."""
    if pid is None:
        return None
    try:
        if sys.platform == "win32":
            ps = (
                "Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;"
                "public struct R{public int L,T,Ri,B;}"
                "public static class W{[DllImport(\"user32.dll\")]public static extern bool GetClientRect(IntPtr h,out R r);"
                "[DllImport(\"user32.dll\")]public static extern uint GetDpiForWindow(IntPtr h);}';"
                f"$h=(Get-Process -Id {pid}).MainWindowHandle;$r=New-Object R;[W]::GetClientRect($h,[ref]$r)|Out-Null;"
                "$d=[W]::GetDpiForWindow($h);if($d -eq 0){$d=96};"
                "Write-Output ('{0} {1} {2}' -f ($r.Ri-$r.L),($r.B-$r.T),$d)"
            )
            out = subprocess.run(["powershell", "-NoProfile", "-Command", ps], timeout=30, check=False, capture_output=True, text=True).stdout.split()
            w, h, dpi = (int(x) for x in out[:3])
            scale = dpi / 96.0
            return {"width": round(w / scale), "height": round(h / scale), "frame": "client"}
        if sys.platform == "darwin":
            script = f'tell application "System Events" to get size of first window of (first process whose unix id is {pid})'
            out = subprocess.run(["osascript", "-e", script], timeout=30, check=False, capture_output=True, text=True).stdout
            w, h = (int(x.strip()) for x in out.split(",")[:2])
            return {"width": w, "height": h, "frame": "outer"}
    except (ValueError, OSError, subprocess.SubprocessError):
        return None
    return None


def os_windows(pid: int | None) -> list[dict] | None:
    """The application's visible top-level windows as the OS lists them: title and size in logical pixels (Windows:
    client area via EnumWindows; macOS: outer frame via System Events). `None` when they cannot be listed."""
    if pid is None:
        return None
    try:
        if sys.platform == "win32":
            ps = (
                "Add-Type -TypeDefinition 'using System;using System.Text;using System.Runtime.InteropServices;"
                "public struct R{public int L,T,Ri,B;}"
                "public static class E{public delegate bool P(IntPtr h,IntPtr l);"
                "[DllImport(\"user32.dll\")]public static extern bool EnumWindows(P p,IntPtr l);"
                "[DllImport(\"user32.dll\")]public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);"
                "[DllImport(\"user32.dll\")]public static extern bool IsWindowVisible(IntPtr h);"
                "[DllImport(\"user32.dll\",CharSet=CharSet.Unicode)]public static extern int GetWindowText(IntPtr h,StringBuilder s,int n);"
                "[DllImport(\"user32.dll\")]public static extern bool GetClientRect(IntPtr h,out R r);"
                "[DllImport(\"user32.dll\")]public static extern uint GetDpiForWindow(IntPtr h);}';"
                f"$out=@();[E]::EnumWindows({{param($h,$l) $q=0;[E]::GetWindowThreadProcessId($h,[ref]$q)|Out-Null;"
                f"if($q -eq {pid} -and [E]::IsWindowVisible($h)){{$b=New-Object Text.StringBuilder 256;[E]::GetWindowText($h,$b,256)|Out-Null;"
                "$r=New-Object R;[E]::GetClientRect($h,[ref]$r)|Out-Null;$d=[E]::GetDpiForWindow($h);if($d -eq 0){$d=96};"
                "$s=$d/96.0;$script:out+=('{0}|{1}|{2}' -f $b.ToString(),[math]::Round(($r.Ri-$r.L)/$s),[math]::Round(($r.B-$r.T)/$s))};$true},[IntPtr]::Zero)|Out-Null;"
                "$out -join \"`n\""
            )
            text = subprocess.run(["powershell", "-NoProfile", "-Command", ps], timeout=30, check=False, capture_output=True, text=True).stdout
            rows = []
            for line in text.splitlines():
                parts = line.rsplit("|", 2)
                if len(parts) == 3 and parts[0]:
                    rows.append({"title": parts[0], "width": int(parts[1]), "height": int(parts[2]), "frame": "client"})
            return rows
        if sys.platform == "darwin":
            script = (
                f'tell application "System Events" to tell (first process whose unix id is {pid})\n'
                'set out to ""\nrepeat with w in windows\nset s to size of w\n'
                'set out to out & (name of w) & "|" & (item 1 of s) & "|" & (item 2 of s) & linefeed\nend repeat\nreturn out\nend tell'
            )
            text = subprocess.run(["osascript", "-e", script], timeout=30, check=False, capture_output=True, text=True).stdout
            rows = []
            for line in text.splitlines():
                parts = line.rsplit("|", 2)
                if len(parts) == 3 and parts[0]:
                    rows.append({"title": parts[0], "width": int(parts[1]), "height": int(parts[2]), "frame": "outer"})
            return rows
    except (ValueError, OSError, subprocess.SubprocessError):
        return None
    return None


async def os_window_titled(title: str, want, bound_s: float = 30.0) -> dict | None:
    """The OS window titled `title` once `want(row)` holds (window managers apply changes asynchronously), bounded."""
    start = time.monotonic()
    last = None
    while time.monotonic() - start < bound_s:
        rows = os_windows(APP_PID["pid"]) or []
        last = next((r for r in rows if r["title"] == title), None)
        if want(last):
            return last
        await asyncio.sleep(0.25)
    return last


def size_matches(kernel: dict, os_size: dict | None) -> bool:
    """The kernel's geometry (inner size) is the OS window's: equal client size on Windows (1 px rounding); on macOS the
    outer frame is the inner size plus a title bar of at most 40 points."""
    if not os_size:
        return False
    w, h = kernel.get("width"), kernel.get("height")
    if not isinstance(w, int) or not isinstance(h, int):
        return False
    if os_size["frame"] == "client":
        return abs(os_size["width"] - w) <= 1 and abs(os_size["height"] - h) <= 1
    return abs(os_size["width"] - w) <= 1 and 0 <= os_size["height"] - h <= 40


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


def _walk(node: dict):
    """Every node of a tree, depth first."""
    yield node
    for c in node.get("children", []) or []:
        yield from _walk(c)


def by_id(node: dict, ident: str) -> dict | None:
    return find(node, lambda n: n.get("id") == ident)


def area_size(root: dict, area: str | None) -> int | None:
    """The rendered size of a dock area (`area.<side>` node of the GUI tree: width of left/right, height of top/bottom)."""
    node = by_id(root, f"area.{area}") if area else None
    try:
        return int(node["value"]) if node else None
    except (KeyError, TypeError, ValueError):
        return None


async def os_titles(want, bound_s: float = 15.0) -> set[str]:
    """The titles of the application's visible OS windows once `want(titles)` holds, bounded."""
    start = time.monotonic()
    titles: set[str] = set()
    while time.monotonic() - start < bound_s:
        titles = {r["title"] for r in (os_windows(APP_PID["pid"]) or [])}
        if want(titles):
            return titles
        await asyncio.sleep(0.25)
    return titles


async def stays(want, hold_s: float = 1.5) -> bool:
    """Whether `want(titles)` keeps holding for `hold_s` (a late reconciliation must not undo it)."""
    start = time.monotonic()
    while time.monotonic() - start < hold_s:
        if not want({r["title"] for r in (os_windows(APP_PID["pid"]) or [])}):
            return False
        await asyncio.sleep(0.25)
    return True


async def float_visibility(session: ClientSession, run: Run) -> None:
    """Global float visibility (C41-R2-10): three floats; minimise and restore, then Toggle floating windows off and on;
    and the other order (off, minimise, restore, on). Every path that shows floats respects the global flag, and no
    superseded reconciliation shows them again."""
    panels = {p["panelId"]: p.get("title") for p in (await run_cmd(session, "window.panel.list")).get("result") or []}
    ids = ["history", "colour", "properties"]
    titles = {panels.get(i) or i for i in ids}
    for i in ids[1:]:
        await run_cmd(session, "window.panel.set", {"panelId": i, "floating": True, "visible": True})
    all_shown = lambda t: titles <= t
    none_shown = lambda t: not (titles & t)
    seen = await os_titles(all_shown)
    run.check(all_shown(seen), "three floats are OS windows of the application", sorted(seen))
    # Minimise, restore, then hide them all (the order of the round-2 reproduction).
    await run_cmd(session, "window.minimized.set", {"minimized": True})
    hidden = await os_titles(none_shown)
    await run_cmd(session, "window.minimized.set", {"minimized": False})
    back = await os_titles(all_shown)
    run.check(none_shown(hidden) and all_shown(back), "minimising the main window hides the floats; restoring shows them",
              {"minimised": sorted(hidden), "restored": sorted(back)})
    env = await run_cmd(session, "window.floatingPanels.set", {"visible": False})
    gone = await os_titles(none_shown)
    run.check(env.get("status") == "ok" and none_shown(gone) and await stays(none_shown),
              "window.floatingPanels.set visible false after minimise and restore: every float window goes and stays gone",
              {"os": sorted(gone), "error": env.get("error")})
    env = await run_cmd(session, "window.floatingPanels.set", {"visible": True})
    back = await os_titles(all_shown)
    run.check(env.get("status") == "ok" and all_shown(back), "window.floatingPanels.set visible true: the three floats return", sorted(back))
    # The other order: hidden first; minimise and restore must not show them; then show.
    await run_cmd(session, "window.floatingPanels.set", {"visible": False})
    await os_titles(none_shown)
    await run_cmd(session, "window.minimized.set", {"minimized": True})
    await asyncio.sleep(0.5)
    await run_cmd(session, "window.minimized.set", {"minimized": False})
    held = await stays(none_shown, 2.0)
    env = await run_cmd(session, "window.floatingPanels.set", {"visible": True})
    back = await os_titles(all_shown)
    run.check(held and env.get("status") == "ok" and all_shown(back),
              "hidden floats stay hidden through minimise and restore, and return when shown again", sorted(back))
    for i in ids[1:]:
        await run_cmd(session, "window.panel.set", {"panelId": i, "floating": False})
    await os_titles(lambda t: not ({panels.get(i) or i for i in ids[1:]} & t))


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
                root = await gui_tree(session, run, lambda r: by_id(r, "tab.history") is not None, "the History tab appears")
                run.check(env.get("status") == "ok" and by_id(root, "tab.history") is not None,
                          "window.panel.set history: the History tab is in the rendered GUI", env.get("error"))
                env = await run_cmd(session, "ui.statusBar.set", {"visible": False})
                root = await gui_tree(session, run, lambda r: by_id(r, "statusBar") is None, "the status bar hides")
                run.check(by_id(root, "statusBar") is None, "ui.statusBar.set false: the GUI drops the status bar")
                env = await run_cmd(session, "ui.statusBar.set", {"visible": True})
                root = await gui_tree(session, run, lambda r: by_id(r, "statusBar") is not None, "the status bar returns")
                run.check(by_id(root, "statusBar") is not None, "ui.statusBar.set true: the GUI shows the status bar")

                # A project: the start screen closes, the window title names the project.
                env = await run_cmd(session, "project.create", {"name": "Bounce", "onUnsaved": "fail"})
                root = await gui_tree(session, run, lambda r: by_id(r, "startScreen") is None, "the start screen closes")
                run.check(env.get("status") == "ok" and by_id(root, "startScreen") is None and by_id(root, "dock") is not None,
                          "project.create: the GUI closes the start screen and shows the workspace", env.get("error"))
                run.check("Bounce" in str(root.get("label", "")), "the window title names the project", root.get("label"))
                run.save("tree-02-project.json", root)
                run.shot("02-project", APP_PID["pid"])

                # Workspaces: selecting Drawing makes the Drawing view central.
                env = await run_cmd(session, "workspace.select", {"name": "Drawing"})
                root = await gui_tree(session, run, lambda r: (by_id(r, "tab.drawingView") or {}).get("checked") is True, "the Drawing view is shown")
                tab = by_id(root, "tab.drawingView") or {}
                run.check(env.get("status") == "ok" and tab.get("checked") is True,
                          "workspace.select Drawing: the GUI shows the Drawing view as its group's selected tab", tab)
                state = (await call(session, "get_state", {})).get("result") or {}
                run.check(state.get("mode") == "drawing" and (state.get("activeView") or {}).get("viewId") == "drawingView",
                          "the kernel's mode follows the view the GUI shows (drawing)", {"mode": state.get("mode"), "activeView": state.get("activeView")})

                # Preferences, a key binding, a user workspace and the window geometry (persisted, read back later).
                env = await run_cmd(session, "prefs.set", {"values": {"interface.theme": "dark", "files.backups.count": 7}})
                root = await gui_tree(session, run, lambda r: (by_id(r, "app") or {}).get("value") == "dark", "the GUI applies the dark theme")
                run.check(env.get("status") == "ok" and (by_id(root, "app") or {}).get("value") == "dark",
                          "prefs.set interface.theme dark: the GUI applies the dark theme", env.get("error"))
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
                run.check(env.get("status") == "ok", "window.geometry.set accepted", env.get("error"))

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
                run.check(by_id(root, "preferences") is not None and (state.get("modal") or {}).get("kind") == "dialog",
                          "the GUI shows Preferences and reports it: app.state.get has the modal dialog", state.get("modal"))
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

                # A splitter drag's command (SI-118, ruling O-3a): a panel tabbed with itself sets its area's size and
                # leaves everything else as it is, in the kernel and in the GUI.
                lay = (await run_cmd(session, "workspace.get")).get("result") or {}
                area = next((a for a in ("right", "bottom", "left", "top") if (lay.get("docks") or {}).get(a)), None)
                tabs = lambda r: [(n.get("id"), n.get("checked")) for n in _walk(r) if n.get("role") == "tab"]
                if area:
                    panel = lay["docks"][area][0]["current"]
                    tabs_before = tabs(await gui_tree(session, run, lambda r: True, "the GUI tree before the resize"))
                    env = await run_cmd(session, "window.panel.move", {"panelId": panel, "area": area, "relativeTo": panel,
                                                                        "position": "tab", "size": 300})
                    after = (env.get("result") or {})
                    expected = json.loads(json.dumps(lay))
                    expected["docks"][area][0]["size"] = 300
                    root = await gui_tree(session, run, lambda r: area_size(r, area) is not None and abs(area_size(r, area) - 300) <= 3,
                                          "the GUI renders the area at the new size")
                    run.check(env.get("status") == "ok" and after.get("docks") == expected["docks"] and tabs(root) == tabs_before
                              and abs((area_size(root, area) or 0) - 300) <= 3,
                              "the same-place window.panel.move sets the area size only: the kernel, the GUI tabs unchanged, "
                              "and the GUI renders the area at the new size",
                              {"area": area, "panel": panel, "rendered": area_size(root, area), "error": env.get("error")})
                    run.splitter_area = area

                # A floating panel is a window of its own (ruling O-2): the OS lists it, the one inspect_ui tree has its
                # node, a geometry change moves it. It stays floating across the restart below.
                env = await run_cmd(session, "window.panel.set", {"panelId": "history", "floating": True,
                                                                   "geometry": {"x": 60, "y": 60, "width": 420, "height": 300}})
                root = await gui_tree(session, run, lambda r: by_id(r, "float-history") is not None, "the float reports its tree")
                run.check(env.get("status") == "ok" and by_id(root, "float-history") is not None,
                          "window.panel.set floating: the combined inspect_ui tree has the float window", env.get("error"))
                row = await os_window_titled("History", lambda r: r is not None)
                run.check(row is not None, "the OS lists the History float as a window of the application", os_windows(APP_PID["pid"]))
                env = await run_cmd(session, "window.panel.set", {"panelId": "history", "geometry": {"x": 140, "y": 90, "width": 380, "height": 280}})
                row = await os_window_titled("History", lambda r: r is not None and size_matches({"width": 380, "height": 280}, r))
                run.check(env.get("status") == "ok" and size_matches({"width": 380, "height": 280}, row),
                          "window.panel.set geometry: the OS float window takes the size", {"os": row, "error": env.get("error")})
                await float_visibility(session, run)

                # Stored with the workspace (as at quit with autosave on, which agents cannot run), restored at start.
                env = await run_cmd(session, "workspace.save", {})
                run.check(env.get("status") == "ok", "workspace.save stores the arrangement with its float", env.get("error"))
                # The geometry the kernel holds before the restart (the window's real size, reported by the host), and the
                # size the operating system reports for the window itself.
                run.geometry = (await run_cmd(session, "window.geometry.get")).get("result") or {}
                size = (run.geometry.get("width"), run.geometry.get("height"))
                measured = os_window_size(APP_PID["pid"])
                run.check(size == (GEOMETRY["width"], GEOMETRY["height"]) and size_matches(run.geometry, measured),
                          "the OS window has the requested size, which the kernel holds", {"kernel": run.geometry, "os": measured})


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
                # The float was restored with the workspace (ruling O-2), then docks back: its window goes.
                root = await gui_tree(session, run, lambda r: by_id(r, "float-history") is not None, "the restored float reports its tree")
                row = await os_window_titled("History", lambda r: r is not None)
                run.check(by_id(root, "float-history") is not None and row is not None,
                          "after the restart the float is a window again (tree and OS)", {"os": os_windows(APP_PID["pid"])})
                env = await run_cmd(session, "window.panel.set", {"panelId": "history", "floating": False})
                root = await gui_tree(session, run, lambda r: by_id(r, "float-history") is None, "the docked float leaves the tree")
                row = await os_window_titled("History", lambda r: r is None)
                run.check(env.get("status") == "ok" and by_id(root, "float-history") is None and row is None,
                          "window.panel.set floating false: the float window closes (tree and OS)", {"os": os_windows(APP_PID["pid"])})
                measured = os_window_size(APP_PID["pid"])
                run.check({k: g.get(k) for k in before} == before and size_matches(g, measured),
                          "the window geometry persisted: the restarted OS window has it", {"before": before, "after": g, "os": measured})
                root = await gui_tree(session, run, lambda r: by_id(r, "startScreen") is not None, "the restarted GUI shows the start screen")
                root = await gui_tree(session, run, lambda r: (by_id(r, "app") or {}).get("value") == "dark", "the restarted GUI applies the dark theme")
                run.check((by_id(root, "app") or {}).get("value") == "dark", "the persisted theme is applied by the restarted GUI")
                # The splitter's size came back with the workspace: the restarted GUI renders the area at the size the
                # kernel restored.
                area = run.splitter_area
                lay = (await run_cmd(session, "workspace.get")).get("result") or {}
                stored = (((lay.get("docks") or {}).get(area) or [{}])[0] or {}).get("size") if area else None
                root = await gui_tree(session, run, lambda r: isinstance(stored, int) and area_size(r, area) is not None
                                      and abs(area_size(r, area) - stored) <= 3, "the restarted GUI renders the restored area size")
                run.check(isinstance(stored, int) and abs((area_size(root, area) or 0) - stored) <= 3,
                          "after the restart the GUI renders the area at the size the splitter set", {"area": area, "kernel": stored, "rendered": area_size(root, area)})
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
