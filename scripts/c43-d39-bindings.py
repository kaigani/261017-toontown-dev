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
spec lines. Output: `rows.json`, `transcript.json`, `manifest.json` (kind `toontown.c43b.d39-bindings`); the committed
receipt is their summary (ruling D-42: `scripts/receipt-summary.py <out> --command ... --delete`). Exit 0 when
every row holds.

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
        if kind == "none":
            return "no input"
        raise ValueError(kind)

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
                how = await self.do(r["action"])
                await asyncio.sleep(r.get("wait", 0.8))
                sent = await self.observed(want)
                after = await r["state"]()
                steps = await self.steps_since(pos)
                label = r.get("label")
                expect_steps = [] if label is None else [[label, self.WHO]]
                got_steps = [list(s) for s in steps]
                effect = bool(r["effect"](before, after, k))
                click = {"input": how, "expected": {"command": want[0], "params": want[1]}, "sent": sent,
                         "effect": effect, "steps": got_steps, "expectedSteps": expect_steps,
                         "before": before, "after": after}
                exact = sent == [{"command": want[0], "params": want[1]}]
                if self.WHO == "agent":
                    click["declared"] = self.declared
                    exact = exact and self.declared == {"command": want[0], "params": want[1]}
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

    async def observed(self, want) -> list:
        return [c for c in self.recorded() if self.kinds.get(c.get("command")) != "query" or c.get("command") == want[0]]

    async def after_failure(self) -> None:
        with contextlib.suppress(Exception):
            key(0x1B)
            key(0x1B)

    async def disabled_checks(self) -> None:
        """After the project is closed (`project.none` refuses every control of the table): each is disabled or gone,
        a real click on it sends nothing and changes nothing; each native menu item is disabled in the menu and in the
        window tree (D-38)."""
        answer = await e2e.call(self.s, "inspect_ui", {"target": "window", "depth": 32})
        tree = (answer.get("result") or answer).get("root") or {}
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
        out = []
        for e in sorted(await self.activity(), key=lambda e: e.get("seq", 0)):
            if e.get("seq", 0) <= self.seq or e.get("tool") not in ("run_command", "key"):
                continue
            if self.kinds.get(e.get("commandId")) == "query" and e.get("commandId") != want[0]:
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
        else:
            raise RuntimeError("an action the MCP form does not perform")
        env = await e2e.run_cmd(self.s, self.declared["command"], self.declared["params"])
        return f"the declared reference through run_command ({env.get('status')})"

    async def disabled_checks(self) -> None:
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
    def menu_row(label, command, params, spec_label, steps, effect, path=None):
        rows.append({"name": f"Drawing › {label}", "spec": f"{spec_line(command)} (label \"{spec_label}\"); 02 §4.4 lines 293-296",
                     "action": ("menu", path or ["Drawing", label]), "expect": (command, params), "label": spec_label,
                     "setup": setup(*steps), "state": drawing, "effect": effect, "disabled": ("menu", path or ["Drawing", label])})

    two = [lambda: rect(-150, 0), lambda: rect(150, 0), select_all]
    layers = lambda d: [n for n in nodes_of(d) if n.get("type") == "layer" and n.get("strokes")]  # noqa: E731
    menu_row("Flatten", "draw.flatten.apply", {}, "Flatten Drawing", two, lambda b, a, k: len(layers(a)) < len(layers(b)))
    menu_row("Optimize", "draw.optimize.apply", {}, "Optimize Drawing", two, lambda b, a, k: len(layers(a)) < len(layers(b)))

    async def wobbly():
        pts = [{"x": -200 + 10 * i, "y": (i % 2) * 0.2} for i in range(40)]
        await cmd("draw.stroke.add", {"kind": "pencil", "points": pts, "fit": False, "layer": "new"})
    anchors = lambda d: sum(len(s.get("bezier") or []) for s in strokes_of(d))  # noqa: E731
    menu_row("Smooth", "draw.smooth.apply", {}, "Smooth Selection", [wobbly, select_all], lambda b, a, k: anchors(a) < anchors(b))
    kinds = lambda d: sorted(s.get("kind") for s in strokes_of(d))  # noqa: E731
    menu_row("Convert Lines to Brush", "draw.linesToBrush.apply", {}, "Convert Lines to Brush",
             [lambda: rect(0, 0), select_all], lambda b, a, k: "brush" in kinds(a) and "brush" not in kinds(b))

    async def text():
        await cmd("draw.text.add", {"position": {"x": -60, "y": 0}, "text": "Ab"})
    menu_row("Break Text Apart", "draw.breakApart.apply", {}, "Break Text Apart", [text, select_all],
             lambda b, a, k: any(n.get("type") == "text" for n in nodes_of(b)) and not any(n.get("type") == "text" for n in nodes_of(a)))

    async def brush():
        await cmd("draw.stroke.add", {"kind": "brush", "points": [{"x": -100, "y": 0}, {"x": 0, "y": 40}, {"x": 100, "y": 0}], "layer": "new"})
    menu_row("Extract Centre Line", "draw.centreLine.apply", {}, "Extract Centre Line", [brush, select_all],
             lambda b, a, k: "pencil" in kinds(a) and "pencil" not in kinds(b))

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
                 path=["Drawing", "Arrange", label])
    geometry = lambda b, a, k: [s.get("bezier") for s in strokes_of(a)] != [s.get("bezier") for s in strokes_of(b)]  # noqa: E731
    one = [lambda: rect(120, 60), select_all]
    menu_row("Flip Horizontal", "draw.flip.apply", {"axis": "horizontal"}, "Flip Horizontal", one, geometry)
    menu_row("Flip Vertical", "draw.flip.apply", {"axis": "vertical"}, "Flip Vertical", one, geometry)
    menu_row("Rotate 90° CW", "draw.rotate.apply", {"by": "90cw"}, "Rotate 90° CW", one, geometry)
    menu_row("Rotate 90° CCW", "draw.rotate.apply", {"by": "90ccw"}, "Rotate 90° CCW", one, geometry)
    menu_row("Rotate 180°", "draw.rotate.apply", {"by": "180"}, "Rotate 180°", one, geometry)
    groups = lambda d: [n for n in nodes_of(d) if n.get("type") == "group"]  # noqa: E731
    menu_row("Group", "draw.group.create", {}, "Group Selection", two, lambda b, a, k: len(groups(a)) == len(groups(b)) + 1)

    async def grouped():
        d = await drawing()
        g = await cmd("draw.group.create", {"ids": [s["id"] for s in strokes_of(d)]})
        # The group itself selected (a person's click on it; `by: all` selects its strokes, not the group).
        await cmd("draw.selection.set", {"by": "ids", "ids": [g["groupId"]]})
    menu_row("Ungroup", "draw.group.delete", {}, "Ungroup Selection", two + [grouped],
             lambda b, a, k: len(groups(a)) == len(groups(b)) - 1)
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
        await asyncio.sleep(0.6)

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

    for flag in ("bold", "italic", "autoKern"):
        rows.append({"name": f"Text panel › {flag}", "spec": f"{spec_line('tool.text.options.set')}; 02 §7.6.1",
                     "action": ("dom", f"text.{flag}"),
                     "expect": (lambda f: lambda b: ("tool.text.options.set", {f: not b.get(f)}))(flag), "label": None,
                     "setup": text_shown, "state": text_state, "toggle": True,
                     "effect": (lambda f: lambda b, a, k: a.get(f) == (not b.get(f)))(flag), "disabled": None})
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

    for r in rows:
        if r.get("skip"):
            await r["setup"]()
            continue
        if only and not any(o in r["name"] for o in only):
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

    await jobs_row(t)
    await recent_hint_row(t)
    # Saved first: the agent's session may not discard (its policy), and the close must not ask.
    await e2e.run_cmd(t.s, "project.saveAs", {"path": str(t.work / "D39-final.animpkg"), "onExists": "overwrite"})
    await asyncio.sleep(1.0)
    await cmd("project.close", {"onUnsaved": "fail"})
    await asyncio.sleep(1.5)
    await t.disabled_checks()


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
        await asyncio.sleep(1.0)
        end = await jobs_text()
        ok = any(x and "1 job running" in x for x in shown) and end == ""
        rec.update({"jobId": jid, "shownWhileRunning": [x for x in shown if x][-1:] or shown[-1:], "afterEnd": end, "ok": ok})
        t.run.check(ok, "D-39 status bar › jobs: a running job shows and goes when it ends (01 §9.1)", rec)
    except Exception as ex:  # noqa: BLE001
        rec.update({"ok": False, "error": repr(ex)})
        t.run.check(False, "D-39 status bar › jobs: ran", repr(ex))
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
