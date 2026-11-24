"""M5 C5.1b: the Timeline's three-way rows through the running window (plan §9; design note
`reviews/M5-design-timeline-view.md` §10), on a diagnostics build.

Usage: python scripts/c51b-timing-three-way.py <toontown diagnostics build> <out dir> [--external]

Each gesture of the plan's §6.3 (the cases of `desktop/src/diagnostics/timingCheck.ts`) is made on the Timeline of the
kernel's sample document; for each, the window's commit, the kernel interpreter's replay of the samples the window sent
(with the window's own layout) and the plan's commands run directly are compared exactly: the plans, and the state the
commit made against the state the direct run made (`desktop/src-tauri/src/timing_check.rs`).

- Without `--external` (CI, both systems): the window makes each gesture through its own event path (pointer events
  dispatched on the Timeline's canvas and layer list, the handlers a pointer reaches).
- With `--external` (the Windows workstation): each gesture is real OS input, a real drag with the modifiers held at the
  points the window publishes (read through the webview's debugging port, as `c43-d39-bindings.py`).

Output: `report.json` (the window's rows), `manifest.json` (kind `toontown.c51b.timing-three-way`). Exit 0 when every
row holds.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BOUND_S = 900


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def real_gestures(proc: subprocess.Popen, report: Path) -> list:
    """Makes each published gesture with real OS input until the report is written; returns what was done."""
    d39 = _load("c43d39", "c43-d39-bindings.py")
    e6, e2e = d39.e6, d39.e2e
    e2e.APP_PID["pid"] = proc.pid
    conn = {"cdp": None}
    # The page is up once it shows the start screen or the main toolbar (a wait for a state; the bound guards a hang).
    first = time.monotonic()
    while conn["cdp"] is None:
        try:
            conn["cdp"] = d39.Cdp(d39.CDP_PORT)
        except RuntimeError:
            if time.monotonic() - first > 30 or proc.poll() is not None:
                raise
            time.sleep(0.5)

    def js(expr: str):
        """The page's answer; a connection that stopped answering is replaced (its socket gives up when idle)."""
        try:
            return conn["cdp"].js(expr)
        except Exception:  # noqa: BLE001 - reconnect once, then let the error through
            conn["cdp"] = d39.Cdp(d39.CDP_PORT)
            return conn["cdp"].js(expr)
    done_cases: list = []
    handled = -1
    start = time.monotonic()
    while proc.poll() is None and not report.exists() and time.monotonic() - start < BOUND_S:
        case = js("window.__ttTimingCheck ?? null")
        if not case or case.get("index", -1) <= handled or case.get("done") != case.get("index", 0) - 1:
            time.sleep(0.2)
            continue
        dpr = js("window.devicePixelRatio || 1")
        client = [(p["x"] * dpr, p["y"] * dpr) for p in case["points"]]
        vks = [{"alt": 0x12, "shift": 0x10, "ctrl": 0x11, "meta": 0x5B}[m] for m in case.get("modifiers") or []]
        times: dict = {}
        for vk in vks:
            d39.u32.keybd_event(vk, 0, 0, 0)
        try:
            if case.get("escape"):
                # A real press and moves, the real Escape, then the release (nothing may be sent).
                h = d39.front()
                origin = d39.wintypes.POINT(0, 0)
                d39.u32.ClientToScreen(h, d39.ctypes.byref(origin))
                d39.u32.SetCursorPos(origin.x + int(client[0][0]), origin.y + int(client[0][1]))
                time.sleep(0.05)
                e6._send_button(True)
                for x, y in client[1:]:
                    time.sleep(0.06)
                    d39.u32.SetCursorPos(origin.x + int(x), origin.y + int(y))
                time.sleep(0.1)
                d39.key(0x1B)
                time.sleep(0.1)
                e6._send_button(False)
            else:
                e6.os_drag(proc.pid, client, 0.06 * max(1, len(client) - 1), 0.05, times)
        finally:
            for vk in reversed(vks):
                d39.u32.keybd_event(vk, 0, 2, 0)
        done_cases.append({"index": case["index"], "name": case.get("name"), "input": "real OS drag", **times})
        print(f"input {case['index']}: {case.get('name')} {times.get('error') or 'made'} under {case.get('under')}", flush=True)
        handled = case["index"]
        # The window finishes its comparison before it publishes the next case (a wait for a state, D-45).
        wait = time.monotonic()
        while time.monotonic() - wait < 60:
            now = js("window.__ttTimingCheck?.done ?? null")
            if now is not None and now >= handled:
                break
            if report.exists() or proc.poll() is not None:
                break
            time.sleep(0.2)
    return done_cases


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("app")
    ap.add_argument("out")
    ap.add_argument("--only", default=None, help="only the cases whose name contains this text (a probe's subset)")
    ap.add_argument("--external", action="store_true",
                    help="real OS input on the Windows workstation instead of the window's dispatched events")
    a = ap.parse_args()
    if a.external and sys.platform != "win32":
        print("--external drives the real window through Windows input; on this platform run without it.")
        return 0
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    report = out / "report.json"
    report.unlink(missing_ok=True)
    work = Path(tempfile.mkdtemp(prefix="tt-c51b-three-way-"))
    env = dict(os.environ)
    if a.external:
        d39 = _load("c43d39", "c43-d39-bindings.py")
        env["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = f"--remote-debugging-port={d39.CDP_PORT}"
        env["WEBVIEW2_USER_DATA_FOLDER"] = str(work / "webview2")
    args = [a.app, "--timing-check", str(report), "--user-dir", str(work / "user")]
    if a.external:
        args.append("--timing-check-external")
    if a.only:
        args += ["--timing-check-only", a.only]
    started = time.monotonic()
    proc = subprocess.Popen(args, env=env)
    done: list = []
    error = None
    try:
        if a.external:
            done = real_gestures(proc, report)
        proc.wait(timeout=BOUND_S)
    except Exception as ex:  # noqa: BLE001 - reported with its text
        error = repr(ex)
    finally:
        if proc.poll() is None:
            proc.kill()
        shutil.rmtree(work, ignore_errors=True)
    frontend = {}
    if report.exists():
        frontend = (json.loads(report.read_text(encoding="utf-8")) or {}).get("frontend") or {}
    rows = frontend.get("rows") or []
    failed = [r.get("name") for r in rows if not r.get("ok")]
    ok = bool(rows) and not failed and frontend.get("ok") is True and error is None
    manifest = {"kind": "toontown.c51b.timing-three-way", "external": a.external, "exitCode": proc.returncode,
                "seconds": round(time.monotonic() - started, 1), "rows": len(rows), "failed": failed,
                "error": error or frontend.get("error"), "platform": frontend.get("platform"), "input": done, "ok": ok}
    (out / "manifest.json").write_text(json.dumps(manifest, indent=1), encoding="utf-8")
    for r in rows:
        print(("ok   " if r.get("ok") else "FAIL ") + str(r.get("name")) +
              ("" if r.get("ok") else f"  {json.dumps({k: r.get(k) for k in ('error', 'pressSeen', 'pressAnswer', 'aborted', 'blurs', 'remounted', 'focusedAtStart', 'planEqual', 'hitsTheGesture', 'committed', 'stateEqual', 'sent', 'unchanged')})}"),
              flush=True)
    print(json.dumps({k: v for k, v in manifest.items() if k != "input"}, indent=1))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
