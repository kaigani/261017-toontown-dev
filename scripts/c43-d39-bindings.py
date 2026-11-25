"""M4 C4.3b ruling D-39: every GUI binding added in C4.3b exercised once through the real window (Windows).

Usage: python scripts/c43-d39-bindings.py <toontown binary> <out dir> [--mode os|mcp] [--only NAME[,NAME...]]

Two forms of the same table (rulings D-39, D-40). `--mode os` (Windows workstation): real OS input, the page's IPC
requests observed through the webview's debugging port. `--mode mcp` (CI, both OSes): the attached MCP interface, see
`McpTable`.

D-27's "declared is not honoured", applied to GUI bindings (C43B-R1-07 and -08 were bindings declared and never
exercised). One table: each row is a person's action in the running application through the operating system's real
pointer or keyboard (a click on a rendered control, a native menu item, a key, a choice in a list, a typed path in a
file dialog), and the row records

- the exact command and parameters the window sent (read from the webview's IPC, recorded through its debugging
  port; the click itself is real OS input),
- the effect on the document or the session state, read back through MCP,
- the undo steps the action made and the spec's label where it makes one (none where the command is UI state),
- the disabled state where the precheck refuses: after the table, the project is closed and every control is checked
  disabled (or gone with the project), with a real click sending nothing and the native menu items disabled in the
  menu and in the window tree (ruling D-38).

A toggle's row clicks twice: the second click must send the opposite value and restore the state. Every row states its
spec lines. `scripts/c43_d39_coverage.py` maps every commanded control of the generated control inventory (the parity
manifest's DOM rows) to its rows here or to a reason (ruling D-59 item 2; `scripts/test_c43_d39_inventory.py` in CI); a
full run checks at the end that every row it names ran. Output: `rows.json`, `transcript.json`, `manifest.json` (kind
`toontown.c43b.d39-bindings`); the committed receipt is their summary (ruling D-42: `scripts/receipt-summary.py <out>
--command ... --delete`). Exit 0 when every row holds.

The webview's debugging port is opened for this run only (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`); the script carries
its own small WebSocket client (standard library), so it needs only the MCP SDK. The OS form is a workstation receipt
(reviews/evidence/M4-C4.3b/d39-bindings): on the CI runners the webview never opened the port (runs 43822938737 and
43830431401, with and without a data folder of its own); CI runs the MCP form.
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import contextlib
import ctypes
import importlib.util
import json
import os
import shutil
import socket
import sys
import tempfile
import time
import urllib.parse
import urllib.request
try:  # Windows only (the OS form); the MCP form runs on every OS
    from ctypes import wintypes
except (ImportError, ValueError):
    wintypes = None
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CDP_PORT = 9237


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


e6 = _load("c43e6", "c43-e6-coexistence.py")
e2e, c42 = e6.e2e, e6.c42


def clipboard_text() -> str | None:
    """The system clipboard's text (Windows), as the person would paste it (D-62 item 2's Copy row)."""
    if u32 is None:
        return None
    k32 = ctypes.windll.kernel32
    u32.GetClipboardData.restype = ctypes.c_void_p
    k32.GlobalLock.restype = ctypes.c_void_p
    k32.GlobalLock.argtypes = [ctypes.c_void_p]
    k32.GlobalUnlock.argtypes = [ctypes.c_void_p]
    if not u32.OpenClipboard(0):
        return None
    try:
        handle = u32.GetClipboardData(13)  # CF_UNICODETEXT
        if not handle:
            return None
        ptr = k32.GlobalLock(handle)
        try:
            return ctypes.wstring_at(ptr) if ptr else None
        finally:
            k32.GlobalUnlock(handle)
    finally:
        u32.CloseClipboard()


def _same_number(shown, value) -> bool:
    """A field's text shows `value` (D-61: the retained value after a refused entry)."""
    try:
        return value is not None and float(shown) == float(value)
    except (TypeError, ValueError):
        return False


def spec_line(command: str) -> str:
    """The spec table row that defines `command`: `<file>:<line>`."""
    for f in sorted((ROOT / "spec").glob("*.md")):
        for n, line in enumerate(f.read_text(encoding="utf-8").splitlines(), 1):
            if line.startswith(f"| `{command}` |"):
                return f"spec/{f.name}:{n}"
    return f"(no spec row for {command})"


# ----- a small WebSocket client for the webview's debugging port (RFC 6455, text frames) ----------------------------

class WebSocket:
    def __init__(self, url: str):
        u = urllib.parse.urlparse(url)
        self.s = socket.create_connection((u.hostname, u.port), timeout=60)
        key = base64.b64encode(os.urandom(16)).decode()
        req = (f"GET {u.path} HTTP/1.1\r\nHost: {u.hostname}:{u.port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
               f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n")
        self.s.sendall(req.encode())
        head = b""
        while b"\r\n\r\n" not in head:
            chunk = self.s.recv(1)
            if not chunk:
                raise RuntimeError("the debugging port closed during the handshake")
            head += chunk
        if b" 101 " not in head.split(b"\r\n")[0]:
            raise RuntimeError(f"no WebSocket upgrade: {head[:200]!r}")

    def _exact(self, n: int) -> bytes:
        out = b""
        while len(out) < n:
            chunk = self.s.recv(n - len(out))
            if not chunk:
                raise RuntimeError("the debugging port closed")
            out += chunk
        return out

    def send(self, text: str) -> None:
        data = text.encode()
        head = bytearray([0x81])
        n = len(data)
        if n < 126:
            head.append(0x80 | n)
        elif n < 65536:
            head.append(0x80 | 126)
            head += n.to_bytes(2, "big")
        else:
            head.append(0x80 | 127)
            head += n.to_bytes(8, "big")
        mask = os.urandom(4)
        head += mask
        self.s.sendall(bytes(head) + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))

    def recv(self) -> str:
        parts = b""
        while True:
            b0, b1 = self._exact(2)
            op, n = b0 & 0x0F, b1 & 0x7F
            if n == 126:
                n = int.from_bytes(self._exact(2), "big")
            elif n == 127:
                n = int.from_bytes(self._exact(8), "big")
            mask = self._exact(4) if b1 & 0x80 else None
            payload = self._exact(n)
            if mask:
                payload = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
            if op == 8:
                raise RuntimeError("the debugging port closed the connection")
            if op == 9:
                continue
            parts += payload
            if b0 & 0x80:
                return parts.decode("utf-8", "replace")


class Cdp:
    """The main window's page over the debugging port. A reader thread takes every message: answers go to their
    callers; a pause at the breakpoint on the page's `invoke` (the recorder) is answered at once, recording the call and
    resuming, so the page never waits for the script."""

    def __init__(self, port: int):
        import threading
        pages = []
        for _ in range(120):
            try:
                pages = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=5))
                if any(p.get("type") == "page" for p in pages):
                    break
            except OSError:
                pass
            time.sleep(0.5)
        self.lock = threading.Lock()
        self.answers: dict = {}
        self.waiting: dict = {}
        self.n = 0
        self.calls: list = []
        self.bodies: set = set()
        self.ws = None
        for p in pages:
            if p.get("type") != "page" or "float" in (p.get("url") or ""):
                continue
            self.ws = WebSocket(p["webSocketDebuggerUrl"])
            self.reader = threading.Thread(target=self._read, daemon=True)
            self.reader.start()
            if self.js("!!document.querySelector('[data-tt-id=\"mainToolbar\"]') || !!document.querySelector('[data-tt-id=\"startScreen\"]')"):
                return
        raise RuntimeError(f"no main window page on the debugging port: {[p.get('url') for p in pages]}")

    def _send(self, method: str, params: dict) -> int:
        import threading
        with self.lock:
            self.n += 1
            ident = self.n
            self.waiting[ident] = threading.Event()
            self.ws.send(json.dumps({"id": ident, "method": method, "params": params}))
        return ident

    def _read(self) -> None:
        while True:
            try:
                r = json.loads(self.ws.recv())
            except Exception:  # noqa: BLE001 - the page went away
                return
            if r.get("method") == "Network.requestWillBeSent":
                req = r["params"].get("request") or {}
                if "/run_command" in (req.get("url") or ""):
                    if req.get("postData"):
                        self._ipc(req["postData"])
                    elif req.get("hasPostData"):
                        with self.lock:
                            self.n += 1
                            ident = self.n
                            self.bodies.add(ident)
                            self.ws.send(json.dumps({"id": ident, "method": "Network.getRequestPostData",
                                                     "params": {"requestId": r["params"]["requestId"]}}))
                continue
            if r.get("id") in self.bodies:
                self.bodies.discard(r["id"])
                self._ipc(((r.get("result") or {}).get("postData")) or "")
                continue
            if r.get("method") == "Debugger.paused":
                frame = r["params"]["callFrames"][0]["callFrameId"]
                self._send("Debugger.evaluateOnCallFrame", {
                    "callFrameId": frame, "returnByValue": True,
                    "expression": "JSON.stringify({cmd: cmd, json: payload && payload.json})"})
                self._send("Debugger.resume", {})
                continue
            ident = r.get("id")
            if ident is None:
                continue
            value = ((r.get("result") or {}).get("result") or {}).get("value")
            if isinstance(value, str) and value.startswith('{"cmd":'):
                try:
                    call = json.loads(value)
                    if call.get("cmd") == "run_command" and call.get("json"):
                        req = json.loads(call["json"])
                        self.calls.append({"command": req.get("command"), "params": req.get("params") or {}})
                except ValueError:
                    pass
            with self.lock:
                self.answers[ident] = r
                ev = self.waiting.pop(ident, None)
            if ev:
                ev.set()

    def _ipc(self, body: str) -> None:
        """One IPC request's body: `{json: "<the command request>"}`."""
        try:
            outer = json.loads(body)
            req = json.loads(outer["json"]) if isinstance(outer, dict) and isinstance(outer.get("json"), str) else outer
            self.calls.append({"command": req.get("command"), "params": req.get("params") or {}})
        except (ValueError, TypeError, KeyError, AttributeError):
            self.calls.append({"unparsed": body[:200]})

    def call(self, method: str, params: dict) -> dict:
        ident = self._send(method, params)
        with self.lock:
            ev = self.waiting.get(ident)
        if ev and not ev.wait(60):
            raise RuntimeError(f"no answer to {method}")
        with self.lock:
            return self.answers.pop(ident, {})

    def js(self, expr: str):
        r = self.call("Runtime.evaluate", {"expression": expr, "returnByValue": True, "awaitPromise": True})
        res = r.get("result") or {}
        if res.get("exceptionDetails"):
            raise RuntimeError(f"page script failed: {res['exceptionDetails']}")
        return (res.get("result") or {}).get("value")

    def record(self) -> None:
        """Records every `run_command` the page sends from now on: Tauri's IPC is an HTTP request to `ipc.localhost`
        whose body is the call's arguments; the Network domain reports each request with its body (passive: the page
        never waits for the script)."""
        self.call("Network.enable", {"maxPostDataSize": 1048576})


# ----- OS input ------------------------------------------------------------------------------------------------------

u32 = ctypes.windll.user32 if sys.platform == "win32" else None
if u32:
    u32.GetMenu.restype = wintypes.HMENU
    u32.GetSubMenu.restype = wintypes.HMENU


def window_handle() -> int:
    return e6._main_window(e2e.APP_PID["pid"])


def front() -> int:
    h = window_handle()
    u32.ShowWindow(h, 9)
    # D-49 item 5: verified, not assumed (SetForegroundWindow alone can leave another window in front).
    if not c42.ensure_foreground(h):
        raise RuntimeError("the application window is not in the foreground: no input is sent (D-49 item 5)")
    time.sleep(0.15)
    return h


def click_screen(x: int, y: int) -> None:
    u32.SetCursorPos(int(x), int(y))
    time.sleep(0.12)
    e6._send_button(True)
    time.sleep(0.05)
    e6._send_button(False)
    time.sleep(0.3)


def _send_right(down: bool) -> bool:
    """One secondary (right) button press or release through `SendInput`; whether it was injected."""
    class MOUSEINPUT(ctypes.Structure):
        _fields_ = [("dx", wintypes.LONG), ("dy", wintypes.LONG), ("mouseData", wintypes.DWORD),
                    ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD), ("dwExtraInfo", ctypes.c_size_t)]

    class MINPUT(ctypes.Structure):
        _fields_ = [("type", wintypes.DWORD), ("mi", MOUSEINPUT)]

    i = MINPUT(type=0, mi=MOUSEINPUT(0, 0, 0, 0x0008 if down else 0x0010, 0, 0))
    return u32.SendInput(1, ctypes.byref(i), ctypes.sizeof(MINPUT)) == 1


def right_click_screen(x: int, y: int) -> None:
    u32.SetCursorPos(int(x), int(y))
    time.sleep(0.12)
    _send_right(True)
    time.sleep(0.05)
    _send_right(False)
    time.sleep(0.3)


# The rendered menus' declared references (the parity manifest's DOM rows, ruling D-35), by id without the view's
# prefix: the MCP form exercises a context menu's items through them (an agent has no secondary click).
PARITY_DOM = {r["id"].split(".", 1)[1]: r for r in json.loads(
    (Path(__file__).resolve().parent.parent / "desktop/src/ui-bindings/parity.json").read_text(encoding="utf-8"))["dom"]
    if ".drawingMenu." in r["id"] or ".sceneMenu." in r["id"]}


def key(vk: int, *mods: int) -> None:
    for m in mods:
        u32.keybd_event(m, 0, 0, 0)
    c42.os_key(vk)
    for m in reversed(mods):
        u32.keybd_event(m, 0, 2, 0)
    time.sleep(0.1)


if wintypes is not None:
    class KEYBDINPUT(ctypes.Structure):
        _fields_ = [("wVk", wintypes.WORD), ("wScan", wintypes.WORD), ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD),
                    ("dwExtraInfo", ctypes.POINTER(ctypes.c_ulong))]


    class INPUT(ctypes.Structure):
        class _U(ctypes.Union):
            _fields_ = [("ki", KEYBDINPUT), ("pad", ctypes.c_byte * 32)]
        _anonymous_ = ("u",)
        _fields_ = [("type", wintypes.DWORD), ("u", _U)]


def type_text(text: str) -> None:
    """Types `text` through the operating system's keyboard (Unicode key events into the focused control)."""
    for ch in text:
        for flags in (0x0004, 0x0004 | 0x0002):
            i = INPUT(type=1)
            i.ki = KEYBDINPUT(0, ord(ch), flags, 0, None)
            u32.SendInput(1, ctypes.byref(i), ctypes.sizeof(INPUT))
        time.sleep(0.005)


def menu_labels(handle) -> list[str]:
    out = []
    for i in range(u32.GetMenuItemCount(handle)):
        b = ctypes.create_unicode_buffer(256)
        u32.GetMenuStringW(handle, i, b, 256, 0x400)
        out.append(b.value.replace("&", "").split("\t")[0].rstrip("….").strip())
    return out


def native_item(path: list[str]):
    """The native menu item at `path`: (its menu handle, its index), or None."""
    h = window_handle()
    handle = u32.GetMenu(h)
    for depth, label in enumerate(path):
        labels = menu_labels(handle)
        if label not in labels:
            return None
        i = labels.index(label)
        if depth == len(path) - 1:
            return handle, i
        handle = u32.GetSubMenu(handle, i)
    return None


def native_enabled(path: list[str]) -> bool | None:
    found = native_item(path)
    if not found:
        return None
    flags = u32.GetMenuState(found[0], found[1], 0x400)
    return not bool(flags & 3)


def choose_menu(path: list[str]) -> None:
    """Opens each menu of `path` with a real click and clicks the item."""
    h = front()
    handle = u32.GetMenu(h)
    for label in path:
        labels = menu_labels(handle)
        if label not in labels:
            raise RuntimeError(f"native menu label absent: {label} (have {labels})")
        i = labels.index(label)
        rect = wintypes.RECT()
        if not u32.GetMenuItemRect(h, handle, i, ctypes.byref(rect)):
            raise RuntimeError(f"native menu rectangle absent: {label}")
        click_screen((rect.left + rect.right) // 2, (rect.top + rect.bottom) // 2)
        handle = u32.GetSubMenu(handle, i)


# ----- the table -------------------------------------------------------------------------------------------------------

class Table:
    def __init__(self, session, run, out: Path, work: Path):
        self.s, self.run, self.out, self.work = session, run, out, work
        self.cdp: Cdp | None = None
        self.rows: list = []
        self.kinds = self._kinds()

    @staticmethod
    def _kinds() -> dict:
        text = (ROOT / "desktop/src/ipc/generated/commands.ts").read_text(encoding="utf-8")
        out, current = {}, None
        for line in text.splitlines():
            if line.startswith('  "') and line.rstrip().endswith(": {"):
                current = line.strip().split('"')[1]
            elif current and line.strip().startswith('kind: "'):
                out[current] = line.split('"')[1]
                current = None
        return out

    async def cmd(self, n: str, p: dict | None = None) -> dict:
        env = await e2e.run_cmd(self.s, n, p or {})
        res = env.get("result")
        jid = res.get("jobId") if isinstance(res, dict) else None
        if jid and env.get("status") == "ok":
            env = await e2e.call(self.s, "job_wait", {"jobId": jid, "timeoutMs": 120000})
        if env.get("status") != "ok":
            raise RuntimeError(f"setup {n} {p}: {json.dumps(env)[:600]}")
        return env.get("result") if env.get("result") is not None else {}

    async def position(self) -> int:
        return (await self.cmd("edit.history.get", {"limit": 1000})).get("position", 0)

    async def steps_since(self, pos: int) -> list:
        h = await self.cmd("edit.history.get", {"limit": 1000})
        return [(e.get("label"), e.get("source")) for e in (h.get("entries") or [])[pos:h.get("position", 0)]]

    def recorded(self) -> list:
        """The page's commands since `clear`, without the canvas's own record of its back-end measurement (a
        `prefs.set` of `display.canvasBackendMeasured` the canvas writes after its back-end self-test; no person's
        action sends it)."""
        time.sleep(0.2)
        own = lambda c: c.get("command") == "prefs.set" and set((c.get("params") or {}).get("values") or {}) == {"display.canvasBackendMeasured"}  # noqa: E731
        return [c for c in self.cdp.calls if not own(c)]

    def clear(self) -> None:
        self.cdp.calls.clear()

    def rect(self, ident: str):
        """The control's centre in screen pixels, scrolled into view, and whether it is enabled; None when absent."""
        r = self.cdp.js(
            "(() => { const e = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']'); if (!e) return null;"
            " e.scrollIntoView({block: 'center', inline: 'center'}); const b = e.getBoundingClientRect();"
            " if (!b.width) return {hidden: true}; const x = b.x + b.width / 2, y = b.y + b.height / 2;"
            " const top = document.elementFromPoint(x, y);"
            " return {x, y, dpr: window.devicePixelRatio || 1, disabled: !!e.disabled,"
            " reachable: !!top && (top === e || e.contains(top))}; })()" % json.dumps(ident))
        return r

    def click(self, ident: str) -> str:
        r = self.rect(ident)
        if not r or r.get("hidden"):
            raise RuntimeError(f"control absent or hidden: {ident}")
        if not r["reachable"]:
            # Covered at its centre (a palette taller than the window): focused, then pressed with the real keyboard.
            front()
            self.cdp.js("document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']').focus()" % json.dumps(ident))
            key(0x20)
            return "focus, then a real Space key"
        h = front()
        origin = wintypes.POINT(0, 0)
        u32.ClientToScreen(h, ctypes.byref(origin))
        click_screen(origin.x + r["x"] * r["dpr"], origin.y + r["y"] * r["dpr"])
        return "real OS click"

    def pressed(self, ident: str):
        return self.cdp.js("document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']')?.getAttribute('aria-pressed')"
                           % json.dumps(ident))

    async def do(self, action) -> str:
        kind = action[0]
        if kind == "dom":
            return self.click(action[1])
        if kind == "menu":
            key(0x1B)
            key(0x1B)
            enabled = native_enabled(action[1])
            choose_menu(action[1])
            return f"real native menu clicks (the item enabled before: {enabled})"
        if kind == "select":
            # C43B-R2-03: the row's option, by its value; the distance from the chosen option is read from the page.
            moves = self.cdp.js(
                "(() => { const s = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']');"
                " return [...s.options].findIndex(o => o.value === %s) - s.selectedIndex; })()"
                % (json.dumps(action[1]), json.dumps(action[2])))
            self.click(action[1])
            await asyncio.sleep(0.3)
            for _ in range(abs(moves)):
                key(0x28 if moves > 0 else 0x26)
            key(0x0D)
            return f"real click, {abs(moves)} arrow keys and Enter"
        if kind == "context":
            key(0x1B)
            r = self.rect(action[1])
            if not r or r.get("hidden"):
                raise RuntimeError(f"control absent or hidden: {action[1]}")
            if len(action) > 3 and action[3]:
                # M5 C5.1b: a point in a canvas, where the item depends on what lies under the click (the Timeline's
                # cells and ruler): `x` pixels in from the left of `x_of`, at the vertical centre of `y_of`.
                at = self.cdp.js(
                    "(() => { const box = id => document.querySelector('[data-tt-id=' + JSON.stringify(id) + ']')"
                    "?.getBoundingClientRect(); const a = box(%s), b = box(%s); if (!a || !b) return null;"
                    " return {x: a.x + %s, y: b.y + b.height / 2}; })()"
                    % (json.dumps(action[3]["x_of"]), json.dumps(action[3]["y_of"]), json.dumps(action[3]["x"])))
                if not at:
                    raise RuntimeError(f"the point's controls are absent: {action[3]}")
                r = {**r, **at}
            h = front()
            origin = wintypes.POINT(0, 0)
            u32.ClientToScreen(h, ctypes.byref(origin))
            right_click_screen(origin.x + r["x"] * r["dpr"], origin.y + r["y"] * r["dpr"])
            # The item shows enabled once the kernel's availability answered (a wait for a state, ruling D-45; the bound
            # only guards a hang).
            start = time.monotonic()
            state = None
            while time.monotonic() - start < 10.0:
                state = self.cdp.js("document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']')?.getAttribute('aria-disabled') ?? null"
                                    % json.dumps(action[2]))
                if state == "false":
                    break
                await asyncio.sleep(0.1)
            if state != "false":
                # Which menu opened, and its items, if any (a click that landed elsewhere names what it opened).
                shown_menu = self.cdp.js(
                    "(() => { const m = document.querySelector('[role=menu]'); return m ? {id: m.dataset.ttId ?? null,"
                    " items: [...m.querySelectorAll('[data-tt-id]')].map(e => e.dataset.ttId).slice(0, 12)} : null; })()")
                where = None
                if len(action) > 3 and action[3]:
                    where = self.cdp.js(
                        "[...document.querySelectorAll('[data-tt-id=' + JSON.stringify(%s) + ']')].map(e => {"
                        " const b = e.getBoundingClientRect(); return [e.tagName, Math.round(b.x), Math.round(b.y),"
                        " Math.round(b.width), Math.round(b.height), e.closest('[data-tt-id=timeline]') ? 'timeline' : 'other']; })"
                        % json.dumps(action[3]["y_of"]))
                raise RuntimeError(f"menu item absent or disabled after the secondary click at {r.get('x')}, "
                                   f"{r.get('y')}: {action[2]} ({state}); the menu shown: {shown_menu}; the point's "
                                   f"row: {where}")
            # The menu is placed again while the kernel's availability answers change its size (D-60 item 1): the item
            # is clicked once its box reads the same twice in a row (a wait for a state, D-45).
            box = None
            for _ in range(50):
                now = self.cdp.js("(() => { const e = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']');"
                                  " if (!e) return null; const b = e.getBoundingClientRect(); return [b.x, b.y, b.width, b.height]; })()"
                                  % json.dumps(action[2]))
                if now is not None and now == box:
                    break
                box = now
                await asyncio.sleep(0.1)
            how = self.click(action[2])
            return f"a real secondary click on {action[1]}, then {how} on the item"
        if kind == "gesture":
            return await self.gesture(action)
        if kind == "key":
            return await self.press_key(action)
        if kind == "none":
            return "no input"
        raise ValueError(kind)

    # ----- C4.4b: canvas gestures and keys (design notes 3-5). A gesture row's `action` is ("gesture", view, tool,
    # points, modifiers): `points` an async function giving view pixels; the commands it sent are the window's record of
    # what it committed (the canvas summary's `lastGesture`), in both forms. A key row's is ("key", vk, keys).
    async def last_gesture(self, view: str):
        answer = await e2e.call(self.s, "inspect_ui", {"target": "window", "depth": 32})
        root = (answer.get("result") or answer).get("root") or {}
        n = McpTable.node(root, f"{view}.canvas") or {}
        try:
            return json.loads(n.get("value") or "{}").get("lastGesture")
        except json.JSONDecodeError:
            return None

    async def gesture(self, action) -> str:
        _, view, tool, points, mods = action
        before = json.dumps(await self.last_gesture(view))
        pts = await points()
        how = await self.input_gesture(view, tool, pts, mods)
        # A wait for a state (D-45): the window's record of the committed gesture; the bound only guards a hang.
        start = time.monotonic()
        rec = None
        answer = getattr(self, "gesture_answer", None)
        # An agent's gesture the kernel refused before the window played it answers here, not in the window's record.
        bound = 5.0 if (answer or {}).get("status") == "error" else 20.0
        while time.monotonic() - start < bound:
            now = await self.last_gesture(view)
            if now is not None and json.dumps(now) != before:
                rec = now
                break
            await asyncio.sleep(0.25)
        if rec is None and answer is not None:
            res = answer.get("result") or {}
            err = answer.get("error") or {}
            cmds = res.get("commands") or ([{"commandId": (err.get("details") or {}).get("gestureCommandId"), "params": {}}]
                                           if (err.get("details") or {}).get("gestureCommandId") else [])
            rec = {"commands": cmds, "code": err.get("code"), "status": answer.get("status"), "source": "the pointer answer",
                   "answer": {"error": err, "result": res}}
        self.gesture_record = rec
        self.gesture_answer = None
        return how

    async def input_gesture(self, view: str, tool: str, pts: list, mods: list) -> str:
        """Real OS input: a left drag through the canvas's view pixels, the modifiers held (Windows)."""
        box = self.cdp.js("(() => { const e = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']');"
                          " if (!e) return null; const b = e.getBoundingClientRect();"
                          " return {x: b.x, y: b.y, dpr: window.devicePixelRatio || 1}; })()" % json.dumps(f"{view}.canvas"))
        if not box:
            raise RuntimeError(f"no canvas {view}")
        client = [((box["x"] + q[0]) * box["dpr"], (box["y"] + q[1]) * box["dpr"]) for q in pts]
        vks = [{"alt": 0x12, "shift": 0x10, "ctrl": 0x11}[m] for m in mods]
        for vk in vks:
            u32.keybd_event(vk, 0, 0, 0)
        times: dict = {}
        try:
            e6.os_drag(e2e.APP_PID["pid"], client, 0.03 * max(1, len(client) - 1), 0.05, times)
        finally:
            for vk in reversed(vks):
                u32.keybd_event(vk, 0, 2, 0)
        if "error" in times:
            raise RuntimeError(times["error"])
        return f"a real drag through {len(pts)} points" + (f" with {'+'.join(mods)} held" if mods else "")

    async def press_key(self, action) -> str:
        # ("key", vk, keys) or ("key", vk, keys, view): the canvas focused before the key (D-83 item 3: the Camera view).
        vk, keys = action[1], action[2]
        view = action[3] if len(action) > 3 else "drawingView"
        front()
        self.cdp.js("document.querySelector('[data-tt-id=\"%s.canvas\"]')?.focus()" % view)
        key(vk)
        return f"the real key {keys}"

    async def row(self, r: dict) -> None:
        name = r["name"]
        rec = {"name": name, "spec": r["spec"], "action": [] if callable(r["action"]) else list(r["action"])}
        try:
            await self.settle_dialogs()
            if r.get("setup"):
                await r["setup"]()
            # An action known only after the setup (C4.4a: a pen's id) is resolved here.
            if callable(r["action"]):
                r = {**r, "action": r["action"]()}
                rec["action"] = list(r["action"])
            await asyncio.sleep(0.3)
            clicks = 2 if r.get("toggle") else 1
            rec["clicks"] = []
            ok_all = True
            for k in range(clicks):
                before = await r["state"]()
                want = r["expect"](before) if callable(r["expect"]) else r["expect"]
                pos = await self.position()
                await self.mark()
                self.want = want
                self.gesture_record = None
                how = await self.do(r["action"])
                await asyncio.sleep(r.get("wait", 0.8))
                sent = await self.observed(want)
                after = await r["state"]()
                steps = await self.steps_since(pos)
                label = r.get("label")
                expect_steps = [] if label is None else [[label, self.WHO]]
                got_steps = [list(s) for s in steps]
                effect = bool(r["effect"](before, after, k))
                # D-61: an expectation of None is a refused entry: nothing may be sent.
                expected = [] if want is None else [{"command": want[0], "params": want[1]}]
                click = {"input": how, "expected": expected[0] if expected else None, "sent": sent,
                         **({"gesture": self.gesture_record} if self.gesture_record is not None else {}),
                         "effect": effect, "steps": got_steps, "expectedSteps": expect_steps,
                         "before": before, "after": after}
                exact = sent == expected
                if r.get("match"):
                    # A gesture's parameters come from the person's samples: the spec's statement is a predicate
                    # (the command, and its parameters derived from the press and release; design notes 3-5).
                    exact = (len(sent) == 1 and want is not None and sent[0].get("command") == want[0]
                             and bool(r["match"](sent[0].get("params") or {}, before)))
                    click["match"] = exact
                if self.WHO == "agent" and not r.get("match") and r["action"][0] not in ("key", "gesture"):
                    click["declared"] = self.declared
                    exact = exact and self.declared == (expected[0] if expected else None)
                    self.shown_after = None
                    shown = await self.shows(r, after)
                    click["shown"] = shown
                    click["shownAfter"] = self.shown_after
                    exact = exact and shown is not False
                ok = exact and effect and got_steps == expect_steps
                click["ok"] = ok
                rec["clicks"].append(click)
                ok_all = ok_all and ok
            rec["ok"] = ok_all
            self.run.check(ok_all, f"D-39 {name}: {self.CLAIM} ({r['spec']})",
                           [{k: c[k] for k in ("expected", "sent", "effect", "steps", "expectedSteps")} for c in rec["clicks"]])
            if r.get("after"):
                await r["after"]()
        except Exception as ex:  # noqa: BLE001 - the row fails with its text, the table goes on
            rec["ok"] = False
            rec["error"] = repr(ex)
            self.run.check(False, f"D-39 {name}: ran", repr(ex))
            await self.after_failure()
        rec["disabledTarget"] = r.get("disabled")
        self.rows.append(rec)
        self.run.save("rows.json", self.rows)

    # The OS form's hooks: real input, the page's IPC requests observed.
    WHO = "user"
    CLAIM = "the window sends exactly the command, changes the state and makes the spec's undo steps"

    async def shows(self, r: dict, after) -> bool | None:
        """In the MCP form: whether the window's tree shows the state after the call (None: nothing to show)."""
        return None

    async def settle_dialogs(self) -> None:
        # A dialog a row before left open (a refusal's message box) would hold every later command.
        for _ in range(3):
            if not self.cdp.js("!!document.querySelector('[role=alertdialog],[role=dialog][data-tt-id]:not([data-tt-id=startScreen])')"):
                break
            key(0x1B)
            await asyncio.sleep(0.3)

    async def mark(self) -> None:
        self.clear()

    #: References the page sends as reads (M5 C5.1b): `timeline.view.set {}` answers the Timeline's view (SI-210), which
    #: the Timeline panel reads after every change; no person's action.
    PAGE_READS = (("timeline.view.set", {}),)

    async def observed(self, want) -> list:
        if self.gesture_record is not None:
            return self.gesture_commands()
        wanted = want[0] if want else None
        read = lambda c: (c.get("command"), c.get("params") or {}) in self.PAGE_READS  # noqa: E731
        return [c for c in self.recorded() if not read(c)
                and (self.kinds.get(c.get("command")) != "query" or c.get("command") == wanted)]

    gesture_record = None
    #: The commands that choose a tool: not the gesture's (the person chose the tool before; an agent's `pointer` names it).
    TOOL_CHOICE = ("tool.active.set", "anim.tool.set")

    def gesture_commands(self) -> list:
        rec = self.gesture_record or {}
        return [{"command": c.get("commandId"), "params": c.get("params") or {}} for c in rec.get("commands") or []
                if c.get("commandId") not in self.TOOL_CHOICE]

    async def after_failure(self) -> None:
        with contextlib.suppress(Exception):
            key(0x1B)
            key(0x1B)

    async def disabled_checks(self) -> None:
        """After the project is closed (`project.none` refuses every control of the table): each is disabled or gone,
        a real click on it sends nothing and changes nothing; each native menu item is disabled in the menu and in the
        window tree (D-38)."""
        # A wait for the state (D-45), bounded, as the MCP form's: every target gone or disabled in the window's tree
        # (the window re-reads the availability after the close), then each is judged.
        async def read_tree() -> dict:
            answer = await e2e.call(self.s, "inspect_ui", {"target": "window", "depth": 32})
            return (answer.get("result") or answer).get("root") or {}

        targets = [r.get("disabledTarget") for r in self.rows if r.get("disabledTarget")]

        def settled(t: dict) -> bool:
            for target in targets:
                n = McpTable.menu_node(t, target[1]) if target[0] == "menu" else McpTable.node(t, target[1])
                if n is not None and n.get("enabled") is not False:
                    return False
            return True

        tree = await read_tree()
        start = time.monotonic()
        while not settled(tree) and time.monotonic() - start < 20.0:
            await asyncio.sleep(0.25)
            tree = await read_tree()
        bar = next((c for c in tree.get("children") or [] if c.get("id") == "menuBar"), {})

        def menu_node(path):
            node = bar
            for label in path:
                node = next((c for c in node.get("children") or []
                             if (c.get("label") or "").rstrip("….").strip() == label), None)
                if not node:
                    return None
            return node

        seen = set()
        for rec in self.rows:
            target = rec.get("disabledTarget")
            if not target or json.dumps(target) in seen:
                continue
            seen.add(json.dumps(target))
            out = {"target": target}
            if target[0] == "menu":
                out["nativeEnabled"] = native_enabled(target[1])
                node = menu_node(target[1])
                out["treeEnabled"] = node.get("enabled") if node else None
                ok = out["nativeEnabled"] is False and out["treeEnabled"] is False
            else:
                r = self.rect(target[1])
                out["control"] = r
                if not r or r.get("hidden"):
                    ok = True
                    out["note"] = "gone with the project"
                else:
                    self.clear()
                    before = await self.cmd("app.state.get")
                    with contextlib.suppress(RuntimeError):
                        self.click(target[1])
                    await asyncio.sleep(0.4)
                    sent = [c for c in self.recorded() if self.kinds.get(c.get("command")) != "query"]
                    out["sent"] = sent
                    out["unchanged"] = before == await self.cmd("app.state.get")
                    ok = r.get("disabled") is True and not sent and out["unchanged"]
            out["ok"] = ok
            rec["disabled"] = out
            self.run.check(ok, f"D-39 {rec['name']}: disabled while the precheck refuses (no project, 01 §1.3 lines 71-75)", out)
        self.run.save("rows.json", self.rows)


# The table's own row runners: a probe that replaces them runs a chosen subset of the rows, for which the coverage check
# at the end (D-59 item 2) does not apply.
_OWN_TABLE_ROW = Table.row


class McpTable(Table):
    """The CI form (ruling D-40): the attached MCP interface the other e2e scripts use. `inspect_ui` finds each control
    and reads the reference it declares (the command and the parameters its click sends: every control of the table
    declares them, and the menu bar's items carry the parameters a click runs, D-38); the row checks that reference
    against the spec's expectation, then invokes it through the application's command entry point (`key` with the
    item's shortcut where it has one), and observes the call in the activity record and its effect through document
    reads; after the call the window's tree must show the new state (an agent's change reflected). After the project is
    closed, every control is disabled or gone in the window's tree. Rows whose action is the operating system's own UI
    (a native file dialog) or a dialog only the native menu opens are named, with the reason, and run in the OS form."""

    WHO = "agent"
    CLAIM = "the window declares exactly the command, which changes the state and makes the spec's undo steps"

    def __init__(self, *a, **k):
        super().__init__(*a, **k)
        self.seq = 0
        self.declared = None

    async def tree(self) -> dict:
        answer = await e2e.call(self.s, "inspect_ui", {"target": "window", "depth": 32})
        return (answer.get("result") or answer).get("root") or {}

    @staticmethod
    def node(tree: dict, ident: str):
        stack = [tree]
        while stack:
            n = stack.pop()
            if n.get("id") == ident:
                return n
            stack.extend(n.get("children") or [])
        return None

    @staticmethod
    def menu_node(tree: dict, path: list):
        node = next((c for c in tree.get("children") or [] if c.get("id") == "menuBar"), {})
        for label in path:
            node = next((c for c in node.get("children") or [] if (c.get("label") or "").rstrip("\u2026.").strip() == label), None)
            if not node:
                return None
        return node

    async def settle_dialogs(self) -> None:
        return None

    async def after_failure(self) -> None:
        return None

    async def shows(self, r: dict, after) -> bool | None:
        """The window shows the state the call made: a toggle's pressed state, a list's value, the tools palette's
        pressed tool, the view's type (the agent's change reflected, C43B-R1S-09). Rows without a shown state: None."""
        show = r.get("shows")
        if not show:
            return None
        # A wait for a state, not a duration (ruling D-45): the window reports its tree after it has rendered the
        # kernel's event, and the row waits until the tree shows the state. The bound only guards a hang: 3 s was a
        # duration a macOS runner exceeded once (run 43867592907, tools palette › editTexture: command, effect and
        # steps right, the pressed state not yet reported), so a state never shown fails after 20 s. How long the
        # state took and the tree's rendered event numbers are recorded with the row.
        start = time.monotonic()
        seqs = []
        while True:
            await asyncio.sleep(0.25)
            answer = await e2e.call(self.s, "inspect_ui", {"target": "window", "depth": 32})
            res = answer.get("result") or answer
            seqs.append(res.get("renderedSeq"))
            n = self.node(res.get("root") or {}, show[0])
            if n is not None and show[1](n, after):
                self.shown_after = {"seconds": round(time.monotonic() - start, 2), "renderedSeq": seqs[-3:]}
                return True
            if time.monotonic() - start > 20.0:
                self.shown_after = {"seconds": None, "renderedSeq": seqs[-5:]}
                return False

    async def activity(self) -> list:
        res = (await e2e.run_cmd(self.s, "agent.activity.list", {"limit": 50})).get("result")
        return res if isinstance(res, list) else (res or {}).get("entries") or []

    async def mark(self) -> None:
        self.seq = max([e.get("seq", 0) for e in await self.activity()] or [0])

    async def observed(self, want) -> list:
        """The calls this row's action made, from the activity record (the declared reference's, or the key's)."""
        if self.gesture_record is not None:
            return self.gesture_commands()
        out = []
        for e in sorted(await self.activity(), key=lambda e: e.get("seq", 0)):
            if e.get("seq", 0) <= self.seq or e.get("tool") not in ("run_command", "key"):
                continue
            # A key whose command refused (`command.disabled`) sent nothing (C41-R2-09).
            if e.get("tool") == "key" and e.get("status") == "error":
                continue
            if self.kinds.get(e.get("commandId")) == "query" and e.get("commandId") != (want[0] if want else None):
                continue
            out.append({"command": e.get("commandId"), "params": e.get("params") or {}})
        return out

    async def do(self, action) -> str:
        kind = action[0]
        tree = await self.tree()
        if kind == "dom":
            n = self.node(tree, action[1])
            if not n:
                raise RuntimeError(f"control absent from the window's tree: {action[1]}")
            if n.get("enabled") is False:
                raise RuntimeError(f"control disabled in the window's tree: {action[1]}")
            ref = n.get("command") or {}
            self.declared = {"command": ref.get("commandId"), "params": ref.get("params") or {}}
        elif kind == "menu":
            # The menu (and the tree that carries it) refreshes after the events that change availability, a moment
            # after the setup's commands answered: the item is read again until it shows enabled (a wait for a state,
            # ruling D-45; the 20 s bound only guards a hang).
            n = self.menu_node(tree, action[1])
            start = time.monotonic()
            while not (n and n.get("enabled") is True) and time.monotonic() - start < 20.0:
                await asyncio.sleep(0.25)
                n = self.menu_node(await self.tree(), action[1])
            if not n:
                raise RuntimeError(f"menu item absent from the window's tree: {action[1]}")
            if n.get("enabled") is False:
                raise RuntimeError(f"menu item disabled in the window's tree: {action[1]}")
            ref = n.get("command") or {}
            self.declared = {"command": ref.get("commandId"), "params": ref.get("params") or {}}
            if n.get("shortcut") and self.declared == {"command": self.want[0], "params": self.want[1]}:
                env = await e2e.call(self.s, "key", {"keys": n["shortcut"]})
                return f"the menu item's shortcut {n['shortcut']} through `key` ({env.get('status')})"
        elif kind == "select":
            # C43B-R2-03: every option of a list is a child node with its label, selected state and the reference
            # choosing it sends; the row checks that declared reference against the spec's expectation.
            n = self.node(tree, action[1])
            if not n or n.get("enabled") is False:
                raise RuntimeError(f"list absent or disabled in the window's tree: {action[1]}")
            o = next((c for c in n.get("children") or [] if c.get("role") == "option" and c.get("value") == action[2]), None)
            if not o:
                raise RuntimeError(f"option {action[2]!r} absent from the list's node: {action[1]}")
            ref = o.get("command") or {}
            self.declared = {"command": ref.get("commandId"), "params": ref.get("params") or {}}
        elif kind == "context" and action[2].startswith("timeline."):
            # M5 ruling E-48: the Timeline's context menus through the keyboard path an agent has, the `key` tool's
            # Shift+F10 on the surface (11 §7.6; D-66 item 3). A keyboard-opened menu acts on the current cell, so the
            # current cell is set to the row and frame the OS form clicks, then put back once the menu's item has been
            # read; the item's live reference (the clicked row's ids, the kernel's completed toggle state) is read from
            # the window's tree and run, as the other MCP rows run what their control declares.
            app = await e2e.run_cmd(self.s, "app.state.get", {})
            was = (app.get("result") or {}) if isinstance(app, dict) else {}
            where = action[3] if len(action) > 3 else None
            if where:
                view = (await e2e.run_cmd(self.s, "timeline.view.set", {})).get("result") or {}
                z = TL_LAYOUT["zoomLevels"][min(int(view.get("zoomLevel", 6)), len(TL_LAYOUT["zoomLevels"]) - 1)]
                frame = int(view.get("firstFrame", 1)) + int(where["x"] // max(1, z["cellWidth"])) * max(
                    1, z.get("framesPerCell", 1))
                row_of = where.get("y_of", "")
                if row_of.startswith("timeline.row."):
                    await e2e.run_cmd(self.s, "xsheet.currentCell.set",
                                      {"elementId": row_of[len("timeline.row."):], "frame": frame})
                else:
                    await e2e.run_cmd(self.s, "timeline.currentFrame.set", {"frame": frame})
            # The menu is built from the rows the window has read: wait until it has read them at the document's present
            # revision (the setup's changes; a wait for a state, D-45: the menu opened at once once named a key the
            # window had not read yet).
            rev = (await e2e.run_cmd(self.s, "doc.state.get", {})).get("documentRev") or 0
            start = time.monotonic()
            while time.monotonic() - start < 20.0:
                shown_rev = (self.node(await self.tree(), "timeline.tracks") or {}).get("value")
                if str(shown_rev).isdigit() and int(shown_rev) >= int(rev):
                    break
                await asyncio.sleep(0.2)
            pressed = await e2e.call(self.s, "key", {"keys": "shift+F10", "view": action[1]})
            # The item is shown enabled once its availability has answered. The window tree marks a disabled control
            # `enabled: false` and leaves the field out for an enabled one: waiting for `enabled: true` waited the whole
            # bound at every row (46 minutes of the Windows `desktop gui` job in run 45400525009, past its 65).
            n = self.node(await self.tree(), action[2])
            start = time.monotonic()
            while not (n and n.get("enabled") is not False) and time.monotonic() - start < 20.0:
                await asyncio.sleep(0.25)
                n = self.node(await self.tree(), action[2])
            if where:
                element = (was.get("element") or {}).get("id") if isinstance(was.get("element"), dict) else None
                if element and was.get("frame"):
                    await e2e.run_cmd(self.s, "xsheet.currentCell.set", {"elementId": element, "frame": was["frame"]})
                elif was.get("frame"):
                    await e2e.run_cmd(self.s, "timeline.currentFrame.set", {"frame": was["frame"]})
            if not n:
                # The key's answer in full (its code and details: run 45471231219 named only the status).
                err = pressed.get("error") or {}
                raise RuntimeError(f"menu item absent from the window's tree after Shift+F10 "
                                   f"({pressed.get('status')} {err.get('code')}: {err.get('message')} "
                                   f"{json.dumps(err.get('details'))[:300]}): {action[2]}")
            if n.get("enabled") is False:
                raise RuntimeError(f"menu item disabled in the window's tree: {action[2]}")
            ref = n.get("command") or {}
            self.declared = {"command": ref.get("commandId"), "params": ref.get("params") or {}}
            # The harness's own calls (the current cell set and put back, the key) are not the row's: the record of
            # what the row sends starts after them.
            await self.mark()
        elif kind == "context":
            # An agent has no secondary click: the item is exercised through the reference the rendered menu declares
            # for it (the parity manifest's DOM row, collected from the same menu; design note 1 §9, plan §4.10).
            row = PARITY_DOM.get(action[2].split(".", 1)[1])
            if not row:
                raise RuntimeError(f"menu item absent from the parity manifest: {action[2]}")
            self.declared = {"command": row["commands"][0], "params": row.get("params") or {}}
        elif kind == "gesture":
            return await self.gesture(action)
        elif kind == "none":
            self.declared = None
            return "no input"
        elif kind == "key":
            # The agent's `key` tool: the one key dispatch a real key takes (11 §7.6).
            env = await e2e.call(self.s, "key", {"keys": action[2]})
            self.key_answer = env
            return f"the key tool ({env.get('status')}, {(env.get('error') or {}).get('code')})"
        else:
            raise RuntimeError("an action the MCP form does not perform")
        env = await e2e.run_cmd(self.s, self.declared["command"], self.declared["params"])
        return f"the declared reference through run_command ({env.get('status')})"

    async def input_gesture(self, view: str, tool: str, pts: list, mods: list) -> str:
        """An agent's `pointer` gesture, played by the window showing the view (design note 2 §6)."""
        events = []
        for i, (x, y) in enumerate(pts):
            ty = "down" if i == 0 else "up" if i == len(pts) - 1 else "move"
            events.append({"type": ty, "x": x, "y": y, "modifiers": mods})
        env = await e2e.call(self.s, "pointer", {"view": view, "tool": tool, "events": events})
        self.gesture_answer = env
        return f"an agent's pointer gesture played by the window ({env.get('status')})"

    async def disabled_checks(self) -> None:
        # A wait for the state (D-45), bounded: the window re-reads the availability after the close and reports its
        # tree a frame or two later. Run 45490862349 (Windows) judged one tree read 1.5 s after the close, before the
        # Timeline toolbar's Add and Show All had been asked again.
        targets = [r.get("disabledTarget") for r in self.rows if r.get("disabledTarget")]

        def settled(t: dict) -> bool:
            for target in targets:
                n = self.menu_node(t, target[1]) if target[0] == "menu" else self.node(t, target[1])
                if n is not None and n.get("enabled") is not False:
                    return False
            return True

        tree = await self.tree()
        start = time.monotonic()
        while not settled(tree) and time.monotonic() - start < 20.0:
            await asyncio.sleep(0.25)
            tree = await self.tree()
        seen = set()
        for rec in self.rows:
            target = rec.get("disabledTarget")
            if not target or json.dumps(target) in seen:
                continue
            seen.add(json.dumps(target))
            n = self.menu_node(tree, target[1]) if target[0] == "menu" else self.node(tree, target[1])
            out = {"target": target, "present": n is not None, "enabled": (n or {}).get("enabled")}
            ok = n is None or n.get("enabled") is False
            if n is None:
                out["note"] = "gone with the project"
            out["ok"] = ok
            rec["disabled"] = out
            self.run.check(ok, f"D-39 {rec['name']}: disabled in the window's tree while the precheck refuses (no project, 01 §1.3 lines 71-75)", out)
        self.run.save("rows.json", self.rows)


# ----- the rows --------------------------------------------------------------------------------------------------------

async def build_and_run(t: Table, only: set) -> None:
    cmd = t.cmd
    await cmd("prefs.set", {"values": {"files.backups.count": 5}})
    await cmd("project.create", {"name": "D39", "onUnsaved": "fail"})
    project = t.work / "D39.animpkg"
    await cmd("project.saveAs", {"path": str(project), "onExists": "overwrite"})
    bob = ((await cmd("element.create", {"type": "drawing", "name": "Bob"})).get("elementIds") or [None])[0]
    await cmd("element.drawing.create", {"elementId": bob, "name": "1", "exposeFrom": 1, "exposeTo": 24})
    await cmd("app.context.set", {"elementId": bob, "frame": 1})
    await cmd("workspace.select", {"name": "Default"})
    for p in ("drawingView", "cameraView"):
        await cmd("window.panel.set", {"panelId": p, "visible": True})
    await asyncio.sleep(1.0)
    if not isinstance(t, McpTable):
        t.cdp = Cdp(CDP_PORT)
        t.cdp.record()
        t.clear()
        # The recorder sees the page's calls: one read-only call made by the page itself.
        t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command', {json: JSON.stringify({command: 'app.info.get', params: {}})}).then(() => true)")
        t.run.check(any(c.get("command") == "app.info.get" for c in t.recorded()), "the IPC recorder sees the page's commands", t.recorded())

    async def drawing():
        return await cmd("draw.drawing.get", {})

    def nodes_of(d):
        out = []

        def walk(ns):
            for n in ns or []:
                out.append(n)
                walk(n.get("children"))
        walk(d.get("nodes"))
        return out

    def strokes_of(d):
        return [s for n in nodes_of(d) for s in n.get("strokes") or []]

    def ids_in_order(d):
        return [s["id"] for s in strokes_of(d)] + [n["id"] for n in nodes_of(d)]

    async def clean():
        # The drawing back to empty and unlocked, whatever the row before left (each step tolerated when it has
        # nothing to do).
        await e2e.run_cmd(t.s, "draw.lock.set", {"locked": False})
        d = await drawing()
        ids = [n["id"] for n in d.get("nodes") or []]
        if ids:
            await e2e.run_cmd(t.s, "draw.object.delete", {"ids": ids})
        await cmd("draw.selection.set", {"by": "none"})

    async def rect(x, y, layer="new"):
        return await cmd("draw.rectangle.add", {"from": {"x": x - 40, "y": y - 30}, "to": {"x": x + 40, "y": y + 30}, "layer": layer})

    async def select_all():
        await cmd("draw.selection.set", {"by": "all"})

    def setup(*steps):
        async def run():
            await clean()
            for s in steps:
                await s()
        return run

    def changed(before, after, _k):
        return before != after

    async def focus_view(view):
        await cmd("window.panel.set", {"panelId": view, "visible": True, "focus": True})
        await cmd("view.active.set", {"viewId": view})

    rows: list = []

    # 1. The Drawing menu (02 §4.4 lines 293-296; the commands' rows and labels in 02 §13.5).
    def menu_row(label, command, params, spec_label, steps, effect, path=None, item=None):
        rows.append({"name": f"Drawing › {label}", "spec": f"{spec_line(command)} (label \"{spec_label}\"); 02 §4.4 lines 293-296",
                     "action": ("menu", path or ["Drawing", label]), "expect": (command, params), "label": spec_label,
                     "setup": setup(*steps), "state": drawing, "effect": effect, "disabled": ("menu", path or ["Drawing", label])})
        if item:
            # C4.4a (design note 1 §9): the Drawing view's context menu sends the main menu's command (02 §4.4 line 296).
            rows.append({"name": f"Drawing view context menu › {label}",
                         "spec": f"{spec_line(command)} (label \"{spec_label}\"); 02 §4.4 lines 291-296",
                         "action": ("context", "drawingView.canvas", f"drawingView.drawingMenu.{item}"),
                         "expect": (command, params), "label": spec_label,
                         "setup": setup(lambda: focus_view("drawingView"), *steps), "state": drawing, "effect": effect,
                         "disabled": None})

    two = [lambda: rect(-150, 0), lambda: rect(150, 0), select_all]
    layers = lambda d: [n for n in nodes_of(d) if n.get("type") == "layer" and n.get("strokes")]  # noqa: E731
    menu_row("Flatten", "draw.flatten.apply", {}, "Flatten Drawing", two, lambda b, a, k: len(layers(a)) < len(layers(b)), item="flatten")
    menu_row("Optimize", "draw.optimize.apply", {}, "Optimize Drawing", two, lambda b, a, k: len(layers(a)) < len(layers(b)), item="optimize")

    async def wobbly():
        pts = [{"x": -200 + 10 * i, "y": (i % 2) * 0.2} for i in range(40)]
        await cmd("draw.stroke.add", {"kind": "pencil", "points": pts, "fit": False, "layer": "new"})
    anchors = lambda d: sum(len(s.get("bezier") or []) for s in strokes_of(d))  # noqa: E731
    menu_row("Smooth", "draw.smooth.apply", {}, "Smooth Selection", [wobbly, select_all], lambda b, a, k: anchors(a) < anchors(b), item="smooth")
    kinds = lambda d: sorted(s.get("kind") for s in strokes_of(d))  # noqa: E731
    menu_row("Convert Lines to Brush", "draw.linesToBrush.apply", {}, "Convert Lines to Brush",
             [lambda: rect(0, 0), select_all], lambda b, a, k: "brush" in kinds(a) and "brush" not in kinds(b), item="linesToBrush")

    async def text():
        await cmd("draw.text.add", {"position": {"x": -60, "y": 0}, "text": "Ab"})
    menu_row("Break Text Apart", "draw.breakApart.apply", {}, "Break Text Apart", [text, select_all],
             lambda b, a, k: any(n.get("type") == "text" for n in nodes_of(b)) and not any(n.get("type") == "text" for n in nodes_of(a)), item="breakApart")

    async def brush():
        await cmd("draw.stroke.add", {"kind": "brush", "points": [{"x": -100, "y": 0}, {"x": 0, "y": 40}, {"x": 100, "y": 0}], "layer": "new"})
    menu_row("Extract Centre Line", "draw.centreLine.apply", {}, "Extract Centre Line", [brush, select_all],
             lambda b, a, k: "pencil" in kinds(a) and "pencil" not in kinds(b), item="centreLine")

    async def select_first():
        d = await drawing()
        await cmd("draw.selection.set", {"by": "ids", "ids": [strokes_of(d)[0]["id"]]})

    async def select_last():
        d = await drawing()
        await cmd("draw.selection.set", {"by": "ids", "ids": [strokes_of(d)[-1]["id"]]})
    order = lambda b, a, k: ids_in_order(a) != ids_in_order(b)  # noqa: E731
    for label, to, sel in (("Bring to Front", "front", select_first), ("Bring Forward", "forward", select_first),
                           ("Send Backward", "backward", select_last), ("Send to Back", "back", select_last)):
        menu_row(label, "draw.arrange.apply", {"to": to}, label, [lambda: rect(-30, 0), lambda: rect(30, 10), sel], order,
                 path=["Drawing", "Arrange", label], item=f"arrange.{to}")
    geometry = lambda b, a, k: [s.get("bezier") for s in strokes_of(a)] != [s.get("bezier") for s in strokes_of(b)]  # noqa: E731
    one = [lambda: rect(120, 60), select_all]
    menu_row("Flip Horizontal", "draw.flip.apply", {"axis": "horizontal"}, "Flip Horizontal", one, geometry, item="flip.horizontal")
    menu_row("Flip Vertical", "draw.flip.apply", {"axis": "vertical"}, "Flip Vertical", one, geometry, item="flip.vertical")
    menu_row("Rotate 90° CW", "draw.rotate.apply", {"by": "90cw"}, "Rotate 90° CW", one, geometry, item="rotate.90cw")
    menu_row("Rotate 90° CCW", "draw.rotate.apply", {"by": "90ccw"}, "Rotate 90° CCW", one, geometry, item="rotate.90ccw")
    menu_row("Rotate 180°", "draw.rotate.apply", {"by": "180"}, "Rotate 180°", one, geometry, item="rotate.180")
    groups = lambda d: [n for n in nodes_of(d) if n.get("type") == "group"]  # noqa: E731
    menu_row("Group", "draw.group.create", {}, "Group Selection", two, lambda b, a, k: len(groups(a)) == len(groups(b)) + 1, item="group")

    async def grouped():
        d = await drawing()
        g = await cmd("draw.group.create", {"ids": [s["id"] for s in strokes_of(d)]})
        # The group itself selected (a person's click on it; `by: all` selects its strokes, not the group).
        await cmd("draw.selection.set", {"by": "ids", "ids": [g["groupId"]]})
    menu_row("Ungroup", "draw.group.delete", {}, "Ungroup Selection", two + [grouped],
             lambda b, a, k: len(groups(a)) == len(groups(b)) - 1, item="ungroup")
    menu_row("Delete Objects", "draw.object.delete", {}, "Delete Object", one, lambda b, a, k: len(strokes_of(a)) < len(strokes_of(b)))
    menu_row("Lock Drawing", "draw.lock.set", {"locked": True}, "Lock Drawing", [lambda: rect(0, 0)],
             lambda b, a, k: a.get("locked") is True and b.get("locked") is not True)

    async def locked():
        await rect(0, 0)
        await cmd("draw.lock.set", {"locked": True})
    rows.append({"name": "Drawing › Unlock Drawing", "spec": f"{spec_line('draw.lock.set')} (label \"Unlock Drawing\")",
                 "action": ("menu", ["Drawing", "Unlock Drawing"]), "expect": ("draw.lock.set", {"locked": False}),
                 "label": "Unlock Drawing", "setup": setup(locked), "state": drawing,
                 "effect": lambda b, a, k: b.get("locked") is True and a.get("locked") is False,
                 "disabled": ("menu", ["Drawing", "Unlock Drawing"])})

    # 2. Edit › Cut Special (01 §12 line 1099): the cells of the selection with the timeline edit options.
    async def drawing_cell():
        # Working in the Drawing view: the edit context is the focused panel's (01 §6.4).
        await focus_view("drawingView")
        await rect(0, 0)
        await select_all()
        sel = await cmd("draw.selection.get", {})
        await cmd("edit.selection.set", {"context": "drawing", "ids": sel.get("selectedIds") or sel.get("ids") or []})
    rows.append({"name": "Edit › Cut Special", "spec": f"{spec_line('edit.cutSpecial')} (label \"Cut Special\")",
                 "action": ("menu", ["Edit", "Cut Special"]), "expect": ("edit.cutSpecial", {}), "label": "Cut Special",
                 "setup": setup(drawing_cell), "state": drawing, "effect": changed, "disabled": ("menu", ["Edit", "Cut Special"])})

    # 3. File › Project Info, Check Project; Help › About: each a dialog showing its query's answer (no undo step).
    async def dialog_text():
        return t.cdp.js("Array.from(document.querySelectorAll('[role=dialog]')).map(d => d.getAttribute('data-tt-id') + ':' + d.textContent).join('|')") or ""

    async def close_dialogs():
        if isinstance(t, McpTable):
            return
        for _ in range(2):
            key(0x1B)
            await asyncio.sleep(0.2)

    async def answers():
        """In the MCP form: the activity record's calls (the query's answer stands for the dialog that shows it)."""
        return [(e.get("seq"), e.get("commandId"), e.get("status")) for e in await t.activity()]
    for label, command, dlg, path in (("Project Info", "project.info.get", "projectInfo", ["File", "Project Info"]),
                                      ("Check Project", "doc.validate", "checkProject", ["File", "Check Project"]),
                                      ("About Toontown", "app.info.get", "about", ["Help", "About Toontown"])):
        if isinstance(t, McpTable):
            state, effect = answers, (lambda c: lambda b, a, k: any(x[1] == c and x[2] == "ok" for x in a if x not in b))(command)
        else:
            state, effect = dialog_text, (lambda d: lambda b, a, k: (d + ":") in a and len(a) > len(d) + 1)(dlg)
        rows.append({"name": f"{path[0]} › {label}", "spec": f"{spec_line(command)}; 01 §12.2",
                     "action": ("menu", path), "expect": (command, {}), "label": None, "setup": None, "state": state,
                     "effect": effect, "after": close_dialogs, "disabled": ("menu", path) if command != "app.info.get" else None})

    # 4. The main toolbar's toggles and group items (01 §5.7; 02 §10), each toggle clicked twice.
    async def aids():
        return await cmd("drawAid.state.get", {})

    def toggle_row(ident, command, field, read, spec):
        rows.append({"name": f"main toolbar › {ident}", "spec": f"{spec_line(command)}; {spec}", "action": ("dom", f"mainToolbar.{ident}"),
                     "expect": lambda b: (command, {field: not read(b)}), "label": None, "setup": None, "state": aids, "toggle": True,
                     "effect": lambda b, a, k: read(a) == (not read(b)), "disabled": ("dom", f"mainToolbar.{ident}"),
                     "shows": (f"mainToolbar.{ident}", lambda n, a: n.get("checked") == read(a))})
    toggle_row("drawAid.grid.set", "drawAid.grid.set", "visible", lambda s: s["grid"]["visible"], "01 §5.7 lines 451-456 (Grid on/off)")
    toggle_row("gridOverlay", "drawAid.grid.set", "overlay", lambda s: s["grid"]["overlay"], "01 §5.7 line 451 (grid overlay)")
    toggle_row("drawAid.onionSkin.set", "drawAid.onionSkin.set", "enabled", lambda s: s["onionSkin"]["enabled"], "01 §5.7 line 452 (onion skin on/off)")
    toggle_row("drawAid.autoLightTable.set", "drawAid.autoLightTable.set", "enabled", lambda s: s["autoLightTable"], "01 §5.7 line 453 (auto light table)")
    # C43B-R2-03: one row per option of each list (01 §5.7 lines 457-458: group items list their members), each choosing
    # that option by its declared reference; the options in an order where each differs from the one chosen before it
    # (choosing the chosen option sends nothing), starting after the state's own.
    start = await aids()

    def rotated(values, current):
        i = values.index(current) if current in values else -1
        return values[i + 1:] + values[:i + 1]

    def option_row(ident, label, value, command, params, read, spec):
        shown = str(value)
        rows.append({"name": f"main toolbar › {label} › {shown}", "spec": f"{spec_line(command)}; {spec}",
                     "action": ("select", f"mainToolbar.{ident}", shown),
                     "expect": (command, params), "label": None, "setup": None, "state": aids,
                     "effect": lambda b, a, k: read(a) == value,
                     "disabled": ("dom", f"mainToolbar.{ident}"),
                     "shows": (f"mainToolbar.{ident}", lambda n, a: n.get("value") == shown and any(
                         c.get("value") == shown and c.get("selected") is True for c in n.get("children") or []))})

    for value in rotated(["normal", "field12", "field16"], start["grid"]["type"]):
        option_row("gridType", "Grid type", value, "drawAid.grid.set", {"type": value}, lambda s: s["grid"]["type"],
                   "01 §5.7 lines 457-458 (group items list their members)")
    for ident, label, field in (("onionSkinPrevious", "Previous drawings", "previous"), ("onionSkinNext", "Next drawings", "next")):
        for value in rotated([0, 1, 2, 3], start["onionSkin"][field]):
            option_row(ident, label, value, "drawAid.onionSkin.set", {field: value},
                       (lambda f: lambda s: s["onionSkin"][f])(field), "01 §5.7 lines 452, 457-458")

    # 5. The view chrome of both views (02 §10): every aid, symmetry, the type buttons.
    flag = lambda v: v is True  # noqa: E731
    readers = {
        "grid": ("visible", lambda s: s["grid"]["visible"]),
        "onionSkin": ("enabled", lambda s: s["onionSkin"]["enabled"]),
        "onionOutline": ("outline", lambda s: s["onionSkinStyle"]["outline"]),
        "autoLightTable": ("enabled", lambda s: flag(s["autoLightTable"])),
        "strokes": ("visible", lambda s: flag(s["strokes"])),
        "currentOnTop": ("enabled", lambda s: flag(s["currentOnTop"])),
        "boxHighlighting": ("boxHighlighting", lambda s: s["selectionDisplay"]["boxHighlighting"]),
        "outlineOnSelection": ("outlineOnSelection", lambda s: s["selectionDisplay"]["outlineOnSelection"]),
        "keepSelectionVisible": ("keepSelectionVisible", lambda s: s["selectionDisplay"]["keepSelectionVisible"]),
        "sceneBackground": ("useSceneColour", lambda s: s["background"]["useSceneColour"]),
    }
    commands = {"grid": "drawAid.grid.set", "onionSkin": "drawAid.onionSkin.set", "onionOutline": "drawAid.onionSkinStyle.set",
                "autoLightTable": "drawAid.autoLightTable.set", "strokes": "drawAid.strokes.set", "currentOnTop": "drawAid.currentOnTop.set",
                "boxHighlighting": "drawAid.selectionDisplay.set", "outlineOnSelection": "drawAid.selectionDisplay.set",
                "keepSelectionVisible": "drawAid.selectionDisplay.set", "sceneBackground": "drawAid.background.set"}
    for view in ("drawingView", "cameraView"):
        async def into(view=view):
            await focus_view(view)
        for aid, (field, read) in readers.items():
            rows.append({"name": f"{view} chrome › {aid}", "spec": f"{spec_line(commands[aid])}; 02 §10",
                         "action": ("dom", f"{view}.aid.{aid}"),
                         "expect": (lambda c, f, r: lambda b: (c, {f: not r(b)}))(commands[aid], field, read),
                         "label": None, "setup": into, "state": aids, "toggle": True,
                         "effect": (lambda r: lambda b, a, k: r(a) == (not r(b)))(read), "disabled": ("dom", f"{view}.aid.{aid}"),
                         "shows": (f"{view}.aid.{aid}", (lambda r: lambda n, a: n.get("checked") == r(a))(read))})
        rows.append({"name": f"{view} chrome › lightTableShade", "spec": f"{spec_line('drawAid.lightTableShade.set')}; 02 §10",
                     "action": ("dom", f"{view}.aid.lightTableShade"),
                     "expect": lambda b: ("drawAid.lightTableShade.set", {"auto": not b["lightTableShade"]["auto"], "static": not b["lightTableShade"]["auto"]}),
                     "label": None, "setup": into, "state": aids, "toggle": True,
                     "effect": lambda b, a, k: a["lightTableShade"]["auto"] == (not b["lightTableShade"]["auto"]),
                     "disabled": ("dom", f"{view}.aid.lightTableShade")})
        for aid, direction in (("onionPrevious", "previous"), ("onionNext", "next")):
            rows.append({"name": f"{view} chrome › {aid}", "spec": f"{spec_line('drawAid.onionSkinCycle.run')}; 02 §10",
                         "action": ("dom", f"{view}.aid.{aid}"), "expect": ("drawAid.onionSkinCycle.run", {"direction": direction}),
                         "label": None, "setup": into, "state": aids,
                         "effect": (lambda f: lambda b, a, k: a["onionSkin"][f] != b["onionSkin"][f])(direction),
                         "disabled": ("dom", f"{view}.aid.{aid}")})

        async def panels():
            return await cmd("window.panel.list", {})
        rows.append({"name": f"{view} chrome › staticLightTablePanel", "spec": f"{spec_line('drawAid.staticLightTablePanel.set')}; 02 §10",
                     "action": ("dom", f"{view}.aid.staticLightTablePanel"), "expect": ("drawAid.staticLightTablePanel.set", {"visible": True}),
                     "label": None, "setup": into, "state": aids, "effect": lambda b, a, k: True,
                     "disabled": ("dom", f"{view}.aid.staticLightTablePanel")})
        for aid, command, params, eff in (
            ("staticLightTableAdd", "drawAid.staticLightTable.add", {}, lambda b, a, k: len(a["staticLightTable"]) == len(b["staticLightTable"]) + 1),
            ("staticLightTableShowAll", "drawAid.staticLightTable.set", {"all": True, "visible": True},
             lambda b, a, k: all(e.get("visible") for e in a["staticLightTable"])),
            ("staticLightTableClear", "drawAid.staticLightTable.remove", {"all": True}, lambda b, a, k: a["staticLightTable"] == []),
        ):
            rows.append({"name": f"{view} chrome › {aid}", "spec": f"{spec_line(command)}; 02 §10.3",
                         "action": ("dom", f"{view}.aid.{aid}"), "expect": (command, params), "label": None,
                         "setup": (lambda v: (lambda: focus_view(v)))(view) if aid != "staticLightTableAdd" else
                         (lambda v: setup(lambda: rect(0, 0), lambda: focus_view(v)))(view),
                         "state": aids, "effect": eff, "disabled": ("dom", f"{view}.aid.{aid}")})

        async def symmetry():
            s = await cmd("tool.settings.get", {})
            return (s.get("symmetry") or {}).get("mode")
        rows.append({"name": f"{view} chrome › symmetry", "spec": f"{spec_line('tool.symmetry.set')}; 02 §6.2",
                     "action": ("dom", f"{view}.symmetry"),
                     "expect": lambda b: ("tool.symmetry.set", {"mode": "mirror" if b == "none" else "none"}),
                     "label": None, "setup": into, "state": symmetry, "toggle": True,
                     "effect": lambda b, a, k: a == ("mirror" if b == "none" else "none"), "disabled": ("dom", f"{view}.symmetry"),
                     "shows": (f"{view}.symmetry", lambda n, a: n.get("checked") == (a != "none"))})

        async def types(view=view):
            listed = await cmd("view.list", {})
            vs = listed if isinstance(listed, list) else listed.get("views") or []
            return next((v.get("type") for v in vs if v.get("viewId") == view), None)
        own = "drawing" if view == "drawingView" else "camera"
        other = "camera" if own == "drawing" else "drawing"
        for ty in (other, own):
            rows.append({"name": f"{view} toolbar › type {ty}", "spec": f"{spec_line('view.type.set')}; 01 §5.3",
                         "action": ("dom", f"{view}.type.{ty}"), "expect": ("view.type.set", {"viewId": view, "type": ty}),
                         "label": None, "setup": into, "state": types, "effect": (lambda x: lambda b, a, k: a == x)(ty),
                         "disabled": ("dom", f"{view}.type.{ty}"),
                         "shows": (f"{view}.type.{ty}", lambda n, a: n.get("checked") is True)})

    # 6. The Drawing tools palette: each tool (02 §1.3, §7).
    async def active_tool():
        return (await cmd("tool.active.get", {})).get("toolId")
    tools = [x.get("toolId") for x in (await cmd("tool.list", {})).get("tools") or []]
    for tool in tools:
        rows.append({"name": f"tools palette › {tool}", "spec": f"{spec_line('tool.active.set')}; 02 §7",
                     "action": ("dom", f"drawingTools.{tool}"), "expect": ("tool.active.set", {"toolId": tool}), "label": None,
                     "setup": None, "state": active_tool, "effect": (lambda x: lambda b, a, k: a == x)(tool),
                     "disabled": ("dom", f"drawingTools.{tool}"),
                     "shows": (f"drawingTools.{tool}", lambda n, a: n.get("checked") is True)})

    # 7. The shortcut editor's Export and Import (01 §8.3 item 8): a real save and open dialog, a typed path.
    exported = t.work / "keymap-export.json"
    edited = t.work / "keymap-import.json"

    async def open_keyboard():
        if exported.exists():
            exported.unlink()
        choose_menu(["File", "Preferences"])
        await asyncio.sleep(0.8)
        t.click("preferences.category.keyboard")
        await asyncio.sleep(0.5)

    async def file_state():
        return {"exists": exported.exists(), "bindings": len(json.loads(exported.read_text(encoding="utf-8")).get("bindings", [])) if exported.exists() else 0}

    async def type_path(path: Path):
        await asyncio.sleep(1.5)
        type_text(str(path))
        key(0x0D)
        await asyncio.sleep(1.0)

    rows.append({"name": "shortcut editor › Export", "spec": f"{spec_line('keymap.export')}; 01 §8.3 item 8",
                 "action": ("dom", "keymap.export"), "expect": ("keymap.export", {"path": str(exported), "onExists": "overwrite"}),
                 "label": None, "setup": open_keyboard, "state": file_state, "wait": 0.1,
                 "effect": lambda b, a, k: a["exists"] and a["bindings"] > 0, "disabled": None,
                 "post_click": lambda: type_path(exported), "after": close_dialogs})

    async def keys_of_save():
        return [b.get("keys") for b in await cmd("keymap.list", {"commandId": "project.save"})]

    async def prepare_import():
        await open_keyboard()
        await cmd("keymap.export", {"path": str(exported), "onExists": "overwrite"})
        data = json.loads(exported.read_text(encoding="utf-8"))
        for b in data.get("bindings", []):
            if b.get("commandId") == "project.save":
                b["keys"] = "Primary+Shift+F11"
        edited.write_text(json.dumps(data), encoding="utf-8")

    async def import_flow():
        t.click("keymap.import")
        await type_path(edited)
        # The application's message box (C43B-R2-08): Merge is its default (ruling D-46), Replace its own button.
        await asyncio.sleep(1.0)
        t.click("message.replace")
        await asyncio.sleep(0.6)
        t.click("preferences.submit")
        await asyncio.sleep(1.0)
    rows.append({"name": "shortcut editor › Import", "spec": f"{spec_line('keymap.import')}; 01 §8.3 items 8-9 (applied by OK)",
                 "action": ("none",), "expect": ("keymap.import", {"path": str(edited), "mode": "replace"}), "label": None,
                 "setup": prepare_import, "state": keys_of_save, "custom": import_flow,
                 "effect": lambda b, a, k: "Primary+Shift+F11" in a, "disabled": None,
                 "after": lambda: cmd("keymap.reset", {"all": True})})
    # ----- C4.4a: the Preferences dialog's Tablet category (01 §7.3; 02 §6.3; D-40 answer 1, SI-165): each key edited,
    # then OK, which sends one `prefs.set` -----
    TABLET_KEYS = ("tablet.ignore", "tablet.portraitCorrection", "tablet.compatibilityMode", "tablet.alternativeInput")

    async def tablet_state():
        return (await cmd("prefs.get", {"keys": list(TABLET_KEYS)})).get("values") or {}

    def tablet_flow(k):
        async def flow():
            choose_menu(["File", "Preferences"])
            await asyncio.sleep(0.8)
            t.click("preferences.category.tablet")
            await asyncio.sleep(0.5)
            # Opening the dialog is the menu item's own command (`ui.preferences.open`, bound in the menu's row): the row
            # records what the Tablet edit and OK send.
            t.clear()
            t.click(f"pref.{k}")
            await asyncio.sleep(0.3)
            t.click("preferences.submit")
            await asyncio.sleep(1.0)
            return f"real clicks in Preferences › Tablet ({k}), then OK"
        return flow

    async def tablet_defaults():
        await cmd("prefs.reset", {"keys": list(TABLET_KEYS)})

    for k in TABLET_KEYS:
        value = True
        rows.append({"name": f"Preferences › Tablet › {k}", "spec": f"{spec_line('prefs.set')}; 01 §7.3 (Tablet); 02 §6.3",
                     "action": ("none",), "custom": tablet_flow(k),
                     "expect": (lambda k_, v_: lambda b: ("prefs.set", {"values": {k_: v_}}))(k, value), "label": None,
                     "setup": tablet_defaults, "state": tablet_state,
                     "effect": (lambda k_, v_: lambda b, a, kk: a.get(k_) == v_)(k, value), "disabled": None,
                     "after": tablet_defaults,
                     "mcpReason": "the Preferences dialog opens from the native menu, which only real input reaches"})

    # A Preferences dialog left open by a failed row would hold the native menu: closed before the next rows.
    rows.append({"name": "(close dialogs)", "spec": "", "action": ("none",), "expect": ("", {}), "skip": True,
                 "setup": close_dialogs})

    # 8. The status bar's jobs (01 §9.1): an agent's job shows while it runs and goes when it ends.
    # 9. File › Open Backup with Open (01 §12.2 lines 1083-1084; C43B-R1-07), last: it replaces the project.
    async def backups_ready():
        await rect(0, 0)
        await cmd("project.save", {})
        await rect(50, 50)
        await cmd("project.save", {})

    async def app_project():
        st = await cmd("app.state.get", {})
        return (st.get("project") or {}).get("path")
    backups: dict = {}

    async def open_backup_flow():
        choose_menu(["File", "Open Backup"])
        await asyncio.sleep(1.0)
        listed = await cmd("project.backup.list", {})
        backups["listed"] = listed
        backups["buttons"] = t.cdp.js("document.querySelectorAll('[data-tt-command=\"project.backup.open\"]').length")
        t.click("openBackup.open.0")
        await asyncio.sleep(2.0)

    rows.append({"name": "File › Open Backup › Open", "spec": f"{spec_line('project.backup.open')}; 01 §12.2 lines 1083-1084",
                 "action": ("none",), "custom": open_backup_flow,
                 "expect": lambda b: ("project.backup.open", {"backupId": (backups.get("listed") or [{}])[0].get("backupId")}),
                 "label": None, "setup": setup(backups_ready), "state": app_project, "late_expect": True,
                 "effect": lambda b, a, k: backups.get("buttons", 0) >= 2 and a != b, "disabled": None})

    # ----- C4.4a: the Pen panel (02 §6.1, §6.2; design note 1 §3), shown docked, the Brush active -----
    pen_target: dict = {}

    async def pens_state():
        return await cmd("tool.pen.list", {})

    def brush_pens(st):
        return [x for x in st.get("pens") or [] if x.get("type") == "brush"]

    async def pen_shown():
        await cmd("window.panel.set", {"panelId": "pen", "visible": True})
        await cmd("tool.active.set", {"toolId": "brush"})
        st = await pens_state()
        if len(brush_pens(st)) < 2:
            await cmd("tool.pen.create", {"type": "brush"})
            st = await pens_state()
        cur = st.get("currentBrushPenId")
        pen_target["other"] = next(x["penId"] for x in brush_pens(st) if x["penId"] != cur)
        shape = next((x.get("tipShape") for x in brush_pens(st) if x["penId"] == cur), "round")
        pen_target["shape"] = "square" if shape != "square" else "round"
        # A wait for a state (D-45): the panel shows the kernel's current pen (run 45035198984: the panel kept an older
        # read's current pen, a product defect fixed with the read gate; a panel that stays stale fails here).
        start = time.monotonic()
        shown = None
        while time.monotonic() - start < 15.0:
            answer = await e2e.call(t.s, "inspect_ui", {"target": "window", "depth": 32})
            shown = McpTable.node((answer.get("result") or answer).get("root") or {}, f"pen.row.{cur}") or {}
            if shown.get("checked") is True:
                break
            await asyncio.sleep(0.25)
        else:
            raise RuntimeError(f"the Pen panel does not show the current pen {cur}: {json.dumps(shown)[:300]}")

    async def settings_state():
        return await cmd("tool.settings.get", {})

    async def pen_shown_advanced():
        await pen_shown()
        if isinstance(t, McpTable):
            return  # the tree reports a closed disclosure's controls
        t.cdp.js("(() => { const d = document.querySelector('.pen-advanced'); if (d && !d.open) d.open = true; return true; })()")
        await asyncio.sleep(0.3)

    def cur_of(st):
        return st.get("currentBrushPenId")

    def pen_of(st, pid):
        return next((x for x in st.get("pens") or [] if x.get("penId") == pid), {})

    rows.append({"name": "Pen panel › select a pen", "spec": f"{spec_line('tool.pen.select')}; 02 §6.1",
                 "action": lambda: ("dom", f"pen.row.{pen_target['other']}"),
                 "expect": lambda b: ("tool.pen.select", {"penId": pen_target["other"]}), "label": None,
                 "setup": pen_shown, "state": pens_state,
                 "effect": lambda b, a, k: cur_of(a) == pen_target["other"] != cur_of(b), "disabled": None})
    rows.append({"name": "Pen panel › New", "spec": f"{spec_line('tool.pen.create')} (label \"New Pen\"); 02 §6.1",
                 "action": ("dom", "pen.new"),
                 "expect": lambda b: ("tool.pen.create", {"type": "brush", "fromPenId": cur_of(b)}), "label": "New Pen",
                 "setup": pen_shown, "state": pens_state,
                 "effect": lambda b, a, k: len(brush_pens(a)) == len(brush_pens(b)) + 1, "disabled": ("dom", "pen.new")})
    rows.append({"name": "Pen panel › tip shape", "spec": f"{spec_line('tool.pen.set')} (label \"Set Pen Style\"); 02 §6.1",
                 "action": lambda: ("select", "pen.tipShape", pen_target["shape"]),
                 "expect": lambda b: ("tool.pen.set", {"penId": cur_of(b), "tipShape": pen_target["shape"]}),
                 "label": "Set Pen Style", "setup": pen_shown, "state": pens_state,
                 "effect": lambda b, a, k: pen_of(a, cur_of(b)).get("tipShape") == pen_target["shape"], "disabled": None})
    rows.append({"name": "Pen panel › Draw Behind", "spec": f"{spec_line('tool.placeBehind.set')}; 02 §6.2",
                 "action": ("dom", "pen.placeBehind"),
                 "expect": lambda b: ("tool.placeBehind.set", {"enabled": not b.get("placeBehind")}), "label": None,
                 "setup": pen_shown_advanced, "state": settings_state, "toggle": True,
                 "effect": lambda b, a, k: a.get("placeBehind") == (not b.get("placeBehind")),
                 "disabled": ("dom", "pen.placeBehind")})

    async def max_size_flow():
        """A real key on the focused Maximum size slider: one step up, sent at the key's release (02 §6.1)."""
        front()
        t.cdp.js("document.querySelector('[data-tt-id=\"pen.maxSize\"]').focus()")
        key(0x27)  # Right arrow: one step
        return "focus, then a real Right arrow key (sent at its release)"

    rows.append({"name": "Pen panel › Maximum size (slider)", "spec": f"{spec_line('tool.pen.set')} (label \"Set Pen Style\"); 02 §6.1",
                 "action": ("none",), "custom": max_size_flow,
                 "expect": lambda b: ("tool.pen.set", {"penId": cur_of(b), "maxSize": pen_of(b, cur_of(b)).get("maxSize", 0) + 0.5}),
                 "label": "Set Pen Style", "setup": pen_shown, "state": pens_state,
                 "effect": lambda b, a, k: pen_of(a, cur_of(b)).get("maxSize") == pen_of(b, cur_of(b)).get("maxSize", 0) + 0.5,
                 "disabled": None,
                 "mcpReason": "a slider's value is chosen by the person's drag or key; the MCP form has no input to move it (its declared reference is the current value)"})

    # D-59 item 2 (C44A-R1S-02): every other slider at its release, and the numeric companion field of every slider
    # (typed, then Enter), each from known values so the expected parameters are exact. Each control is gone with the
    # project (the precheck's refusal: no project, no pen), which the disabled checks after the table assert.
    def field_refused(ident, retained):
        """D-61: the field shows the retained value again and says why (its `data-tt-problem`)."""
        got = t.cdp.js("(() => { const e = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']');"
                       " return e ? {value: e.value, problem: e.dataset.ttProblem ?? null} : null; })()" % json.dumps(ident))
        return bool(got) and got["problem"] is not None and _same_number(got["value"], retained)

    PEN_START = {"minSize": 2, "maxSize": 20, "smoothness": 3, "tipAngle": 10}
    PEN_LABELS = {"minSize": "Minimum size", "maxSize": "Maximum size", "smoothness": "Smoothness", "tipAngle": "Tip angle"}

    async def pen_known():
        await pen_shown()
        cur = cur_of(await pens_state())
        for k_, v_ in PEN_START.items():
            await cmd("tool.pen.set", {"penId": cur, k_: v_})
        await asyncio.sleep(0.5)

    def slider_flow(setting):
        async def flow():
            front()
            t.cdp.js("document.querySelector('[data-tt-id=\"pen.%s\"]').focus()" % setting)
            key(0x27)  # Right arrow: one step, sent at the key's release
            return "focus, then a real Right arrow key (sent at its release)"
        return flow

    def pen_field_flow(setting, typed):
        async def flow():
            front()
            t.cdp.js("(() => { const e = document.querySelector('[data-tt-id=\"pen.%s.field\"]'); e.focus(); e.select(); return true; })()"
                     % setting)
            type_text(typed)
            key(0x0D)
            return f"{typed!r} typed in the focused field, then a real Enter"
        return flow

    for setting, step in (("minSize", 0.5), ("smoothness", 1), ("tipAngle", 1)):
        value = PEN_START[setting] + step
        rows.append({"name": f"Pen panel › {PEN_LABELS[setting]} (slider)",
                     "spec": f"{spec_line('tool.pen.set')} (label \"Set Pen Style\"); 02 §6.1",
                     "action": ("none",), "custom": slider_flow(setting),
                     "expect": (lambda k_, v_: lambda b: ("tool.pen.set", {"penId": cur_of(b), k_: v_}))(setting, value),
                     "label": "Set Pen Style", "setup": pen_known, "state": pens_state,
                     "effect": (lambda k_, v_: lambda b, a, k: pen_of(a, cur_of(b)).get(k_) == v_)(setting, value),
                     "disabled": ("dom", f"pen.{setting}"),
                     "mcpReason": "a slider's value is chosen by the person's drag or key; the MCP form has no input to move it (its declared reference is the current value)"})
    for setting, typed, value in (("minSize", "4", 4), ("maxSize", "30", 30), ("smoothness", "6", 6), ("tipAngle", "45", 45)):
        rows.append({"name": f"Pen panel › {PEN_LABELS[setting]} (typed)",
                     "spec": f"{spec_line('tool.pen.set')} (label \"Set Pen Style\"); 02 §6.1",
                     "action": ("none",), "custom": pen_field_flow(setting, typed),
                     "expect": (lambda k_, v_: lambda b: ("tool.pen.set", {"penId": cur_of(b), k_: v_}))(setting, value),
                     "label": "Set Pen Style", "setup": pen_known, "state": pens_state,
                     "effect": (lambda k_, v_: lambda b, a, k: pen_of(a, cur_of(b)).get(k_) == v_)(setting, value),
                     "disabled": ("dom", f"pen.{setting}.field"),
                     "mcpReason": "a typed value is the person's input; the field's declared reference is the current value"})
    # D-61 (C44A-R1-20, -21): letters and an out-of-range value are refused at the field: nothing is sent, the pen
    # keeps its value, and the field shows it again with the reason.
    for setting, out_of_range in (("minSize", "-1"), ("maxSize", "101"), ("smoothness", "11"), ("tipAngle", "361")):
        for kind, typed in (("invalid", "abc"), ("out of range", out_of_range)):
            rows.append({"name": f"Pen panel › {PEN_LABELS[setting]} ({kind})",
                         "spec": f"{spec_line('tool.pen.set')}; 02 §6.1; D-61",
                         "action": ("none",), "custom": pen_field_flow(setting, typed),
                         "expect": None, "label": None, "setup": pen_known, "state": pens_state,
                         "effect": (lambda k_: lambda b, a, k: a == b and field_refused(
                             f"pen.{k_}.field", pen_of(b, cur_of(b)).get(k_)))(setting),
                         "disabled": None,
                         "mcpReason": "a typed value is the person's input; the field's declared reference is the current value"})

    async def rename_flow():
        """A real double click on the current pen, a typed name, Enter (02 §12.1 "Rename Pen")."""
        st = await pens_state()
        ident = f"pen.row.{cur_of(st)}"
        r = t.rect(ident)
        h = front()
        origin = wintypes.POINT(0, 0)
        u32.ClientToScreen(h, ctypes.byref(origin))
        x, y = int(origin.x + r["x"] * r["dpr"]), int(origin.y + r["y"] * r["dpr"])
        u32.SetCursorPos(x, y)
        time.sleep(0.1)
        for _ in range(2):
            e6._send_button(True)
            time.sleep(0.02)
            e6._send_button(False)
            time.sleep(0.05)
        time.sleep(0.4)
        type_text("Renamed D39")
        key(0x0D)
        return "a real double click on the pen, the name typed, Enter"

    rows.append({"name": "Pen panel › rename", "spec": f"{spec_line('tool.pen.rename')} (label \"Rename Pen\"); 02 §12.1",
                 "action": ("none",), "custom": rename_flow,
                 "expect": lambda b: ("tool.pen.rename", {"penId": cur_of(b), "name": "Renamed D39"}),
                 "label": "Rename Pen", "setup": pen_shown, "state": pens_state,
                 "effect": lambda b, a, k: pen_of(a, cur_of(b)).get("name") == "Renamed D39", "disabled": None,
                 "mcpReason": "the rename field opens on a double click or F2 on the pen, real input the MCP form does not perform"})
    # ----- D-66 item 2 (C44A-R2-01; 01 §8.4 "platform text-editing shortcuts act on the focused text field"): with the
    # focus in a text field, the real Ctrl+A then a typed key replaces the field's text, the real Ctrl+Z does not undo
    # the document, and a tool's letter (B, Brush) types the letter. Nothing is sent and no history step is made. One
    # numeric field (the pen's Maximum size), the pen rename field and a Preferences text field (its search). -----
    text_keys_case: dict = {}

    def field_text(ident):
        return t.cdp.js("(() => { const e = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']');"
                        " return e ? {value: e.value, start: e.selectionStart, end: e.selectionEnd,"
                        " focused: document.activeElement === e} : null; })()" % json.dumps(ident))

    async def focused_field(ident, timeout=5.0):
        # A wait for a state (D-45): the field has the focus.
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            got = field_text(ident)
            if got and got["focused"]:
                return got
            await asyncio.sleep(0.1)
        return None

    async def history_state():
        return await cmd("edit.history.get", {"limit": 1000})

    async def undoable_change():
        # A document change for Ctrl+Z to find: had the key reached the keymap, it would undo it.
        await e2e.run_cmd(t.s, "draw.rectangle.add", {"from": {"x": -30, "y": -20}, "to": {"x": 30, "y": 20}, "layer": "new"})
        h = await cmd("edit.history.get", {"limit": 1})
        if not h or not h.get("position"):
            raise RuntimeError(f"no undoable step to test Ctrl+Z against: {h}")

    def text_keys_flow(name, ident, open_field):
        async def flow():
            front()
            how = await open_field()
            got = await focused_field(ident)
            if not got:
                raise RuntimeError(f"the field did not take the focus: {ident}")
            case = {"field": ident, "opened": how, "initial": got["value"]}
            # Opening it is not this row's (the Preferences menu item has its own row): the row records the keys only.
            t.clear()
            if not got["value"]:
                # An empty field (the search): a known text typed first, with real keys, so select all has text to select.
                for vk in (0x51, 0x52, 0x53):
                    key(vk)
                await asyncio.sleep(0.2)
            # The real Ctrl+A selects the field's whole text.
            key(0x41, 0x11)
            await asyncio.sleep(0.2)
            got = field_text(ident)
            case["afterSelectAll"] = got
            selected = bool(got) and got["start"] == 0 and got["end"] == len(got["value"]) and len(got["value"]) > 0
            # Typing replaces it (a real X key).
            key(0x58)
            await asyncio.sleep(0.2)
            got = field_text(ident)
            case["afterTyping"] = got
            replaced = bool(got) and got["value"].lower() == "x"
            # A tool's letter types the letter (a real B key; B is Brush in the keymap).
            key(0x42)
            await asyncio.sleep(0.3)
            got = field_text(ident)
            case["afterToolLetter"] = got
            letter = bool(got) and got["value"].lower() == "xb" and got["focused"]
            # The real Ctrl+Z is the field's: the document's history stays where it was.
            hist = await cmd("edit.history.get", {"limit": 1})
            key(0x5A, 0x11)
            await asyncio.sleep(0.5)
            hist_after = await cmd("edit.history.get", {"limit": 1})
            got = field_text(ident)
            case["afterUndo"] = got
            kept = (hist or {}).get("position") == (hist_after or {}).get("position") and bool(got) and got["focused"]
            case.update({"selectAllSelects": selected, "typingReplaces": replaced, "toolLetterTypes": letter,
                         "undoLeavesDocument": kept, "historyBefore": (hist or {}).get("position"),
                         "historyAfter": (hist_after or {}).get("position")})
            case["ok"] = selected and replaced and letter and kept
            text_keys_case[name] = case
            t.run.save("text-keys-" + ident + ".json", case)
            # Escape: the field's own rule (the numeric field and the rename restore; the dialog closes).
            key(0x1B)
            await asyncio.sleep(0.3)
            return ("real keys in the focused field: Ctrl+A, X, B, Ctrl+Z, then Escape (" + how + ")")
        return flow

    async def open_numeric():
        t.click("pen.maxSize.field")
        return "a real click on the field"

    async def open_rename():
        st = await pens_state()
        r = t.rect(f"pen.row.{cur_of(st)}")
        h = front()
        origin = wintypes.POINT(0, 0)
        u32.ClientToScreen(h, ctypes.byref(origin))
        u32.SetCursorPos(int(origin.x + r["x"] * r["dpr"]), int(origin.y + r["y"] * r["dpr"]))
        time.sleep(0.1)
        for _ in range(2):
            e6._send_button(True)
            time.sleep(0.02)
            e6._send_button(False)
            time.sleep(0.05)
        await asyncio.sleep(0.4)
        return "a real double click on the pen"

    async def open_prefs_search():
        choose_menu(["File", "Preferences"])
        end = time.monotonic() + 10.0
        while time.monotonic() < end and not field_text("preferences.search"):
            await asyncio.sleep(0.1)
        t.click("preferences.search")
        return "File › Preferences, then a real click on its search field"

    async def numeric_setup():
        await pen_known()
        await undoable_change()

    async def rename_setup():
        await pen_shown()
        await undoable_change()

    for tk_name, tk_ident, tk_open, tk_setup in (
            ("Pen panel › Maximum size (text-editing keys)", "pen.maxSize.field", open_numeric, numeric_setup),
            ("Pen panel › rename (text-editing keys)", "pen.rename", open_rename, rename_setup),
            ("Preferences › search (text-editing keys)", "preferences.search", open_prefs_search, undoable_change)):
        rows.append({"name": tk_name, "spec": "01 §8.4; D-66 item 2 (C44A-R2-01)",
                     "action": ("none",), "custom": text_keys_flow(tk_name, tk_ident, tk_open),
                     "expect": None, "label": None, "setup": tk_setup, "state": history_state,
                     "effect": (lambda n_: lambda b, a, k: a == b and bool(text_keys_case.get(n_, {}).get("ok")))(tk_name),
                     "disabled": None, "osOnly": True, "after": close_dialogs,
                     "mcpReason": "real keys typed into a focused text field: an agent's `key` tool is refused while a text field has the focus"})
    rows.append({"name": "Pen panel › Delete", "spec": f"{spec_line('tool.pen.delete')} (label \"Delete Pen\"); 02 §6.1",
                 "action": ("dom", "pen.delete"),
                 "expect": lambda b: ("tool.pen.delete", {"penId": cur_of(b)}), "label": "Delete Pen",
                 "setup": pen_shown, "state": pens_state,
                 "effect": lambda b, a, k: len(brush_pens(a)) == len(brush_pens(b)) - 1, "disabled": ("dom", "pen.delete")})

    # ----- C4.4a: the Properties panel's Tool options page (02 §7.7, §8.8-§8.11; design note 1 §4) and Colour ›
    # Auto Close Gap (03 §8.8) -----
    opt_target: dict = {}

    def tool_page(tool):
        async def run():
            await cmd("window.panel.set", {"panelId": "properties", "visible": True})
            await cmd("tool.placeBehind.set", {"enabled": False})
            await cmd("tool.active.set", {"toolId": tool})
            if tool == "eraser":
                erasers = [x for x in (await pens_state()).get("pens") or [] if x.get("type") == "eraser"]
                if len(erasers) < 2:
                    await cmd("tool.pen.create", {"type": "eraser"})
            st = await pens_state()
            fam = [x for x in st.get("pens") or [] if x.get("type") == ("eraser" if tool == "eraser" else "brush")]
            cur = st.get("currentEraserPenId" if tool == "eraser" else "currentBrushPenId")
            opt_target["pen"] = next((x["penId"] for x in fam if x["penId"] != cur), None)
            opt = (await cmd("tool.options.get", {"toolId": tool})).get("options") or {}
            opt_target["rectRule"] = "intersect" if opt.get("rectRule") != "intersect" else "inside"
            opt_target["source"] = "fixed" if opt.get("thicknessSource") != "fixed" else "penMax"
            gap = (await cmd("colour.gapClosing.get", {})).get("mode")
            opt_target["gap"] = "medium" if gap != "medium" else "small"
            await asyncio.sleep(0.6)
        return run

    def options_of(tool):
        async def read():
            return (await cmd("tool.options.get", {"toolId": tool})).get("options") or {}
        return read

    async def gap_state():
        return await cmd("colour.gapClosing.get", {})

    rows.append({"name": "Tool options › Select: rectangle rule", "spec": f"{spec_line('tool.select.options.set')}; 02 §7.7",
                 "action": lambda: ("select", "toolOptions.rectRule", opt_target["rectRule"]),
                 "expect": lambda b: ("tool.select.options.set", {"rectRule": opt_target["rectRule"]}), "label": None,
                 "setup": tool_page("select"), "state": options_of("select"),
                 "effect": lambda b, a, k: a.get("rectRule") == opt_target["rectRule"], "disabled": None})
    for tool, command in (("brush", "tool.brush.options.set"), ("eraser", "tool.eraser.options.set")):
        rows.append({"name": f"Tool options › {tool.title()}: pen", "spec": f"{spec_line(command)}; 02 §7.7",
                     "action": lambda: ("select", "toolOptions.pen", opt_target["pen"]),
                     "expect": (lambda c: lambda b: (c, {"penId": opt_target["pen"]}))(command), "label": None,
                     "setup": tool_page(tool), "state": pens_state,
                     "effect": (lambda t: lambda b, a, k: a.get("currentEraserPenId" if t == "eraser" else "currentBrushPenId") == opt_target["pen"])(tool),
                     "disabled": None})
    rows.append({"name": "Tool options › Pencil: line thickness source", "spec": f"{spec_line('tool.pencil.options.set')}; 02 §7.7",
                 "action": lambda: ("select", "toolOptions.thicknessSource", opt_target["source"]),
                 "expect": lambda b: ("tool.pencil.options.set", {"thicknessSource": "fixed", "thickness": b.get("thickness") if isinstance(b.get("thickness"), (int, float)) else 1}
                                      if opt_target["source"] == "fixed" else {"thicknessSource": "penMax"}),
                 "label": None, "setup": tool_page("pencil"), "state": options_of("pencil"),
                 "effect": lambda b, a, k: a.get("thicknessSource") == opt_target["source"], "disabled": None})
    rows.append({"name": "Tool options › Polyline: snapping", "spec": f"{spec_line('tool.polyline.options.set')}; 02 §7.7",
                 "action": ("dom", "toolOptions.snap"),
                 "expect": lambda b: ("tool.polyline.options.set", {"snap": not b.get("snap", True)}), "label": None,
                 "setup": tool_page("polyline"), "state": options_of("polyline"), "toggle": True,
                 "effect": lambda b, a, k: a.get("snap") == (not b.get("snap", True)), "disabled": None})
    rows.append({"name": "Tool options › Paint: automatic gap closing", "spec": f"{spec_line('colour.gapClosing.set')}; 03 §8.8",
                 "action": lambda: ("select", "toolOptions.gapClosing", opt_target["gap"]),
                 "expect": lambda b: ("colour.gapClosing.set", {"mode": opt_target["gap"]}), "label": None,
                 "setup": tool_page("paint"), "state": gap_state,
                 "effect": lambda b, a, k: a.get("mode") == opt_target["gap"], "disabled": None})
    for tid, command, setting in (("drawTopLayer", "tool.drawTopLayer.set", "drawTopLayer"),
                              ("placeBehind", "tool.placeBehind.set", "placeBehind"),
                              ("snapToContour", "tool.snapToContour.set", "snapToContour")):
        rows.append({"name": f"Tool options › {tid}", "spec": f"{spec_line(command)}; 02 §8.9-§8.11",
                     "action": ("dom", f"toolOptions.{tid}"),
                     "expect": (lambda c, k_: lambda b: (c, {"enabled": not b.get(k_)}))(command, setting), "label": None,
                     "setup": tool_page("brush"), "state": settings_state, "toggle": True,
                     "effect": (lambda k_: lambda b, a, k: a.get(k_) == (not b.get(k_)))(setting),
                     "disabled": ("dom", f"toolOptions.{tid}")})

    async def thickness_flow():
        """A real typed value in the fixed thickness field, then Enter."""
        await cmd("tool.pencil.options.set", {"thicknessSource": "fixed", "thickness": 2})
        await asyncio.sleep(0.6)
        front()
        t.cdp.js("(() => { const e = document.querySelector('[data-tt-id=\"toolOptions.thickness\"]'); e.focus(); e.select(); return true; })()")
        type_text("3.5")
        key(0x0D)
        return "the value typed in the focused field, then a real Enter"

    rows.append({"name": "Tool options › Pencil: fixed thickness", "spec": f"{spec_line('tool.pencil.options.set')}; 02 §7.7",
                 "action": ("none",), "custom": thickness_flow,
                 "expect": lambda b: ("tool.pencil.options.set", {"thicknessSource": "fixed", "thickness": 3.5}), "label": None,
                 "setup": tool_page("pencil"), "state": options_of("pencil"),
                 "effect": lambda b, a, k: a.get("thickness") == 3.5, "disabled": None,
                 "mcpReason": "a typed value is the person's input; the field's declared reference is the current value"})

    async def snap_radius_known():
        await cmd("tool.polyline.options.set", {"snap": True, "snapRadius": 8})
        await tool_page("polyline")()

    async def snap_radius_flow():
        """D-59 item 2: a real typed value in the snap radius field, then Enter."""
        front()
        t.cdp.js("(() => { const e = document.querySelector('[data-tt-id=\"toolOptions.snapRadius\"]'); e.focus(); e.select(); return true; })()")
        type_text("12")
        key(0x0D)
        return "the value typed in the focused field, then a real Enter"

    def typed_flow(ident, typed):
        async def flow():
            front()
            t.cdp.js("(() => { const e = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']'); e.focus(); e.select(); return true; })()"
                     % json.dumps(ident))
            type_text(typed)
            key(0x0D)
            return f"{typed!r} typed in the focused field, then a real Enter"
        return flow

    async def thickness_known():
        await cmd("tool.pencil.options.set", {"thicknessSource": "fixed", "thickness": 2})
        await tool_page("pencil")()

    for ident, setup_, state_, read_, out_of_range, title in (
            ("toolOptions.thickness", thickness_known, options_of("pencil"), lambda st: st.get("thickness"), "101",
             "Pencil: fixed thickness"),
            ("toolOptions.snapRadius", snap_radius_known, options_of("polyline"), lambda st: st.get("snapRadius"), "51",
             "Polyline: snap radius")):
        for kind, typed in (("invalid", "abc"), ("out of range", out_of_range)):
            rows.append({"name": f"Tool options › {title} ({kind})", "spec": "02 §7.7; D-61",
                         "action": ("none",), "custom": typed_flow(ident, typed), "expect": None, "label": None,
                         "setup": setup_, "state": state_,
                         "effect": (lambda i_, r_: lambda b, a, k: a == b and field_refused(i_, r_(b)))(ident, read_),
                         "disabled": None,
                         "mcpReason": "a typed value is the person's input; the field's declared reference is the current value"})

    rows.append({"name": "Tool options › Polyline: snap radius", "spec": f"{spec_line('tool.polyline.options.set')}; 02 §7.7",
                 "action": ("none",), "custom": snap_radius_flow,
                 "expect": ("tool.polyline.options.set", {"snapRadius": 12}), "label": None,
                 "setup": snap_radius_known, "state": options_of("polyline"),
                 "effect": lambda b, a, k: b.get("snapRadius") == 8 and a.get("snapRadius") == 12,
                 "disabled": ("dom", "toolOptions.snapRadius"),
                 "mcpReason": "a typed value is the person's input; the field's declared reference is the current value"})
    for mode, label in (("small", "Small"), ("large", "Large"), ("off", "Off")):
        rows.append({"name": f"Colour › Auto Close Gap › {label}", "spec": f"{spec_line('colour.gapClosing.set')}; 03 §8.8 (\"from the main menu\")",
                     "action": ("menu", ["Colour", "Auto Close Gap", label]),
                     "expect": (lambda m: lambda b: ("colour.gapClosing.set", {"mode": m}))(mode), "label": None,
                     "setup": (lambda m: (lambda: cmd("colour.gapClosing.set", {"mode": "medium" if m != "medium" else "off"})))(mode),
                     "state": gap_state, "effect": (lambda m: lambda b, a, k: a.get("mode") == m)(mode), "disabled": None})

    # ----- C4.4a: the Text panel as the defaults for new text (02 §7.6.1; design note 1 §5) -----
    async def text_state():
        return (await cmd("tool.options.get", {"toolId": "text"})).get("options") or {}

    async def text_shown():
        await cmd("window.panel.set", {"panelId": "text", "visible": True})
        await asyncio.sleep(0.5)

    async def text_outlined():
        await cmd("tool.text.options.set", {"outlineColourId": "D39 outline"})
        await text_shown()

    for text_flag in ("bold", "italic", "autoKern"):
        rows.append({"name": f"Text panel › {text_flag}", "spec": f"{spec_line('tool.text.options.set')}; 02 §7.6.1",
                     "action": ("dom", f"text.{text_flag}"),
                     "expect": (lambda f: lambda b: ("tool.text.options.set", {f: not b.get(f)}))(text_flag), "label": None,
                     "setup": text_shown, "state": text_state, "toggle": True,
                     "effect": (lambda f: lambda b, a, k: a.get(f) == (not b.get(f)))(text_flag), "disabled": None})
    for key_, value in (("align", "centre"), ("baseline", "superscript")):
        rows.append({"name": f"Text panel › {key_}", "spec": f"{spec_line('tool.text.options.set')}; 02 §7.6.1",
                     "action": ("select", f"text.{key_}", value),
                     "expect": (lambda k_, v_: lambda b: ("tool.text.options.set", {k_: v_}))(key_, value), "label": None,
                     "setup": text_shown, "state": text_state,
                     "effect": (lambda k_, v_: lambda b, a, k: a.get(k_) == v_)(key_, value), "disabled": None})
    rows.append({"name": "Text panel › Remove outline", "spec": f"{spec_line('tool.text.options.set')}; 02 §7.6.1",
                 "action": ("dom", "text.removeOutline"),
                 "expect": lambda b: ("tool.text.options.set", {"outlineColourId": None}), "label": None,
                 "setup": text_outlined, "state": text_state,
                 "effect": lambda b, a, k: b.get("outlineColourId") is not None and a.get("outlineColourId") is None,
                 "disabled": None})

    def text_field_flow(ident, typed):
        async def flow():
            front()
            t.cdp.js("(() => { const e = document.querySelector('[data-tt-id=\"%s\"]'); e.focus(); e.select(); return true; })()" % ident)
            type_text(typed)
            key(0x0D)
            return f"{typed!r} typed in the focused field, then a real Enter"
        return flow

    for ident, typed, expect_params, read in (("text.size", "48", {"size": 48}, lambda a: a.get("size") == 48),
                                              ("text.font", "Arial", {"font": "Arial"}, lambda a: a.get("font") == "Arial")):
        rows.append({"name": f"Text panel › {ident[5:]} (typed)", "spec": f"{spec_line('tool.text.options.set')}; 02 §7.6.1",
                     "action": ("none",), "custom": text_field_flow(ident, typed),
                     "expect": (lambda e_: lambda b: ("tool.text.options.set", e_))(expect_params), "label": None,
                     "setup": text_shown, "state": text_state,
                     "effect": (lambda r_: lambda b, a, k: r_(a))(read), "disabled": None,
                     "mcpReason": "a typed value is the person's input; the field's declared reference is the current value"})

    # D-59 item 2 (C44A-R1S-02): Kerning, the margins, Indent and Line spacing, each typed then Enter from a known value
    # (kerning is sent rounded). Each field is gone with the project (the precheck's refusal), asserted after the table.
    def text_known(field):
        async def run():
            await cmd("tool.text.options.set", {field: 0})
            await text_shown()
        return run

    for field, typed, value in (("kerning", "5", 5), ("leftMargin", "12", 12), ("rightMargin", "8", 8),
                                ("indent", "4", 4), ("lineSpacing", "3", 3)):
        rows.append({"name": f"Text panel › {field} (typed)", "spec": f"{spec_line('tool.text.options.set')}; 02 §7.6.1",
                     "action": ("none",), "custom": text_field_flow(f"text.{field}", typed),
                     "expect": (lambda f_, v_: lambda b: ("tool.text.options.set", {f_: v_}))(field, value), "label": None,
                     "setup": text_known(field), "state": text_state,
                     "effect": (lambda f_, v_: lambda b, a, k: b.get(f_) == 0 and a.get(f_) == v_)(field, value),
                     "disabled": ("dom", f"text.{field}"),
                     "mcpReason": "a typed value is the person's input; the field's declared reference is the current value"})

    # D-61: each Text number refuses letters and an out-of-range value at the field (the values the review typed).
    for field, out_of_range in (("size", "513"), ("kerning", "61"), ("leftMargin", "-1"), ("rightMargin", "1001"),
                                ("indent", "-100"), ("lineSpacing", "1001")):
        for kind, typed in (("invalid", "abc"), ("out of range", out_of_range)):
            rows.append({"name": f"Text panel › {field} ({kind})", "spec": f"{spec_line('tool.text.options.set')}; 02 §7.6.1; D-61",
                         "action": ("none",), "custom": text_field_flow(f"text.{field}", typed),
                         "expect": None, "label": None, "setup": text_shown, "state": text_state,
                         "effect": (lambda f_: lambda b, a, k: a == b and field_refused(f"text.{f_}", b.get(f_, 0)))(field),
                         "disabled": None,
                         "mcpReason": "a typed value is the person's input; the field's declared reference is the current value"})

    # ----- C4.4b: the four drawing tools (design notes 3-5) and the Contour Editor's selected line (SI-196) -----
    # Gesture rows: the window's record of the committed gesture gives the commands (both forms); the parameters come
    # from the samples, so the spec's statement is a predicate (`match`) on them. Every drag is in the Drawing view.
    c44b: dict = {}
    near = lambda a, b: abs(a - b) < 1e-6  # noqa: E731

    async def view_map():
        return (await cmd("draw.viewMapping.get", {"view": "drawingView"})).get("drawingToView") or [1, 0, 0, 1, 0, 0]

    def to_view(m, x, y):
        return (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])

    async def current_drawing_id():
        return (await cmd("draw.drawing.get", {})).get("drawingId")

    async def anchors_of(sid):
        d = await cmd("draw.drawing.get", {"flatten": True})
        n = next((x for x in d.get("nodes") or [] if x.get("id") == sid), {}) or {}
        b = n.get("bezier") or []
        return [(b[k * 3]["x"], b[k * 3]["y"]) for k in range((len(b) + 2) // 3)]

    async def tool_fixture(tool, snap=False, locked=False, lock_all=False, points=False):
        """Bob's drawing at frame 1 in the Drawing view at 100 %, a selected square on its own layer, `tool` active."""
        await cmd("window.panel.set", {"panelId": "drawingView", "visible": True})
        await cmd("view.active.set", {"viewId": "drawingView"})
        await cmd("view.transform.reset", {"viewId": "drawingView"})
        await cmd("app.context.set", {"elementId": bob, "frame": 1})
        ids = [x.get("id") for x in (await cmd("element.drawing.list", {"elementId": bob})).get("drawings") or []]
        await cmd("element.drawingLock.set", {"drawingIds": ids, "locked": False})
        await cmd("tool.snapToContour.set", {"enabled": snap})
        r = await cmd("draw.rectangle.add", {"from": {"x": -60, "y": -40}, "to": {"x": 60, "y": 40}, "layer": "new"})
        c44b["sid"] = (r.get("strokeIds") or [None])[0]
        c44b["drawing"] = await current_drawing_id()
        await cmd("draw.selection.set", {"by": "ids", "ids": [c44b["sid"]], "mode": "replace"})
        await cmd("draw.pointSelection.set", {"points": [{"strokeId": c44b["sid"], "anchor": 1}] if points else []})
        c44b["box"] = (await cmd("draw.selection.get", {})).get("bounds") or {}
        # Reposition All's box is the drawing's (SI-189): earlier rows leave other strokes in it.
        if tool == "repositionAll":
            c44b["box"] = (await cmd("draw.bounds.get", {})).get("bounds") or c44b["box"]
        if locked:
            await cmd("element.drawingLock.set", {"drawingIds": [c44b["drawing"]], "locked": True})
        if lock_all:
            await cmd("element.drawingLock.set", {"drawingIds": ids, "locked": True})
        await cmd("tool.active.set", {"toolId": tool})
        await asyncio.sleep(0.8)

    async def summary_state():
        answer = await e2e.call(t.s, "inspect_ui", {"target": "window", "depth": 32})
        n = McpTable.node((answer.get("result") or answer).get("root") or {}, "drawingView.canvas") or {}
        try:
            v = json.loads(n.get("value") or "{}")
        except json.JSONDecodeError:
            v = {}
        return {"drawing": await cmd("draw.drawing.get", {}), "toolBox": v.get("toolBox")}

    async def square_state():
        return {"anchors": await anchors_of(c44b["sid"]), "drawing": await cmd("draw.drawing.get", {})}

    def corner_pts(dx, dy, out=0.0):
        async def pts():
            m = await view_map()
            b = c44b["box"]
            x, y = to_view(m, b["x"] + b["width"], b["y"] + b["height"])
            x, y = x + out, y - out
            return [(x, y), (x + dx / 2, y + dy / 2), (x + dx, y + dy)]
        return pts

    def anchor_pts(k, dx, dy):
        async def pts():
            m = await view_map()
            a = (await anchors_of(c44b["sid"]))[k]
            x, y = to_view(m, *a)
            return [(x, y), (x + dx / 2, y + dy / 2), (x + dx, y + dy)]
        return pts

    def gesture_failed(code):
        return lambda: (t.gesture_record or {}).get("code") == code

    rows.append({"name": "Contour Editor › Snap to Contour on, then a drag",
                 "spec": f"{spec_line('draw.point.move')}; 02 §7.1.2 lines 450, 461; §8.9 (SI-187: the setting made explicit)",
                 "action": ("gesture", "drawingView", "contourEditor", anchor_pts(0, 14.0, -5.0), []),
                 "expect": ("draw.point.move", "points [square, 0], delta (14, 5) du, snap true"), "label": "Contour Editing",
                 "match": lambda prm, b: (prm.get("points") == [{"strokeId": c44b["sid"], "anchor": 0}] and prm.get("snap") is True
                                          and near((prm.get("delta") or {}).get("x", 0), 14.0)
                                          and near((prm.get("delta") or {}).get("y", 0), 5.0)),
                 "setup": lambda: tool_fixture("contourEditor", snap=True), "state": square_state,
                 "effect": lambda b, a, k: a["anchors"][0] != b["anchors"][0], "disabled": None})
    rows.append({"name": "Contour Editor › Delete", "spec": f"{spec_line('draw.point.delete')}; 02 §7.1.2 line 457 (SI-184)",
                 "action": ("key", 0x2E, "Delete"), "expect": ("draw.point.delete", {}), "label": "Contour Editing",
                 "setup": lambda: tool_fixture("contourEditor", points=True), "state": square_state,
                 "effect": lambda b, a, k: len(a["anchors"]) == len(b["anchors"]) - 1, "disabled": None})
    # Ruling D-83 item 3 (C44B-R2S-03): the person's key and Tool options edits follow the gesture path's rule. The
    # element is locked in the Camera view, the active view: refused, nothing changes.
    async def camera_locked(setup):
        await setup()
        await cmd("window.panel.set", {"panelId": "cameraView", "visible": True})
        await cmd("view.active.set", {"viewId": "cameraView"})
        await cmd("element.lock.set", {"elementIds": [bob], "locked": True})
        await asyncio.sleep(0.5)

    async def camera_unlocked():
        await close_dialogs()
        await cmd("element.lock.set", {"elementIds": [bob], "locked": False})
        await cmd("view.active.set", {"viewId": "drawingView"})

    rows.append({"name": "Contour Editor › Delete in the Camera view, element locked",
                 "spec": f"{spec_line('draw.point.delete')}; 05 §6.6, 04 §10.4; D-80; D-83 item 3 (refused, nothing sent, nothing changes)",
                 "action": ("key", 0x2E, "Delete", "cameraView"), "expect": None, "label": None,
                 "setup": lambda: camera_locked(lambda: tool_fixture("contourEditor", points=True)), "state": square_state,
                 "effect": lambda b, a, k: a == b, "disabled": None, "after": camera_unlocked})
    # Ruling D-87 (C44B-R3-01): the person is told why, on the status line, for a key and for a native menu item as for
    # a gesture or a panel. The status line is cleared first, so the row reads what this action put there.
    def status_cleared(setup):
        async def run():
            await setup()
            t.cdp.js("window.dispatchEvent(new CustomEvent('tt-status-line', { detail: null }))")
            await asyncio.sleep(0.3)
        return run

    async def square_and_status():
        st = await square_state()
        st["status"] = t.cdp.js("document.querySelector('[data-tt-id=\"status.line\"]')?.textContent ?? ''")
        return st

    def told(b, a, k):
        return a["anchors"] == b["anchors"] and a["drawing"] == b["drawing"] and "element is locked" in (a["status"] or "")

    rows.append({"name": "Contour Editor › Delete by key in the Camera view, element locked: the status line says why",
                 "spec": f"{spec_line('draw.point.delete')}; D-83 item 3; D-87 (the refusal's reason on the status line)",
                 "action": ("key", 0x2E, "Delete", "cameraView"), "expect": None, "label": None,
                 "setup": status_cleared(lambda: camera_locked(lambda: tool_fixture("contourEditor", points=True))),
                 "state": square_and_status, "effect": told, "disabled": None, "osOnly": True, "after": camera_unlocked,
                 "mcpReason": "the status line is the window's: an agent's key answers the refusal to the agent"})
    rows.append({"name": "Edit › Delete (native menu) in the Camera view, element locked: the status line says why",
                 "spec": f"{spec_line('edit.delete')}; SI-148 (the drawing tools act on the drawing's selection); D-83 item 3; D-87",
                 "action": ("menu", ["Edit", "Delete"]), "expect": ("edit.delete", "the drawing's selection; refused"),
                 "match": lambda prm, b: True, "label": None,
                 "setup": status_cleared(lambda: camera_locked(lambda: tool_fixture("select"))),
                 "state": square_and_status, "effect": told, "disabled": None, "osOnly": True, "after": camera_unlocked,
                 "mcpReason": "the status line is the window's: an agent's call answers the refusal to the agent"})
    rows.append({"name": "Contour Editor › Delete with no anchor selected",
                 "spec": f"{spec_line('draw.point.delete')}; 02 §7.1.2 line 457; C41-R2-09 (a refused key is consumed silently)",
                 "action": ("key", 0x2E, "Delete"), "expect": None, "label": None,
                 "setup": lambda: tool_fixture("contourEditor"), "state": square_state,
                 "effect": lambda b, a, k: a == b, "disabled": None})
    rows.append({"name": "Perspective › corner drag", "spec": f"{spec_line('draw.envelope.apply')}; 02 §7.1.3 lines 468-473 (SI-188)",
                 "action": ("gesture", "drawingView", "perspective", corner_pts(-20.0, 0.0), []),
                 "expect": ("draw.envelope.apply", "the top-right corner 20 du left, the others kept, mapping auto"),
                 "label": "Perspective Tool",
                 "match": lambda prm, b: (prm.get("mapping") == "auto" and len(prm.get("corners") or []) == 4
                                          and near(prm["corners"][2]["x"], prm["corners"][1]["x"] - 20.0)
                                          and near(prm["corners"][2]["y"], prm["corners"][3]["y"])
                                          and near(prm["corners"][0]["y"], prm["corners"][1]["y"])),
                 "setup": lambda: tool_fixture("perspective"), "state": square_state,
                 "effect": lambda b, a, k: a["drawing"] != b["drawing"], "disabled": None})

    def side_pts(dx, dy):
        async def pts():
            m = await view_map()
            b = c44b["box"]
            x, y = to_view(m, b["x"] + b["width"] / 2, b["y"] + b["height"])
            return [(x, y), (x + dx / 2, y + dy / 2), (x + dx, y + dy)]
        return pts

    rows.append({"name": "Perspective › alternate side drag",
                 "spec": f"{spec_line('draw.envelope.apply')}; 02 §7.1.3 line 470 (SI-188: symmetric about the side's middle)",
                 "action": ("gesture", "drawingView", "perspective", side_pts(15.0, -4.0), ["alt"]),
                 "expect": ("draw.envelope.apply", "the top corners spread 15 du each way and rise 4 du"), "label": "Perspective Tool",
                 "match": lambda prm, b: (len(prm.get("corners") or []) == 4
                                          and near(prm["corners"][2]["x"] - prm["corners"][1]["x"], 15.0)
                                          and near(prm["corners"][3]["x"] - prm["corners"][0]["x"], -15.0)
                                          and near(prm["corners"][2]["y"], prm["corners"][3]["y"])),
                 "setup": lambda: tool_fixture("perspective"), "state": square_state,
                 "effect": lambda b, a, k: a["drawing"] != b["drawing"], "disabled": None})
    async def missed_then_marquee():
        """D-76 item 3: a press off the box (a marquee with nothing inside: the selection empties, the box goes), then a
        marquee around the square with Perspective still active (the square selected again, the box back)."""
        await tool_fixture("perspective")
        m = await view_map()
        b = c44b["box"]
        x0, y0 = to_view(m, b["x"] + b["width"] + 40, b["y"] + b["height"] + 30)
        await t.input_gesture("drawingView", "perspective", [(x0, y0), (x0 - 5, y0), (x0 - 10, y0)], [])
        await asyncio.sleep(0.8)
        c44b["afterMiss"] = (await cmd("draw.selection.get", {})).get("selectedIds") or []
        (ax, ay), (bx, by) = to_view(m, b["x"] - 20, b["y"] - 20), to_view(m, b["x"] + b["width"] + 20, b["y"] + b["height"] + 20)
        await t.input_gesture("drawingView", "perspective", [(ax, ay), ((ax + bx) / 2, (ay + by) / 2), (bx, by)], [])
        await asyncio.sleep(0.8)
        sel = await cmd("draw.selection.get", {})
        c44b["afterMarquee"] = sel.get("selectedIds") or []
        # The marquee also takes the earlier rows' strokes inside it: the handles are on the new selection's box.
        c44b["box"] = sel.get("bounds") or c44b["box"]

    rows.append({"name": "Perspective › a missed press, then a marquee and a corner drag",
                 "spec": f"{spec_line('draw.envelope.apply')}; 02 §7.1.3 lines 468-473; SI-188 (a drag off the handles selects); D-76 item 3",
                 "action": ("gesture", "drawingView", "perspective", corner_pts(-20.0, 0.0), []),
                 "expect": ("draw.envelope.apply", "after the miss emptied the selection and the marquee selected the square again: the corner drag"),
                 "label": "Perspective Tool",
                 "match": lambda prm, b: prm.get("mapping") == "auto" and len(prm.get("corners") or []) == 4,
                 "setup": missed_then_marquee, "state": square_state,
                 "effect": lambda b, a, k: (a["drawing"] != b["drawing"] and c44b.get("afterMiss") == []
                                            and c44b["sid"] in (c44b.get("afterMarquee") or [])),
                 "disabled": None})
    rows.append({"name": "Perspective › locked drawing",
                 "spec": f"{spec_line('draw.envelope.apply')}; 02 line 414; design note 4 §7 (grey handles, the kernel's refusal)",
                 "action": ("gesture", "drawingView", "perspective", corner_pts(-20.0, 0.0), []),
                 # The refused gesture commits nothing: the window's record holds the refusal (`drawing.locked`).
                 "expect": None, "label": None,
                 "setup": lambda: tool_fixture("perspective", locked=True), "state": summary_state,
                 "effect": lambda b, a, k: (a["drawing"] == b["drawing"] and (b.get("toolBox") or {}).get("locked") is True
                                            and gesture_failed("drawing.locked")()),
                 "disabled": None})

    def inside_pts(dx, dy):
        async def pts():
            m = await view_map()
            b = c44b["box"]
            x, y = to_view(m, b["x"] + b["width"] / 3, b["y"] + b["height"] / 3)
            return [(x, y), (x + dx / 2, y + dy / 2), (x + dx, y + dy)]
        return pts

    rows.append({"name": "Reposition All Drawings › offset drag",
                 "spec": f"{spec_line('draw.repositionAll.apply')}; 02 §7.1.4 lines 478-481 (SI-189)",
                 "action": ("gesture", "drawingView", "repositionAll", inside_pts(30.0, -10.0), []),
                 "expect": ("draw.repositionAll.apply", "translate (30, 10) du"), "label": "Reposition All Drawings",
                 "match": lambda prm, b: (set(prm) == {"translate"} and near(prm["translate"]["x"], 30.0)
                                          and near(prm["translate"]["y"], 10.0)),
                 "setup": lambda: tool_fixture("repositionAll"), "state": square_state,
                 "effect": lambda b, a, k: a["drawing"] != b["drawing"], "disabled": None})
    rows.append({"name": "Reposition All Drawings › rotation drag",
                 "spec": f"{spec_line('draw.repositionAll.apply')}; 02 §7.1.4 line 479 (SI-141 item 3: the rotation zone)",
                 "action": ("gesture", "drawingView", "repositionAll", corner_pts(-30.0, -20.0, out=10.0), []),
                 "expect": ("draw.repositionAll.apply", "rotate about the box's centre"), "label": "Reposition All Drawings",
                 "match": lambda prm, b: set(prm) == {"rotate", "pivot"} and abs(prm["rotate"]) > 0.0,
                 "setup": lambda: tool_fixture("repositionAll"), "state": square_state,
                 "effect": lambda b, a, k: a["drawing"] != b["drawing"], "disabled": None})
    rows.append({"name": "Reposition All Drawings › every drawing locked",
                 "spec": f"{spec_line('draw.repositionAll.apply')}; 02 line 414; SI-189 (refused only when nothing can move)",
                 "action": ("gesture", "drawingView", "repositionAll", inside_pts(30.0, -10.0), []),
                 "expect": None, "label": None,
                 "setup": lambda: tool_fixture("repositionAll", lock_all=True), "state": summary_state,
                 "effect": lambda b, a, k: (a["drawing"] == b["drawing"] and (b.get("toolBox") or {}).get("locked") is True
                                            and gesture_failed("drawing.nothingToDo")()),
                 "disabled": None})

    # The Contour Editor's selected line (SI-196): one pencil line selected, the Tool options page shows its properties.
    async def line_shown():
        await tool_fixture("contourEditor")
        r = await cmd("draw.line.add", {"from": {"x": -80, "y": 60}, "to": {"x": 80, "y": 70}, "layer": "new"})
        c44b["line"] = (r.get("strokeIds") or [None])[0]
        await cmd("draw.selection.set", {"by": "ids", "ids": [c44b["line"]], "mode": "replace"})
        await cmd("window.panel.set", {"panelId": "properties", "visible": True})
        await asyncio.sleep(0.8)

    async def line_state():
        d = await cmd("draw.drawing.get", {"flatten": True})
        n = next((x for x in d.get("nodes") or [] if x.get("id") == c44b.get("line")), {}) or {}
        return {"kind": n.get("kind"), "thickness": (n.get("lineStyle") or {}).get("thickness")}

    rows.append({"name": "Tool options › Contour Editor: line invisible",
                 "spec": f"{spec_line('draw.stroke.set')} (label \"Edit Stroke\"); 02 §8 line properties (SI-196)",
                 "action": ("dom", "toolOptions.selectedLine.invisible"),
                 "expect": lambda b: ("draw.stroke.set", {"strokeId": f"{c44b['drawing']}/{c44b['line']}",
                                                          "kind": "pencil" if b.get("kind") == "invisible" else "invisible"}),
                 "label": "Edit Stroke", "setup": line_shown, "state": line_state, "toggle": True,
                 "effect": lambda b, a, k: a.get("kind") != b.get("kind"), "disabled": None})
    rows.append({"name": "Tool options › Contour Editor: line thickness",
                 "spec": f"{spec_line('draw.stroke.set')} (label \"Edit Stroke\"); 02 §8 line properties (SI-196); D-61",
                 "action": ("none",), "custom": typed_flow("toolOptions.selectedLine.thickness", "4.5"),
                 "expect": lambda b: ("draw.stroke.set", {"strokeId": f"{c44b['drawing']}/{c44b['line']}", "thickness": 4.5}),
                 "label": "Edit Stroke", "setup": line_shown, "state": line_state,
                 "effect": lambda b, a, k: a.get("thickness") == 4.5, "disabled": None,
                 "mcpReason": "a typed value is the person's input; the field's declared reference is the current value"})
    rows.append({"name": "Tool options › Contour Editor: line invisible in the Camera view, element locked",
                 "spec": f"{spec_line('draw.stroke.set')}; 05 §6.6, 04 §10.4; D-80; D-83 item 3 (refused, no undo step, nothing changes)",
                 "action": ("dom", "toolOptions.selectedLine.invisible"),
                 "expect": lambda b: ("draw.stroke.set", {"strokeId": f"{c44b['drawing']}/{c44b['line']}", "kind": "invisible"}),
                 "label": None, "setup": lambda: camera_locked(line_shown), "state": line_state,
                 "effect": lambda b, a, k: a == b, "disabled": None, "osOnly": True, "after": camera_unlocked,
                 "mcpReason": "an agent's direct call addresses the element by id and still acts (05 §6.6): the rule is the person's"})

    # The Text panel formatting the text the Text tool highlights (design note 5 §6): a session opened by the table's
    # agent connection (a Text tool click, `type_text`, then Shift+Home), so in the OS form the person's clicks format
    # the agent's session's highlight; the caret-only rows need the session's own origin and run in the MCP form.
    async def dock_settled(panel_id, visible):
        """A wait for a state (D-45): the dock shows `panel_id`'s tab or not, as just set, and the Drawing view's canvas
        has drawn. The window rebuilds the dock's grid on every arrangement change (`applyPlan`), so hiding or showing a
        panel remounts the view's canvas, and a gesture sent meanwhile can reach the canvas being unmounted, which
        refuses it (`ui.viewNotFound`, `windowClosed`; SI-141): run 45020557010, macOS, where the 0.8 s pause of
        `tool_fixture` was not enough."""
        start = time.monotonic()
        last = {}
        while time.monotonic() - start < 15.0:
            answer = await e2e.call(t.s, "inspect_ui", {"target": "window", "depth": 32})
            root = (answer.get("result") or answer).get("root") or {}
            tab = McpTable.node(root, f"tab.{panel_id}")
            try:
                last = json.loads((McpTable.node(root, "drawingView.canvas") or {}).get("value") or "{}")
            except json.JSONDecodeError:
                last = {}
            if (tab is not None) == visible and (last.get("drawn") or 0) > 0:
                return
            await asyncio.sleep(0.25)
        raise RuntimeError(f"the dock did not settle (tab.{panel_id} shown: {visible}); {json.dumps(last)[:300]}")

    async def text_session(highlight=True, two_runs=False, panel_after=False):
        # A fresh session on a fresh object: the last row's session ends with its tool, its objects go.
        await cmd("tool.active.set", {"toolId": "select"})
        nodes = (await cmd("draw.drawing.get", {"flatten": True})).get("nodes") or []
        old = [n.get("id") for n in nodes if "text" in n and "runs" in n]
        if old:
            await cmd("draw.object.delete", {"ids": old})
        # The Text panel first: showing it after the view could take the view's place in the dock. With `panel_after`
        # (D-85 item 2) it is hidden while the session opens and shown once it is open, so it mounts after the session.
        await cmd("window.panel.set", {"panelId": "text", "visible": not panel_after})
        await tool_fixture("text")
        await dock_settled("text", not panel_after)
        m = await view_map()
        x, y = to_view(m, -50, 75)
        env = await e2e.call(t.s, "pointer", {"view": "drawingView", "tool": "text",
                                              "events": [{"type": "down", "x": x, "y": y}, {"type": "up", "x": x, "y": y}]})
        res = env.get("result") or {}
        add = next((c for c in res.get("commands") or [] if c.get("commandId") == "draw.text.add"), {}) or {}
        c44b["text"] = ((add.get("envelope") or {}).get("result") or {}).get("textId")
        if not c44b["text"]:
            nodes = (await cmd("draw.drawing.get", {"flatten": True})).get("nodes") or []
            c44b["text"] = next((n.get("id") for n in reversed(nodes) if "text" in n and "runs" in n), None)
        if not c44b["text"]:
            raise RuntimeError(f"the Text tool's click made no text object: {json.dumps(env)[:400]}")
        await e2e.call(t.s, "type_text", {"text": "Hello"})
        if two_runs:
            await cmd("draw.textFormat.set", {"textId": c44b["text"], "start": 0, "length": 2, "bold": True})
        if highlight:
            await e2e.call(t.s, "key", {"keys": "Shift+Home"})
        if panel_after:
            await cmd("window.panel.set", {"panelId": "text", "visible": True})
        # A wait for a state (D-45): the panel shows the session's role.
        want = "Selected text" if highlight else "Next typed text"
        start = time.monotonic()
        while time.monotonic() - start < 15.0:
            answer = await e2e.call(t.s, "inspect_ui", {"target": "window", "depth": 32})
            n = McpTable.node((answer.get("result") or answer).get("root") or {}, "text.role") or {}
            if n.get("label") == want:
                break
            await asyncio.sleep(0.25)
        await asyncio.sleep(0.3)

    async def text_runs():
        d = await cmd("draw.drawing.get", {"flatten": True})
        n = next((x for x in d.get("nodes") or [] if x.get("id") == c44b.get("text")), {}) or {}
        runs = n.get("runs") or [{}]
        paras = n.get("paragraphs") or [{}]
        r0, p0 = runs[0], paras[0]
        return {"bold": r0.get("bold"), "italic": r0.get("italic"), "autoKern": r0.get("autoKern"),
                "align": p0.get("align"), "baseline": r0.get("baseline"), "runs": runs, "paragraphs": paras,
                "text": n.get("text")}

    def fmt(field, value):
        return ("draw.textFormat.set", {"drawingId": c44b["drawing"], "textId": c44b["text"], "start": 0, "length": 5,
                                        field: value})

    for fmt_flag in ("bold", "italic", "autoKern"):
        rows.append({"name": f"Text panel (selected text) › {fmt_flag}",
                     "spec": f"{spec_line('draw.textFormat.set')} (\"Text Property Modified\"); 02 §7.6.1 lines 677-678",
                     "action": ("dom", f"text.{fmt_flag}"),
                     "expect": (lambda f: lambda b: fmt(f, not b.get(f)))(fmt_flag), "label": "Text Property Modified",
                     "setup": text_session, "state": text_runs, "toggle": True,
                     "effect": (lambda f: lambda b, a, k: a.get(f) == (not b.get(f)))(fmt_flag), "disabled": None})
    # A choice other than the current one (the defaults rows above may have left New text centred or superscript).
    def choice_session(key_, values):
        async def run():
            await text_session()
            cur = (await text_runs()).get(key_)
            c44b[f"target.{key_}"] = next(v for v in values if v != cur)
        return run

    for key_, values in (("align", ("centre", "right")), ("baseline", ("superscript", "subscript"))):
        rows.append({"name": f"Text panel (selected text) › {key_}",
                     "spec": f"{spec_line('draw.textFormat.set')} (\"Text Property Modified\"); 02 §7.6.1",
                     "action": (lambda k_: lambda: ("select", f"text.{k_}", c44b[f"target.{k_}"]))(key_),
                     "expect": (lambda k_: lambda b: fmt(k_, c44b[f"target.{k_}"]))(key_), "label": "Text Property Modified",
                     "setup": choice_session(key_, values), "state": text_runs,
                     "effect": (lambda k_: lambda b, a, k: a.get(k_) == c44b[f"target.{k_}"])(key_), "disabled": None})
    # Values other than those the defaults rows above type (new text takes the defaults).
    for ident, typed, field, value in (("text.size", "52", "size", 52), ("text.font", "Verdana", "font", "Verdana"),
                                       ("text.kerning", "7", "kerning", 7), ("text.leftMargin", "14", "leftMargin", 14),
                                       ("text.rightMargin", "9", "rightMargin", 9), ("text.indent", "6", "indent", 6),
                                       ("text.lineSpacing", "5", "lineSpacing", 5)):
        rows.append({"name": f"Text panel (selected text) › {field} (typed)",
                     "spec": f"{spec_line('draw.textFormat.set')} (\"Text Property Modified\"); 02 §7.6.1; D-61",
                     "action": ("none",), "custom": text_field_flow(ident, typed),
                     "expect": (lambda f_, v_: lambda b: fmt(f_, v_))(field, value), "label": "Text Property Modified",
                     "setup": text_session, "state": text_runs,
                     "effect": lambda b, a, k: a != b, "disabled": None,
                     "mcpReason": "a typed value is the person's input; the field's declared reference is the current value"})

    async def carried():
        # The next keystroke carries the pending field (SI-195): its run is bold.
        await e2e.call(t.s, "type_text", {"text": "!"})
        st = await text_runs()
        last = (st.get("runs") or [{}])[-1]
        t.run.check(last.get("bold") is True and last.get("length") == 1,
                    "D-39 Text panel (caret) › bold: the next typed character carries it (SI-195)", st.get("runs"))

    rows.append({"name": "Text panel (caret) › bold",
                 "spec": f"{spec_line('draw.textFormat.set')}; 02 §7.6.1 lines 677-679 (SI-195: no step; the next keystroke)",
                 "action": ("dom", "text.bold"),
                 "expect": lambda b: ("draw.textFormat.set", {"drawingId": c44b["drawing"], "textId": c44b["text"], "start": 5,
                                                              "length": 0, "bold": True}),
                 "label": None, "setup": lambda: text_session(highlight=False), "state": text_runs,
                 "effect": lambda b, a, k: a == b, "disabled": None, "after": carried,
                 "mcpOnly": True, "osReason": "the pending field belongs to the session's own origin (the table's agent "
                                             "connection opened it); the person's own session is the three-way script's"})

    rows.append({"name": "Text panel (caret) › opened while editing, then Bold, then a keystroke",
                 "spec": f"{spec_line('draw.textFormat.set')}; 02 §7.6.1 lines 677-679; D-85 item 2 (C44B-R2-02: a panel "
                         "that mounts after the session began reads it)",
                 "action": ("dom", "text.bold"),
                 "expect": lambda b: ("draw.textFormat.set", {"drawingId": c44b["drawing"], "textId": c44b["text"], "start": 5,
                                                              "length": 0, "bold": True}),
                 "label": None, "setup": lambda: text_session(highlight=False, panel_after=True), "state": text_runs,
                 "effect": lambda b, a, k: a == b, "disabled": None, "after": carried,
                 "mcpOnly": True, "osReason": "the pending field belongs to the session's own origin (the table's agent "
                                             "connection opened it); the person's own session is the three-way script's"})

    async def mixed_shown():
        answer = await e2e.call(t.s, "inspect_ui", {"target": "window", "depth": 32})
        root = (answer.get("result") or answer).get("root") or {}
        return {"bold": (McpTable.node(root, "text.bold") or {}).get("value"),
                "italic": (McpTable.node(root, "text.italic") or {}).get("value"),
                "role": (McpTable.node(root, "text.role") or {}).get("label")}

    async def typing_setup():
        """A real click on empty canvas with the Text tool: a new text object and its session (design note 5 §2)."""
        await cmd("tool.active.set", {"toolId": "select"})
        # The Text panel's rows leave a text object at the click's place: the canvas is emptied of text first.
        nodes = (await cmd("draw.drawing.get", {"flatten": True})).get("nodes") or []
        old = [n.get("id") for n in nodes if "text" in n and "runs" in n]
        if old:
            await cmd("draw.object.delete", {"ids": old})
        await tool_fixture("text")
        m = await view_map()
        x, y = to_view(m, -50, 75)
        await t.input_gesture("drawingView", "text", [(x, y), (x, y)], [])
        await asyncio.sleep(1.0)

    async def typing_flow():
        """Real typing into the canvas's editor (design note 5 §4)."""
        c44b["focus"] = t.cdp.js("(() => { const e = document.activeElement; return e ? (e.getAttribute('data-tt-id') || e.tagName) : null; })()")
        type_text("Hi")
        await asyncio.sleep(1.0)
        return f"'Hi' typed by real keys into the focused editor (focus: {c44b['focus']})"

    async def typed_texts():
        nodes = (await cmd("draw.drawing.get", {"flatten": True})).get("nodes") or []
        return sorted(n.get("text") for n in nodes if "text" in n and "runs" in n)

    rows.append({"name": "Text tool › click, then typing in the editor",
                 "spec": f"{spec_line('draw.text.set')} (\"Typing\"); 02 §7.6 lines 669-670; design note 5 §4; SI-191",
                 "action": ("none",), "custom": typing_flow,
                 # The keystrokes reach the kernel through `text_input` (not `run_command`): one "Typing" step.
                 "expect": None, "label": "Typing",
                 "setup": typing_setup, "state": typed_texts,
                 "effect": lambda b, a, k: "Hi" in a, "disabled": None,
                 "mcpReason": "real keys typed into the canvas's editor; an agent types with type_text (the Text panel rows)"})

    async def typed_hi():
        """A real click on empty canvas with the Text tool, then "Hi" typed by real keys (one Typing step)."""
        await typing_setup()
        type_text("Hi")
        await asyncio.sleep(1.0)

    async def undo_in_editor():
        """SI-193 (finding C44B-R1S-04, ruling D-78 item 5): Ctrl+Z pressed with the canvas's editor focused."""
        c44b["focus"] = t.cdp.js("(() => { const e = document.activeElement; return e ? (e.getAttribute('data-tt-id') || e.tagName) : null; })()")
        key(0x5A, 0x11)
        await asyncio.sleep(1.0)
        return f"Ctrl+Z pressed by real keys in the focused editor (focus: {c44b['focus']})"

    rows.append({"name": "Text tool › Primary+Z in the editor undoes the typing (the document's undo)",
                 "spec": "SI-193 (Primary+Z is the document's undo while text is edited in a view); 01 §8.4; finding C44B-R1S-04",
                 "action": ("none",), "custom": undo_in_editor,
                 # The keymap sends the document's undo (the editor's text area keeps no history of its own).
                 "expect": ("edit.undo", "the document's undo, sent by the keymap"), "label": None,
                 "match": lambda prm, b: True,
                 "setup": typed_hi, "state": typed_texts,
                 "effect": lambda b, a, k: any("Hi" in x for x in b) and not any("Hi" in x for x in a), "disabled": None,
                 "mcpReason": "real keys in the canvas's editor; the kernel's resolution of the chord in that field is "
                              "c44b_text.rs in_the_canvas_text_editor_undo_is_the_documents"})

    rows.append({"name": "Text panel (selected text) › a range over two runs shows Mixed",
                 "spec": "02 §7.6.1; design note 5 §6 (a field that differs over the range is Mixed)",
                 "action": ("none",), "expect": None, "label": None,
                 "setup": lambda: text_session(two_runs=True), "state": mixed_shown,
                 "effect": lambda b, a, k: a.get("bold") == "mixed" and a.get("italic") == "false" and a.get("role") == "Selected text",
                 "disabled": None})

    async def agent_bold():
        await text_session()
        await cmd("draw.textFormat.set", {"textId": c44b["text"], "start": 0, "length": 5, "italic": True})
        await asyncio.sleep(0.8)

    rows.append({"name": "Text panel (selected text) › an agent's change is shown",
                 "spec": "02 §7.6.1; D-40 (an agent's draw.textFormat.set shows in the panel)",
                 "action": ("none",), "expect": None, "label": None, "setup": agent_bold, "state": mixed_shown,
                 "effect": lambda b, a, k: a.get("italic") == "true", "disabled": None})

    # ----- C4.4a: the drawing inspector, the Properties panel's Drawing page (02 §9; design note 1 §6) -----
    async def inspector_shown():
        await cmd("window.panel.set", {"panelId": "properties", "visible": True})
        await cmd("app.context.set", {"elementId": bob, "frame": 1})
        await asyncio.sleep(0.6)
        t.click("properties.page.drawing")
        await asyncio.sleep(0.6)

    async def preview_state():
        return (await cmd("prefs.get", {"keys": ["drawing.inspectorPreview"]})).get("values") or {}

    async def drawings_of_bob():
        return await cmd("element.drawing.list", {"elementId": bob})

    def first_drawing(st):
        return ((st.get("drawings") or [{}])[0])

    rows.append({"name": "Drawing inspector › Show preview", "spec": f"{spec_line('draw.inspectorPreview.set')}; 02 §9",
                 "action": ("dom", "inspector.preview"),
                 "expect": lambda b: ("draw.inspectorPreview.set", {"visible": b.get("drawing.inspectorPreview") is False}),
                 "label": None, "setup": inspector_shown, "state": preview_state, "toggle": True,
                 "effect": lambda b, a, k: a.get("drawing.inspectorPreview") != b.get("drawing.inspectorPreview"),
                 "disabled": None, "osOnly": True,
                 "mcpReason": "the Drawing page opens with a click on its tab, the window's own state that real input sets"})

    async def rename_drawing_flow():
        front()
        t.cdp.js("(() => { const e = document.querySelector('[data-tt-id=\"inspector.name\"]'); e.focus(); e.select(); return true; })()")
        type_text("D39 key")
        key(0x0D)
        return "the name typed in the focused field, then a real Enter"

    rows.append({"name": "Drawing inspector › rename", "spec": f"{spec_line('draw.drawing.rename')} (label \"Rename Drawing\"); 02 §9",
                 "action": ("none",), "custom": rename_drawing_flow,
                 "expect": lambda b: ("draw.drawing.rename", {"drawingId": first_drawing(b).get("id"), "name": "D39 key"}),
                 "label": "Rename Drawing", "setup": inspector_shown, "state": drawings_of_bob,
                 "effect": lambda b, a, k: first_drawing(a).get("name") == "D39 key", "disabled": None,
                 "mcpReason": "the Drawing page opens with a click on its tab, and the name is typed"})

    # ----- C4.4a: the main toolbar's camera list (01 §5.7; design note 1 §8) -----
    cam_target: dict = {}

    async def cameras_state():
        listed = (await cmd("anim.camera.list", {})).get("cameras") or []
        return {"cameras": listed, "current": (await cmd("anim.camera.current.get", {})).get("cameraId")}

    async def two_cameras():
        st = await cameras_state()
        if len(st["cameras"]) < 2:
            await cmd("anim.camera.create", {"name": "D39 Wide"})
            st = await cameras_state()
        cam_target["id"] = next(c["id"] for c in st["cameras"] if c["id"] != st["current"])
        await asyncio.sleep(0.6)

    rows.append({"name": "main toolbar › camera list", "spec": f"{spec_line('anim.camera.current.set')} (label \"Set Current Camera\"); 01 §5.7",
                 "action": lambda: ("select", "mainToolbar.cameraList", cam_target["id"]),
                 "expect": lambda b: ("anim.camera.current.set", {"cameraId": cam_target["id"]}), "label": "Set Current Camera",
                 "setup": two_cameras, "state": cameras_state,
                 "effect": lambda b, a, k: a["current"] == cam_target["id"] != b["current"], "disabled": None})

    # ----- C4.4a: Window › Customize Toolbar… (01 §5.7; design note 1 §7): the sheet's Apply sends the items -----
    async def toolbar_items():
        listed = await cmd("window.toolbar.list", {})
        listed = listed if isinstance(listed, list) else []
        return next((x.get("items") for x in listed if x.get("toolbarId") == "main"), [])

    async def toolbar_reset():
        await cmd("window.toolbar.items.reset", {})
        await asyncio.sleep(0.4)

    tb_before: dict = {}

    async def customize_flow():
        tb_before["items"] = await toolbar_items()
        choose_menu(["Window", "Customize Toolbar"])
        await asyncio.sleep(1.0)
        t.clear()
        t.click("toolbarSheet.item.0")
        t.click("toolbarSheet.remove")
        await asyncio.sleep(0.3)
        t.click("toolbarSheet.submit")
        await asyncio.sleep(1.0)
        return "real native menu Window › Customize Toolbar, the first item removed, a real click on Apply"

    rows.append({"name": "Window › Customize Toolbar", "spec": f"{spec_line('window.toolbar.items.set')}; 01 §5.7 lines 445-464",
                 "action": ("none",), "custom": customize_flow,
                 "expect": lambda b: ("window.toolbar.items.set", {"toolbarId": "main", "items": b[1:]}), "label": None,
                 "setup": toolbar_reset, "state": toolbar_items,
                 "effect": lambda b, a, k: a == b[1:], "disabled": None, "after": toolbar_reset,
                 "mcpReason": "the sheet opens from the native menu, which only real input reaches"})

    # ----- C4.4a: the views' context menus' edit items (01 §6; 02 §4.4; D-40 answer 12; design note 1 §9) -----
    def in_bob(*steps):
        # Bob's drawing current whatever the rows before chose (the drawing steps act on the current drawing).
        async def run():
            await cmd("app.context.set", {"elementId": bob, "frame": 1})
            await setup(*steps)()
        return run

    async def edit_state():
        d = await drawing()
        sel = await cmd("draw.selection.get", {})
        clip = (await e2e.run_cmd(t.s, "edit.clipboard.get", {})).get("result") or {}
        return {"strokes": len(strokes_of(d)), "selected": len(sel.get("ids") or sel.get("selectedIds") or []),
                "clipboard": json.dumps(clip, sort_keys=True)[:4000]}

    async def deselect():
        await cmd("draw.selection.set", {"by": "none"})

    async def copy_left_only():
        d = await drawing()
        await cmd("draw.selection.set", {"by": "ids", "ids": [strokes_of(d)[0]["id"]]})
        # The drawing's selection named: in the Camera view the focused context is the staging one (D-60 item 2).
        await cmd("edit.copy", {"context": "drawing"})

    async def copy_all():
        await select_all()
        await cmd("edit.copy", {"context": "drawing"})

    for item, label, command, spec_label, steps, eff in (
        ("cut", "Cut", "edit.cut", "Cut", [lambda: rect(0, 0), select_all], lambda b, a, k: a["strokes"] < b["strokes"]),
        ("copy", "Copy", "edit.copy", None, [lambda: rect(-150, 0), copy_left_only, lambda: rect(150, 0), select_all],
         lambda b, a, k: a["clipboard"] != b["clipboard"]),
        ("paste", "Paste", "edit.paste", "Paste", [lambda: rect(0, 0), copy_all], lambda b, a, k: a["strokes"] > b["strokes"]),
        ("selectAll", "Select All", "edit.selectAll", None, [lambda: rect(-100, 0), lambda: rect(100, 0), deselect],
         lambda b, a, k: b["selected"] == 0 and a["selected"] > 0),
        ("deselectAll", "Deselect All", "edit.deselectAll", None, [lambda: rect(0, 0), select_all],
         lambda b, a, k: b["selected"] > 0 and a["selected"] == 0),
    ):
        rows.append({"name": f"Drawing view context menu › {label}", "spec": f"{spec_line(command)}; 01 §6; 02 §4.4 lines 291-296",
                     "action": ("context", "drawingView.canvas", f"drawingView.drawingMenu.{item}"),
                     "expect": (command, {"context": "drawing"}),
                     "label": spec_label, "setup": in_bob(lambda: focus_view("drawingView"), *steps), "state": edit_state,
                     "effect": eff, "disabled": None})

    # D-60 item 2 (C44A-R1-02): the Camera view's Drawing family (a drawing tool active) acts on the drawing's
    # selection, never the staging one (01 §6.4). Each row starts with a staging selection the item must leave as it
    # was: Bob selected in staging (empty for Select All, which must not select the scene's elements).
    async def both_state():
        st = await edit_state()
        staging = await cmd("edit.selection.get", {"context": "staging"})
        els = (await e2e.run_cmd(t.s, "element.list", {})).get("result") or {}
        els = els.get("elements") if isinstance(els, dict) else els
        return {**st, "staging": sorted(staging.get("ids") or []), "elements": len(els or [])}

    async def camera_drawing_family():
        await focus_view("cameraView")
        # The family chosen last runs in the Camera view (SI-148): a drawing tool chosen now.
        await cmd("tool.active.set", {"toolId": "brush"})
        await cmd("tool.active.set", {"toolId": "select"})

    async def staging_bob():
        await cmd("edit.selection.set", {"context": "staging", "ids": [bob]})

    async def staging_none():
        await cmd("edit.selection.set", {"context": "staging", "ids": []})

    def kept(eff):
        return lambda b, a, k: eff(b, a, k) and a["staging"] == b["staging"] and a["elements"] == b["elements"]

    for item, label, command, spec_label, steps, eff in (
        ("cut", "Cut", "edit.cut", "Cut", [lambda: rect(0, 0), select_all, staging_bob],
         lambda b, a, k: a["strokes"] < b["strokes"]),
        ("copy", "Copy", "edit.copy", None, [lambda: rect(-150, 0), copy_left_only, lambda: rect(150, 0), select_all, staging_bob],
         lambda b, a, k: a["clipboard"] != b["clipboard"]),
        ("paste", "Paste", "edit.paste", "Paste", [lambda: rect(0, 0), copy_all, staging_bob],
         lambda b, a, k: a["strokes"] > b["strokes"]),
        ("selectAll", "Select All", "edit.selectAll", None, [lambda: rect(-100, 0), lambda: rect(100, 0), deselect, staging_none],
         lambda b, a, k: b["selected"] == 0 and a["selected"] > 0),
        ("deselectAll", "Deselect All", "edit.deselectAll", None, [lambda: rect(0, 0), select_all, staging_bob],
         lambda b, a, k: b["selected"] > 0 and a["selected"] == 0),
    ):
        rows.append({"name": f"Camera view context menu (drawing tool) › {label}",
                     "spec": f"{spec_line(command)}; 01 §6.4; 02 §4.4 lines 291-296; D-60 item 2",
                     "action": ("context", "cameraView.canvas", f"cameraView.drawingMenu.{item}"),
                     "expect": (command, {"context": "drawing"}),
                     "label": spec_label, "setup": in_bob(camera_drawing_family, *steps), "state": both_state,
                     "effect": kept(eff), "disabled": None})

    # The Camera view's scene family (a scene tool running): the same items act on the staging selection.
    ann = {}

    async def scene_state():
        els = (await e2e.run_cmd(t.s, "element.list", {})).get("result") or {}
        els = els.get("elements") if isinstance(els, dict) else els
        sel = await cmd("edit.selection.get", {"context": "staging"})
        clip = (await e2e.run_cmd(t.s, "edit.clipboard.get", {})).get("result") or {}
        return {"elements": len(els or []), "selected": len(sel.get("ids") or []),
                "clipboard": json.dumps(clip, sort_keys=True)[:4000]}

    async def scene_ready():
        await cmd("app.context.set", {"elementId": bob, "frame": 1})
        await clean()
        await focus_view("cameraView")
        # The family chosen last runs in the Camera view (SI-148): a scene tool chosen now, so the change is announced
        # even when Select was the scene tool already (a row before ran the Drawing family).
        await cmd("anim.tool.set", {"tool": "transform"})
        await cmd("anim.tool.set", {"tool": "select"})
        if not ann.get("id"):
            ann["id"] = ((await cmd("element.create", {"type": "drawing", "name": "Ann"})).get("elementIds") or [None])[0]
            await cmd("element.drawing.create", {"elementId": ann["id"], "name": "1", "exposeFrom": 1, "exposeTo": 24})
        await cmd("edit.selection.set", {"context": "staging", "ids": []})

    async def select_ann():
        await cmd("edit.selection.set", {"context": "staging", "ids": [ann["id"]]})

    async def copy_ann():
        await select_ann()
        await cmd("edit.copy", {})

    async def ann_back():
        # The cut element is pasted back for the rows after it.
        ann["id"] = None

    def scene_setup(*steps):
        async def run():
            await scene_ready()
            for st in steps:
                await st()
        return run

    for item, label, command, spec_label, steps, eff, after in (
        ("copy", "Copy", "edit.copy", None, [select_ann], lambda b, a, k: a["clipboard"] != b["clipboard"], None),
        ("selectAll", "Select All", "edit.selectAll", None, [], lambda b, a, k: b["selected"] == 0 and a["selected"] > 0, None),
        ("deselectAll", "Deselect All", "edit.deselectAll", None, [select_ann],
         lambda b, a, k: b["selected"] > 0 and a["selected"] == 0, None),
        ("paste", "Paste", "edit.paste", "Paste", [copy_ann], lambda b, a, k: a["elements"] > b["elements"], None),
        ("cut", "Cut", "edit.cut", "Cut", [select_ann], lambda b, a, k: a["elements"] < b["elements"], ann_back),
    ):
        rows.append({"name": f"Camera view context menu (scene tool) › {label}", "spec": f"{spec_line(command)}; 01 §6; D-40 answer 12",
                     "action": ("context", "cameraView.canvas", f"cameraView.sceneMenu.{item}"),
                     "expect": (command, {"context": "staging"}),
                     "label": spec_label, "setup": scene_setup(*steps), "state": scene_state, "effect": eff,
                     "disabled": None, **({"after": after} if after else {})})

    # ----- C4.4a: the dock tab's panel checklist on the shared menu (01 §5.4 lines 387-388; design note 1 §9): each
    # panel's visibility toggled as the kernel completes the reference (shown and in front: hide; else show) -----
    async def panel_list():
        return {p["panelId"]: p for p in await cmd("window.panel.list", {}) or []}

    def front_shown(b, pid):
        e = b.get(pid) or {}
        return bool(e.get("visible") and (e.get("current") or e.get("floating")))

    checklist = sorted(i.split(".", 1)[1] for i in (r["id"] for r in json.loads(
        (ROOT / "desktop/src/ui-bindings/parity.json").read_text(encoding="utf-8"))["dom"]) if i.startswith("panelChecklist."))
    # The central view's item is disabled: it is never hidden or floated (prechecks.rs `window_panel_set`; the checklist
    # shows the kernel's reason), so its row is the Vitest's disabled item, not a toggle here.
    for pid in (p_ for p_ in checklist if p_ != "cameraView"):
        tab = "tab.cameraView"
        rows.append({"name": f"dock tab menu › {pid}", "spec": f"{spec_line('window.panel.set')}; 01 §5.4 lines 387-388",
                     "action": ("context", tab, f"panelChecklist.{pid}"),
                     "expect": (lambda p_: lambda b: ("window.panel.set", {"panelId": p_, "visible": not front_shown(b, p_)}))(pid),
                     "label": None, "setup": None, "state": panel_list, "toggle": True,
                     "effect": (lambda p_: lambda b, a, k: (a.get(p_) or {}) != (b.get(p_) or {}))(pid), "disabled": None,
                     "osOnly": True,
                     "mcpReason": "the checklist opens with a secondary click on a tab; its reference is the Window menu's "
                                  "panel item's, completed by the kernel as a toggle"})

    # ----- C4.4a: Preferences › Agent access (01 §7.3; 11 §15.1; design note 2 §2): each control edited, then OK, which
    # sends that group's one command (never prefs.set). The server's own check box is not a row here: switching it off
    # ends this table's HTTP session (the agent e2e switches it off and on); Allow remote connections is disabled. -----
    SEED_POLICY = {"policy": "autonomous", "maxJobs": 2, "maxBatchSteps": 1000, "maxInlineImagePx": 2048,
                   "maxInlineBytes": 52428800, "askInApp": True, "letAgentMoveView": True, "highlightAgentChanges": True}
    AGENT_KEYS = ["agent.server.port", "agent.policy.defaults", "agent.typedTools", "agent.activity.persist"]

    async def agent_prefs():
        return (await cmd("prefs.get", {"keys": AGENT_KEYS})).get("values") or {}

    async def user_cmd(command, params):
        # The person's call, from the page (scope `app` refuses an agent's): the setup's own, before the row records.
        t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command', {json: JSON.stringify({command: %s, params: %s})}).then(() => true)"
                 % (json.dumps(command), json.dumps(params)))
        await asyncio.sleep(0.3)

    async def agent_defaults():
        # The server runs from the launch flag; its preference on too, so the page's port field is enabled.
        await user_cmd("prefs.set", {"values": {"agent.server.enabled": True, "agent.server.port": 47215}})
        await user_cmd("agent.policy.defaults.set", SEED_POLICY)
        await user_cmd("agent.typedTools.set", {"enabled": False, "namespaces": []})
        await user_cmd("agent.activity.persist.set", {"enabled": False})

    async def shown_dom(ident, timeout=10.0):
        # A wait for a state (D-45): the element is in the page.
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if t.cdp.js("!!document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']')" % json.dumps(ident)):
                return True
            await asyncio.sleep(0.1)
        return False

    def agent_flow(edit, label):
        async def flow():
            choose_menu(["File", "Preferences"])
            if not await shown_dom("preferences.category.agent"):
                raise RuntimeError("the Preferences dialog did not open")
            t.click("preferences.category.agent")
            if not await shown_dom("agentAccess.status"):
                raise RuntimeError("the Agent access page did not show")
            # Opening the dialog is the menu item's own command (its own row): the row records what OK sends.
            t.clear()
            await edit()
            await asyncio.sleep(0.3)
            t.click("preferences.submit")
            await asyncio.sleep(1.2)
            return f"real input in Preferences › Agent access ({label}), then OK"
        return flow

    def typed_into(ident, text):
        async def edit():
            await shown_dom(ident)
            front()
            t.cdp.js("(() => { const e = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']'); e.focus(); e.select(); return true; })()"
                     % json.dumps(ident))
            type_text(text)
        return edit

    def clicked(ident):
        async def edit():
            t.click(ident)
        return edit

    async def choose_observe():
        # The policy list: a real click, then the arrow keys to Observe (the first option) and Enter.
        t.click("agentAccess.policy")
        await asyncio.sleep(0.3)
        for _ in range(3):
            key(0x26)
        key(0x0D)

    pol = lambda v: v.get("agent.policy.defaults") or {}  # noqa: E731
    agent_rows = [
        ("port", typed_into("agentAccess.port", "47300"), ("agent.server.set", {"enabled": True, "port": 47300}),
         lambda b, a, k: a.get("agent.server.port") == 47300),
        ("policy", choose_observe, ("agent.policy.defaults.set", {"policy": "observe"}),
         lambda b, a, k: pol(a).get("policy") == "observe"),
        ("maxJobs", typed_into("agentAccess.maxJobs", "3"), ("agent.policy.defaults.set", {"maxJobs": 3}),
         lambda b, a, k: pol(a).get("maxJobs") == 3),
        ("maxBatchSteps", typed_into("agentAccess.maxBatchSteps", "500"), ("agent.policy.defaults.set", {"maxBatchSteps": 500}),
         lambda b, a, k: pol(a).get("maxBatchSteps") == 500),
        ("maxInlineImagePx", typed_into("agentAccess.maxInlineImagePx", "1024"),
         ("agent.policy.defaults.set", {"maxInlineImagePx": 1024}), lambda b, a, k: pol(a).get("maxInlineImagePx") == 1024),
        ("maxInlineBytes", typed_into("agentAccess.maxInlineBytes", "8"), ("agent.policy.defaults.set", {"maxInlineBytes": 8388608}),
         lambda b, a, k: pol(a).get("maxInlineBytes") == 8388608),
        ("askInApp", clicked("agentAccess.askInApp"), ("agent.policy.defaults.set", {"askInApp": False}),
         lambda b, a, k: pol(a).get("askInApp") is False),
        ("letAgentMoveView", clicked("agentAccess.letAgentMoveView"), ("agent.policy.defaults.set", {"letAgentMoveView": False}),
         lambda b, a, k: pol(a).get("letAgentMoveView") is False),
        ("highlightAgentChanges", clicked("agentAccess.highlightAgentChanges"),
         ("agent.policy.defaults.set", {"highlightAgentChanges": False}), lambda b, a, k: pol(a).get("highlightAgentChanges") is False),
        ("typedTools", clicked("agentAccess.typedTools"), ("agent.typedTools.set", {"enabled": True, "namespaces": []}),
         lambda b, a, k: (a.get("agent.typedTools") or {}).get("enabled") is True),
        ("persist", clicked("agentAccess.persist"), ("agent.activity.persist.set", {"enabled": True}),
         lambda b, a, k: a.get("agent.activity.persist") is True),
    ]
    for a_ident, a_edit, a_expect, a_eff in agent_rows:
        rows.append({"name": f"Preferences › Agent access › {a_ident}", "spec": f"{spec_line(a_expect[0])}; 01 §7.3 (Agent access); 11 §15.1",
                     "action": ("none",), "custom": agent_flow(a_edit, a_ident), "expect": a_expect, "label": None,
                     "setup": agent_defaults, "state": agent_prefs, "effect": a_eff, "disabled": None, "after": agent_defaults,
                     "mcpReason": "the controls are staged in the Preferences dialog and sent by its OK"})

    # D-61: the dialog's numeric fields refuse letters and an out-of-range value at the field (read after a real Tab
    # moves the focus out of it); OK then sends nothing and the preferences keep their values.
    seen_fields: dict = {}

    def typed_then_tab(ident, text):
        async def edit():
            await typed_into(ident, text)()
            key(0x09)
            await asyncio.sleep(0.3)
            seen_fields[ident] = t.cdp.js(
                "(() => { const e = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']');"
                " return e ? {value: e.value, problem: e.dataset.ttProblem ?? null} : null; })()" % json.dumps(ident))
        return edit

    def staged_refused(ident, retained):
        got = seen_fields.get(ident) or {}
        return got.get("problem") is not None and _same_number(got.get("value"), retained)

    MB = 1048576
    for a_ident, a_bad, a_read in (
            ("port", "80", lambda v: v.get("agent.server.port")),
            ("maxJobs", "9", lambda v: pol(v).get("maxJobs")),
            ("maxBatchSteps", "100001", lambda v: pol(v).get("maxBatchSteps")),
            ("maxInlineImagePx", "100", lambda v: pol(v).get("maxInlineImagePx")),
            ("maxInlineBytes", "2000", lambda v: (pol(v).get("maxInlineBytes") or 0) / MB)):
        for a_kind, a_text in (("invalid", "abc"), ("out of range", a_bad)):
            f_ident = f"agentAccess.{a_ident}"
            rows.append({"name": f"Preferences › Agent access › {a_ident} ({a_kind})", "spec": "01 §7.3 (Agent access); D-61",
                         "action": ("none",), "custom": agent_flow(typed_then_tab(f_ident, a_text), a_ident),
                         "expect": None, "label": None, "setup": agent_defaults, "state": agent_prefs,
                         "effect": (lambda i_, r_: lambda b, a, k: a == b and staged_refused(i_, r_(b)))(f_ident, a_read),
                         "disabled": None, "after": agent_defaults,
                         "mcpReason": "the controls are staged in the Preferences dialog and sent by its OK"})

    # D-62 item 2 (C44A-R1-41): the agent token in Preferences. Copy puts it on the clipboard (read back and compared
    # with the discovery file's by hash); Show reveals it to the person while no surface an agent reads carries it: the
    # window tree reports it redacted, a capture made while it is shown has the mask painted (the field's pixels equal
    # those of the masked field), and neither the activity export nor the application's log contains it.
    token_case: dict = {}

    def discovery_token():
        return json.loads((t.work / "discovery.json").read_text(encoding="utf-8")).get("token") or ""

    def sha(text):
        import hashlib
        return hashlib.sha256((text or "").encode("utf-8")).hexdigest()

    async def open_agent_page():
        choose_menu(["File", "Preferences"])
        if not await shown_dom("preferences.category.agent"):
            raise RuntimeError("the Preferences dialog did not open")
        t.click("preferences.category.agent")
        if not await shown_dom("agentAccess.token"):
            raise RuntimeError("the Agent access page did not show")

    def token_field():
        return t.cdp.js("(() => { const e = document.querySelector('[data-tt-id=\"agentAccess.token\"]'); if (!e) return null;"
                        " const b = e.getBoundingClientRect(); return {value: e.value, x: b.x, y: b.y, w: b.width, h: b.height,"
                        " vw: window.innerWidth}; })()")

    def crop(png, f):
        w, h, ch, rows_ = c42.png_rgba(png)
        k = w / f["vw"]
        x0, y0 = int(f["x"] * k), int(f["y"] * k)
        x1, y1 = min(w, int((f["x"] + f["w"]) * k)), min(h, int((f["y"] + f["h"]) * k))
        return [bytes(r[x0 * ch:x1 * ch]) for r in rows_[y0:y1]]

    def overpainted(png, f):
        """Every pixel of the field's rectangle is the host's overpaint colour (D-69 item 1: 0x80 grey)."""
        w, h, ch, rows_ = c42.png_rgba(png)
        k = w / f["vw"]
        x0, y0 = int(f["x"] * k), int(f["y"] * k)
        x1, y1 = min(w, int((f["x"] + f["w"]) * k)), min(h, int((f["y"] + f["h"]) * k))
        grey = bytes([0x80, 0x80, 0x80])
        return x1 > x0 and y1 > y0 and all(bytes(r[x * ch:x * ch + 3]) == grey
                                           for r in rows_[y0:y1] for x in range(x0, x1))

    async def token_copy_flow():
        await open_agent_page()
        t.clear()
        t.click("agentAccess.token.copy")
        await asyncio.sleep(0.6)
        token_case["clipboard"] = sha(clipboard_text())
        token_case["copyTree"] = json.dumps(await e2e.call(t.s, "inspect_ui", {"target": "window", "depth": 32}))
        key(0x1B)
        await asyncio.sleep(0.5)
        return "real clicks in Preferences › Agent access: Copy"

    async def token_show_flow():
        await open_agent_page()
        before = token_field()
        masked = c42.png_of(await e2e.run_cmd(t.s, "inspect.screenshot", {"target": "window"}))
        t.clear()
        t.click("agentAccess.token.show")
        await asyncio.sleep(0.6)
        shown = token_field()
        token_case["shown"] = sha((shown or {}).get("value"))
        # What an agent reads while the person has it shown.
        token_case["tree"] = json.dumps(await e2e.call(t.s, "inspect_ui", {"target": "window", "depth": 32}))
        shot = c42.png_of(await e2e.run_cmd(t.s, "inspect.screenshot", {"target": "window"}))
        await asyncio.sleep(0.3)
        token_case["afterCapture"] = (token_field() or {}).get("value")
        token_case["sameAsMasked"] = bool(masked and shot and before and crop(masked, before) == crop(shot, before))
        # D-69 item 1: the host painted the field's rectangle over in the capture taken while it is shown.
        token_case["overpainted"] = bool(shot and before and overpainted(shot, before))
        export = t.work / "token-activity.jsonl"
        await e2e.run_cmd(t.s, "agent.activity.export", {"path": str(export), "onExists": "overwrite"})
        await asyncio.sleep(0.5)
        token_case["export"] = export.read_text(encoding="utf-8") if export.exists() else ""
        log = t.work / "discovery.log"
        token_case["log"] = log.read_text(encoding="utf-8", errors="replace") if log.exists() else ""
        key(0x1B)
        await asyncio.sleep(0.5)
        return "real clicks in Preferences › Agent access: Show, then an agent's inspect_ui, screenshot and export"

    def token_absent(text):
        tok = discovery_token()
        return bool(tok) and tok not in (text or "")

    async def nothing_state():
        return {}

    rows.append({"name": "Preferences › Agent access › token Copy", "spec": "11 §4 lines 107-108; D-62 item 2",
                 "action": ("none",), "custom": token_copy_flow, "expect": None, "label": None,
                 "setup": None, "state": nothing_state,
                 "effect": lambda b, a, k: token_case.get("clipboard") == sha(discovery_token())
                 and token_absent(token_case.get("copyTree")),
                 "disabled": None, "osOnly": True, "after": close_dialogs,
                 "mcpReason": "the Preferences dialog opens from the native menu, which only real input reaches"})
    rows.append({"name": "Preferences › Agent access › token Show", "spec": "11 §4 lines 107-108; D-62 item 2",
                 "action": ("none",), "custom": token_show_flow, "expect": None, "label": None,
                 "setup": None, "state": nothing_state,
                 "effect": lambda b, a, k: token_case.get("shown") == sha(discovery_token())
                 and token_absent(token_case.get("tree")) and '"redacted"' in (token_case.get("tree") or "")
                 and token_case.get("sameAsMasked") is True and token_case.get("overpainted") is True
                 and token_absent(token_case.get("afterCapture"))
                 and token_absent(token_case.get("export")) and token_absent(token_case.get("log")),
                 "disabled": None, "osOnly": True, "after": close_dialogs,
                 "mcpReason": "the Preferences dialog opens from the native menu, which only real input reaches"})

    async def typed_on():
        await agent_defaults()
        await user_cmd("agent.typedTools.set", {"enabled": True, "namespaces": []})
    rows.append({"name": "Preferences › Agent access › namespace", "spec": f"{spec_line('agent.typedTools.set')}; 11 §5",
                 "action": ("none",), "custom": agent_flow(clicked("agentAccess.namespace.doc"), "the doc namespace"),
                 "expect": ("agent.typedTools.set", {"enabled": True, "namespaces": ["doc"]}), "label": None,
                 "setup": typed_on, "state": agent_prefs,
                 "effect": lambda b, a, k: (a.get("agent.typedTools") or {}).get("namespaces") == ["doc"],
                 "disabled": None, "after": agent_defaults,
                 "mcpReason": "the controls are staged in the Preferences dialog and sent by its OK"})

    # ----- C4.4a: the Activity panel (11 §11, §6.4; design note 2 §3; SI-174). Its session controls are `app` scope (an
    # agent cannot grant, pause or disconnect: 11 §15.1), so the MCP form names them; the OS form acts on a second,
    # test-only session so this table's own session goes on. -----
    second = {}

    async def second_session():
        if second.get("id"):
            return second["id"]
        d = json.loads((t.work / "discovery.json").read_text(encoding="utf-8"))
        before = {x.get("sessionId") for x in (await cmd("agent.server.status", {})).get("sessions") or []}
        stack = contextlib.AsyncExitStack()
        client = await stack.enter_async_context(
            e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {d['token']}"}, timeout=e2e.httpx2.Timeout(30.0)))
        streams = await stack.enter_async_context(e2e.streamable_http_client(d["endpoint"], http_client=client))
        s2 = await stack.enter_async_context(e2e.ClientSession(streams[0], streams[1]))
        await s2.initialize()
        await asyncio.sleep(0.5)
        after = {x.get("sessionId") for x in (await cmd("agent.server.status", {})).get("sessions") or []}
        fresh = sorted(after - before)
        if not fresh:
            raise RuntimeError("the second session did not register")
        second.update({"id": fresh[0], "stack": stack})
        return fresh[0]

    async def activity_open():
        await cmd("window.panel.set", {"panelId": "activity", "visible": True, "focus": True})
        await asyncio.sleep(0.8)

    async def with_second():
        await second_session()
        await activity_open()

    async def session_state():
        sid = second.get("id")
        return next((x for x in (await cmd("agent.server.status", {})).get("sessions") or [] if x.get("sessionId") == sid), {})

    APP_SCOPE = "scope `app`: an agent cannot grant, pause or disconnect agent sessions (11 §15.1)"
    rows.append({"name": "Activity › session policy", "spec": f"{spec_line('agent.session.grant')}; 11 §11",
                 "action": lambda: ("select", f"activity.session.{second['id']}.policy", "observe"),
                 "expect": lambda b: ("agent.session.grant", {"sessionId": second["id"], "policy": "observe"}), "label": None,
                 "setup": with_second, "state": session_state, "effect": lambda b, a, k: a.get("policy") == "observe",
                 "disabled": None, "osOnly": True, "mcpReason": APP_SCOPE})

    async def scopes_flow():
        sid = second["id"]
        t.click(f"activity.session.{sid}.scopes")
        await asyncio.sleep(0.4)
        t.click(f"activity.session.{sid}.scope.ui")
        await asyncio.sleep(0.2)
        t.clear()
        t.click(f"activity.session.{sid}.scopes.ok")
        await asyncio.sleep(0.8)
        return "real clicks: Scopes…, the ui scope, OK"

    def scopes_after(b):
        order = ["read", "edit", "ui", "input", "files.read", "files.write", "prefs", "library.global", "network", "print"]
        have = set(b.get("scopes") or [])
        have ^= {"ui"}
        return [x for x in order if x in have]
    rows.append({"name": "Activity › session scopes", "spec": f"{spec_line('agent.session.grant')}; 01 §12 (per-session grant)",
                 "action": ("none",), "custom": scopes_flow,
                 "expect": lambda b: ("agent.session.grant", {"sessionId": second["id"], "scopes": scopes_after(b)}), "label": None,
                 "setup": with_second, "state": session_state,
                 "effect": lambda b, a, k: ("ui" in (a.get("scopes") or [])) != ("ui" in (b.get("scopes") or [])),
                 "disabled": None, "osOnly": True, "mcpReason": APP_SCOPE})
    for verb, session_cmd in (("pause", "agent.session.pause"), ("resume", "agent.session.resume")):
        rows.append({"name": f"Activity › session {verb}", "spec": f"{spec_line(session_cmd)}; 11 §6.4",
                     "action": (lambda v: lambda: ("dom", f"activity.session.{second['id']}.{v}"))(verb),
                     "expect": (lambda c: lambda b: (c, {"sessionId": second["id"]}))(session_cmd), "label": None,
                     "setup": with_second, "state": session_state,
                     "effect": (lambda v: lambda b, a, k: a.get("paused") is (v == "pause"))(verb),
                     "disabled": None, "osOnly": True, "mcpReason": APP_SCOPE})

    async def agent_step():
        await cmd("app.context.set", {"elementId": bob, "frame": 1})
        await cmd("draw.rectangle.add", {"from": {"x": -20, "y": -20}, "to": {"x": 20, "y": 20}})
        log = await cmd("agent.activity.list", {"limit": 50})
        mine = [e for e in (log if isinstance(log, list) else []) if e.get("commandId") == "draw.rectangle.add" and e.get("undoStepId")]
        history = await cmd("edit.history.get", {"limit": 1000})
        second["seq"] = mine[-1]["seq"]
        second["index"] = next(h.get("index") for h in history.get("entries") or [] if h.get("stepId") == mine[-1]["undoStepId"])
        await activity_open()

    async def undo_back_flow():
        # The panel shows the entry once its log is read (a wait for a state, ruling D-45; the bound guards a hang).
        start = time.monotonic()
        while time.monotonic() - start < 10.0:
            r = t.rect(f"activity.entry.{second['seq']}")
            if r and not r.get("hidden"):
                break
            await asyncio.sleep(0.2)
        t.click(f"activity.entry.{second['seq']}")
        await asyncio.sleep(0.4)
        t.clear()
        t.click("activity.undoBack")
        await asyncio.sleep(0.8)
        return "real clicks: the agent's entry, then Undo back to before this entry (no step of yours after it: no question)"

    async def history_position():
        return (await cmd("edit.history.get", {"limit": 1})).get("position")
    rows.append({"name": "Activity › Undo back to before an entry", "spec": f"{spec_line('edit.history.move')}; 11 §11",
                 "action": ("none",), "custom": undo_back_flow,
                 "expect": lambda b: ("edit.history.move", {"position": second["index"] - 1}), "label": None,
                 "setup": agent_step, "state": history_position, "effect": lambda b, a, k: a == second["index"] - 1,
                 "disabled": None, "mcpReason": "the entry is selected in the panel first (window state a click sets)"})

    activity_log = t.work / "D39-activity.jsonl"

    async def export_flow():
        t.click("activity.export")
        await asyncio.sleep(1.8)
        type_text(str(activity_log))
        key(0x0D)
        await asyncio.sleep(1.5)
        return "a real click on Export…, the path typed in the platform's save dialog, Enter"

    async def activity_log_state():
        return activity_log.exists() and activity_log.stat().st_size > 0
    rows.append({"name": "Activity › Export", "spec": f"{spec_line('agent.activity.export')}; 11 §11",
                 "action": ("none",), "custom": export_flow,
                 "expect": ("agent.activity.export", {"path": str(activity_log), "format": "jsonl", "onExists": "overwrite"}),
                 "label": None, "setup": activity_open, "state": activity_log_state, "effect": lambda b, a, k: a is True,
                 "disabled": None, "mcpReason": "the path is chosen in the platform's save dialog"})

    async def panel_state():
        return next((x for x in await cmd("window.panel.list", {}) or [] if x.get("panelId") == "activity"), {})

    async def activity_hidden():
        await cmd("window.panel.set", {"panelId": "activity", "visible": False})
        await asyncio.sleep(0.5)

    async def indicator_flow():
        # Opened only when closed (a click on the indicator toggles it), then the item waited for (D-45).
        if not t.cdp.js("!!document.querySelector('[data-tt-id=\"agent.popover\"]')"):
            t.click("agent.toggle")
        for _ in range(100):
            if t.cdp.js("!!document.querySelector('[data-tt-id=\"agent.openActivity\"]')"):
                break
            await asyncio.sleep(0.1)
        t.clear()
        t.click("agent.openActivity")
        await asyncio.sleep(0.8)
        return "real clicks: the agent indicator, then Activity…"
    rows.append({"name": "agent indicator › Activity…", "spec": f"{spec_line('window.panel.set')}; 11 §6.4",
                 "action": ("none",), "custom": indicator_flow,
                 "expect": ("window.panel.set", {"panelId": "activity", "visible": True, "focus": True}), "label": None,
                 "setup": activity_hidden, "state": panel_state, "effect": lambda b, a, k: a.get("visible") is True,
                 "disabled": None, "mcpReason": "the popover opens with a click on the indicator (window state)"})
    rows.append({"name": "Window › Activity", "spec": f"{spec_line('window.panel.set')}; 11 §6.4 (SI-174)",
                 "action": ("menu", ["Window", "Panels", "Activity"]),
                 "expect": ("window.panel.set", {"panelId": "activity", "visible": True}), "label": None,
                 "setup": activity_hidden, "state": panel_state, "effect": lambda b, a, k: a.get("visible") is True,
                 "disabled": None})

    async def close_second():
        stack = second.pop("stack", None)
        if stack:
            with contextlib.suppress(Exception):
                await stack.aclose()

    async def disconnect_flow():
        t.clear()
        t.click(f"activity.session.{second['id']}.disconnect")
        await asyncio.sleep(1.0)
        return "a real click on Disconnect"

    async def second_gone():
        return any(x.get("sessionId") == second.get("id") for x in (await cmd("agent.server.status", {})).get("sessions") or [])
    rows.append({"name": "Activity › session disconnect", "spec": f"{spec_line('agent.session.disconnect')}; 11 §6.4",
                 "action": ("none",), "custom": disconnect_flow,
                 "expect": lambda b: ("agent.session.disconnect", {"sessionId": second["id"]}), "label": None,
                 "setup": with_second, "state": second_gone, "effect": lambda b, a, k: b is True and a is False,
                 "disabled": None, "osOnly": True, "mcpReason": APP_SCOPE, "after": close_second})

    # D-59 item 2: the indicator's popover acts on a session as the Activity panel does (a fresh second session: the
    # one above was disconnected). The popover opens with a real click on the indicator when it is not open.
    async def fresh_second():
        await close_second()
        second.pop("id", None)
        await second_session()
        await asyncio.sleep(0.5)

    async def live_second():
        if not second.get("stack"):
            await fresh_second()

    def popover_flow(verb):
        async def flow():
            # The popover open (the toggle's aria-expanded, clicked again while it reads closed) and the session's
            # button listed (waits for states, D-45); what was tried is recorded on failure.
            ident = json.dumps(f"agent.{second['id']}.{verb}")
            tried = []
            for _ in range(3):
                state = t.cdp.js("(() => { const b = document.querySelector('[data-tt-id=\"agent.toggle\"]');"
                                 " return {expanded: b && b.getAttribute('aria-expanded'),"
                                 " popover: !!document.querySelector('[data-tt-id=\"agent.popover\"]')}; })()")
                if state and state.get("expanded") == "true" and state.get("popover"):
                    break
                tried.append({"before": state, "click": t.click("agent.toggle"),
                              "rect": t.rect("agent.toggle")})
                await asyncio.sleep(0.5)
            for _ in range(100):
                if t.cdp.js("!!document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']')" % ident):
                    break
                await asyncio.sleep(0.1)
            else:
                listed = t.cdp.js("[...document.querySelectorAll('[data-tt-id^=\"agent.\"]')].map(e => e.dataset.ttId)")
                raise RuntimeError(f"the popover does not list {second['id']}: {listed}; tried {tried}")
            t.clear()
            t.click(f"agent.{second['id']}.{verb}")
            await asyncio.sleep(1.0)
            return f"real clicks: the agent indicator (when closed), then {verb.title()}"
        return flow

    for verb, session_cmd, setup_, eff_ in (
            ("pause", "agent.session.pause", fresh_second, lambda b, a, k: b.get("paused") is not True and a.get("paused") is True),
            ("resume", "agent.session.resume", live_second, lambda b, a, k: b.get("paused") is True and a.get("paused") is False)):
        rows.append({"name": f"agent indicator › {verb.title()}", "spec": f"{spec_line(session_cmd)}; 11 §6.4",
                     "action": ("none",), "custom": popover_flow(verb),
                     "expect": (lambda c: lambda b: (c, {"sessionId": second["id"]}))(session_cmd), "label": None,
                     "setup": setup_, "state": session_state, "effect": eff_,
                     "disabled": None, "osOnly": True, "mcpReason": APP_SCOPE})

    async def forget_second():
        await close_second()
        second.pop("id", None)
    rows.append({"name": "agent indicator › Stop", "spec": f"{spec_line('agent.session.disconnect')}; 11 §6.4",
                 "action": ("none",), "custom": popover_flow("stop"),
                 "expect": lambda b: ("agent.session.disconnect", {"sessionId": second["id"]}), "label": None,
                 "setup": live_second, "state": second_gone, "effect": lambda b, a, k: b is True and a is False,
                 "disabled": None, "osOnly": True, "mcpReason": APP_SCOPE, "after": forget_second})

    # M5 C5.1b: the Timeline panel and the main toolbar's playback items, last (each row makes its own elements).
    rows.extend(await timeline_rows(t, bob))

    for r in rows:
        if r.get("skip"):
            await r["setup"]()
            continue
        if only and not any(o in r["name"] for o in only):
            continue
        if not isinstance(t, McpTable) and r.get("mcpOnly"):
            t.rows.append({"name": r["name"], "spec": r["spec"], "ok": True, "notRun": r["osReason"]})
            t.run.check(True, f"D-39 {r['name']}: runs in the MCP form only ({r['osReason']})", None)
            continue
        if isinstance(t, McpTable) and (r.get("custom") or r.get("post_click") or r.get("osOnly")):
            reason = r.get("mcpReason") or (
                "its action is the operating system's own file dialog (a typed path), which only real input reaches"
                if "shortcut editor" in r["name"] else
                "the dialog that lists the backups opens only from the native menu, which only real input reaches")
            t.rows.append({"name": r["name"], "spec": r["spec"], "ok": True, "notRun": reason})
            t.run.check(True, f"D-39 {r['name']}: runs in the OS form only ({reason})", None)
            continue
        if r.get("custom"):
            await custom_row(t, r)
        else:
            if r.get("post_click"):
                act = r["action"]
                post = r["post_click"]

                async def chained(act=act, post=post):
                    how = await Table.do(t, act)
                    await post()
                    return how
                r = {**r, "custom": chained}
                await custom_row(t, r, own_action=True)
            else:
                await t.row(r)
        # D-60 item 3: a row that creates or selects something (a camera made current, an element created) leaves the
        # stated fixture behind it: the current element is Bob at frame 1 again, so a later row or a probe starts from
        # it. Tolerated when it has nothing to do (a row that replaced the project).
        await e2e.run_cmd(t.s, "app.context.set", {"elementId": bob, "frame": 1})

    if not only or any("context menus" in o for o in only):
        await menu_bounds_row(t, bob)
    if not only or any("Timeline" in o for o in only):
        await e2e.run_cmd(t.s, "window.panel.set", {"panelId": "timeline", "visible": True, "focus": True})
        await timeline_layout_row(t)
    if not only or any("browser" in o for o in only):
        await browser_chrome_row(t)
    await jobs_row(t)
    await recent_hint_row(t)
    # Saved first: the agent's session may not discard (its policy), and the close must not ask.
    await e2e.run_cmd(t.s, "project.saveAs", {"path": str(t.work / "D39-final.animpkg"), "onExists": "overwrite"})
    await asyncio.sleep(1.0)
    if not only or any("load report" in o for o in only):
        await load_report_row(t, t.work / "D39-final.animpkg")
    await cmd("project.close", {"onUnsaved": "fail"})
    await t.disabled_checks()
    # D-59 item 2: every row the control coverage names ran, or was named with its reason in the MCP form (a commanded
    # control cannot be listed and not exercised).
    own = Table.row is _OWN_TABLE_ROW and custom_row is _OWN_CUSTOM_ROW
    if not only and not own:
        # A probe replaced the row runners (the reviewer's c43b-r2-live.py a3 runs its own subset of the rows).
        print("note D-39 coverage: not checked, the row runners were replaced (a probe's subset of the rows)", flush=True)
    if not only and own:
        cov39 = _load("c43d39coverage", "c43_d39_coverage.py")
        ran = {r.get("name") for r in t.rows}
        missing = sorted(cov39.named_rows() - ran)
        t.run.check(not missing and not cov39.problems(),
                    "D-39 coverage: every commanded control has its rows or a reason, and every row named ran",
                    {"missing": missing, "problems": cov39.problems()})
    # A test-only session a failed row left open is closed here, in the task that opened it (left open, it ends the
    # table's own session with an error at its exit).
    await close_second()


# ----- M5 C5.1b: the Timeline panel (04 §9-§13, lines 600-710; reviews/M5-design-timeline-view.md) -----------------

# M5 ruling E-48: the context menus' items run in both forms; the MCP form opens each menu through the keyboard path
# (the `key` tool's Shift+F10 on the surface, the current cell where the OS form clicks), see `McpTable.do`.
TL_SLIDER = ("a slider's value is chosen by the person's drag or key; the MCP form has no input to move it (its declared "
             "reference is the current value)")
TL_LAYOUT = json.loads((ROOT / "desktop/src/timeline/timeline-layout.json").read_text(encoding="utf-8"))


async def timeline_rows(t: Table, bob: str) -> list:
    """Every commanded control of the Timeline panel and the main toolbar's playback items (M5 C5.1b): the layer list's
    toolbar and row controls (both forms), and each command item of the row, cell and ruler context menus (a real
    secondary click on the row, the cell or the ruler, then a real click on the item: the OS form). The dialog items are
    the D-44 rows' (scripts/c43-d44-flows.py `timeline_rows`). Each row starts from Bob alone with the row's own
    elements (TlA, TlP, ...), and the last row leaves Bob alone again."""
    rows: list = []
    cmd = t.cmd
    ctx: dict = {}
    mcp = isinstance(t, McpTable)
    default_zoom = TL_LAYOUT.get("defaultZoomLevel", 0)

    async def run(n, p):  # a step tolerated when it has nothing to do
        return await e2e.run_cmd(t.s, n, p)

    async def elements():
        return (await cmd("element.list", {})).get("elements") or []

    async def shown(ident, timeout=20.0):
        # A wait for a state (D-45); 20 s as the table's other tree waits (a CI runner once needed more than 10 s for
        # the row to reach the window's tree, run 45285128916). The panel is asked for again halfway.
        start = time.monotonic()
        asked_again = False
        while time.monotonic() - start < timeout:
            if not asked_again and time.monotonic() - start > timeout / 2:
                asked_again = True
                await run("window.panel.set", {"panelId": "timeline", "visible": True, "focus": True})
            if mcp:
                if McpTable.node(await t.tree(), ident) is not None:
                    return True
            elif t.cdp.js("!!document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']')" % json.dumps(ident)):
                return True
            await asyncio.sleep(0.2)
        raise RuntimeError(f"the Timeline does not show {ident}")

    async def create(type_, name, **extra):
        res = await cmd("element.create", {"type": type_, "name": name, **extra})
        return (res.get("elementIds") or [None])[0]

    async def bob_alone():
        # One at a time: the scene's last camera is not deletable (01 §3), and one refusal would refuse a batch. A
        # deleted element's children move to its level (an effect's sprite drawing, a peg's children): again until
        # none is left.
        for _ in range(6):
            top = [e["id"] for e in await elements() if e["id"] != bob and not e.get("parentId") and e.get("type") != "camera"]
            if not top:
                break
            for i in top:
                await run("element.delete", {"elementIds": [i]})
        cameras = [e["id"] for e in await elements() if e.get("type") == "camera"]
        for c in cameras[1:]:
            await run("element.delete", {"elementIds": [c]})
        left = [e.get("name") for e in await elements() if e["id"] != bob and e.get("type") not in ("camera",)
                and not e.get("parentId")]
        if left:
            raise RuntimeError(f"the Timeline rows' elements were not removed: {left}")
        await run("element.parent.remove", {"elementIds": [bob]})
        await run("element.visibility.set", {"elementIds": [bob], "scope": "scene", "visible": True})
        await run("element.lock.set", {"elementIds": [bob], "locked": False})
        await run("element.colour.set", {"elementIds": [bob], "marker": "", "background": ""})
        await run("element.solo.set", {"enabled": False})
        await run("playback.stop", {})
        await run("playback.loop.set", {"enabled": False})
        await run("timeline.playbackRange.set", {"mode": "auto"})
        await run("timeline.effectParameterRows.set", {"visible": True})
        await run("timeline.view.set", {"zoomLevel": default_zoom, "firstFrame": 1})
        await run("edit.selection.set", {"context": "timeline", "ids": []})
        await cmd("app.context.set", {"elementId": bob, "frame": 1})

    async def fixture(kind):
        """The row's elements: `a` TlA (a drawing exposed on 1-12); `three` TlA between Bob and TlB; `hier` TlP (a peg)
        with TlC1 and TlC2; `next` TlA directly above TlP; `skeleton` TlS; `annotation` TlA and TlN (an annotation);
        `effect` TlE (a clipping, whose mask is an effect-parameter row)."""
        await bob_alone()
        ctx.clear()
        if kind in ("a", "three", "annotation"):
            ctx["a"] = await create("drawing", "TlA")
            await cmd("element.drawing.create", {"elementId": ctx["a"], "name": "1", "exposeFrom": 1, "exposeTo": 12})
        if kind == "three":
            ctx["b"] = await create("drawing", "TlB")
            order = [e["id"] for e in await elements() if not e.get("parentId")]
            rest = [i for i in order if i != ctx["a"]]
            await cmd("element.order.set", {"order": "timeline", "elementIds": [rest[0], ctx["a"], *rest[1:]]})
        if kind in ("hier", "next"):
            ctx["p"] = await create("peg", "TlP")
        if kind == "hier":
            ctx["c1"] = await create("drawing", "TlC1", parentId=ctx["p"])
            ctx["c2"] = await create("drawing", "TlC2", parentId=ctx["p"])
            kids = [e["id"] for e in await elements() if e.get("parentId") == ctx["p"]]
            ctx["first"], ctx["second"] = kids[0], kids[1]
        if kind == "next":
            ctx["a"] = await create("drawing", "TlA", above=ctx["p"])
        if kind == "skeleton":
            ctx["s"] = await create("skeleton", "TlS")
        if kind == "annotation":
            ctx["n"] = await create("annotation", "TlN")
        if kind == "effect":
            ctx["e"] = await create("clipping", "TlE")
        await cmd("window.panel.set", {"panelId": "timeline", "visible": True, "focus": True})

    def setup(kind, target, *steps):
        async def go():
            await fixture(kind)
            for s in steps:
                await s()
            await shown(f"timeline.row.{ctx[target]}")
            await asyncio.sleep(0.5)
        return go

    def select(key):
        async def go():
            await cmd("element.select", {"elementIds": [ctx[key]]})
        return go

    def hide(*keys):
        async def go():
            for k in keys:
                await cmd("element.visibility.set", {"elementIds": [bob if k == "bob" else ctx[k]], "scope": "scene",
                                                     "visible": False})
        return go

    async def state():
        els = []
        for e in await elements():
            g = await cmd("element.get", {"elementId": e["id"]})
            els.append({"type": g.get("elementType"),
                        **{k: g.get(k) for k in ("id", "name", "parentId", "timelineIndex", "visibleInScene", "locked",
                                                 "collapsed", "markerColour", "backgroundColour", "annotation")}})
        sel = await cmd("edit.selection.get", {"context": "timeline"})
        clip = (await run("edit.clipboard.get", {})).get("result") or {}
        listed = (await run("timeline.rows.list", {"from": 1, "to": 24})).get("result") or {}
        view = (await run("timeline.view.set", {})).get("result") or {}
        play = await cmd("playback.state.get", {})
        cells = ""
        if ctx.get("a"):
            got = (await run("xsheet.exposures.get", {"elementIds": [ctx["a"]], "from": 1, "to": 24})).get("result")
            cells = json.dumps(got, sort_keys=True)[:3000]
        # E-2 (b): each row's keys (frame, segment after), and the peg's path (Constant Z per motion keyframe).
        keys = {r["elementId"]: [(k.get("frame"), k.get("segmentAfter")) for k in r.get("keyframes") or []]
                for r in listed.get("rows") or []}
        path = ""
        velocity = ""
        if ctx.get("p"):
            path = json.dumps((await run("anim.path.get", {"nodeId": ctx["p"]})).get("result"), sort_keys=True)
            velocity = json.dumps((await run("anim.function.get", {"nodeId": ctx["p"], "channel": "velocity"})).get(
                "result"), sort_keys=True)
        return {"els": els, "sel": sorted(i for i in sel.get("ids") or [] if str(i).startswith("el_")),
                "clip": json.dumps(clip, sort_keys=True)[:3000], "range": await cmd("timeline.playbackRange.get", {}),
                "rows": len(listed.get("rows") or []), "zoom": view.get("zoomLevel"), "first": view.get("firstFrame"),
                "playing": play.get("playing"), "frame": play.get("frame"), "cells": cells, "keys": keys,
                "path": path, "velocity": velocity}

    def el(s, key):
        want = bob if key == "bob" else ctx.get(key)
        return next((e for e in s["els"] if e["id"] == want), None)

    def new_of(b, a, type_=None):
        old = {e["id"] for e in b["els"]}
        return [e for e in a["els"] if e["id"] not in old and (type_ is None or e["type"] == type_)]

    def others(s, key):
        return [e for e in s["els"] if e["id"] != ctx[key] and e["type"] != "effectParameter"]

    def menu_row(menu, item, label, command, params, kind, target, effect, *steps, point=None, step=None):
        surface = {"row": lambda: f"timeline.row.{ctx[target]}", "cell": lambda: "timeline.tracks",
                   "ruler": lambda: "timeline.ruler"}[menu]
        where = {"cell": lambda: {"x_of": "timeline.tracks", "x": 40, "y_of": f"timeline.row.{ctx[target]}"},
                 "ruler": lambda: {"x_of": "timeline.ruler", "x": 60, "y_of": "timeline.ruler"}}.get(menu, lambda: None)
        rows.append({"name": f"Timeline {menu} menu › {label}",
                     "spec": f"{spec_line(command)}; 04 lines 704-706; 09 line 85",
                     "action": lambda: ("context", surface(), f"timeline.{menu}Menu.timeline.{menu}.{item}", where()),
                     "expect": lambda b: (command, params() if callable(params) else params),
                     **({"label": step} if step else {"undoOf": command}), "setup": setup(kind, target, *steps),
                     "state": state, "effect": effect, "disabled": None,
                     **({"match": point} if point else {})})

    ids = lambda key: (lambda: {"elementIds": [ctx[key]]})  # noqa: E731

    # The row menu (04 line 704): Add, the edit items, Arrange, Collapse, Select, Attach and Detach, Visibility,
    # Default Colour, the effect-parameter display, Set as Annotation, Lock, Clone and Duplicate, Peg Reset.
    for type_, label in (("drawing", "Drawing"), ("annotation", "Annotation"), ("image", "Image"), ("sound", "Sound"),
                         ("media", "Media"), ("peg", "Peg"), ("camera", "Camera"), ("skeleton", "Skeleton"),
                         ("colourTransform", "Colour Transform"), ("clipping", "Clipping"),
                         ("dropShadow", "Drop Shadow"), ("blur", "Blur"), ("shake", "Shake"),
                         ("particleNature", "Particle Nature"), ("particleImpact", "Particle Impact")):
        menu_row("row", f"add.{type_}", f"Add {label}", "element.create",
                 (lambda ty: lambda: {"type": ty, "above": ctx["a"]})(type_), "a", "a",
                 (lambda ty: lambda b, a, k: len(new_of(b, a, ty)) == 1)(type_))
    menu_row("row", "addParentPeg", "Add Parent Peg", "element.parentPeg.create", ids("a"), "a", "a",
             lambda b, a, k: any(e["id"] == el(a, "a")["parentId"] and e["type"] == "peg" for e in new_of(b, a)))
    menu_row("row", "cut", "Cut", "edit.cut", {"context": "timeline"}, "a", "a",
             lambda b, a, k: el(a, "a") is None and a["clip"] != b["clip"], select("a"))
    async def cells_on_clipboard():
        await cmd("cell.select", {"elementIds": [ctx["a"]], "from": 2, "to": 3})
        await cmd("edit.copy", {"context": "timeline"})
    menu_row("row", "copy", "Copy", "edit.copy", {"context": "timeline"}, "a", "a",
             lambda b, a, k: el(a, "a") is not None and a["clip"] != b["clip"], cells_on_clipboard, select("a"))

    async def copied_a():
        await cmd("element.select", {"elementIds": [ctx["a"]]})
        await cmd("edit.copy", {"context": "timeline"})
    menu_row("row", "paste", "Paste", "edit.paste", {"context": "timeline"}, "a", "a",
             lambda b, a, k: len(new_of(b, a)) >= 1, copied_a)
    menu_row("row", "delete", "Delete", "element.delete", ids("a"), "a", "a", lambda b, a, k: el(a, "a") is None)
    for to, label in (("front", "Bring to Front"), ("forward", "Bring Forward"), ("backward", "Send Backward"),
                      ("back", "Send to Back")):
        menu_row("row", f"arrange.{to}", label, "element.order.move",
                 (lambda to_: lambda: {"elementIds": [ctx["a"]], "to": to_, "order": "timeline"})(to), "three", "a",
                 lambda b, a, k: el(a, "a")["timelineIndex"] != el(b, "a")["timelineIndex"])
    menu_row("row", "collapse", "Collapse", "element.collapsed.set",
             lambda: {"elementIds": [ctx["p"]], "collapsed": True}, "hier", "p",
             lambda b, a, k: el(b, "p")["collapsed"] is False and el(a, "p")["collapsed"] is True)
    for rel, label, target, want in (("parent", "Select Parent", "c1", ["p"]),
                                     ("children", "Select Children", "p", ["p", "c1", "c2"]),
                                     ("firstChild", "Select Child", "p", ["first"]),
                                     ("previousSibling", "Select Previous Sibling", "second", ["first"]),
                                     ("nextSibling", "Select Next Sibling", "first", ["second"]),
                                     ("skeletonRoot", "Select Skeleton Root", "s", ["s"])):
        menu_row("row", f"select.{rel}", label, "element.hierarchy.select",
                 (lambda r, tg: lambda: {"relation": r, "elementId": ctx[tg]})(rel, target),
                 "skeleton" if rel == "skeletonRoot" else "hier", target,
                 (lambda w: lambda b, a, k: a["sel"] == sorted(ctx[x] for x in w))(want))
    menu_row("row", "attachNext", "Attach to Next Object", "element.parent.set",
             lambda: {"elementIds": [ctx["a"]], "parentId": ctx["p"]}, "next", "a",
             lambda b, a, k: el(a, "a")["parentId"] == ctx["p"])
    menu_row("row", "detach", "Detach from Parent", "element.parent.remove", ids("c1"), "hier", "c1",
             lambda b, a, k: el(b, "c1")["parentId"] == ctx["p"] and el(a, "c1")["parentId"] is None)
    vis = lambda s, key: el(s, key)["visibleInScene"]  # noqa: E731
    menu_row("row", "showHide", "Hide", "element.visibility.set",
             lambda: {"elementIds": [ctx["a"]], "scope": "scene", "visible": False}, "a", "a",
             lambda b, a, k: vis(b, "a") and not vis(a, "a"))
    menu_row("row", "showAll", "Show All", "element.visibility.set", {"scope": "scene", "visible": True, "apply": "all"},
             "a", "a", lambda b, a, k: not vis(b, "bob") and all(e["visibleInScene"] for e in a["els"]), hide("bob"))
    menu_row("row", "hideOthers", "Hide All Others", "element.visibility.set",
             lambda: {"elementIds": [ctx["a"]], "scope": "scene", "visible": False, "apply": "others"}, "a", "a",
             lambda b, a, k: vis(a, "a") and not any(e["visibleInScene"] for e in others(a, "a")))
    menu_row("row", "showOthers", "Show All Others", "element.visibility.set",
             lambda: {"elementIds": [ctx["a"]], "scope": "scene", "visible": True, "apply": "others"}, "a", "a",
             lambda b, a, k: not vis(b, "bob") and vis(a, "bob"), hide("bob"))
    menu_row("row", "hideAll", "Hide All", "element.visibility.set", {"scope": "scene", "visible": False, "apply": "all"},
             "a", "a", lambda b, a, k: not any(e["visibleInScene"] for e in a["els"] if e["type"] != "effectParameter"))
    menu_row("row", "showAnnotations", "Show All Annotation Elements", "element.visibility.set",
             {"scope": "scene", "visible": True, "apply": "all", "types": ["annotation"]}, "annotation", "a",
             lambda b, a, k: not vis(b, "n") and vis(a, "n"), hide("n"))
    menu_row("row", "hideAnnotations", "Hide All Annotation Elements", "element.visibility.set",
             {"scope": "scene", "visible": False, "apply": "all", "types": ["annotation"]}, "annotation", "a",
             lambda b, a, k: vis(b, "n") and not vis(a, "n") and vis(a, "a"))

    async def coloured():
        await cmd("element.colour.set", {"elementIds": [ctx["a"]], "marker": "#FF0000", "background": "#00FF00"})
    menu_row("row", "defaultColour", "Default Colour", "element.colour.set",
             lambda: {"elementIds": [ctx["a"]], "marker": "", "background": ""}, "a", "a",
             lambda b, a, k: el(b, "a")["markerColour"] and not el(a, "a")["markerColour"]
             and not el(a, "a")["backgroundColour"], coloured)

    async def parameters_hidden():
        await cmd("timeline.effectParameterRows.set", {"visible": False})
    menu_row("row", "showAllParameters", "Show All Effect Parameters", "timeline.effectParameterRows.set",
             {"visible": True}, "effect", "e", lambda b, a, k: a["rows"] > b["rows"], parameters_hidden)
    menu_row("row", "hideAllParameters", "Hide All Effect Parameters", "timeline.effectParameterRows.set",
             {"visible": False}, "effect", "e", lambda b, a, k: a["rows"] < b["rows"])
    menu_row("row", "annotation", "Set as Annotation", "element.annotation.set",
             lambda: {"elementId": ctx["a"], "annotation": True}, "a", "a",
             lambda b, a, k: el(a, "a")["type"] == "annotation" or el(a, "a")["annotation"] is True)
    locked = lambda s, key: el(s, key)["locked"]  # noqa: E731
    menu_row("row", "lock", "Lock", "element.lock.set", lambda: {"elementIds": [ctx["a"]], "locked": True}, "a", "a",
             lambda b, a, k: not locked(b, "a") and locked(a, "a"))
    menu_row("row", "lockOthers", "Lock All Others", "element.lock.set",
             lambda: {"elementIds": [ctx["a"]], "locked": True, "apply": "others"}, "a", "a",
             lambda b, a, k: not locked(a, "a") and locked(a, "bob"))
    menu_row("row", "lockAll", "Lock All", "element.lock.set", {"locked": True, "apply": "all"}, "a", "a",
             lambda b, a, k: locked(a, "a") and locked(a, "bob"))

    async def all_locked():
        await cmd("element.lock.set", {"locked": True, "apply": "all"})
    menu_row("row", "unlockAll", "Unlock All", "element.lock.set", {"locked": False, "apply": "all"}, "a", "a",
             lambda b, a, k: locked(b, "a") and not locked(a, "a") and not locked(a, "bob"), all_locked,
             step="Unlock Elements")
    menu_row("row", "clone", "Clone", "element.clone", lambda: {"elementId": ctx["a"]}, "a", "a",
             lambda b, a, k: len(new_of(b, a)) == 1)
    menu_row("row", "duplicate", "Duplicate", "element.duplicate", ids("a"), "a", "a",
             lambda b, a, k: len(new_of(b, a)) == 1)

    # The cell menu (04 line 705): the edit items on the cells selected in TlA's track (frames 2-3).
    async def cells_selected():
        await cmd("cell.select", {"elementIds": [ctx["a"]], "from": 2, "to": 3})

    async def cells_copied():
        await cmd("cell.select", {"elementIds": [ctx["a"]], "from": 2, "to": 3})
        await cmd("edit.copy", {"context": "timeline"})
        # Pasted at the current frame: 15, past TlA's exposure (1-12), so the paste changes the track.
        await cmd("app.context.set", {"elementId": ctx["a"], "frame": 15})
        await cmd("cell.select", {"elementIds": [ctx["a"]], "from": 15, "to": 15})

    async def element_on_clipboard_then_cells():
        await cmd("element.select", {"elementIds": [ctx["a"]]})
        await cmd("edit.copy", {"context": "timeline"})
        await cells_selected()
    cells_changed = lambda b, a, k: a["cells"] != b["cells"]  # noqa: E731
    menu_row("cell", "cut", "Cut", "edit.cut", {"context": "timeline"}, "a", "a",
             lambda b, a, k: cells_changed(b, a, k) and a["clip"] != b["clip"], cells_selected)
    menu_row("cell", "copy", "Copy", "edit.copy", {"context": "timeline"}, "a", "a",
             lambda b, a, k: a["clip"] != b["clip"] and a["cells"] == b["cells"], element_on_clipboard_then_cells)
    menu_row("cell", "paste", "Paste", "edit.paste", {"context": "timeline"}, "a", "a", cells_changed, cells_copied)
    menu_row("cell", "delete", "Delete", "edit.delete", {"context": "timeline"}, "a", "a", cells_changed,
             cells_selected)

    # Rulings E-2 (b), E-47: the cell menu's keyframe items (05 §10) on TlA at the clicked frame (40 pixels in: the
    # frame is the window's, so the parameters are a predicate), and the row menu's Remove All Keyframes and Toggle
    # Constant Z on TlP.
    def keyed(key, frames, constant=False):
        async def go():
            for f in frames:
                await cmd("anim.key.add", {"nodeIds": [ctx[key]], "frame": f})
            if constant:
                for f in frames[:-1]:
                    await cmd("anim.segment.set", {"nodeIds": [ctx[key]], "frame": f, "constant": True})
        return go

    def key_count(s, key):
        return len(s["keys"].get(ctx[key]) or [])

    def segments(s, key, kind):
        return sum(1 for _, seg in s["keys"].get(ctx[key]) or [] if seg == kind)

    def at_click(extra):
        return lambda prm, b: (prm.get("nodeIds") == [ctx["a"]] and isinstance(prm.get("frame"), int)
                               and 1 <= prm["frame"] <= 12 and all(prm.get(k) == v for k, v in extra.items()))

    menu_row("cell", "addKey", "Add Keyframe", "anim.key.add", {"nodeIds": ["TlA"], "frame": "the clicked frame"},
             "a", "a", lambda b, a, k: key_count(a, "a") == key_count(b, "a") + 1, point=at_click({}))
    menu_row("cell", "removeKey", "Remove Keyframe", "anim.key.remove",
             {"nodeIds": ["TlA"], "frame": "the clicked frame"}, "a", "a",
             lambda b, a, k: key_count(a, "a") == key_count(b, "a") - 1, keyed("a", list(range(1, 13))),
             point=at_click({}))
    menu_row("cell", "constant", "Set Constant Segment", "anim.segment.set",
             {"nodeIds": ["TlA"], "frame": "the clicked frame", "constant": True}, "a", "a",
             lambda b, a, k: segments(a, "a", "constant") > segments(b, "a", "constant"),
             keyed("a", [1, 12]), point=at_click({"constant": True}))
    menu_row("cell", "nonConstant", "Set Non-Constant Segment", "anim.segment.set",
             {"nodeIds": ["TlA"], "frame": "the clicked frame", "constant": False}, "a", "a",
             lambda b, a, k: segments(a, "a", "constant") < segments(b, "a", "constant"),
             keyed("a", [1, 12], constant=True), point=at_click({"constant": False}))
    # Uniform Left / Right on TlP's cells (the clicked frame, 3, lies in the segment from key 1 to key 6): a path with an
    # uneven speed in that segment, so making it uniform changes the node's timing.
    async def uneven_path():
        await cmd("anim.path.set", {"nodeId": ctx["p"], "points": [
            {"kind": "key", "frame": 1, "x": 0, "y": 0, "z": 0}, {"kind": "key", "frame": 6, "x": 1, "y": 0, "z": 0},
            {"kind": "key", "frame": 12, "x": 10, "y": 0, "z": 0}]})
        # A velocity key inside the first segment (the speed uneven there): Uniform removes it (05 §10.6). A fresh path's
        # speed is already uniform, and Uniform then changes nothing (the first run of these rows).
        await cmd("anim.function.key.add", {"nodeId": ctx["p"], "channel": "velocity", "frame": 3, "value": 0.9})
    for item, label, side in (("uniformLeft", "Uniform Left", "left"), ("uniformRight", "Uniform Right", "right")):
        menu_row("cell", item, label, "anim.segment.uniform.apply",
                 {"nodeId": "TlP", "frame": "the clicked frame", "side": side}, "next", "p",
                 lambda b, a, k: a["velocity"] != b["velocity"], uneven_path,
                 point=(lambda sd: lambda prm, b: (prm.get("nodeId") == ctx["p"] and prm.get("side") == sd
                                                   and isinstance(prm.get("frame"), int)
                                                   and 1 <= prm["frame"] <= 12))(side))

    menu_row("row", "removeKeys", "Remove All Keyframes", "anim.key.remove",
             lambda: {"nodeIds": [ctx["p"]], "all": True}, "next", "p",
             lambda b, a, k: key_count(b, "p") == 2 and key_count(a, "p") == 0, keyed("p", [1, 5]))
    menu_row("row", "constantZ", "Toggle Constant Z", "anim.path.lockDepth.set",
             lambda: {"nodeId": ctx["p"], "frame": 1, "enabled": True}, "next", "p",
             lambda b, a, k: '"lockDepth": true' in a["path"] and '"lockDepth": true' not in b["path"],
             keyed("p", [1, 5]))

    # The ruler menu (04 line 706; 09 line 85): the playback range's mode and its ends at the clicked frame (60 pixels
    # in: the frame there is the window's, so the parameters are a predicate: one frame of the scene).
    async def range_free():
        await cmd("timeline.playbackRange.set", {"mode": "free", "start": 2, "end": 20})
    menu_row("ruler", "rangeAuto", "Automatic Fit", "timeline.playbackRange.set", {"mode": "auto"}, "a", "a",
             lambda b, a, k: b["range"]["mode"] == "free" and a["range"]["mode"] == "auto", range_free)
    menu_row("ruler", "rangeFree", "Free", "timeline.playbackRange.set", {"mode": "free"}, "a", "a",
             lambda b, a, k: b["range"]["mode"] == "auto" and a["range"]["mode"] == "free")
    for item, label, end in (("startHere", "Set Playback Start Here", "start"), ("endHere", "Set Playback End Here", "end")):
        menu_row("ruler", item, label, "timeline.playbackRange.set", {end: "the clicked frame"}, "a", "a",
                 (lambda e: lambda b, a, k: a["range"][e] != b["range"][e])(end), range_free,
                 point=(lambda e: lambda p, b: set(p) == {e} and isinstance(p[e], int) and 1 <= p[e] <= 24)(end))

    # The layer list's toolbar (04 line 604) and each row's disclosure and visibility controls (04 line 605).
    def dom_row(name, ident, command, params, kind, target, effect, *steps, toggle=False, disabled=True):
        rows.append({"name": f"Timeline › {name}", "spec": f"{spec_line(command)}; 04 lines 604-605",
                     "action": lambda: ("dom", ident() if callable(ident) else ident),
                     "expect": lambda b: (command, params(b) if callable(params) else params),
                     "undoOf": command, "setup": setup(kind, target, *steps), "state": state,
                     "effect": effect, "toggle": toggle,
                     "disabled": ("dom", ident) if disabled and not callable(ident) else None})

    # ----- Ruling E-50 item 1 (04 §4.1): rename in place. A real double click on a row's name opens the editor, prefilled
    # with the name; Enter commits (and the Play key, Enter, starts no playback), Escape restores; while it is open the
    # keys the Timeline binds (/, the arrows, Delete) act in the field and send nothing. -----
    rename_case: dict = {}

    def name_field(ident):
        return t.cdp.js("(() => { const e = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']');"
                        " return e ? {value: e.value, focused: document.activeElement === e} : null; })()"
                        % json.dumps(ident))

    async def double_click_name(key_):
        """A real double click at the centre of the row's name; waits for the editor to hold the focus."""
        front()
        r = t.rect(f"timeline.row.{ctx[key_]}.label")
        if not r or r.get("hidden"):
            raise RuntimeError(f"the row's name is not shown: {key_}")
        h = front()
        origin = wintypes.POINT(0, 0)
        u32.ClientToScreen(h, ctypes.byref(origin))
        u32.SetCursorPos(int(origin.x + r["x"] * r["dpr"]), int(origin.y + r["y"] * r["dpr"]))
        time.sleep(0.15)
        for _ in range(2):
            e6._send_button(True)
            time.sleep(0.02)
            e6._send_button(False)
            time.sleep(0.05)
        ident = f"timeline.row.{ctx[key_]}.name"
        end = time.monotonic() + 5.0
        while time.monotonic() < end:
            got = name_field(ident)
            if got and got["focused"]:
                return ident, got
            await asyncio.sleep(0.1)
        raise RuntimeError(f"a real double click on the row's name did not open the editor: {ident}")

    def rename_flow(case, typed, finish):
        async def flow():
            ident, got = await double_click_name("a")
            rename_case[case] = {"opened": got}
            t.clear()
            key(0x41, 0x11)  # Ctrl+A: the editor's text
            await asyncio.sleep(0.1)
            type_text(typed)
            await asyncio.sleep(0.2)
            rename_case[case]["typed"] = name_field(ident)
            if case == "keys":
                # The keys the Timeline binds, typed into the editor: /, the four arrows, Delete.
                for vk in (0xBF, 0x25, 0x27, 0x26, 0x28, 0x2E):
                    key(vk)
                    await asyncio.sleep(0.1)
                rename_case[case]["afterKeys"] = name_field(ident)
            key(finish)
            # Until the editor has closed (Enter commits, Escape restores; a wait for the state, D-45), bounded.
            end = time.monotonic() + 5.0
            while time.monotonic() < end and name_field(ident) is not None:
                await asyncio.sleep(0.1)
            rename_case[case]["after"] = name_field(ident)
            t.run.save(f"timeline-rename-{case}.json", rename_case[case])
            return f"a real double click on the row's name, the name typed, {'Enter' if finish == 0x0D else 'Escape'}"
        return flow

    def name_of_a(s):
        return (el(s, "a") or {}).get("name")

    async def context_rename():
        """A real secondary click on the row, a real click on Rename, the name typed in the editor it opens, Enter."""
        r = t.rect(f"timeline.row.{ctx['a']}.label")
        h = front()
        origin = wintypes.POINT(0, 0)
        u32.ClientToScreen(h, ctypes.byref(origin))
        right_click_screen(origin.x + r["x"] * r["dpr"], origin.y + r["y"] * r["dpr"])
        item = "timeline.rowMenu.timeline.row.rename"
        end = time.monotonic() + 10.0
        while time.monotonic() < end and t.cdp.js(
                "document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']')?.getAttribute('aria-disabled') ?? null"
                % json.dumps(item)) != "false":
            await asyncio.sleep(0.1)
        t.click(item)
        ident = f"timeline.row.{ctx['a']}.name"
        end = time.monotonic() + 5.0
        got = None
        while time.monotonic() < end:
            got = name_field(ident)
            if got and got["focused"]:
                break
            await asyncio.sleep(0.1)
        if not (got and got["focused"]):
            raise RuntimeError(f"the row context's Rename did not open the editor: {ident}")
        rename_case["context"] = {"opened": got, "dialog": bool(t.cdp.js(
            "!!document.querySelector('[data-tt-id=\"timelineRename\"]')"))}
        t.clear()
        key(0x41, 0x11)
        type_text("TlContext")
        key(0x0D)
        end = time.monotonic() + 5.0
        while time.monotonic() < end and name_field(ident) is not None:
            await asyncio.sleep(0.1)
        rename_case["context"]["after"] = name_field(ident)
        t.run.save("timeline-rename-context.json", rename_case["context"])
        return "a real secondary click on the row, a real click on Rename, the name typed, Enter"

    rows.append({"name": "Timeline row menu › Rename (in place)", "spec": f"{spec_line('element.rename')}; 04 §4.1",
                 "action": ("none",), "custom": context_rename,
                 "expect": lambda b: ("element.rename", {"elementId": ctx["a"], "name": "TlContext"}),
                 # 04 §4.1: the undo label names the element type.
                 "label": "Rename Drawing Element", "setup": setup("a", "a"), "state": state,
                 "effect": lambda b, a, k: (rename_case["context"]["opened"]["value"] == "TlA"
                                            and not rename_case["context"]["dialog"] and name_of_a(a) == "TlContext"),
                 "disabled": None, "osOnly": True,
                 "mcpReason": "the name is typed by the person into the editor the item opens"})
    rows.append({"name": "Timeline › row rename (double click, Enter)", "spec": f"{spec_line('element.rename')}; 04 §4.1",
                 "action": ("none",), "custom": rename_flow("enter", "TlRenamed", 0x0D),
                 "expect": lambda b: ("element.rename", {"elementId": ctx["a"], "name": "TlRenamed"}),
                 # 04 §4.1: the undo label names the element type.
                 "label": "Rename Drawing Element", "setup": setup("a", "a"), "state": state,
                 "effect": lambda b, a, k: (rename_case["enter"]["opened"]["value"] == "TlA"
                                            and name_of_a(a) == "TlRenamed" and not a["playing"]
                                            and rename_case["enter"]["after"] is None),
                 "disabled": None, "osOnly": True,
                 "mcpReason": "the editor opens on a real double click on the row's name; the typed name is the person's"})
    rows.append({"name": "Timeline › row rename (Escape)", "spec": "04 §4.1",
                 "action": ("none",), "custom": rename_flow("escape", "TlNot", 0x1B),
                 "expect": None, "label": None, "setup": setup("a", "a"), "state": state,
                 "effect": lambda b, a, k: (a == b and rename_case["escape"]["typed"]["value"] == "TlNot"
                                            and rename_case["escape"]["after"] is None),
                 "disabled": None, "osOnly": True,
                 "mcpReason": "the editor opens on a real double click on the row's name; Escape is the person's"})
    rows.append({"name": "Timeline › row rename (keys stay in the editor)", "spec": "04 §4.1; 01 §8.4; D-66 item 2",
                 "action": ("none",), "custom": rename_flow("keys", "TlKeys", 0x1B),
                 "expect": None, "label": None, "setup": setup("a", "a"), "state": state,
                 # Nothing sent: the same state after / (Show Timeline Selection), the arrows (the current cell) and
                 # Delete (edit.delete); the slash typed into the field and Delete acting in it.
                 "effect": lambda b, a, k: (a == b and (rename_case["keys"]["afterKeys"] or {}).get("focused") is True
                                            and "/" in (rename_case["keys"]["afterKeys"] or {}).get("value", "")
                                            and rename_case["keys"]["after"] is None),
                 "disabled": None, "osOnly": True,
                 "mcpReason": "real keys typed into the open editor: an agent's `key` tool is refused while a text field has "
                              "the focus"})

    # ----- Ruling E-53 (C51B-R2-01; 04 §4.1 with §3.2's codes): every name typed into the editor goes to element.rename
    # and the kernel decides. A refused name (empty, a forbidden character, a duplicate, too long) changes nothing, the
    # status line says why (D-87), and the editor stays open with the text and the focus; Escape then closes it. -----
    def refused_rename(case, typed):
        async def flow():
            ident, got = await double_click_name("a")
            t.cdp.js("window.dispatchEvent(new CustomEvent('tt-status-line', { detail: null }))")
            rename_case[case] = {"opened": got}
            t.clear()
            key(0x41, 0x11)  # Ctrl+A: the editor's text, replaced by what is typed (Backspace for the empty name)
            await asyncio.sleep(0.1)
            if typed:
                type_text(typed)
            else:
                key(0x08)
            await asyncio.sleep(0.2)
            rename_case[case]["typed"] = name_field(ident)
            key(0x0D)
            end = time.monotonic() + 5.0
            status = ""
            while time.monotonic() < end and not status:
                status = t.cdp.js("document.querySelector('[data-tt-id=\"status.line\"]')?.textContent ?? ''") or ""
                await asyncio.sleep(0.1)
            # The editor shows the reason once the refusal has come back (a wait for the state, D-45), bounded.
            problem = None
            end = time.monotonic() + 5.0
            while time.monotonic() < end and not problem:
                problem = t.cdp.js(
                    "document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']')?.dataset.ttProblem ?? null"
                    % json.dumps(ident))
                await asyncio.sleep(0.1)
            rename_case[case]["status"] = status
            rename_case[case]["after"] = name_field(ident)
            rename_case[case]["problem"] = problem
            rename_case[case]["messageBox"] = bool(t.cdp.js("!!document.querySelector('[role=\"alertdialog\"]')"))
            key(0x1B)
            # Until the editor has left the page (a wait for the state, D-45), bounded.
            end = time.monotonic() + 5.0
            while time.monotonic() < end and name_field(ident) is not None:
                await asyncio.sleep(0.1)
            rename_case[case]["afterEscape"] = name_field(ident)
            t.run.save(f"timeline-rename-{case}.json", rename_case[case])
            return "a real double click on the row's name, the name typed, Enter (refused), then Escape"
        return flow

    def refused_told(case, typed, words):
        def effect(b, a, k):
            c = rename_case.get(case) or {}
            after = c.get("after") or {}
            # Nothing renamed (the double click may move the current cell, which is the click's, not the rename's).
            names = [(e["id"], e["name"]) for e in a["els"]] == [(e["id"], e["name"]) for e in b["els"]]
            return (names and after.get("focused") is True and after.get("value") == typed
                    and words in (c.get("status") or "") and words in (c.get("problem") or "")
                    and c.get("messageBox") is False and c.get("afterEscape") is None)
        return effect

    for case, kind, typed, words, code in (
            ("empty", "a", "", "name must not be empty", "element.nameEmpty"),
            ("invalid", "a", "Tl/A", "name must not contain `/`", "element.nameInvalid"),
            ("duplicate", "three", "TlB", "already named `TlB`", "element.nameDuplicate"),
            ("tooLong", "a", "T" * 65, "name is longer than 64 characters", "element.nameTooLong")):
        rows.append({"name": f"Timeline › row rename refused ({code}): the status line says why, the editor stays open",
                     "spec": f"{spec_line('element.rename')}; 04 §3.2 ({code}), §4.1; D-87; ruling E-53",
                     "action": ("none",), "custom": refused_rename(case, typed),
                     # The name typed is sent (the kernel decides) and refused: no undo step, nothing changed.
                     "expect": (lambda typed_: lambda b: ("element.rename", {"elementId": ctx["a"], "name": typed_}))(typed),
                     "label": None, "setup": setup(kind, "a"), "state": state,
                     "effect": refused_told(case, typed, words), "disabled": None, "osOnly": True,
                     "mcpReason": "the name is typed by the person into the editor a real double click opens; an agent's "
                                  "element.rename answers the refusal to the agent"})

    dom_row("toolbar Add", "timeline.add", "element.create", {"type": "drawing"}, "a", "a",
            lambda b, a, k: len(new_of(b, a, "drawing")) == 1)
    dom_row("toolbar Parent Peg", "timeline.addParentPeg", "element.parentPeg.create",
            lambda b: {"elementIds": [ctx["a"]]}, "a", "a",
            lambda b, a, k: any(e["id"] == el(a, "a")["parentId"] and e["type"] == "peg" for e in new_of(b, a)),
            select("a"))

    async def solo_state():
        """The state, and whether the Solo button shows solo on (aria-pressed; the window tree's `checked`)."""
        s = await state()
        if mcp:
            answer = await e2e.call(t.s, "inspect_ui", {"target": "window", "depth": 32})
            node = McpTable.node((answer.get("result") or answer).get("root") or {}, "timeline.solo") or {}
            s["solo"] = node.get("checked") is True
        else:
            s["solo"] = t.cdp.js("document.querySelector('[data-tt-id=\"timeline.solo\"]')?.getAttribute('aria-pressed')") == "true"
        return s
    dom_row("toolbar Solo", "timeline.solo", "element.solo.set",
            lambda b: {"elementIds": [ctx["a"]], "enabled": not b["solo"]}, "a", "a",
            lambda b, a, k: a["solo"] != b["solo"], select("a"), toggle=True)
    rows[-1]["state"] = solo_state
    dom_row("toolbar Show All", "timeline.showAll", "element.visibility.set",
            {"scope": "scene", "visible": True, "apply": "all"}, "a", "a",
            lambda b, a, k: not vis(b, "a") and all(e["visibleInScene"] for e in a["els"]), hide("a"))
    dom_row("row disclosure", lambda: f"timeline.row.{ctx['p']}.collapsed", "element.collapsed.set",
            lambda b: {"elementIds": [ctx["p"]], "collapsed": not el(b, "p")["collapsed"]}, "hier", "p",
            lambda b, a, k: el(a, "p")["collapsed"] != el(b, "p")["collapsed"], toggle=True)
    dom_row("row visibility", lambda: f"timeline.row.{ctx['a']}.visible", "element.visibility.set",
            lambda b: {"elementIds": [ctx["a"]], "scope": "scene", "visible": not vis(b, "a")}, "a", "a",
            lambda b, a, k: vis(a, "a") != vis(b, "a"), toggle=True)

    def slider_flow(ident):
        async def flow():
            front()
            t.cdp.js("document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']').focus()" % json.dumps(ident))
            key(0x27)  # Right arrow: one step
            return "focus, then a real Right arrow key"
        return flow

    for name, ident, member, effect in (("toolbar Zoom", "timeline.zoom", "zoomLevel", "zoom"),
                                        ("scroll bar", "timeline.scroll", "firstFrame", "first")):
        rows.append({"name": f"Timeline › {name}", "spec": f"{spec_line('timeline.view.set')}; 04 lines 600-610",
                     "action": ("none",), "custom": slider_flow(ident),
                     "expect": (lambda m, e: lambda b: ("timeline.view.set", {m: b[e] + 1}))(member, effect),
                     "label": None, "setup": setup("a", "a"), "state": state,
                     "effect": (lambda e: lambda b, a, k: a[e] == b[e] + 1)(effect), "disabled": ("dom", ident),
                     "mcpReason": TL_SLIDER})

    # The main toolbar's playback items (01 §5.7; 09 §2.2): Play/Stop (clicked twice; Loop on, so playback runs until
    # the second click) and Last frame.
    async def looping():
        await cmd("playback.loop.set", {"enabled": True})

    rows.append({"name": "main toolbar › playStop", "spec": f"{spec_line('playback.start')}; {spec_line('playback.stop')}; 09 §2.2",
                 "action": ("dom", "mainToolbar.playStop"),
                 "expect": lambda b: ("playback.stop" if b["playing"] else "playback.start", {}), "label": None,
                 "setup": setup("a", "a", looping), "state": state, "toggle": True,
                 "effect": lambda b, a, k: a["playing"] is (not b["playing"]), "disabled": ("dom", "mainToolbar.playStop")})
    rows.append({"name": "main toolbar › lastFrame", "spec": f"{spec_line('timeline.currentFrame.move')}; 09 §2.2",
                 "action": ("dom", "mainToolbar.lastFrame"),
                 "expect": ("timeline.currentFrame.move", {"direction": "last", "withinRange": True}), "label": None,
                 "setup": setup("a", "a"), "state": state,
                 "effect": lambda b, a, k: a["frame"] == a["range"]["end"] and a["frame"] != b["frame"],
                 "disabled": ("dom", "mainToolbar.lastFrame")})

    # The undo step each command makes: its label as the kernel declares it (`describe_command`'s `undoLabel`; none
    # for UI state, selections and view settings).
    labels: dict = {}
    for r in rows:
        command = r.pop("undoOf", None)
        if command is not None:
            if command not in labels:
                d = await e2e.call(t.s, "describe_command", {"id": command})
                labels[command] = (d.get("result") or d).get("undoLabel")
            r["label"] = labels[command]

    # Bob alone again, for whatever runs after the table.
    rows.append({"name": "Timeline › fixture back to Bob alone", "spec": "D-60 item 3", "skip": True, "setup": bob_alone})
    return rows


async def load_report_row(t: Table, clean: Path) -> None:
    """The load report window (10 §7.3; plan §4.7; design note 2 §7): opening a project that needs repair (a folder of
    a newer minor format whose element uses a value this version does not know: the C1.3 recipe) shows one window; the
    clean project opened next shows none. MCP form: the window tree; OS form: the page."""
    rec = {"name": "load report window", "spec": f"{spec_line('doc.loadReport.get')}; 10 §7.3"}
    try:
        folder = t.work / "D39-repair.animpkg"
        await t.cmd("project.create", {"name": "D39 repair", "onUnsaved": "fail"})
        await t.cmd("element.create", {"type": "drawing", "name": "Old"})
        await t.cmd("project.saveAs", {"path": f"{folder}/", "onExists": "overwrite"})
        await t.cmd("project.close", {"onUnsaved": "fail"})
        m = json.loads((folder / "manifest.json").read_text(encoding="utf-8"))
        m["formatVersion"] = "1.3"
        (folder / "manifest.json").write_text(json.dumps(m), encoding="utf-8")
        element = next(q for q in sorted(folder.rglob("*.json")) if "elements" in q.parts)
        el = json.loads(element.read_text(encoding="utf-8"))
        el["loopType"] = "pingPong"
        element.write_text(json.dumps(el, indent=2), encoding="utf-8")

        async def windows():
            if isinstance(t, McpTable):
                tree = await t.tree()
                n = t.node(tree, "loadReport")
                return (1 if n is not None else 0), (t.node(tree, "loadReport.summary") or {}).get("label")
            return (t.cdp.js("document.querySelectorAll('[data-tt-id=\"loadReport\"]').length"),
                    t.cdp.js("document.querySelector('[data-tt-id=\"loadReport.summary\"]')?.textContent ?? null"))

        await t.cmd("project.open", {"path": str(folder), "onUnsaved": "fail"})
        shown = (0, None)
        for _ in range(50):
            shown = await windows()
            if shown[0]:
                break
            await asyncio.sleep(0.1)
        report = await t.cmd("doc.loadReport.get", {})
        await t.cmd("project.open", {"path": str(clean), "onUnsaved": "fail"})
        await asyncio.sleep(1.5)
        after = await windows()
        ok = shown[0] == 1 and bool(shown[1]) and after[0] == 0
        rec.update({"shownAfterRepairOpen": shown, "issues": [i.get("code") for i in report.get("issues") or []][:10],
                    "readOnly": report.get("readOnly"), "afterCleanOpen": after, "ok": ok})
        t.run.check(ok, f"D-39 {rec['name']}: one window after an open that needed repair, none after a clean open "
                        f"({rec['spec']})", rec)
    except Exception as ex:  # noqa: BLE001
        rec.update({"ok": False, "error": repr(ex)})
        t.run.check(False, f"D-39 {rec['name']}: ran", repr(ex))
    t.rows.append(rec)
    t.run.save("rows.json", t.rows)


async def custom_row(t: Table, r: dict, own_action: bool = False) -> None:
    """A row whose action is a flow (dialogs on the way): run as `Table.row` with the flow as the action."""
    flow = r["custom"]

    async def do(_action, flow=flow):
        res = await flow()
        return res if isinstance(res, str) else "real OS input through the flow"
    original = t.do
    t.do = do
    try:
        if r.get("late_expect"):
            # The expected backup id is known once the dialog listed it: evaluated after the action.
            await _row_late(t, r, r["expect"])
        else:
            await t.row(r)
    finally:
        t.do = original



_OWN_CUSTOM_ROW = custom_row


async def _row_late(t: Table, r: dict, expect) -> None:
    rec = {"name": r["name"], "spec": r["spec"], "action": list(r["action"])}
    try:
        if r.get("setup"):
            await r["setup"]()
        before = await r["state"]()
        pos = await t.position()
        t.clear()
        how = await t.do(r["action"])
        sent = [c for c in t.recorded() if t.kinds.get(c.get("command")) != "query"]
        want = expect(before)
        after = await r["state"]()
        steps = [list(s) for s in await t.steps_since(pos)] if after else []
        effect = bool(r["effect"](before, after, 0))
        ok = sent == [{"command": want[0], "params": want[1]}] and effect
        rec.update({"clicks": [{"input": how, "expected": {"command": want[0], "params": want[1]}, "sent": sent, "effect": effect,
                                "steps": steps, "before": before, "after": after, "ok": ok}], "ok": ok})
        t.run.check(ok, f"D-39 {r['name']}: the window sends exactly the command and changes the state ({r['spec']})", rec["clicks"][0])
    except Exception as ex:  # noqa: BLE001
        import traceback
        rec.update({"ok": False, "error": repr(ex), "trace": traceback.format_exc()[-1500:]})
        t.run.check(False, f"D-39 {r['name']}: ran", repr(ex))
    rec["disabledTarget"] = r.get("disabled")
    t.rows.append(rec)
    t.run.save("rows.json", t.rows)


async def jobs_row(t: Table) -> None:
    """The status bar's jobs segment (01 §9.1): an agent's render job shows "1 job running" while it runs, nothing after."""
    seq = t.work / "frames"
    rec = {"name": "status bar › jobs", "spec": f"{spec_line('job.list')}; 01 §9.1 (status bar: running jobs)"}
    try:
        env = await e2e.run_cmd(t.s, "render.range.run", {"from": 1, "to": 240, "step": 1, "scale": 2.0, "directory": str(seq),
                                                           "createDirs": True, "onConflict": "overwrite"})
        jid = (env.get("result") or {}).get("jobId")
        shown, running = [], []
        async def jobs_text():
            if isinstance(t, McpTable):
                n = t.node(await t.tree(), "status.jobs")
                return (n or {}).get("label") or "" if n is not None else None
            return t.cdp.js("document.querySelector('[data-tt-id=\"status.jobs\"]')?.textContent ?? null")
        for _ in range(100):
            text = await jobs_text()
            jobs = (await e2e.run_cmd(t.s, "job.list", {})).get("result")
            listed = jobs if isinstance(jobs, list) else (jobs or {}).get("jobs") or []
            running.append(sum(1 for j in listed if j.get("state") == "running"))
            shown.append(text)
            if text and "running" in text:
                break
            await asyncio.sleep(0.1)
        rec["runningPerKernel"] = running[-5:]
        if jid:
            await e2e.call(t.s, "job_wait", {"jobId": jid, "timeoutMs": 120000})
        # Waits for states (D-45): the job has ended in the kernel (`job_wait` is bounded; a slow render outlives it),
        # then the window's segment empties within the bound (it read once after a fixed second before).
        still = None
        for _ in range(600):
            jobs = (await e2e.run_cmd(t.s, "job.list", {})).get("result")
            listed = jobs if isinstance(jobs, list) else (jobs or {}).get("jobs") or []
            still = sum(1 for j in listed if j.get("state") == "running")
            if still == 0:
                break
            await asyncio.sleep(0.5)
        rec["runningAfterWait"] = still
        end = await jobs_text()
        for _ in range(150):
            if end == "":
                break
            await asyncio.sleep(0.1)
            end = await jobs_text()
        ok = any(x and "1 job running" in x for x in shown) and end == ""
        rec.update({"jobId": jid, "shownWhileRunning": [x for x in shown if x][-1:] or shown[-1:], "afterEnd": end, "ok": ok})
        t.run.check(ok, "D-39 status bar › jobs: a running job shows and goes when it ends (01 §9.1)", rec)
    except Exception as ex:  # noqa: BLE001
        rec.update({"ok": False, "error": repr(ex)})
        t.run.check(False, "D-39 status bar › jobs: ran", repr(ex))
    t.rows.append(rec)
    t.run.save("rows.json", t.rows)


MENU_GEOMETRY_JS = """(() => {
  const menu = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']');
  if (!menu) return null;
  const box = (e) => { const r = e.getBoundingClientRect(); return {l: r.left, t: r.top, r: r.right, b: r.bottom}; };
  const list = menu.querySelector('.context-menu-items');
  // Read as laid out now: nothing is scrolled by the script (D-66 item 1: the person's real scrolling only).
  const items = [...menu.querySelectorAll('[role^="menuitem"]')].map((e) => ({
    id: e.dataset.ttId, box: box(e), disabled: e.getAttribute('aria-disabled') === 'true',
    command: e.dataset.ttCommand, params: JSON.parse(e.dataset.ttParams || '{}')}));
  const arrows = [...menu.querySelectorAll('.context-menu-arrow')].map((a) => ({
    dir: a.dataset.ttArrow, box: box(a), spent: a.classList.contains('spent')}));
  return {menu: box(menu), list: list ? box(list) : null, scrollTop: list ? list.scrollTop : 0,
          maxScroll: list ? list.scrollHeight - list.clientHeight : 0, scrolls: menu.classList.contains('scrolls'),
          arrows, items, focused: document.activeElement && document.activeElement.dataset
            ? document.activeElement.dataset.ttId || null : null,
          area: {w: document.documentElement.clientWidth, h: document.documentElement.clientHeight},
          dpr: window.devicePixelRatio || 1};
})()"""


def _send_mouse(flags: int, data: int = 0) -> bool:
    """One mouse event through `SendInput` (0x0002/0x0004 left down/up, 0x0800 wheel with `data` in 120ths)."""
    class MOUSEINPUT(ctypes.Structure):
        _fields_ = [("dx", wintypes.LONG), ("dy", wintypes.LONG), ("mouseData", wintypes.DWORD),
                    ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD), ("dwExtraInfo", ctypes.c_size_t)]

    class MINPUT(ctypes.Structure):
        _fields_ = [("type", wintypes.DWORD), ("mi", MOUSEINPUT)]

    i = MINPUT(type=0, mi=MOUSEINPUT(0, 0, data & 0xFFFFFFFF, flags, 0, 0))
    return u32.SendInput(1, ctypes.byref(i), ctypes.sizeof(MINPUT)) == 1


def wheel_screen(x: int, y: int, notches: int) -> None:
    """The real wheel over (x, y): `notches` > 0 scrolls down."""
    u32.SetCursorPos(int(x), int(y))
    time.sleep(0.08)
    for _ in range(abs(notches)):
        _send_mouse(0x0800, -120 if notches > 0 else 120)
        time.sleep(0.06)
    time.sleep(0.25)


# The client areas the row runs at: the product's window (1280 by 800) and the least the host lets the person resize it
# to (tauri.conf.json `minWidth` 800, `minHeight` 500, the inner size; a program's own SetWindowPos is not held to it,
# so the client area is set to exactly that size, as the reviewer's probe does).
MENU_SIZES = (("1280 by 800", 1280, 800), ("the minimum window size, 800 by 500", 800, 500))


def resize_client(w: int, h: int, dpr: float) -> None:
    """Sets the main window's client area to `w` by `h` CSS pixels (the outer size from the frame's own size)."""
    handle = front()
    outer, client = wintypes.RECT(), wintypes.RECT()
    u32.GetWindowRect(handle, ctypes.byref(outer))
    u32.GetClientRect(handle, ctypes.byref(client))
    frame_w = (outer.right - outer.left) - client.right
    frame_h = (outer.bottom - outer.top) - client.bottom
    u32.MoveWindow(handle, 30, 30, int(w * dpr) + frame_w, int(h * dpr) + frame_h, True)


async def timeline_layout_row(t: Table) -> None:
    """M5 C5.1b: the Timeline's tracks canvas fills the area the layout gives it and the layout settles. The canvas once
    sized its own box (its pixel height was its layout height, and the size observer fed each resize back): it grew by
    about 1,500 pixels a second without end, from start-up, re-rendering every frame (the likely cause of the bundle job's
    IPC benchmark timing out, runs 45258623690 and 45262565711). Sampled three times over two seconds: unchanged, and
    inside the panel."""
    rec = {"name": "Timeline › the layout settles", "spec": "04 lines 600-610; M5 C5.1b"}
    if isinstance(t, McpTable):
        reason = "the canvas's size is the window's layout, read through the webview's debugging port in the OS form"
        rec.update({"ok": True, "notRun": reason})
        t.run.check(True, f"D-39 {rec['name']}: runs in the OS form only ({reason})", None)
        t.rows.append(rec)
        t.run.save("rows.json", t.rows)
        return
    js = ("(() => { const c = document.querySelector('[data-tt-id=\"timeline.tracks\"]');"
          " const p = document.querySelector('[data-tt-id=\"timeline\"]'); if (!c || !p) return null;"
          " const a = c.getBoundingClientRect(), b = p.getBoundingClientRect();"
          " return {canvas: [Math.round(a.height), c.height], panel: [Math.round(b.top), Math.round(b.bottom)],"
          " canvasBottom: Math.round(a.bottom), scroll: p.scrollHeight - p.clientHeight}; })()")
    samples = []
    for _ in range(3):
        samples.append(t.cdp.js(js))
        await asyncio.sleep(1.0)
    ok = (all(x is not None for x in samples) and len({json.dumps(x["canvas"]) for x in samples}) == 1
          and all(x["canvasBottom"] <= x["panel"][1] and x["scroll"] <= 0 for x in samples))
    rec.update({"samples": samples, "ok": ok})
    t.run.check(ok, f"D-39 {rec['name']}: the tracks canvas keeps its size and stays inside the panel", rec)
    t.rows.append(rec)
    t.run.save("rows.json", t.rows)


async def menu_bounds_row(t: Table, bob) -> None:
    """D-60 item 1 and D-66 item 1 (C44A-R1-01): at a 1280 by 800 client area and at the least the host allows (800 by
    500, set exactly), each view's context menu opened where it
    would cross an edge (the Drawing view's centre, as reported, and the Camera view's bottom-right corner, with a
    drawing tool) has its outer box and both arrows inside the client area. A menu that scrolls is then scrolled by
    real input only: End and Home by the real keyboard (the focused item fully inside the visible items each time), a
    real click on the lower arrow (it scrolls), the real wheel (up to the top, then down until the last item shows),
    and the pointer resting on the lower arrow (the items scroll until the last item shows). The last item is then
    activated by a real click and sends its declared reference once."""
    rec = {"name": "context menus › inside the client area at 1280 by 800 and at the minimum window size",
           "spec": "D-60 item 1, D-66 item 1 (C44A-R1-01); 01 §5.3 line 373; 02 §4.4 lines 291-296; design note 1 §9"}
    if isinstance(t, McpTable):
        reason = "the menu's geometry is the window's layout, read through the webview's debugging port in the OS form"
        rec.update({"ok": True, "notRun": reason})
        t.run.check(True, f"D-39 {rec['name']}: runs in the OS form only ({reason})", None)
        t.rows.append(rec)
        t.run.save("rows.json", t.rows)
        return
    cases = []

    def geometry(menu_id):
        return t.cdp.js(MENU_GEOMETRY_JS % json.dumps(menu_id))

    def inside(b, w, hh):
        return b is not None and b["l"] >= 0 and b["t"] >= 0 and b["r"] <= w and b["b"] <= hh

    def shows(item, g):
        lb = g["list"]
        return lb is not None and item["box"]["t"] >= lb["t"] - 0.5 and item["box"]["b"] <= lb["b"] + 0.5

    def screen_of(g, x, y):
        hw = front()
        origin = wintypes.POINT(0, 0)
        u32.ClientToScreen(hw, ctypes.byref(origin))
        return origin.x + x * g["dpr"], origin.y + y * g["dpr"]

    def centre(b):
        return (b["l"] + b["r"]) / 2, (b["t"] + b["b"]) / 2

    async def until(menu_id, test, bound_s=10.0):
        """D-45: waits for a state of the menu, bounded."""
        g = None
        for _ in range(int(bound_s / 0.1)):
            g = geometry(menu_id)
            if g and test(g):
                return g, True
            await asyncio.sleep(0.1)
        return g, False

    try:
        for size_name, ww, wh in MENU_SIZES:
            front()
            resize_client(ww, wh, t.cdp.js("window.devicePixelRatio || 1"))
            await asyncio.sleep(1.0)
            for view, corner in (("drawingView", "centre"), ("cameraView", "bottomRight")):
                await e2e.run_cmd(t.s, "app.context.set", {"elementId": bob, "frame": 1})
                await t.cmd("window.panel.set", {"panelId": view, "visible": True, "focus": True})
                await t.cmd("view.active.set", {"viewId": view})
                # A drawing tool chosen now: the Camera view runs the Drawing family (SI-148).
                await t.cmd("tool.active.set", {"toolId": "brush"})
                await t.cmd("tool.active.set", {"toolId": "select"})
                # The last item (Break Text Apart) enabled: a text object and a rectangle, both selected.
                await t.cmd("draw.rectangle.add", {"from": {"x": -60, "y": -40}, "to": {"x": 60, "y": 40}, "layer": "new"})
                await t.cmd("draw.text.add", {"position": {"x": -40, "y": 60}, "text": "Ab"})
                await t.cmd("draw.selection.set", {"by": "all"})
                await asyncio.sleep(0.6)
                key(0x1B)
                canvas = t.cdp.js("(() => { const r = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']')"
                                  ".getBoundingClientRect(); return {l: r.left, t: r.top, r: r.right, b: r.bottom,"
                                  " dpr: window.devicePixelRatio || 1}; })()" % json.dumps(f"{view}.canvas"))
                x, y = ((canvas["l"] + canvas["r"]) / 2, (canvas["t"] + canvas["b"]) / 2) if corner == "centre" else (
                    canvas["r"] - 12, canvas["b"] - 12)
                sx, sy = screen_of({"dpr": canvas["dpr"]}, x, y)
                right_click_screen(sx, sy)
                menu_id = f"{view}.drawingMenu"
                geo, opened = await until(menu_id, lambda g: g["items"] and not all(i["disabled"] for i in g["items"]), 5.0)
                if not geo:
                    under = t.cdp.js("document.elementsFromPoint(%s,%s).slice(0,5).map(e=>e.tagName+'.'+e.className+'#'"
                                     "+(e.dataset.ttId||''))" % (x, y))
                    area = t.cdp.js("({w: document.documentElement.clientWidth, h: document.documentElement.clientHeight})")
                    raise RuntimeError(f"the menu did not open: {menu_id} (size {size_name}, area {area}, canvas {canvas},"
                                       f" opened at {x},{y}, under it {under})")
                w, hh = geo["area"]["w"], geo["area"]["h"]
                case = {"size": size_name, "view": view, "openedAt": {"x": x, "y": y}, "area": geo["area"],
                        "menu": geo["menu"], "arrows": geo["arrows"], "scrolls": geo["scrolls"],
                        "items": len(geo["items"])}
                boxes_ok = inside(geo["menu"], w, hh) and all(inside(a["box"], w, hh) for a in geo["arrows"])
                case["outerBoxAndArrowsInside"] = boxes_ok
                last_index = len(geo["items"]) - 1
                checks = {}
                # D-68 item 5: the page itself never scrolls, at either size (its panels do).
                page = t.cdp.js("(() => { const d = document.documentElement; return {sh: d.scrollHeight, ch: d.clientHeight,"
                                " sw: d.scrollWidth, cw: d.clientWidth, iw: window.innerWidth}; })()")
                case["page"] = page
                checks["pageDoesNotScroll"] = (page["sh"] <= page["ch"] and page["sw"] <= page["cw"]
                                               and page["cw"] == page["iw"])
                if geo["scrolls"]:
                    # The real keyboard: End focuses the last item, fully inside the visible items; Home the first.
                    key(0x23)
                    g, _ = await until(menu_id, lambda g: g["focused"] == g["items"][-1]["id"], 3.0)
                    checks["endShowsLast"] = bool(g) and g["focused"] == g["items"][-1]["id"] and shows(g["items"][-1], g) \
                        and inside(g["items"][-1]["box"], w, hh)
                    key(0x24)
                    g, _ = await until(menu_id, lambda g: g["focused"] == g["items"][0]["id"] and g["scrollTop"] == 0, 3.0)
                    checks["homeShowsFirst"] = bool(g) and g["scrollTop"] == 0 and shows(g["items"][0], g)
                    # A real click on the lower arrow scrolls (the pointer leaves it before the hold would start).
                    down = next(a for a in geo["arrows"] if a["dir"] == "down")
                    ax, ay = screen_of(geo, *centre(down["box"]))
                    lx, ly = screen_of(geo, *centre(geo["list"]))
                    u32.SetCursorPos(int(ax), int(ay))
                    time.sleep(0.03)
                    _send_mouse(0x0002)
                    time.sleep(0.03)
                    _send_mouse(0x0004)
                    u32.SetCursorPos(int(lx), int(ly))
                    g, moved = await until(menu_id, lambda g: g["scrollTop"] > 0, 2.0)
                    checks["arrowClickScrolls"] = moved
                    case["afterArrowClick"] = g["scrollTop"] if g else None
                    # The real wheel: up to the top, then down until the last item shows.
                    wheel_screen(lx, ly, -10)
                    g, top = await until(menu_id, lambda g: g["scrollTop"] == 0, 2.0)
                    checks["wheelUpScrolls"] = top
                    reached = False
                    for _ in range(40):
                        wheel_screen(lx, ly, 1)
                        g = geometry(menu_id)
                        if g and shows(g["items"][-1], g):
                            reached = True
                            break
                    checks["wheelDownReachesLast"] = reached
                    wheel_screen(lx, ly, -40)
                    # The pointer resting on the lower arrow scrolls until the last item shows.
                    u32.SetCursorPos(int(ax), int(ay))
                    g, rested = await until(menu_id, lambda g: shows(g["items"][-1], g), 15.0)
                    checks["arrowHoverReachesLast"] = rested
                    geo = g or geometry(menu_id)
                last = geo["items"][last_index]
                checks["lastShows"] = (not geo["scrolls"] or shows(last, geo)) and inside(last["box"], w, hh)
                await asyncio.sleep(0.3)
                t.clear()
                cx, cy = screen_of(geo, *centre(last["box"]))
                click_screen(cx, cy)
                await asyncio.sleep(1.0)
                sent = [c for c in t.recorded() if t.kinds.get(c.get("command")) != "query"]
                want = [{"command": last["command"], "params": last["params"]}]
                gone = t.cdp.js("!document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']')" % json.dumps(menu_id))
                ok = (boxes_ok and all(checks.values()) and not last["disabled"] and sent == want and bool(gone))
                case.update({"checks": checks, "last": last, "input": "real scrolling, then a real OS click",
                             "sent": sent, "expected": want, "closed": gone, "ok": ok})
                cases.append(case)
                key(0x1B)
                await e2e.run_cmd(t.s, "draw.selection.set", {"by": "none"})
        # The window back at the product's size for the rows after this one.
        u32.SetWindowPos(front(), 0, 0, 0, 1280, 800, 0x0002 | 0x0004)
        rec["areas"] = [c["area"] for c in cases]
        await asyncio.sleep(1.0)
        ok_all = len(cases) == 2 * len(MENU_SIZES) and all(c["ok"] for c in cases)
        rec.update({"cases": cases, "ok": ok_all})
        t.run.check(ok_all, f"D-39 {rec['name']}: the outer box and arrows inside, scrolled by real input, the last item "
                    "activated by a real click", cases)
    except Exception as ex:  # noqa: BLE001
        rec.update({"ok": False, "error": repr(ex), "cases": cases})
        t.run.check(False, f"D-39 {rec['name']}: ran", repr(ex))
    t.rows.append(rec)
    t.run.save("rows.json", t.rows)


def visible_popups() -> set:
    """The visible top-level windows of the desktop (a native context menu adds one: `#32768` or a Chromium popup)."""
    found = set()
    if not u32:
        return found
    proc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def each(h, _):
        if u32.IsWindowVisible(h):
            name = ctypes.create_unicode_buffer(128)
            u32.GetClassNameW(h, name, 128)
            found.add((int(h or 0), name.value))
        return True
    u32.EnumWindows(proc(each), 0)
    return found


async def browser_chrome_row(t: Table) -> None:
    """D-70 item 2 (product build): real secondary clicks on a main toolbar button and on a Pen panel row open no native
    popup (no new visible top-level window; the event's default cancelled), and F5 and Ctrl+R with the focus in the
    token field and on the canvas do not reload the page (a marker set in the page survives)."""
    rec = {"name": "browser chrome › no native menu, no reload", "spec": "D-70 item 2"}
    if isinstance(t, McpTable):
        reason = "real secondary clicks and real keys of the person, which the MCP form does not perform"
        rec.update({"ok": True, "notRun": reason})
        t.run.check(True, f"D-39 {rec['name']}: runs in the OS form only ({reason})", None)
        t.rows.append(rec)
        t.run.save("rows.json", t.rows)
        return
    checks = {}
    try:
        front()
        t.cdp.js("window.__ttMenus = []; window.addEventListener('contextmenu', (e) => window.__ttMenus.push("
                 "e.defaultPrevented)); window.__ttMarker = 'd70'; true")

        async def right_click(ident):
            r = t.rect(ident)
            if not r or r.get("hidden"):
                raise RuntimeError(f"absent: {ident}")
            h = front()
            origin = wintypes.POINT(0, 0)
            u32.ClientToScreen(h, ctypes.byref(origin))
            before = visible_popups()
            right_click_screen(origin.x + r["x"] * r["dpr"], origin.y + r["y"] * r["dpr"])
            await asyncio.sleep(0.8)
            new = sorted(c for _, c in visible_popups() - before)
            key(0x1B)
            await asyncio.sleep(0.3)
            return new

        button = t.cdp.js("document.querySelector('[data-tt-id=\"mainToolbar\"] button[data-tt-id]')?.dataset.ttId ?? null")
        if not button:
            raise RuntimeError("no main toolbar button")
        checks["toolbarNoPopup"] = await right_click(button)
        # A list box on the main toolbar (the onion skin's): not text entry, so no menu either (round 3 live).
        select = t.cdp.js("document.querySelector('[data-tt-id=\"mainToolbar\"] select[data-tt-id]')?.dataset.ttId ?? null")
        checks["toolbarSelect"] = select
        checks["toolbarSelectNoPopup"] = await right_click(select) if select else []
        await t.cmd("window.panel.set", {"panelId": "pen", "visible": True, "focus": True})
        await asyncio.sleep(0.6)
        pens = await t.cmd("tool.pen.list", {})
        row = f"pen.row.{pens.get('currentBrushPenId')}"
        checks["penRowNoPopup"] = await right_click(row)
        prevented = t.cdp.js("window.__ttMenus") or []
        checks["defaultsCancelled"] = prevented

        def marker():
            try:
                return t.cdp.js("window.__ttMarker ?? null")
            except Exception:  # noqa: BLE001 - a reloaded page drops the connection
                return None

        async def reloads(where):
            out = {}
            for name, (vk, mods) in (("F5", (0x74, ())), ("Ctrl+R", (0x52, (0x11,)))):
                key(vk, *mods)
                await asyncio.sleep(1.5)
                out[name] = marker()
            return {where: out}

        # The canvas focused.
        t.click("drawingView.canvas")
        await asyncio.sleep(0.3)
        checks.update(await reloads("canvas"))
        # The token field focused (Preferences › Agent access).
        choose_menu(["File", "Preferences"])
        await asyncio.sleep(1.0)
        t.click("preferences.category.agent")
        await asyncio.sleep(0.6)
        t.click("agentAccess.token")
        await asyncio.sleep(0.3)
        checks["tokenFocused"] = t.cdp.js("document.activeElement?.dataset?.ttId ?? null")
        checks.update(await reloads("tokenField"))
        key(0x1B)
        await asyncio.sleep(0.5)
        ok = (checks["toolbarNoPopup"] == [] and checks["toolbarSelectNoPopup"] == [] and checks["penRowNoPopup"] == []
              and bool(select) and len(prevented) >= 3
              and all(prevented) and checks["tokenFocused"] == "agentAccess.token"
              and all(v == "d70" for w in ("canvas", "tokenField") for v in checks[w].values()))
        rec.update({"checks": checks, "ok": ok})
        t.run.check(ok, f"D-39 {rec['name']}: no native popup on a toolbar button or a panel row; F5 and Ctrl+R reload "
                    "nothing in the token field or on the canvas", checks)
    except Exception as ex:  # noqa: BLE001
        rec.update({"ok": False, "error": repr(ex), "checks": checks})
        t.run.check(False, f"D-39 {rec['name']}: ran", repr(ex))
    t.rows.append(rec)
    t.run.save("rows.json", t.rows)


async def recent_hint_row(t: Table) -> None:
    """Open Recent's full path (01 §3.3 line 190; ruling D-48 item 8; reviews/M4-design-panels.md §13). The project is
    saved under a known path, so it heads Open Recent. MCP form: the window tree's menu bar names the entry's full path
    (`hint`). OS form (Windows): real clicks open File, then Open Recent; the cursor rests on the first entry; the status
    bar's left line is its full path; Escape closes the menu and the line is what it was before. No command is sent."""
    rec = {"name": "File › Open Recent › an entry's full path", "spec": "01 §3.3 line 190 (\"the full path is shown in the status bar or tooltip\")"}
    try:
        path = t.work / "D39-recent.animpkg"
        await t.cmd("project.saveAs", {"path": str(path), "onExists": "overwrite"})  # waits for the save job
        listed = (await e2e.run_cmd(t.s, "project.recent.list", {})).get("result")
        entries = listed if isinstance(listed, list) else (listed or {}).get("projects") or (listed or {}).get("items") or []
        want = (entries[0] or {}).get("path") if entries else None
        if not (want or "").endswith(path.name):
            raise RuntimeError(f"the saved project does not head Open Recent: {entries[:2]}")
        await asyncio.sleep(1.5)  # the native menu is rebuilt from the kernel's recent list
        if isinstance(t, McpTable):
            entry = t.menu_node(await t.tree(), ["File", "Open Recent"])
            first = ((entry or {}).get("children") or [{}])[0]
            ok = want is not None and first.get("hint") == want and first.get("id") == "dyn:recent:0"
            rec.update({"form": "mcp", "expected": want, "node": {k: first.get(k) for k in ("id", "label", "hint")}, "ok": ok})
            t.run.check(ok, f"D-39 {rec['name']}: the window tree's Open Recent entry names its full path ({rec['spec']})", rec)
        else:
            def line():
                return t.cdp.js("document.querySelector('[data-tt-id=\"status.line\"]')?.textContent ?? null")
            before = line()
            t.clear()
            h = front()
            handle = u32.GetMenu(h)
            for label in ("File", "Open Recent"):
                labels = menu_labels(handle)
                i = labels.index(label)
                r = wintypes.RECT()
                if not u32.GetMenuItemRect(h, handle, i, ctypes.byref(r)):
                    raise RuntimeError(f"native menu rectangle absent: {label}")
                click_screen((r.left + r.right) // 2, (r.top + r.bottom) // 2)
                handle = u32.GetSubMenu(handle, i)
            r = wintypes.RECT()
            if not u32.GetMenuItemRect(h, handle, 0, ctypes.byref(r)):
                raise RuntimeError("the first Open Recent entry has no rectangle")
            # Resting on the entry highlights it (no click: choosing it would open the project).
            u32.SetCursorPos((r.left + r.right) // 2 + 1, (r.top + r.bottom) // 2)
            time.sleep(0.15)
            u32.SetCursorPos((r.left + r.right) // 2, (r.top + r.bottom) // 2)
            shown = None
            for _ in range(30):
                time.sleep(0.1)
                shown = line()
                if shown == want:
                    break
            key(0x1B)
            key(0x1B)
            key(0x1B)
            time.sleep(0.5)
            after = line()
            sent = [c for c in t.recorded() if t.kinds.get(c.get("command")) != "query"]
            ok = want is not None and shown == want and after == before and sent == []
            rec.update({"form": "os", "expected": want, "shownWhileHighlighted": shown, "before": before, "afterClose": after,
                        "sent": sent, "ok": ok})
            t.run.check(ok, f"D-39 {rec['name']}: the status bar shows the highlighted entry's full path, then its line "
                            f"again; no command ({rec['spec']})", rec)
    except Exception as ex:  # noqa: BLE001
        rec.update({"ok": False, "error": repr(ex)})
        t.run.check(False, f"D-39 {rec['name']}: ran", repr(ex))
    t.rows.append(rec)
    t.run.save("rows.json", t.rows)


async def session_all(app: str, run, out: Path, work: Path, only: set, mode: str = "os") -> list:
    user = work / "user"
    (user / "Settings").mkdir(parents=True)
    seed = {"agent.policy.defaults": {"policy": "autonomous", "maxJobs": 2, "maxBatchSteps": 1000, "maxInlineImagePx": 2048,
                                      "maxInlineBytes": 52428800, "askInApp": True, "letAgentMoveView": True, "highlightAgentChanges": True}}
    (user / "Settings" / "preferences.json").write_text(json.dumps({"format": "toontown-settings", "kind": "preferences", "formatVersion": "1.0", "data": seed}), encoding="utf-8")
    discovery = work / "discovery.json"
    if mode == "os":
        os.environ["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = f"--remote-debugging-port={CDP_PORT}"
    # A webview browser process still running from an earlier launch with the same data folder (an earlier step of the
    # same CI job) would take this window and ignore the argument: this run gets a data folder of its own (WebView2's
    # environment variables override the application's values).
        os.environ["WEBVIEW2_USER_DATA_FOLDER"] = str(work / "webview2")
    proc = e2e.launch(app, user, discovery)
    try:
        d = e2e.wait_discovery(discovery, proc)
        async with contextlib.AsyncExitStack() as stack:
            client = await stack.enter_async_context(
                e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {d['token']}"}, timeout=e2e.httpx2.Timeout(60.0, read=300.0)))
            streams = await stack.enter_async_context(e2e.streamable_http_client(d["endpoint"], http_client=client))
            from mcp import types

            async def roots(_ctx):
                return types.ListRootsResult(roots=[types.Root(uri=work.resolve().as_uri(), name="work")])
            s = await stack.enter_async_context(e2e.ClientSession(streams[0], streams[1], list_roots_callback=roots))
            await s.initialize()
            await e2e.gui_tree(s, run, lambda r: e2e.by_id(r, "startScreen") is not None, "the GUI is up")
            t = (McpTable if mode == "mcp" else Table)(s, run, out, work)
            await build_and_run(t, only)
            return t.rows
    finally:
        e2e.stop(proc)


def main() -> int:
    e6.dpi_aware()
    ap = argparse.ArgumentParser()
    ap.add_argument("app")
    ap.add_argument("out")
    ap.add_argument("--only", default="")
    ap.add_argument("--mode", choices=["os", "mcp"], default="os",
                    help="os: real Windows input, the page's IPC observed (workstation); mcp: the attached MCP interface (CI)")
    a = ap.parse_args()
    if a.mode == "os" and sys.platform != "win32":
        print("D-39's OS form drives the real window through Windows input; on this platform run --mode mcp.")
        return 0
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    work = Path(tempfile.mkdtemp(prefix="tt-d39-"))
    started = time.monotonic()
    rows: list = []
    try:
        rows = asyncio.run(session_all(a.app, run, out, work, {x for x in a.only.split(",") if x}, a.mode))
    except Exception as ex:  # noqa: BLE001 - reported as a failure with its text
        import traceback
        traceback.print_exception(ex)
        run.check(False, "the table ran", repr(ex))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    manifest = {"kind": "toontown.c43b.d39-bindings", "mode": a.mode, "seconds": round(time.monotonic() - started, 1), "rows": len(rows),
                "failed": [r.get("name") for r in rows if not r.get("ok")], "failures": run.failures}
    run.save("rows.json", rows)
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps(manifest, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
