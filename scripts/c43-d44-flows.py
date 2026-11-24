"""M4 C4.3b ruling D-44: every dialog or message box that offers a choice, one row per choice, through the real window.

Usage: python scripts/c43-d44-flows.py <toontown binary> <out dir> [--only NAME[,NAME...]]

D-39's table covers one action per control; a dialog's branches are flows it cannot see (C43B-R2-07: Open Backup after
Don't Save kept the dirty document; C43B-R2-08: keymap Import could never merge). Each row here is one choice in one
dialog, through the operating system's real input: the path to the dialog (a native menu item, a control), the button
chosen, the commands and parameters the window sent (the page's IPC, recorded through the webview's debugging port as
in `c43-d39-bindings.py`, whose machinery this script reuses), and the effect read back through MCP or on disk.

Rows:
- the unsaved-changes question (01 §3.5) in front of New, Open, Open Recent, Open Backup, Close and Quit, each with
  Save, Don't Save and Cancel (recovery open is named: its only entry, the start screen, shows only without a project,
  so the question never comes up in the window; `shell/unsavedFlows.test.tsx` covers its three choices);
- Save As onto an existing file: the native dialog's own overwrite question, then the application's Replace / Cancel;
- the shortcut editor: Import with Merge, Replace and Cancel; Reset all with Reset All and Cancel; a conflict with
  Override and Cancel (01 §8.3);
- the workspaces: New Workspace with Create and Cancel, Rename Workspace with Rename and Cancel (01 §5.5);
- Preferences: OK and Cancel with a changed value (01 §7.1);
- the progress window's Cancel on an agent session's running job (01 §9.5, C43B-R2-06);
- the engine restart question is named: it needs an engine that stops answering (`App.test.tsx` covers it).

Quit's Don't Save and Save end the application: each runs in a session of its own, after the others.

Output: `rows.json`, `transcript.json`, `manifest.json` (kind `toontown.c43b.d44-flows`); the committed receipt is their
summary (ruling D-42: `scripts/receipt-summary.py <out> --command ... --delete`). Exit 0 when every row holds.
"""

from __future__ import annotations

import argparse
import asyncio
import contextlib
import hashlib
import importlib.util
import json
import os
import shutil
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


d39 = _load("c43d39flows", "c43-d39-bindings.py")
e2e, e6 = d39.e2e, d39.e6
choose_menu, key, type_text, spec_line = d39.choose_menu, d39.key, d39.type_text, d39.spec_line

VK_CONTROL, VK_SHIFT, VK_MENU, VK_RETURN, VK_ESCAPE = 0x11, 0x10, 0x12, 0x0D, 0x1B
#: The commands a flow's choice can send; anything else the page sends is recorded beside the row, not compared.
FLOW_COMMANDS = {
    "project.save", "project.saveAs", "project.create", "project.open", "project.backup.open", "project.recovery.open",
    "project.close", "app.quit", "keymap.set", "keymap.import", "keymap.reset", "keymap.remove", "workspace.create",
    "workspace.rename", "prefs.set", "job.cancel", "project.properties.set", "doc.repair",
    # M4 C4.4a: the toolbar customisation sheet; Preferences › Agent access's Regenerate token.
    "window.toolbar.items.set", "window.toolbar.items.reset", "agent.token.regenerate",
    # The Activity panel's Undo back to before an entry; the confirmation sheet (D-58).
    "edit.history.move", "agent.confirmation.respond",
    # M5 C5.1b: the Timeline's dialogs (the Element and Play menus, the Timeline's context menus and toolbar, Edit's
    # Paste Special and Cut Special in a cell context).
    "element.create", "element.rename", "element.note.set", "element.colour.set", "element.timing.set",
    "element.childExposure.apply", "app.frame.set", "playback.range.set", "timeline.currentFrame.set",
    "timeline.playbackRange.set", "timeline.transferOptions.set", "edit.pasteSpecial", "edit.cutSpecial",
}


def _ipc(self, body: str) -> None:
    """One IPC request's body, with its dry-run option kept (the shortcut editor checks a key with a dry run first)."""
    try:
        outer = json.loads(body)
        req = json.loads(outer["json"]) if isinstance(outer, dict) and isinstance(outer.get("json"), str) else outer
        call = {"command": req.get("command"), "params": req.get("params") or {}}
        if (req.get("options") or {}).get("dryRun"):
            call["dryRun"] = True
        self.calls.append(call)
    except (ValueError, TypeError, KeyError, AttributeError):
        self.calls.append({"unparsed": body[:200]})


d39.Cdp._ipc = _ipc


def sha(path: Path) -> str | None:
    return hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else None


def same_path(a, b) -> bool:
    return a is not None and b is not None and os.path.normcase(os.path.abspath(str(a))) == os.path.normcase(os.path.abspath(str(b)))


def matches(sent: list, want: list) -> bool:
    """`want`: (command, must, absent) each; `must` values are equal or a predicate."""
    if len(sent) != len(want):
        return False
    for c, (command, must, absent) in zip(sent, want):
        p = c.get("params") or {}
        if c.get("command") != command:
            return False
        for k, v in must.items():
            if callable(v):
                if not v(p.get(k)):
                    return False
            elif p.get(k) != v:
                return False
        if any(k in p for k in absent):
            return False
    return True


def printable(want: list) -> list:
    return [{"command": c, "must": {k: ("<predicate>" if callable(v) else v) for k, v in m.items()}, "absent": a} for c, m, a in want]


VK_OF = {**{chr(c): c for c in range(ord("A"), ord("Z") + 1)}, **{str(d): 0x30 + d for d in range(10)},
         **{f"F{n}": 0x6F + n for n in range(1, 13)}, "Enter": 0x0D}


def press(keys: str) -> None:
    """A canonical binding (`Primary+Shift+B`) through the real keyboard (Windows: Primary is Control)."""
    *mods, last = keys.split("+")
    vk = VK_OF.get(last.upper() if len(last) == 1 else last)
    if vk is None:
        raise RuntimeError(f"no virtual key for {keys}")
    m = [{"Primary": VK_CONTROL, "Control": VK_CONTROL, "Shift": VK_SHIFT, "Alt": VK_MENU}[x] for x in mods]
    key(vk, *m)


def select_field() -> None:
    """Selects the focused field's text with Home and Shift+End (Control+A is the Edit menu's Select All accelerator)."""
    key(0x24)
    key(0x23, VK_SHIFT)


class Flows(d39.Table):
    def __init__(self, session, run, out: Path, work: Path, proc):
        super().__init__(session, run, out, work)
        self.proc = proc
        self.fx: dict = {}

    def exists(self, ident: str) -> bool:
        return bool(self.cdp.js("!!document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']')" % json.dumps(ident)))

    async def wait_for(self, ident: str, timeout: float = 10.0) -> bool:
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if self.exists(ident):
                await asyncio.sleep(0.3)
                return True
            await asyncio.sleep(0.2)
        return False

    async def settle(self) -> None:
        """No native menu, dialog or message box left from a row before (each would hold the next row's commands)."""
        d39.front()
        key(VK_ESCAPE)
        key(VK_ESCAPE)
        for _ in range(4):
            open_ = self.cdp.js("!!document.querySelector('[role=alertdialog],[role=dialog][data-tt-id]:not([data-tt-id=startScreen])')")
            if not open_:
                return
            d39.front()
            key(VK_ESCAPE)
            await asyncio.sleep(0.4)

    async def project_state(self) -> dict:
        s = await self.cmd("app.state.get", {})
        p = s.get("project") or None
        return {"open": p is not None, "path": (p or {}).get("path"), "name": (p or {}).get("name"), "dirty": (p or {}).get("dirty"),
                "flow": sha(self.fx["flow"]), "other": sha(self.fx["other"])}

    async def prepare_clean(self, _ctx=None) -> None:
        """The fixture project open from its file with no unsaved change (no unsaved-changes question comes first)."""
        await self.prepare_dirty()
        await self.cmd("project.save", {})
        await asyncio.sleep(0.8)

    async def prepare_dirty(self, _ctx=None) -> None:
        """The fixture project open from its file, with one unsaved change."""
        st = await self.cmd("app.state.get", {})
        p = st.get("project")
        if p and same_path(p.get("path"), self.fx["flow"]) and not p.get("readOnly"):
            # Already the fixture (reopening the file it holds would open it read-only: its own lock).
            if not p.get("dirty"):
                await self.dirty()
            await asyncio.sleep(0.8)
            return
        if p and p.get("dirty"):
            n = len(list(self.work.glob("scratch-*.animpkg")))
            await self.cmd("project.saveAs", {"path": str(self.work / f"scratch-{n}.animpkg"), "onExists": "overwrite"})
        await self.cmd("project.open", {"path": str(self.fx["flow"]), "onUnsaved": "fail"})
        await self.dirty()
        await asyncio.sleep(1.0)

    async def dirty(self) -> None:
        found = await self.cmd("doc.object.find", {"type": "element", "name": "Bob"})
        bob = (found.get("items") or found.get("matches") or [{}])[0].get("id")
        await self.cmd("app.context.set", {"elementId": bob, "frame": 1})
        k = int(time.monotonic() * 10) % 200
        await self.cmd("draw.rectangle.add", {"from": {"x": k, "y": 0}, "to": {"x": k + 10, "y": 10}})

    async def open_preferences(self, category: str) -> None:
        """File › Preferences, then the category by a real click. D-45 (run of 2026-11-21: a click landed on the row
        above, Agent access instead of Keyboard, while the opening dialog still laid out): the click waits for the
        category's box to read the same twice, and the category is confirmed selected (`aria-pressed`, its page shown);
        a second click is made once if it is not."""
        choose_menu(["File", "Preferences"])
        ident = f"preferences.category.{category}"
        if not await self.wait_for(ident):
            raise RuntimeError("Preferences did not open")
        box_js = ("(() => { const e = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']');"
                  " if (!e) return null; const b = e.getBoundingClientRect(); return [b.x, b.y, b.width, b.height]; })()"
                  % json.dumps(ident))
        chosen_js = ("(() => { const e = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']');"
                     " return !!e && e.getAttribute('aria-pressed') === 'true'"
                     " && !!document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']'); })()"
                     % (json.dumps(ident), json.dumps(f"preferences.{category}")))
        for _attempt in range(2):
            box = None
            for _ in range(30):
                now = self.cdp.js(box_js)
                if now is not None and now == box:
                    break
                box = now
                await asyncio.sleep(0.1)
            self.click(ident)
            for _ in range(20):
                if self.cdp.js(chosen_js):
                    await asyncio.sleep(0.3)
                    return
                await asyncio.sleep(0.1)
        raise RuntimeError(f"the Preferences category {category} could not be chosen")

    async def keys_of(self, command: str) -> list:
        return sorted(b.get("keys") for b in await self.cmd("keymap.list", {"commandId": command}))

    async def flow_row(self, r: dict) -> None:
        rec = {"name": r["name"], "spec": r["spec"], "path": r["path"], "choice": r["choice"]}
        try:
            await self.settle()
            ctx: dict = {}
            if r.get("setup"):
                await r["setup"](ctx)
            before = await r["state"](ctx)
            self.clear()
            how = await r["flow"](ctx)
            await asyncio.sleep(r.get("wait", 1.5))
            calls = self.recorded()
            sent = [c for c in calls if not c.get("dryRun") and c.get("command") in FLOW_COMMANDS]
            want = r["expect"](ctx)
            after = await r["state"](ctx)
            effect = bool(r["effect"](before, after, ctx))
            exact = matches(sent, want)
            ok = exact and effect
            rec.update({"input": how, "expected": printable(want), "sent": sent, "dryRuns": [c for c in calls if c.get("dryRun")],
                        "otherSent": [c for c in calls if not c.get("dryRun") and c.get("command") not in FLOW_COMMANDS
                                      and self.kinds.get(c.get("command")) != "query"],
                        "exact": exact, "effect": effect, "before": before, "after": after, "ok": ok,
                        # What the row observed beyond the commands (answers of held calls, refusals, queue text).
                        "observed": {k: ctx[k] for k in ("answers", "gone", "second", "readWhileHeld", "queue", "ended",
                                                          "default", "order") if k in ctx}})
            self.run.check(ok, f"D-44 {r['name']}: the choice sends exactly its commands and has its effect ({r['spec']})",
                           {k: rec[k] for k in ("expected", "sent", "exact", "effect")})
        except Exception as ex:  # noqa: BLE001 - the row fails with its text, the table goes on
            import traceback
            rec.update({"ok": False, "error": repr(ex), "trace": traceback.format_exc()[-1500:]})
            with contextlib.suppress(Exception):
                rec["diag"] = {
                    "calls": self.recorded(),
                    "dialogs": self.cdp.js("Array.from(document.querySelectorAll('[role=dialog],[role=alertdialog]'))"
                                           ".map(e => (e.getAttribute('data-tt-id') || '') + ': ' + (e.getAttribute('aria-label') || '')"
                                           " + ' ' + e.textContent.slice(0, 160))"),
                    "windowTitle": self.cdp.js("document.title"),
                }
            self.run.check(False, f"D-44 {r['name']}: ran", repr(ex))
            with contextlib.suppress(Exception):
                await self.settle()
        if r.get("cleanup"):
            with contextlib.suppress(Exception):
                await r["cleanup"]()
        self.rows.append(rec)
        self.run.save("rows.json", self.rows)

    def named(self, name: str, spec: str, reason: str) -> None:
        self.rows.append({"name": name, "spec": spec, "ok": True, "notRun": reason})
        self.run.check(True, f"D-44 {name}: named, not run in the window ({reason})", None)
        self.run.save("rows.json", self.rows)


# ----- the rows --------------------------------------------------------------------------------------------------------

CHOICES = (("save", "Save"), ("discard", "Don't Save"), ("cancel", "Cancel"))


def unsaved_rows(t: Flows, only_quit: str | None) -> list:
    fx = t.fx
    spec = "01 §3.5 (unsaved changes: Save / Don't Save / Cancel); " + "{}"

    async def new_done(ctx):
        if not await t.wait_for("newProject.name"):
            raise RuntimeError("the New Project dialog did not open")
        t.click("newProject.name")
        select_field()
        type_text("FlowNew")
        await asyncio.sleep(0.3)
        t.click("newProject.create")
        await asyncio.sleep(2.0)

    async def open_done(ctx):
        await asyncio.sleep(1.8)
        type_text(str(fx["other"]))
        key(VK_RETURN)
        await asyncio.sleep(3.0)

    async def backup_done(ctx):
        if not await t.wait_for("openBackup.open.0"):
            raise RuntimeError("the Open Backup dialog listed no backup")
        params = t.cdp.js("document.querySelector('[data-tt-id=\"openBackup.open.0\"]').dataset.ttParams")
        ctx["backupId"] = json.loads(params).get("backupId")
        t.click("openBackup.open.0")
        await asyncio.sleep(3.0)

    async def wait_only(ctx):
        await asyncio.sleep(2.0)

    commands = [
        ("New", ["File", "New Project"], "project.create", lambda ctx: {"name": "FlowNew"}, new_done,
         lambda a, ctx: a["open"] and a["path"] is None and a["name"] == "FlowNew"),
        ("Open", ["File", "Open"], "project.open", lambda ctx: {"path": lambda v: same_path(v, fx["other"])}, open_done,
         lambda a, ctx: same_path(a["path"], fx["other"])),
        ("Open Recent", ["File", "Open Recent", "Other"], "project.open", lambda ctx: {"path": lambda v: same_path(v, fx["other"])},
         wait_only, lambda a, ctx: same_path(a["path"], fx["other"])),
        ("Open Backup", ["File", "Open Backup"], "project.backup.open", lambda ctx: {"backupId": ctx.get("backupId")}, backup_done,
         lambda a, ctx: a["open"] and a["path"] is None),
        ("Close", ["File", "Close Project"], "project.close", lambda ctx: {}, wait_only, lambda a, ctx: not a["open"]),
    ]
    rows = []
    if only_quit is None:
        for label, path, command, must, done, reached in commands:
            for choice, button in CHOICES:
                rows.append(unsaved_row(t, label, path, command, must, done, reached, choice, button, spec.format(spec_line(command))))
        rows.append(quit_row(t, "cancel", "Cancel"))
    else:
        rows.append(quit_row(t, only_quit, dict(CHOICES)[only_quit]))
    return rows


def unsaved_row(t: Flows, label, path, command, must, done, reached, choice, button, spec) -> dict:
    async def flow(ctx):
        choose_menu(path)
        if not await t.wait_for("message.project.unsavedChanges"):
            raise RuntimeError("the unsaved-changes question did not appear")
        t.click(f"message.{choice}")
        if choice != "cancel":
            await done(ctx)
        return f"real native menu {' › '.join(path)}, then a real click on {button}"

    def expect(ctx):
        if choice == "cancel":
            return []
        m = must(ctx)
        if choice == "save":
            return [("project.save", {}, []), (command, m, ["onUnsaved"])]
        return [(command, {**m, "onUnsaved": "discard"}, [])]

    def effect(b, a, ctx):
        if choice == "cancel":
            return same_path(a["path"], t.fx["flow"]) and a["dirty"] is True and a["flow"] == b["flow"]
        saved = a["flow"] != b["flow"]
        return reached(a, ctx) and (saved if choice == "save" else not saved)

    return {"name": f"{label} › {button}", "spec": spec, "path": path, "choice": button, "setup": t.prepare_dirty,
            "state": lambda ctx: t.project_state(), "flow": flow, "expect": expect, "effect": effect}


def quit_row(t: Flows, choice: str, button: str) -> dict:
    async def state(_ctx):
        alive = t.proc.poll() is None
        out = {"alive": alive, "flow": sha(t.fx["flow"])}
        if alive:
            with contextlib.suppress(Exception):
                out.update({k: v for k, v in (await t.project_state()).items() if k in ("path", "dirty")})
        return out

    async def flow(ctx):
        choose_menu(["File", "Quit"])
        if not await t.wait_for("message.project.unsavedChanges"):
            raise RuntimeError("the unsaved-changes question did not appear")
        t.click(f"message.{choice}")
        for _ in range(60):
            if t.proc.poll() is not None or choice == "cancel":
                break
            await asyncio.sleep(0.25)
        return f"real native menu File › Quit, then a real click on {button}"

    def expect(ctx):
        if choice == "cancel":
            return []
        if choice == "save":
            return [("project.save", {}, []), ("app.quit", {}, ["onUnsaved"])]
        return [("app.quit", {"onUnsaved": "discard"}, [])]

    def effect(b, a, ctx):
        if choice == "cancel":
            return a["alive"] and a.get("dirty") is True and a["flow"] == b["flow"]
        return not a["alive"] and ((a["flow"] != b["flow"]) if choice == "save" else (a["flow"] == b["flow"]))

    return {"name": f"Quit › {button}", "spec": f"01 §3.5; {spec_line('app.quit')}", "path": ["File", "Quit"], "choice": button,
            "setup": t.prepare_dirty, "state": state, "flow": flow, "expect": expect, "effect": effect, "wait": 1.0}


def save_as_rows(t: Flows) -> list:
    """Ruling D-49 item 1 (C43B-R3-01): one replacement decision per Save As, the native picker's own (Windows and macOS
    ask before replacing; reviews/M4-design-dialogs.md §1). Its Yes saves over the file with `onExists: overwrite` and no
    application question; its No leaves the picker open, and Escape then sends nothing."""
    fx = t.fx
    rows = []
    for choice, button in (("yes", "native Yes"), ("no", "native No")):
        async def flow(ctx, choice=choice, button=button):
            choose_menu(["File", "Save As"])
            await asyncio.sleep(1.8)
            type_text(str(fx["other"]))
            key(VK_RETURN)
            await asyncio.sleep(1.2)
            key(0x59 if choice == "yes" else 0x4E)  # the native "replace?" question: Y or N
            if choice == "no":
                await asyncio.sleep(0.8)
                key(VK_ESCAPE)  # the picker, still open after No, cancelled
            await asyncio.sleep(2.5)
            ctx["questions"] = t.cdp.js("document.querySelectorAll('[role=alertdialog]').length")
            return f"real native menu File › Save As, a typed existing path, the native question's {button}"

        def expect(ctx, choice=choice):
            if choice == "no":
                return []
            return [("project.saveAs", {"path": lambda v: same_path(v, fx["other"]), "onExists": "overwrite"}, [])]

        def effect(b, a, ctx, choice=choice):
            if ctx.get("questions"):
                return False  # no application question after the native one
            if choice == "no":
                return same_path(a["path"], fx["flow"]) and a["other"] == b["other"]
            return same_path(a["path"], fx["other"]) and a["other"] != b["other"]

        async def cleanup():
            shutil.copyfile(fx["otherPristine"], fx["other"])

        rows.append({"name": f"Save As (existing file) › {button}", "spec": f"01 §3.6, §9.5; {spec_line('project.saveAs')}",
                     "path": ["File", "Save As"], "choice": button, "setup": t.prepare_dirty, "state": lambda ctx: t.project_state(),
                     "flow": flow, "expect": expect, "effect": effect, "cleanup": cleanup})
    return rows


def default_rows(t: Flows) -> list:
    """Ruling D-49 item 2 (C43B-R3-02): each message box opens on its focused default button, and Enter takes it."""
    fx = t.fx
    rows = []

    async def close_enter(ctx):
        choose_menu(["File", "Close Project"])
        if not await t.wait_for("message.project.unsavedChanges"):
            raise RuntimeError("the unsaved-changes question did not appear")
        ctx["default"] = t.cdp.js("document.activeElement && document.activeElement.dataset.ttId")
        key(VK_RETURN)
        await asyncio.sleep(2.0)
        return "real native menu File › Close Project, then a real Enter on the focused default"

    rows.append({"name": "unsaved changes › Enter (default Save)", "spec": "01 §3.5, §9.5; D-49 item 2",
                 "path": ["File", "Close Project"], "choice": "Enter", "setup": t.prepare_dirty,
                 "state": lambda ctx: t.project_state(), "flow": close_enter,
                 "expect": lambda ctx: [("project.save", {}, []), ("project.close", {}, ["onUnsaved"])],
                 "effect": lambda b, a, ctx: ctx.get("default") == "message.save" and not a["open"] and a["flow"] != b["flow"]})

    keys_file = t.work / "flow-keys-default.json"

    async def reset_setup(ctx):
        await t.cmd("keymap.reset", {"all": True})
        await t.cmd("keymap.set", {"commandId": "project.save", "keys": "Primary+Shift+F11", "replaceExisting": True})
        await t.open_preferences("keyboard")

    async def reset_enter(ctx):
        t.click("keymap.resetAll")
        if not await t.wait_for("message.keymap.resetAll"):
            raise RuntimeError("the Reset All question did not appear")
        ctx["default"] = t.cdp.js("document.activeElement && document.activeElement.dataset.ttId")
        key(VK_RETURN)
        await asyncio.sleep(0.6)
        t.click("preferences.submit")
        await asyncio.sleep(1.5)
        return "Preferences › Keyboard › Reset all, then a real Enter on the focused default, then OK"

    rows.append({"name": "shortcut editor › Reset all › Enter (default Cancel)", "spec": "01 §8.3; D-49 item 2",
                 "path": ["File", "Preferences", "Keyboard", "Reset all"], "choice": "Enter", "setup": reset_setup,
                 "state": lambda ctx: t.keys_of("project.save"), "flow": reset_enter, "expect": lambda ctx: [],
                 "effect": lambda b, a, ctx: ctx.get("default") == "message.cancel" and "Primary+Shift+F11" in a,
                 "cleanup": lambda: t.cmd("keymap.reset", {"all": True})})

    async def import_setup(ctx):
        await t.cmd("keymap.reset", {"all": True})
        await t.cmd("keymap.export", {"path": str(keys_file), "onExists": "overwrite"})
        data = json.loads(keys_file.read_text(encoding="utf-8"))
        for b in data.get("bindings", []):
            if b.get("commandId") == "project.save":
                b["keys"] = "Primary+Shift+F11"
        keys_file.write_text(json.dumps(data), encoding="utf-8")
        await t.open_preferences("keyboard")

    async def import_enter(ctx):
        t.click("keymap.import")
        await asyncio.sleep(1.8)
        type_text(str(keys_file))
        key(VK_RETURN)
        if not await t.wait_for("message.keymap.importMode"):
            raise RuntimeError("the Merge / Replace / Cancel question did not appear")
        ctx["default"] = t.cdp.js("document.activeElement && document.activeElement.dataset.ttId")
        key(VK_RETURN)
        await asyncio.sleep(0.6)
        t.click("preferences.submit")
        await asyncio.sleep(1.5)
        return "Preferences › Keyboard › Import, a typed path, then a real Enter on the focused default, then OK"

    rows.append({"name": "shortcut editor › Import › Enter (default Merge)", "spec": "01 §8.3 item 8; D-46; D-49 item 2",
                 "path": ["File", "Preferences", "Keyboard", "Import"], "choice": "Enter", "setup": import_setup,
                 "state": lambda ctx: t.keys_of("project.save"), "flow": import_enter,
                 "expect": lambda ctx: [("keymap.import", {"path": lambda v: same_path(v, keys_file), "mode": "merge"}, [])],
                 "effect": lambda b, a, ctx: ctx.get("default") == "message.merge" and a == ["Primary+S", "Primary+Shift+F11"],
                 "cleanup": lambda: t.cmd("keymap.reset", {"all": True})})
    return rows


def modal_kind_rows(t: Flows) -> list:
    """Ruling D-49 item 3 (C43B-R3-03): one row per modal kind: Tab past the last control keeps the focus inside, and
    Escape then takes Cancel (nothing is sent; the background stays as it was)."""
    rows = []

    def kind(name, path, ident, setup, state, effect):
        async def flow(ctx):
            choose_menu(path)
            if not await t.wait_for(ident):
                raise RuntimeError(f"{name} did not open")
            count = t.cdp.js("document.querySelectorAll('[aria-modal=true] button:not([disabled]), [aria-modal=true] input:not([disabled]), "
                             "[aria-modal=true] select, [aria-modal=true] summary').length") or 0
            inside = []
            for _ in range(int(count) + 2):
                key(0x09)
                await asyncio.sleep(0.1)
                inside.append(bool(t.cdp.js("!!document.activeElement && !!document.activeElement.closest('[aria-modal=true]')")))
            ctx["inside"] = all(inside)
            key(VK_ESCAPE)
            await asyncio.sleep(1.0)
            ctx["closed"] = not t.cdp.js("!!document.querySelector('[aria-modal=true]')")
            return f"real native menu {' › '.join(path)}, {int(count) + 2} real Tabs, then a real Escape"

        rows.append({"name": f"modal ({name}) › Tab past the last control, Escape", "spec": "01 §9.5; D-49 item 3",
                     "path": path, "choice": "Tab, Escape", "setup": setup, "state": state, "flow": flow,
                     "expect": lambda ctx: [],
                     "effect": lambda b, a, ctx: ctx.get("inside") is True and ctx.get("closed") is True and effect(b, a)})

    kind("message box", ["File", "Close Project"], "message.project.unsavedChanges", t.prepare_dirty,
         lambda ctx: t.project_state(), lambda b, a: a["open"] and a["dirty"] is True and a["flow"] == b["flow"])
    kind("form", ["File", "Project Properties"], "projectProperties.width", t.prepare_clean,
         lambda ctx: t.project_state(), lambda b, a: a == b)
    return rows


def keymap_rows(t: Flows) -> list:
    fx = t.fx
    rows = []
    keys_file = t.work / "flow-keys.json"

    async def import_setup(ctx):
        await t.cmd("keymap.reset", {"all": True})
        await t.cmd("keymap.export", {"path": str(keys_file), "onExists": "overwrite"})
        data = json.loads(keys_file.read_text(encoding="utf-8"))
        for b in data.get("bindings", []):
            if b.get("commandId") == "project.save":
                b["keys"] = "Primary+Shift+F11"
        keys_file.write_text(json.dumps(data), encoding="utf-8")
        await t.open_preferences("keyboard")

    async def save_keys(_ctx):
        return await t.keys_of("project.save")

    for choice, button, wanted in (("merge", "Merge", ["Primary+S", "Primary+Shift+F11"]), ("replace", "Replace", ["Primary+Shift+F11"]),
                                   ("cancel", "Cancel", ["Primary+S"])):
        async def flow(ctx, choice=choice, button=button):
            t.click("keymap.import")
            await asyncio.sleep(1.8)
            type_text(str(keys_file))
            key(VK_RETURN)
            if not await t.wait_for("message.keymap.importMode"):
                raise RuntimeError("the Merge / Replace / Cancel question did not appear")
            # Ruling D-46 (SI-161): Merge, which destroys nothing, is the default button (focused, Enter takes it).
            focused = t.cdp.js("document.activeElement && document.activeElement.dataset.ttId")
            if focused != "message.merge":
                raise RuntimeError(f"the default button is {focused}, not Merge")
            t.click(f"message.{choice}")
            await asyncio.sleep(0.6)
            t.click("preferences.submit")
            await asyncio.sleep(1.5)
            return f"Preferences › Keyboard › Import, a typed path, a real click on {button}, then OK"

        rows.append({"name": f"shortcut editor › Import › {button}", "spec": f"01 §8.3 items 8-9; {spec_line('keymap.import')}",
                     "path": ["File", "Preferences", "Keyboard", "Import"], "choice": button, "setup": import_setup, "state": save_keys,
                     "flow": flow,
                     "expect": (lambda c: lambda ctx: [] if c == "cancel" else [("keymap.import", {"path": lambda v: same_path(v, keys_file), "mode": c}, [])])(choice),
                     "effect": (lambda w: lambda b, a, ctx: a == sorted(w))(wanted),
                     "cleanup": lambda: t.cmd("keymap.reset", {"all": True})})

    async def reset_setup(ctx):
        await t.cmd("keymap.reset", {"all": True})
        await t.cmd("keymap.set", {"commandId": "project.save", "keys": "Primary+Shift+F11", "replaceExisting": True})
        await t.open_preferences("keyboard")

    for choice, button in (("reset", "Reset All"), ("cancel", "Cancel")):
        async def flow(ctx, choice=choice, button=button):
            t.click("keymap.resetAll")
            if not await t.wait_for("message.keymap.resetAll"):
                raise RuntimeError("the Reset All question did not appear")
            t.click(f"message.{choice}")
            await asyncio.sleep(0.6)
            t.click("preferences.submit")
            await asyncio.sleep(1.5)
            return f"Preferences › Keyboard › Reset all, a real click on {button}, then OK"

        rows.append({"name": f"shortcut editor › Reset all › {button}", "spec": f"01 §8.3; {spec_line('keymap.reset')}",
                     "path": ["File", "Preferences", "Keyboard", "Reset all"], "choice": button, "setup": reset_setup,
                     "state": save_keys, "flow": flow,
                     "expect": (lambda c: lambda ctx: [("keymap.reset", {"all": True}, [])] if c == "reset" else [])(choice),
                     "effect": (lambda c: lambda b, a, ctx: (a == ["Primary+S"]) if c == "reset" else ("Primary+Shift+F11" in a))(choice),
                     "cleanup": lambda: t.cmd("keymap.reset", {"all": True})})

    # A conflict: a global binding whose command no menu item carries (a menu accelerator would take the key before the
    # page), given to a command without one.
    async def conflict_setup(ctx):
        await t.cmd("keymap.reset", {"all": True})
        # Since M5 every default global binding but the Alt keys and print has a menu item: the holder is a free function
        # key given to a command on no menu item (Next Element, whose defaults are Alt keys).
        await t.cmd("keymap.set", {"commandId": "app.currentElement.move", "params": {"direction": "next"},
                                   "keys": "Shift+F8", "context": "global"})
        menu = json.loads((ROOT / "desktop/src/ui-bindings/menu.json").read_text(encoding="utf-8"))
        in_menu = set()

        def walk(items):
            for it in items:
                if it.get("kind") == "command":
                    in_menu.add(it.get("command"))
                walk(it.get("items") or [])
        for m in menu["menus"]:
            walk(m.get("items") or [])
        bindings = await t.cmd("keymap.list", {"context": "global"})
        # A global binding on no menu item, with a key the page receives (Control+P would be the webview's print, an Alt
        # key the menu bar's): Shift+Enter while Stop had no menu item; the M5 Play menu carries Stop, so the binding the
        # setup made (view note M5 C5.1b section 6).
        def receivable(b):
            k = b.get("keys") or ""
            return b.get("commandId") not in in_menu and not k.startswith("Alt+") and k not in ("Primary+P", "F11")
        pick = next((b for b in bindings if b.get("keys") == "Shift+F8" and receivable(b)), None) or next(
            b for b in bindings if receivable(b))
        ctx["keys"], ctx["holder"] = pick["keys"], pick["commandId"]
        ctx["target"] = "app.info.get"
        await t.open_preferences("keyboard")
        t.click("keymap.filter")
        type_text(ctx["target"])
        await asyncio.sleep(0.5)
        if not await t.wait_for(f"keymap.command.{ctx['target']}", 3):
            raise RuntimeError(f"the shortcut editor does not list {ctx['target']}")
        t.click(f"keymap.command.{ctx['target']}")
        if not await t.wait_for("keymap.capture", 3):
            listed = t.cdp.js("Array.from(document.querySelectorAll('[data-tt-id^=\"keymap.command.\"]')).slice(0, 8).map(e => e.dataset.ttId)")
            raise RuntimeError(f"selecting {ctx['target']} showed no details; listed: {listed}")

    async def conflict_state(ctx):
        return {"holder": await t.keys_of(ctx["holder"]), "target": await t.keys_of(ctx["target"])}

    for choice, button in (("override", "Override"), ("cancel", "Cancel")):
        async def flow(ctx, choice=choice, button=button):
            t.click("keymap.capture")
            await asyncio.sleep(0.3)
            press(ctx["keys"])
            if not await t.wait_for("keymap.conflict"):
                raise RuntimeError(f"no conflict shown for {ctx['keys']}")
            t.click(f"keymap.conflict.{choice}")
            await asyncio.sleep(0.4)
            t.click("preferences.submit")
            await asyncio.sleep(1.5)
            return f"Preferences › Keyboard › Add shortcut, the real keys {ctx['keys']}, a real click on {button}, then OK"

        rows.append({"name": f"shortcut editor › conflict › {button}", "spec": f"01 §8.3 item 6; {spec_line('keymap.set')}",
                     "path": ["File", "Preferences", "Keyboard", "Add shortcut"], "choice": button, "setup": conflict_setup,
                     "state": conflict_state, "flow": flow,
                     "expect": (lambda c: lambda ctx: [("keymap.set", {"commandId": ctx["target"], "keys": ctx["keys"], "onConflict": "override"}, [])]
                                if c == "override" else [])(choice),
                     "effect": (lambda c: lambda b, a, ctx: (ctx["keys"] in a["target"] and ctx["keys"] not in a["holder"]) if c == "override"
                                else (ctx["keys"] in a["holder"] and ctx["keys"] not in a["target"]))(choice),
                     "cleanup": lambda: t.cmd("keymap.reset", {"all": True})})
    return rows


def workspace_rows(t: Flows) -> list:
    rows = []

    async def names(_ctx):
        return sorted(w.get("name") for w in await t.cmd("workspace.list", {}))

    async def drop(*ws):
        await t.cmd("workspace.select", {"name": "Default"})
        for w in ws:
            with contextlib.suppress(Exception):
                await t.cmd("workspace.delete", {"name": w})

    for choice, button in (("submit", "Create"), ("cancel", "Cancel")):
        async def flow(ctx, choice=choice, button=button):
            choose_menu(["Window", "Workspace", "New Workspace"])
            if not await t.wait_for("workspaceNew.name"):
                raise RuntimeError("the New Workspace dialog did not open")
            t.click("workspaceNew.name")
            select_field()
            type_text("FlowSpace")
            await asyncio.sleep(0.4)
            t.click(f"workspaceNew.{choice}")
            await asyncio.sleep(1.2)
            return f"real native menu Window › Workspace › New Workspace, a typed name, a real click on {button}"

        rows.append({"name": f"New Workspace › {button}", "spec": f"01 §5.5; {spec_line('workspace.create')}",
                     "path": ["Window", "Workspace", "New Workspace"], "choice": button, "setup": lambda ctx: drop("FlowSpace"),
                     "state": names, "flow": flow,
                     "expect": (lambda c: lambda ctx: [("workspace.create", {"name": "FlowSpace"}, [])] if c == "submit" else [])(choice),
                     "effect": (lambda c: lambda b, a, ctx: ("FlowSpace" in a) == (c == "submit"))(choice),
                     "cleanup": lambda: drop("FlowSpace")})

    async def rename_setup(ctx):
        await drop("FlowSpace", "FlowSpace2")
        await t.cmd("workspace.create", {"name": "FlowSpace"})
        await t.cmd("workspace.select", {"name": "FlowSpace"})
        await asyncio.sleep(0.6)

    for choice, button in (("submit", "Rename"), ("cancel", "Cancel")):
        async def flow(ctx, choice=choice, button=button):
            choose_menu(["Window", "Workspace", "Rename Workspace"])
            if not await t.wait_for("workspaceRename.name"):
                raise RuntimeError("the Rename Workspace dialog did not open")
            t.click("workspaceRename.name")
            select_field()
            type_text("FlowSpace2")
            await asyncio.sleep(0.4)
            t.click(f"workspaceRename.{choice}")
            await asyncio.sleep(1.2)
            return f"real native menu Window › Workspace › Rename Workspace, a typed name, a real click on {button}"

        rows.append({"name": f"Rename Workspace › {button}", "spec": f"01 §5.5; {spec_line('workspace.rename')}",
                     "path": ["Window", "Workspace", "Rename Workspace"], "choice": button, "setup": rename_setup, "state": names,
                     "flow": flow,
                     "expect": (lambda c: lambda ctx: [("workspace.rename", {"name": "FlowSpace", "newName": "FlowSpace2"}, [])]
                                if c == "submit" else [])(choice),
                     "effect": (lambda c: lambda b, a, ctx: ("FlowSpace2" in a and "FlowSpace" not in a) if c == "submit"
                                else ("FlowSpace" in a and "FlowSpace2" not in a))(choice),
                     "cleanup": lambda: drop("FlowSpace", "FlowSpace2")})
    return rows


def preferences_rows(t: Flows) -> list:
    rows = []

    async def setup(ctx):
        await t.open_preferences("general")
        found = t.cdp.js("(() => { const e = document.querySelector('input[type=checkbox][data-tt-id^=\"pref.\"]');"
                         " return e ? {id: e.dataset.ttId, key: e.dataset.ttLabel, checked: e.checked} : null; })()")
        if not found:
            raise RuntimeError("no check box on the General page")
        ctx.update(found)

    async def value(ctx):
        if not ctx.get("key"):
            return None
        return ((await t.cmd("prefs.get", {"keys": [ctx["key"]]})).get("values") or {}).get(ctx["key"])

    for choice, button in (("submit", "OK"), ("cancel", "Cancel")):
        async def flow(ctx, choice=choice, button=button):
            t.click(ctx["id"])
            await asyncio.sleep(0.3)
            t.click(f"preferences.{choice}")
            await asyncio.sleep(1.2)
            return f"File › Preferences › General, a real click on the {ctx['key']} check box, then {button}"

        rows.append({"name": f"Preferences › {button}", "spec": f"01 §7.1; {spec_line('prefs.set')}",
                     "path": ["File", "Preferences", "General"], "choice": button, "setup": setup, "state": value, "flow": flow,
                     "expect": (lambda c: lambda ctx: [("prefs.set", {"values": {ctx["key"]: not ctx["checked"]}}, [])] if c == "submit" else [])(choice),
                     "effect": (lambda c: lambda b, a, ctx: (a == (not ctx["checked"])) if c == "submit" else (a == ctx["checked"]))(choice)})
    return rows


# C43B-R3S-03 (ruling D-48): every choice of every form that sends a mutating command, from a project without unsaved
# changes (the unsaved-changes question is its own rows), and one row per dismiss-only box asserting that dismissing it
# sends nothing. `scripts/c43_d44_coverage.py` lists every dialog of the window with its rows or its reason.
UNUSED_ASSET = {
    "format": "animjson", "formatVersion": "1.0", "scenes": [], "elements": [], "drawingBanks": [], "palettes": [],
    "pens": [], "blobs": {},
    "assets": [{"id": "as_01K7AAAAAAAAAAAAAAAAAAAAAA", "type": "asset", "rev": 1, "name": "line.wav", "kind": "sound",
                "mediaType": "audio/wav", "storage": "linked", "entry": None,
                "link": {"kind": "file", "path": "elsewhere/line.wav", "absolutePath": "/nowhere/elsewhere/line.wav",
                         "libraryId": None, "itemId": None, "versionId": None},
                "size": 20, "sha256": "0" * 64, "importedAt": "2026-10-19T00:00:00.000Z", "sourcePath": None}],
}


def confirm_rows(t: Flows) -> list:
    """The confirmation sheet (11 §6.3 step 2; M4 C4.4a, ruling D-58), in a session of its own. Each request is raised by
    a test-only MCP session (a real client, held on its call) whose policy is Assist, so an overwrite asks; this runner's
    own session drives the rest. Each row checks the command the sheet sends and the answer the held call receives."""
    rows = []
    agents: list = []

    async def agent_session(n: int):
        while len(agents) < n:
            d = json.loads(t.discovery.read_text(encoding="utf-8"))
            before = {x.get("sessionId") for x in (await t.cmd("agent.server.status", {})).get("sessions") or []}
            stack = contextlib.AsyncExitStack()
            client = await stack.enter_async_context(
                e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {d['token']}"}, timeout=e2e.httpx2.Timeout(400.0)))
            streams = await stack.enter_async_context(e2e.streamable_http_client(d["endpoint"], http_client=client))
            s2 = await stack.enter_async_context(e2e.ClientSession(streams[0], streams[1]))
            await s2.initialize()
            await asyncio.sleep(0.4)
            after = {x.get("sessionId") for x in (await t.cmd("agent.server.status", {})).get("sessions") or []}
            sid = sorted(after - before)[0]
            # Assist: an overwrite asks (the person's grant, from the page; scope `app`).
            t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command', {json: JSON.stringify({command: 'agent.session.grant',"
                     " params: {sessionId: %s, policy: 'assist'}})}).then(() => true)" % json.dumps(sid))
            await asyncio.sleep(0.4)
            agents.append({"session": s2, "id": sid, "stack": stack})
        return agents[:n]

    async def close_agents():
        # The test-only sessions end with the group (closed in the task that opened them).
        while agents:
            with contextlib.suppress(Exception):
                await agents.pop()["stack"].aclose()

    def ask(a: dict, n: int):
        f = t.work / f"confirm-{n}.jsonl"
        f.write_text("x", encoding="utf-8")
        return asyncio.create_task(e2e.run_cmd(a["session"], "agent.activity.export",
                                               {"path": str(f), "format": "jsonl", "onExists": "overwrite"}))

    async def pending_ids():
        st = await t.cmd("agent.server.status", {})
        return [p.get("confirmationId") for p in st.get("pendingConfirmations") or [] if p.get("inApp")]

    async def raised(ctx, count=1, order=None):
        # `order`: which test-only session makes each request (D-63 item 1: A, B, A; a session's second confirmable
        # call joins the queue at once, while its first is held).
        order = order or list(range(count))
        sessions = await agent_session(max(order) + 1)
        ctx["calls"] = [ask(sessions[a], i) for i, a in enumerate(order)]
        for _ in range(100):
            ids = await pending_ids()
            if len(ids) >= len(order):
                break
            await asyncio.sleep(0.1)
        ctx["ids"] = await pending_ids()
        if not await t.wait_for("confirmSheet.accept"):
            raise RuntimeError("the confirmation sheet did not appear")
        ctx["focus"] = t.cdp.js("document.activeElement && document.activeElement.dataset.ttId")

    async def answers(ctx):
        out = []
        for c in ctx.get("calls") or []:
            env = await asyncio.wait_for(c, timeout=400)
            out.append(env.get("status") if env.get("status") != "error" else (env.get("error") or {}).get("code"))
        return out

    async def nothing(_ctx):
        return None

    def answer_row(name, button, accept):
        async def flow(ctx):
            await raised(ctx)
            t.click(f"confirmSheet.{button}")
            await asyncio.sleep(0.6)
            ctx["answers"] = await answers(ctx)
            return f"a test-only agent's overwrite asked in the sheet (focus not taken: {ctx['focus']}), a real click on {name}"
        rows.append({"name": f"Confirmation sheet › {name}", "spec": "11 §6.3 step 2; D-58", "path": ["agent request", "sheet"],
                     "choice": name, "setup": nothing, "state": nothing, "flow": flow, "wait": 0.5,
                     "expect": lambda ctx: [("agent.confirmation.respond", {"confirmationId": ctx["ids"][0], "accept": accept}, [])],
                     "effect": lambda b, a, ctx: ctx["answers"] == (["ok"] if accept else ["confirmation.declined"])
                     and ctx["focus"] != f"confirmSheet.{button}"})
    answer_row("Accept", "accept", True)
    answer_row("Decline", "decline", False)

    async def enter_flow(ctx):
        await raised(ctx)
        t.cdp.js("document.querySelector('[data-tt-id=\"confirmSheet.decline\"]').focus()")
        d39.key(VK_RETURN)
        await asyncio.sleep(0.6)
        ctx["answers"] = await answers(ctx)
        return "the focus moved into the sheet, then a real Return (Decline is the default)"
    rows.append({"name": "Confirmation sheet › Return (default Decline)", "spec": "D-58; D-49 item 2", "path": ["agent request", "sheet"],
                 "choice": "Return", "setup": nothing, "state": nothing, "flow": enter_flow, "wait": 0.5,
                 "expect": lambda ctx: [("agent.confirmation.respond", {"confirmationId": ctx["ids"][0], "accept": False}, [])],
                 "effect": lambda b, a, ctx: ctx["answers"] == ["confirmation.declined"]})

    async def revision_flow(ctx):
        await raised(ctx)
        # The person edits the document while the request is open (from the page: the user's origin).
        scene = (await t.cmd("doc.object.find", {"type": "scene"})).get("items", [{}])[0].get("id")
        t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command', {json: JSON.stringify({command: 'doc.object.patch', params:"
                 " {targets: [%s], props: {meta: {d44: 1}}}})}).then(() => true)" % json.dumps(scene))
        await asyncio.sleep(1.0)
        ctx["answers"] = await answers(ctx)
        ctx["ended"] = t.cdp.js("document.querySelector('[data-tt-id=\"confirmSheet.ended\"]')?.textContent ?? null")
        return "a request open in the sheet, then the person's own edit of the document"
    rows.append({"name": "Confirmation sheet › document changed", "spec": "11 §6.3 (bound to the revision); D-58", "path": ["agent request", "sheet"],
                 "choice": "revision change", "setup": nothing, "state": nothing, "flow": revision_flow, "wait": 0.5,
                 "expect": lambda ctx: [],
                 "effect": lambda b, a, ctx: ctx["answers"] == ["confirmation.expired"] and "document changed" in (ctx.get("ended") or "")})

    async def elsewhere_flow(ctx):
        await raised(ctx)
        # Answered by another window: its own command entry point, not this sheet's buttons.
        t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command', {json: JSON.stringify({command: 'agent.confirmation.respond',"
                 " params: {confirmationId: %s, accept: true}})}).then(() => true)" % json.dumps(ctx["ids"][0]))
        await asyncio.sleep(0.3)
        # D-59 (C44A-R1S-01): a second, conflicting answer (a window whose sheet had not caught up) is refused and changes
        # nothing; the held call receives the first decision.
        ctx["second"] = t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command', {json: JSON.stringify({command:"
                                 " 'agent.confirmation.respond', params: {confirmationId: %s, accept: false}})})"
                                 # The host answers bytes (an ArrayBuffer): decoded to the envelope's text.
                                 ".then(r => typeof r === 'string' ? r : (r instanceof ArrayBuffer || ArrayBuffer.isView(r))"
                                 " ? new TextDecoder().decode(r) : JSON.stringify(r), e => String(e))"
                                 % json.dumps(ctx["ids"][0]))
        await asyncio.sleep(0.7)
        ctx["answers"] = await answers(ctx)
        ctx["gone"] = not t.exists("confirmSheet.accept")
        return "a request open in the sheet, answered by another window (no click in the sheet), then a conflicting second answer"
    rows.append({"name": "Confirmation sheet › decided in another window", "spec": "D-58", "path": ["agent request", "sheet"],
                 "choice": "elsewhere", "setup": nothing, "state": nothing, "flow": elsewhere_flow, "wait": 0.5,
                 "expect": lambda ctx: [("agent.confirmation.respond", {"confirmationId": ctx["ids"][0], "accept": True}, []),
                                        ("agent.confirmation.respond", {"confirmationId": ctx["ids"][0], "accept": False}, [])],
                 # The second answer is refused with `confirmation.expired` and changes nothing: "already decided"
                 # while the held call has not used the token yet, "no pending confirmation" once it has (the request
                 # is gone); either way the held call keeps the first decision.
                 "effect": lambda b, a, ctx: ctx["answers"] == ["ok"] and ctx["gone"]
                 and "confirmation.expired" in (ctx.get("second") or "")
                 and ("already decided" in (ctx.get("second") or "") or "no pending confirmation" in (ctx.get("second") or ""))})

    async def queued_flow(ctx):
        # D-63 item 1 (C44A-R1-60): sessions A, B, A. A's second request is shown at once (the sheet counts all three),
        # and while the requests are open a plain read of B's session is answered.
        await raised(ctx, 3, [0, 1, 0])
        started = time.monotonic()
        state = await asyncio.wait_for(e2e.run_cmd(agents[1]["session"], "app.state.get", {}), timeout=10)
        ctx["readWhileHeld"] = {"status": state.get("status"), "seconds": round(time.monotonic() - started, 2),
                                "stillOpen": len(await pending_ids())}
        ctx["queue"] = t.cdp.js("document.querySelector('[data-tt-id=\"confirmSheet.queue\"]')?.textContent ?? null")
        ctx["order"] = []
        for button in ("decline", "accept", "decline"):
            shown = t.cdp.js("document.querySelector('[data-tt-id=\"confirmSheet.accept\"]')?.dataset.ttParams ?? null")
            ctx["order"].append(json.loads(shown or "{}").get("confirmationId"))
            t.click(f"confirmSheet.{button}")
            await asyncio.sleep(0.8)
        ctx["answers"] = sorted(await answers(ctx))
        return "two test-only agents' requests A, B, A queued in the sheet, a read of B answered meanwhile: Decline, Accept, Decline in turn"
    rows.append({"name": "Confirmation sheet › three queued", "spec": "D-58 (queued requests)", "path": ["agent request", "sheet"],
                 "choice": "queue", "setup": nothing, "state": nothing, "flow": queued_flow, "wait": 0.5,
                 "expect": lambda ctx: [("agent.confirmation.respond", {"confirmationId": ctx["order"][i], "accept": acc}, [])
                                        for i, acc in enumerate((False, True, False))],
                 "effect": lambda b, a, ctx: "1 of 3" in (ctx.get("queue") or "")
                 and (ctx.get("readWhileHeld") or {}).get("status") == "ok"
                 and (ctx.get("readWhileHeld") or {}).get("stillOpen") == 3
                 and ctx["answers"] == sorted(["confirmation.declined", "ok", "confirmation.declined"])})

    async def expiry_flow(ctx):
        await raised(ctx)
        # The token lives five minutes (11 §6.3); nothing is clicked.
        ctx["answers"] = await answers(ctx)
        await asyncio.sleep(1.5)
        ctx["ended"] = t.cdp.js("document.querySelector('[data-tt-id=\"confirmSheet.ended\"]')?.textContent ?? null")
        return "a request left unanswered for the token's five minutes"
    rows.append({"name": "Confirmation sheet › expiry", "spec": "11 §6.3 (five minutes); D-58", "path": ["agent request", "sheet"],
                 "choice": "expiry", "setup": nothing, "state": nothing, "flow": expiry_flow, "wait": 0.5,
                 "expect": lambda ctx: [],
                 "effect": lambda b, a, ctx: ctx["answers"] == ["confirmation.expired"] and "expired" in (ctx.get("ended") or ""),
                 "cleanup": close_agents})
    return rows


def token_rows(t: Flows) -> list:
    """Preferences › Agent access › Regenerate token (11 §15.1; M4 C4.4a, design note 2 §2; SI-178), in a session of its
    own: Regenerate ends this runner's HTTP session, so it runs last and its effect is read without it, from the
    discovery file and two HTTP requests (the old token refused, the new one accepted, at the same endpoint)."""
    rows = []

    async def setup(ctx):
        await t.open_preferences("agent")

    async def discovery(_ctx):
        d = json.loads(t.discovery.read_text(encoding="utf-8"))
        return {"endpoint": d.get("endpoint"), "token": d.get("token")}

    def accepted(endpoint, token) -> int:
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "d44-probe", "version": "1"}}}).encode()
        req = urllib.request.Request(endpoint, data=body, method="POST", headers={
            "Authorization": f"Bearer {token}", "Content-Type": "application/json",
            "Accept": "application/json, text/event-stream"})
        try:
            with urllib.request.urlopen(req, timeout=10) as r:
                return r.status
        except urllib.error.HTTPError as e:
            return e.code

    async def ask(ctx):
        t.click("agentAccess.regenerate")
        if not await t.wait_for("message.agent.tokenRegenerate"):
            raise RuntimeError("the Regenerate token question did not appear")
        ctx["default"] = t.cdp.js("document.activeElement && document.activeElement.dataset.ttId")

    for choice, button in (("cancel", "Cancel"), ("enter", "Enter (default Cancel)"), ("regenerate", "Regenerate")):
        async def flow(ctx, choice=choice, button=button):
            await ask(ctx)
            if choice == "enter":
                key(VK_RETURN)
            else:
                t.click(f"message.{choice}")
            await asyncio.sleep(2.0)
            return f"Preferences › Agent access › Regenerate token…, then {'a real Enter' if choice == 'enter' else 'a real click on ' + button}"

        def effect(b, a, ctx, choice=choice):
            if choice != "regenerate":
                return a == b and (choice != "enter" or ctx.get("default") == "message.cancel")
            ctx["probe"] = {"oldToken": accepted(a["endpoint"], b["token"]), "newToken": accepted(a["endpoint"], a["token"])}
            return (a["endpoint"] == b["endpoint"] and a["endpoint"] and a["token"] != b["token"]
                    and ctx["probe"]["oldToken"] == 401 and ctx["probe"]["newToken"] == 200)

        rows.append({"name": f"Agent access › Regenerate token › {button}", "spec": f"11 §15.1; {spec_line('agent.token.regenerate')}",
                     "path": ["File", "Preferences", "Agent access", "Regenerate token"], "choice": button, "setup": setup,
                     "state": discovery, "flow": flow, "wait": 2.5,
                     "expect": (lambda c: lambda ctx: [("agent.token.regenerate", {}, [])] if c == "regenerate" else [])(choice),
                     "effect": effect})
    return rows


def undo_back_rows(t: Flows) -> list:
    """The Activity panel's Undo back to before an entry (11 §11; M4 C4.4a, design note 2 §3): an agent's step (this
    runner's call) with a step of the person's after it, so the question names one of yours. Undo moves the history to
    just before the agent's step; Cancel and Enter (its default) leave it."""
    rows = []

    async def setup(ctx):
        await t.cmd("draw.rectangle.add", {"from": {"x": -20, "y": -20}, "to": {"x": 20, "y": 20}})
        log = await t.cmd("agent.activity.list", {"limit": 50})
        mine = [e for e in (log if isinstance(log, list) else []) if e.get("commandId") == "draw.rectangle.add" and e.get("undoStepId")]
        if not mine:
            raise RuntimeError("the agent's step is not in the activity log")
        ctx["seq"] = mine[-1]["seq"]
        step = mine[-1]["undoStepId"]
        # The person's step after it, from the page (user origin).
        t.cdp.js("window.__TAURI_INTERNALS__.invoke('run_command', {json: JSON.stringify({command: 'draw.rectangle.add', params:"
                 " {from: {x: 30, y: 30}, to: {x: 60, y: 60}}})}).then(() => true)")
        await asyncio.sleep(0.8)
        history = await t.cmd("edit.history.get", {"limit": 1000})
        index = next((h.get("index") for h in history.get("entries") or [] if h.get("stepId") == step), None)
        if index is None:
            raise RuntimeError("the agent's step is not in the history")
        ctx["index"] = index
        await t.cmd("window.panel.set", {"panelId": "activity", "visible": True, "focus": True})
        if not await t.wait_for(f"activity.entry.{ctx['seq']}"):
            raise RuntimeError("the Activity panel does not show the agent's entry")

    async def position(_ctx):
        return (await t.cmd("edit.history.get", {"limit": 1})).get("position")

    for choice, button in (("undo", "Undo"), ("cancel", "Cancel"), ("enter", "Enter (default Cancel)")):
        async def flow(ctx, choice=choice, button=button):
            # The agent's entry selected (its aria-pressed reads true), clicked again if a click did not take (a wait
            # for a state, D-45: the list can still be growing with this runner's own calls when the first click lands).
            entry = json.dumps(f"activity.entry.{ctx['seq']}")
            for _ in range(3):
                t.click(f"activity.entry.{ctx['seq']}")
                pressed = False
                for _ in range(20):
                    if t.cdp.js("document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']')?.getAttribute('aria-pressed') === 'true'"
                                % entry):
                        pressed = True
                        break
                    await asyncio.sleep(0.1)
                if pressed:
                    break
            # The item is enabled once the panel has read the history after the entry was selected (a wait for a
            # state, D-45: a click before that hits a disabled item and asks nothing).
            for _ in range(100):
                if t.cdp.js("document.querySelector('[data-tt-id=\"activity.undoBack\"]')?.disabled === false"):
                    break
                await asyncio.sleep(0.1)
            t.click("activity.undoBack")
            if not await t.wait_for("message.agent.undoBack"):
                item = t.cdp.js("(() => { const e = document.querySelector('[data-tt-id=\"activity.undoBack\"]');"
                                " return e ? {disabled: e.disabled, title: e.title, text: e.textContent} : null; })()")
                selected = t.cdp.js("[...document.querySelectorAll('[aria-selected=\"true\"]')].map(e => e.dataset.ttId)")
                raise RuntimeError(f"the Undo back question did not appear: item {item}, selected {selected}, "
                                   f"entry {ctx['seq']}, step index {ctx['index']}")
            ctx["default"] = t.cdp.js("document.activeElement && document.activeElement.dataset.ttId")
            if choice == "enter":
                key(VK_RETURN)
            else:
                t.click(f"message.{choice}")
            return f"Activity panel: the agent's entry, Undo back to before this entry, then {'a real Enter' if choice == 'enter' else 'a real click on ' + button}"

        rows.append({"name": f"Activity › Undo back › {button}", "spec": f"11 §11; {spec_line('edit.history.move')}",
                     "path": ["Window", "Activity", "Undo back to before this entry"], "choice": button, "setup": setup,
                     "state": position, "flow": flow,
                     "expect": (lambda c: lambda ctx: [("edit.history.move", {"position": ctx["index"] - 1}, [])] if c == "undo" else [])(choice),
                     "effect": (lambda c: lambda b, a, ctx: (a == ctx["index"] - 1) if c == "undo"
                                else (a == b and (c != "enter" or ctx.get("default") == "message.cancel")))(choice)})
    return rows


def toolbar_rows(t: Flows) -> list:
    """The toolbar customisation sheet (01 §5.7; M4 C4.4a, design note 1 §7): Apply sends the staged items, Cancel
    nothing, Restore Default the reset at once."""
    rows = []

    async def items(_ctx):
        listed = await t.cmd("window.toolbar.list", {})
        listed = listed if isinstance(listed, list) else []
        return next((x.get("items") for x in listed if x.get("toolbarId") == "main"), [])

    async def reset(_ctx=None):
        await t.cmd("window.toolbar.items.reset", {})
        await asyncio.sleep(0.4)

    async def open_sheet():
        choose_menu(["Window", "Customize Toolbar"])
        if not await t.wait_for("toolbarSheet.item.0"):
            raise RuntimeError("the toolbar sheet did not open")
        await asyncio.sleep(0.4)

    for choice, button in (("submit", "Apply"), ("cancel", "Cancel")):
        async def flow(ctx, choice=choice, button=button):
            ctx["before"] = await items(ctx)
            await open_sheet()
            t.click("toolbarSheet.item.0")
            t.click("toolbarSheet.remove")
            await asyncio.sleep(0.3)
            t.click(f"toolbarSheet.{choice}")
            await asyncio.sleep(1.0)
            return f"real native menu Window › Customize Toolbar, the first item removed in the sheet, a real click on {button}"

        rows.append({"name": f"Customize Toolbar › {button}", "spec": f"01 §5.7 lines 445-464; {spec_line('window.toolbar.items.set')}",
                     "path": ["Window", "Customize Toolbar"], "choice": button, "setup": reset, "state": items, "flow": flow,
                     "expect": (lambda c: lambda ctx: [("window.toolbar.items.set", {"toolbarId": "main", "items": ctx["before"][1:]}, [])]
                                if c == "submit" else [])(choice),
                     "effect": (lambda c: lambda b, a, ctx: a == (b[1:] if c == "submit" else b))(choice),
                     "cleanup": reset})

    async def customised(ctx):
        await t.cmd("window.toolbar.items.set", {"toolbarId": "main", "items": ["project.save", "separator", "edit.undo"]})
        await asyncio.sleep(0.4)

    async def restore_flow(ctx):
        await open_sheet()
        t.click("toolbarSheet.restore")
        await asyncio.sleep(1.0)
        return "real native menu Window › Customize Toolbar, a real click on Restore Default"

    rows.append({"name": "Customize Toolbar › Restore Default", "spec": f"01 §5.7; {spec_line('window.toolbar.items.reset')}",
                 "path": ["Window", "Customize Toolbar"], "choice": "Restore Default", "setup": customised, "state": items,
                 "flow": restore_flow, "expect": lambda ctx: [("window.toolbar.items.reset", {}, [])],
                 "effect": lambda b, a, ctx: len(a) > 3 and a != b, "cleanup": reset})
    return rows


def form_rows(t: Flows) -> list:
    fx = t.fx
    rows = []

    async def props_state(_ctx):
        props = await t.cmd("project.properties.get", {})
        presets = await t.cmd("project.presets.list", {})
        listed = presets if isinstance(presets, list) else presets.get("presets") or []
        default = next((x for x in listed if x.get("kind") == "default" or x.get("id") == "default"), {})
        return {"width": props.get("cameraWidth"), "defaultWidth": default.get("width")}

    async def props_restore():
        await t.cmd("project.properties.set", {"cameraWidth": 1920, "setAsDefault": True})

    for choice, button in (("submit", "OK (set as default)"), ("cancel", "Cancel")):
        async def flow(ctx, choice=choice, button=button):
            choose_menu(["File", "Project Properties"])
            if not await t.wait_for("projectProperties.width"):
                raise RuntimeError("the Project Properties dialog did not open")
            t.click("projectProperties.width")
            select_field()
            type_text("1280")
            t.click("projectProperties.setAsDefault")
            await asyncio.sleep(0.4)
            t.click(f"projectProperties.{choice}")
            await asyncio.sleep(1.2)
            return f"real native menu File › Project Properties, width typed, Set as default checked, a real click on {button}"

        rows.append({"name": f"Project Properties › {button}", "spec": f"01 §3.8; {spec_line('project.properties.set')}",
                     "path": ["File", "Project Properties"], "choice": button, "setup": t.prepare_clean, "state": props_state,
                     "flow": flow,
                     "expect": (lambda c: lambda ctx: [("project.properties.set", {"cameraWidth": 1280, "setAsDefault": True}, [])]
                                if c == "submit" else [])(choice),
                     "effect": (lambda c: lambda b, a, ctx: (a["width"] == 1280 and a["defaultWidth"] == 1280) if c == "submit"
                                else a == b)(choice),
                     "cleanup": props_restore})

    async def project_of(_ctx):
        return await t.project_state()

    for choice, button in (("create", "Create"), ("cancel", "Cancel")):
        async def flow(ctx, choice=choice, button=button):
            choose_menu(["File", "New Project"])
            if not await t.wait_for("newProject.name"):
                raise RuntimeError("the New Project dialog did not open")
            t.click("newProject.name")
            select_field()
            type_text("FlowFresh")
            await asyncio.sleep(0.3)
            t.click(f"newProject.{choice}")
            await asyncio.sleep(2.0)
            return f"real native menu File › New Project (no unsaved changes), a typed name, a real click on {button}"

        rows.append({"name": f"New Project › {button}", "spec": f"01 §3.1; {spec_line('project.create')}",
                     "path": ["File", "New Project"], "choice": button, "setup": t.prepare_clean, "state": project_of,
                     "flow": flow,
                     "expect": (lambda c: lambda ctx: [("project.create", {"name": "FlowFresh"}, ["onUnsaved"])]
                                if c == "create" else [])(choice),
                     "effect": (lambda c: lambda b, a, ctx: (a["path"] is None and a["name"] == "FlowFresh") if c == "create"
                                else same_path(a["path"], fx["flow"]))(choice)})

    for choice, button in (("open", "Open"), ("cancel", "Cancel")):
        async def flow(ctx, choice=choice, button=button):
            choose_menu(["File", "Open Backup"])
            if not await t.wait_for("openBackup.open.0"):
                raise RuntimeError("the Open Backup dialog listed no backup")
            ctx["backupId"] = json.loads(t.cdp.js("document.querySelector('[data-tt-id=\"openBackup.open.0\"]').dataset.ttParams")).get("backupId")
            t.click("openBackup.open.0" if choice == "open" else "openBackup.cancel")
            await asyncio.sleep(2.5)
            return f"real native menu File › Open Backup (no unsaved changes), a real click on {button}"

        rows.append({"name": f"backup chooser › {button}", "spec": f"01 §3.7; {spec_line('project.backup.open')}",
                     "path": ["File", "Open Backup"], "choice": button, "setup": t.prepare_clean, "state": project_of,
                     "flow": flow,
                     "expect": (lambda c: lambda ctx: [("project.backup.open", {"backupId": ctx.get("backupId")}, ["onUnsaved"])]
                                if c == "open" else [])(choice),
                     "effect": (lambda c: lambda b, a, ctx: (a["open"] and a["path"] is None) if c == "open"
                                else same_path(a["path"], fx["flow"]) and a["flow"] == b["flow"])(choice)})

    async def issues_setup(ctx):
        await t.prepare_clean()
        await t.cmd("doc.import.json", {"source": UNUSED_ASSET, "idPolicy": "preserve"})
        await asyncio.sleep(0.6)

    async def issues(_ctx):
        v = await t.cmd("doc.validate", {})
        return sorted({i.get("code") for i in v.get("issues") or []})

    for choice, button in (("repair", "Repair"), ("cancel", "Close")):
        async def flow(ctx, choice=choice, button=button):
            choose_menu(["File", "Check Project"])
            if not await t.wait_for("checkProject.issues"):
                raise RuntimeError("the Check Project dialog did not list its issues")
            await asyncio.sleep(0.6)
            t.click(f"checkProject.{choice}")
            await asyncio.sleep(1.5)
            if choice == "repair":
                t.click("checkProject.cancel")
                await asyncio.sleep(0.6)
            return f"real native menu File › Check Project (an unused asset reported), a real click on {button}"

        rows.append({"name": f"Check Project › {button}", "spec": f"01 §6.7; {spec_line('doc.repair')}",
                     "path": ["File", "Check Project"], "choice": button, "setup": issues_setup, "state": issues,
                     "flow": flow,
                     "expect": (lambda c: lambda ctx: [("doc.repair", {"all": True}, [])] if c == "repair" else [])(choice),
                     "effect": (lambda c: lambda b, a, ctx: ("unusedAssets" in b and "unusedAssets" not in a) if c == "repair"
                                else a == b and "unusedAssets" in b)(choice)})
    return rows


def dismiss_rows(t: Flows) -> list:
    """One row per dismiss-only box: opening it may read, dismissing it sends no mutating command."""
    fx = t.fx
    rows = []

    async def nothing_state(_ctx):
        return await t.project_state()

    def box(name, path, ident, cancel, spec):
        async def flow(ctx):
            choose_menu(path)
            if not await t.wait_for(ident):
                raise RuntimeError(f"{name} did not open")
            await asyncio.sleep(0.5)
            t.clear()
            t.click(cancel)
            await asyncio.sleep(0.8)
            return f"real native menu {' › '.join(path)}, a real click on its close button"
        rows.append({"name": f"{name} › Close", "spec": spec, "path": path, "choice": "Close", "setup": t.prepare_clean,
                     "state": nothing_state, "flow": flow, "expect": lambda ctx: [],
                     "effect": lambda b, a, ctx: a == b})

    box("About", ["Help", "About Toontown"], "about.cancel", "about.cancel", "01 §12.2 (About)")
    box("Project Info", ["File", "Project Info"], "projectInfo.cancel", "projectInfo.cancel", "01 §3.8 (Project Info)")
    box("Help", ["Help", "Documentation"], "help.text", "help.cancel", "01 §9.3 (Help)")

    junk = t.work / "not-a-project.animpkg"

    async def error_flow(ctx):
        junk.write_bytes(b"this is not a project package")
        choose_menu(["File", "Open"])
        await asyncio.sleep(1.8)
        type_text(str(junk))
        key(VK_RETURN)
        await asyncio.sleep(2.5)
        ident = t.cdp.js("document.querySelector('[role=alertdialog]')?.dataset.ttId")
        if not ident:
            raise RuntimeError("no error box after opening a file that is not a project")
        ctx["box"] = ident
        t.clear()
        t.click("message.ok")
        await asyncio.sleep(0.8)
        return f"File › Open on a file that is not a project, the error box {ident}, a real click on OK"

    rows.append({"name": "error box › OK", "spec": "01 §9.5 (messages)", "path": ["File", "Open"], "choice": "OK",
                 "setup": t.prepare_clean, "state": nothing_state, "flow": error_flow, "expect": lambda ctx: [],
                 # 01 §3.2: the current project is closed before the new one is loaded; on an error the new one is not
                 # opened, so no project is open. The project's file is unchanged.
                 "effect": lambda b, a, ctx: b["open"] and not a["open"] and a["flow"] == b["flow"]})

    async def picker_flow(ctx):
        choose_menu(["File", "Open"])
        await asyncio.sleep(1.8)
        key(VK_ESCAPE)
        await asyncio.sleep(1.0)
        return "File › Open, the native picker cancelled with a real Escape"

    rows.append({"name": "native picker › Cancel", "spec": "01 §3.2 (Open)", "path": ["File", "Open"], "choice": "Cancel",
                 "setup": t.prepare_clean, "state": nothing_state, "flow": picker_flow, "expect": lambda ctx: [],
                 "effect": lambda b, a, ctx: a == b})
    return rows


def jobs_row(t: Flows) -> dict:
    async def flow(ctx):
        env = await e2e.run_cmd(t.s, "render.range.run", {"from": 1, "to": 2400, "scale": 2, "directory": str(t.work / "cancel-frames"),
                                                          "createDirs": True, "onConflict": "overwrite"})
        ctx["jobId"] = (env.get("result") or {}).get("jobId")
        if not ctx["jobId"]:
            raise RuntimeError(f"no job: {env}")
        ident = f"jobsWindow.cancel.{ctx['jobId']}"
        if not await t.wait_for(ident):
            raise RuntimeError("the progress window did not offer Cancel")
        t.clear()
        t.click(ident)
        await e2e.call(t.s, "job_wait", {"jobId": ctx["jobId"], "timeoutMs": 120000})
        return "an agent session's render job; the progress window opened by itself; a real click on its Cancel"

    async def state(ctx):
        if not ctx.get("jobId"):
            return None
        listed = (await e2e.run_cmd(t.s, "job.list", {"jobId": ctx["jobId"]})).get("result") or []
        return (listed[0] if listed else {}).get("state")

    return {"name": "progress window › Cancel", "spec": f"01 §9.5 lines 925-927; {spec_line('job.cancel')}",
            "path": ["status bar", "progress window"], "choice": "Cancel", "state": state, "flow": flow,
            "expect": lambda ctx: [("job.cancel", {"jobId": ctx["jobId"]}, [])], "effect": lambda b, a, ctx: a == "cancelled", "wait": 0.5}


# ----- sessions --------------------------------------------------------------------------------------------------------

def timeline_rows(t: Flows) -> list:
    """The Timeline's dialogs (M5 C5.1b; reviews/M5-design-timeline-view.md section 7), each choice of each: opened from
    the Element and Play menus (real native menu clicks), from the Timeline's row, cell and ruler context menus (a real
    secondary click, then a real click on the item), from Edit with the Timeline focused, and from the Timeline's
    toolbar; a typed value or a toggled option, then a real click on OK or Cancel. OK sends exactly the command the
    dialog completes; Cancel sends nothing and changes nothing."""
    rows = []
    cmd = t.cmd

    async def find(name):
        f = await cmd("doc.object.find", {"type": "element", "name": name})
        items = f.get("items") or f.get("matches") or []
        return items[0].get("id") if items else None

    async def base(ctx):
        await t.prepare_dirty()
        ctx["bob"] = await find("Bob")
        peg = await find("FlowPeg")
        if peg is None:
            peg = ((await cmd("element.create", {"type": "peg", "name": "FlowPeg"})).get("elementIds") or [None])[0]
        ctx["peg"] = peg
        await cmd("app.context.set", {"elementId": ctx["bob"], "frame": 1})
        await cmd("window.panel.set", {"panelId": "timeline", "visible": True, "focus": True})
        if not await t.wait_for(f"timeline.row.{ctx['bob']}"):
            raise RuntimeError("the Timeline does not show Bob's row")
        await asyncio.sleep(0.6)

    def menu(path):
        async def open_(ctx):
            choose_menu(path)
            return "real native menu " + " › ".join(path)
        return open_

    def right_click_at(x, y):
        h = d39.front()
        origin = d39.wintypes.POINT(0, 0)
        d39.u32.ClientToScreen(h, d39.ctypes.byref(origin))
        dpr = t.cdp.js("window.devicePixelRatio || 1")
        d39.right_click_screen(origin.x + x * dpr, origin.y + y * dpr)

    def context(target, menu_id, item_id):
        """A real secondary click on `target(ctx)` (a CSS pixel point in the page), then a real click on the item."""
        async def open_(ctx):
            x, y = target(ctx)
            right_click_at(x, y)
            ident = f"{menu_id}.{item_id}"
            if not await t.wait_for(ident, 5):
                raise RuntimeError(f"the context menu has no {ident}")
            await asyncio.sleep(0.3)
            t.click(ident)
            return f"a real secondary click in the Timeline, a real click on {item_id}"
        return open_

    def centre_of(ident_of):
        def at(ctx):
            ident = ident_of(ctx)
            r = t.cdp.js("(() => { const e = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']');"
                         " if (!e) return null; e.scrollIntoView({block: 'center'}); const b = e.getBoundingClientRect();"
                         " return [b.x + Math.min(60, b.width / 2), b.y + b.height / 2]; })()" % json.dumps(ident))
            if not r:
                raise RuntimeError(f"absent: {ident}")
            return r
        return at

    def track_of(key):
        """A point in the tracks on the row of `ctx[key]`, a few frames in."""
        def at(ctx):
            r = t.cdp.js("(() => { const row = document.querySelector('[data-tt-id=' + JSON.stringify(%s) + ']');"
                         " const tracks = document.querySelector('[data-tt-id=\"timeline.tracks\"]');"
                         " if (!row || !tracks) return null; const a = row.getBoundingClientRect();"
                         " const b = tracks.getBoundingClientRect(); return [b.x + 40, a.y + a.height / 2]; })()"
                         % json.dumps(f"timeline.row.{ctx[key]}"))
            if not r:
                raise RuntimeError("the Timeline's tracks or row are absent")
            return r
        return at

    def row(name, spec, path, open_, dialog, set_value, expect, state, effect, setup=base, cleanup=None):
        """One dialog, both choices: `set_value(ctx)` types or toggles in the dialog; `expect(ctx)` the commands OK sends."""
        for choice, button in (("submit", "OK"), ("cancel", "Cancel")):
            async def flow(ctx, choice=choice, button=button):
                how = await open_(ctx)
                if not await t.wait_for(dialog, 8):
                    raise RuntimeError(f"the dialog {dialog} did not open")
                await asyncio.sleep(0.4)
                await set_value(ctx)
                await asyncio.sleep(0.4)
                t.click(f"{dialog}.{choice}")
                await asyncio.sleep(1.0)
                return f"{how}, a real click on {button}"

            rows.append({"name": f"{name} › {button}", "spec": spec, "path": path, "choice": button, "setup": setup,
                         "state": state, "flow": flow,
                         "expect": (lambda c: (lambda ctx: expect(ctx)) if c == "submit" else (lambda ctx: []))(choice),
                         "effect": (lambda c: lambda b, a, ctx: effect(b, a, ctx, c == "submit"))(choice),
                         "cleanup": cleanup})

    def typed(field, value):
        async def set_(ctx):
            t.click(field)
            select_field()
            type_text(value(ctx) if callable(value) else value)
        return set_

    def toggled(box):
        async def set_(ctx):
            if not await t.wait_for(box, 8):
                raise RuntimeError(f"the options did not load ({box})")
            t.click(box)
        return set_

    async def el(ctx, key="bob"):
        return await cmd("element.get", {"elementId": ctx[key]})

    def changed_to(get, want):
        return lambda b, a, ctx, ok: (get(a) == want) == ok and (ok or get(a) == get(b))

    # ----- the Element menu ---------------------------------------------------------------------------------------
    async def names(ctx):
        return sorted(e.get("name") for e in (await cmd("element.list", {})).get("elements", []))

    async def drop_flow_el():
        while (fid := await find("FlowEl")) is not None:
            await cmd("element.delete", {"elementIds": [fid]})

    row("New Elements", f"04 §3.2; {spec_line('element.create')}", ["Element", "New Elements"],
        menu(["Element", "New Elements"]), "timelineNewElements", typed("timelineNewElements.name", "FlowEl"),
        lambda ctx: [("element.create", {"type": "drawing", "count": 1, "name": "FlowEl"}, [])],
        names, lambda b, a, ctx, ok: ("FlowEl" in a) == ok, cleanup=drop_flow_el)

    async def name_of(ctx):
        return (await el(ctx)).get("name")

    async def rename_back():
        bob = await find("FlowBob")
        if bob:
            await cmd("element.rename", {"elementId": bob, "name": "Bob"})

    # The Timeline row context's Rename opens the in-place editor (ruling E-50 item 1): its D-39 row.
    for label, path, open_ in (
        ("Rename Element", ["Element", "Rename"], menu(["Element", "Rename"])),
    ):
        row(label, f"04 §9.6; {spec_line('element.rename')}", path, open_, "timelineRename",
            typed("timelineRename.value", "FlowBob"),
            lambda ctx: [("element.rename", {"elementId": ctx["bob"], "name": "FlowBob"}, [])],
            name_of, changed_to(lambda v: v, "FlowBob"), cleanup=rename_back)

    async def note_of(ctx):
        return (await el(ctx)).get("note") or ""

    async def clear_note():
        bob = await find("Bob")
        await cmd("element.note.set", {"elementId": bob, "text": ""})

    for label, path, open_ in (
        ("Element Note", ["Element", "Element Note"], menu(["Element", "Element Note"])),
        ("Element Note (Timeline row)", ["Timeline", "row", "Element Note"],
         context(centre_of(lambda ctx: f"timeline.row.{ctx['bob']}"), "timeline.rowMenu", "timeline.row.note")),
    ):
        row(label, f"04 §9.7; {spec_line('element.note.set')}", path, open_, "timelineNote",
            typed("timelineNote.value", "flow note"),
            lambda ctx: [("element.note.set", {"elementId": ctx["bob"], "text": "flow note"}, [])],
            note_of, changed_to(lambda v: v, "flow note"), cleanup=clear_note)

    # C51B-R1S-05 (04 lines 405-408): with a note, the action is Update Element Note and the editor opens with the note;
    # OK without typing keeps it (it deleted it before), Cancel changes nothing.
    async def with_note(ctx):
        await base(ctx)
        await cmd("element.note.set", {"elementId": ctx["bob"], "text": "kept note"})

    async def prefilled(ctx):
        shown = t.cdp.js("document.querySelector('[data-tt-id=\"timelineNote.value\"]')?.value ?? null")
        if shown != "kept note":
            raise RuntimeError(f"the editor did not open with the note: {shown!r}")

    for label, path, open_ in (
        ("Update Element Note", ["Element", "Element Note"], menu(["Element", "Element Note"])),
        ("Update Element Note (Timeline row)", ["Timeline", "row", "Update Element Note"],
         context(centre_of(lambda ctx: f"timeline.row.{ctx['bob']}"), "timeline.rowMenu", "timeline.row.note")),
    ):
        row(label, f"04 §7.1 lines 405-408; {spec_line('element.note.set')}", path, open_, "timelineNote", prefilled,
            lambda ctx: [("element.note.set", {"elementId": ctx["bob"], "text": "kept note"}, [])],
            note_of, lambda b, a, ctx, ok: a == "kept note" and b == "kept note", setup=with_note, cleanup=clear_note)

    async def colours_of(ctx):
        e = await el(ctx)
        return {"marker": e.get("markerColour"), "background": e.get("backgroundColour")}

    async def default_colours():
        bob = await find("Bob")
        await cmd("element.colour.set", {"elementIds": [bob], "marker": "", "background": ""})

    async def select_bob(ctx):
        await base(ctx)
        await cmd("element.select", {"elementIds": [ctx["bob"]]})

    row("Element Colours", f"04 §9.6; {spec_line('element.colour.set')}", ["Element", "Element Colours"],
        menu(["Element", "Element Colours"]), "timelineColours", typed("timelineColours.marker", "#FF0000"),
        lambda ctx: [("element.colour.set", {"marker": "#FF0000"}, ["background"])],
        colours_of, lambda b, a, ctx, ok: (str(a["marker"] or "").upper().startswith("#FF0000")) == ok,
        setup=select_bob, cleanup=default_colours)
    row("Marker Colour (Timeline row)", f"04 §9.6; {spec_line('element.colour.set')}", ["Timeline", "row", "Marker Colour"],
        context(centre_of(lambda ctx: f"timeline.row.{ctx['bob']}"), "timeline.rowMenu", "timeline.row.markerColour"),
        "timelineMarkerColour", typed("timelineMarkerColour.value", "#00FF00"),
        lambda ctx: [("element.colour.set", {"elementIds": [ctx["bob"]], "marker": "#00FF00"}, ["background"])],
        colours_of, lambda b, a, ctx, ok: (str(a["marker"] or "").upper().startswith("#00FF00")) == ok,
        cleanup=default_colours)
    row("Background Colour (Timeline row)", f"04 §9.6; {spec_line('element.colour.set')}",
        ["Timeline", "row", "Background Colour"],
        context(centre_of(lambda ctx: f"timeline.row.{ctx['bob']}"), "timeline.rowMenu", "timeline.row.backgroundColour"),
        "timelineBackgroundColour", typed("timelineBackgroundColour.value", "#0000FF"),
        lambda ctx: [("element.colour.set", {"elementIds": [ctx["bob"]], "background": "#0000FF"}, ["marker"])],
        colours_of, lambda b, a, ctx, ok: (str(a["background"] or "").upper().startswith("#0000FF")) == ok,
        cleanup=default_colours)

    # ----- the row's timing (a peg's bar) -------------------------------------------------------------------------
    async def timing_of(ctx):
        e = await el(ctx, "peg")
        return {"start": e.get("start"), "duration": e.get("duration"), "loops": e.get("loops")}

    async def peg_timing(ctx):
        await base(ctx)
        await cmd("element.timing.set", {"elementIds": [ctx["peg"]], "start": 1, "duration": 24, "loops": 1})

    async def reset_peg():
        peg = await find("FlowPeg")
        if peg:
            await cmd("element.timing.set", {"elementIds": [peg], "start": 1, "duration": 24, "loops": 1})

    for label, item, dialog, value, key, want in (
        ("Change Start Frame", "timeline.row.changeStart", "timelineStart", "5", "start", 5),
        ("Change Duration", "timeline.row.changeDuration", "timelineDuration", "30", "duration", 30),
        ("Change Loops", "timeline.row.changeLoops", "timelineLoops", "2", "loops", 2),
    ):
        row(label, f"04 §10.2; {spec_line('element.timing.set')}", ["Timeline", "row", label],
            context(centre_of(lambda ctx: f"timeline.row.{ctx['peg']}"), "timeline.rowMenu", item), dialog,
            typed(f"{dialog}.value", value),
            (lambda k, w: lambda ctx: [("element.timing.set", {"elementIds": [ctx["peg"]], k: w}, [])])(key, want),
            timing_of, (lambda k, w: lambda b, a, ctx, ok: (a[k] == w) == ok and (ok or a == b))(key, want),
            setup=peg_timing, cleanup=reset_peg)

    async def last_exposed(ctx):
        sheet = await cmd("xsheet.exposures.get", {"elementIds": [ctx["bob"]]})
        runs = [r for c in sheet.get("columns", []) for r in c.get("exposures", [])]
        return max((r.get("to", 0) for r in runs), default=0)

    async def bob_under_peg(ctx):
        await peg_timing(ctx)
        await cmd("element.parent.set", {"elementIds": [ctx["bob"]], "parentId": ctx["peg"]})
        await asyncio.sleep(0.6)

    async def bob_back():
        with contextlib.suppress(Exception):
            await cmd("edit.undo", {})
        bob = await find("Bob")
        with contextlib.suppress(Exception):
            await cmd("element.parent.remove", {"elementIds": [bob]})

    row("Extend Children Exposure", f"04 §13.8; {spec_line('element.childExposure.apply')}",
        ["Timeline", "cells", "Extend Children Exposure"],
        context(track_of("peg"), "timeline.cellMenu", "timeline.cell.extendChildren"), "timelineExtendChildren",
        typed("timelineExtendChildren.value", "30"),
        lambda ctx: [("element.childExposure.apply", {"pegId": ctx["peg"], "toFrame": 30}, [])],
        last_exposed, lambda b, a, ctx, ok: (a >= 30) == ok and (ok or a == b), setup=bob_under_peg, cleanup=bob_back)

    # ----- the current frame and the playback range ---------------------------------------------------------------
    async def frame_of(_ctx):
        return (await cmd("app.state.get", {})).get("frame")

    async def frame_one():
        await cmd("app.frame.set", {"frame": 1})

    row("Change Current Frame", f"09 §2.2; {spec_line('app.frame.set')}", ["Play", "Change Current Frame"],
        menu(["Play", "Change Current Frame"]), "timelineCurrentFrame", typed("timelineCurrentFrame.value", "6"),
        lambda ctx: [("app.frame.set", {"frame": 6}, [])], frame_of, changed_to(lambda v: v, 6), cleanup=frame_one)
    row("Set Current Frame (Timeline ruler)", f"04 §11.9; {spec_line('timeline.currentFrame.set')}",
        ["Timeline", "ruler", "Set Current Frame"],
        context(centre_of(lambda ctx: "timeline.ruler"), "timeline.rulerMenu", "timeline.ruler.currentFrame"),
        "timelineCurrentFrame", typed("timelineCurrentFrame.value", "7"),
        lambda ctx: [("timeline.currentFrame.set", {"frame": 7}, [])], frame_of, changed_to(lambda v: v, 7),
        cleanup=frame_one)

    async def range_of(_ctx):
        r = await cmd("playback.range.get", {})
        return {"start": r.get("start"), "end": r.get("end")}

    async def auto_range():
        await cmd("playback.range.mode.set", {"mode": "auto"})

    for label, path, open_, dialog, command, key, value in (
        ("Change Playback Start Frame", ["Play", "Playback Range", "Change Playback Start Frame"],
         menu(["Play", "Playback Range", "Change Playback Start Frame"]), "timelineRangeStart", "playback.range.set",
         "start", 3),
        ("Change Playback End Frame", ["Play", "Playback Range", "Change Playback End Frame"],
         menu(["Play", "Playback Range", "Change Playback End Frame"]), "timelineRangeEnd", "playback.range.set", "end", 10),
        ("Set Playback Start Frame (Timeline ruler)", ["Timeline", "ruler", "Set Playback Start Frame"],
         context(centre_of(lambda ctx: "timeline.ruler"), "timeline.rulerMenu", "timeline.ruler.rangeStart"),
         "timelineRangeStart", "timeline.playbackRange.set", "start", 4),
        ("Set Playback End Frame (Timeline ruler)", ["Timeline", "ruler", "Set Playback End Frame"],
         context(centre_of(lambda ctx: "timeline.ruler"), "timeline.rulerMenu", "timeline.ruler.rangeEnd"),
         "timelineRangeEnd", "timeline.playbackRange.set", "end", 12),
    ):
        row(label, f"09 §2.6; {spec_line(command)}", path, open_, dialog, typed(f"{dialog}.value", str(value)),
            (lambda c, k, v: lambda ctx: [(c, {k: v}, [])])(command, key, value),
            range_of, (lambda k, v: lambda b, a, ctx, ok: (a[k] == v) == ok and (ok or a == b))(key, value),
            cleanup=auto_range)

    # ----- the transfer options (04 §11.14) ------------------------------------------------------------------------
    def options_of(operation):
        async def get(_ctx):
            return (await cmd("timeline.transferOptions.get", {"operation": operation})).get("options") or {}
        return get

    def restore(operation):
        async def back():
            # The defaults of the 04 family (SI-201): the boundary-keys and auto-extend options off.
            o = (await cmd("timeline.transferOptions.get", {"operation": operation})).get("options") or {}
            o.update({"boundaryKeys": False, "autoExtend": False})
            await cmd("timeline.transferOptions.set", {"operation": operation, "options": o})
        return back

    async def cells_copied(ctx):
        await base(ctx)
        await cmd("cell.select", {"elementIds": [ctx["bob"]], "from": 1, "to": 2})
        await cmd("edit.copy", {"context": "timeline"})
        await cmd("cell.select", {"elementIds": [ctx["bob"]], "from": 30, "to": 30})
        await cmd("window.panel.set", {"panelId": "timeline", "visible": True, "focus": True})

    async def cells_selected(ctx):
        await base(ctx)
        await cmd("cell.select", {"elementIds": [ctx["bob"]], "from": 20, "to": 21})
        await cmd("window.panel.set", {"panelId": "timeline", "visible": True, "focus": True})

    def undo_then(back):
        async def run_():
            with contextlib.suppress(Exception):
                await back()
        return run_

    async def undo_cut_and_restore():
        with contextlib.suppress(Exception):
            await cmd("edit.undo", {})
        await restore("cut")()

    toggle_effect = (lambda key: lambda b, a, ctx, ok: (a.get(key) != b.get(key)) == ok)
    row("Paste Special", f"04 §11.14; {spec_line('edit.pasteSpecial')}", ["Edit", "Paste Special"],
        menu(["Edit", "Paste Special"]), "timelineTransfer", toggled("timelineTransfer.boundaryKeys"),
        lambda ctx: [("timeline.transferOptions.set", {"operation": "paste"}, []), ("edit.pasteSpecial", {}, ["options"])],
        options_of("paste"), toggle_effect("boundaryKeys"), setup=cells_copied,
        cleanup=undo_then(restore("paste")))
    row("Cut Special", f"04 §11.14; {spec_line('edit.cutSpecial')}", ["Edit", "Cut Special"],
        menu(["Edit", "Cut Special"]), "timelineTransfer", toggled("timelineTransfer.boundaryKeys"),
        lambda ctx: [("timeline.transferOptions.set", {"operation": "cut"}, []), ("edit.cutSpecial", {}, ["options"])],
        options_of("cut"), toggle_effect("boundaryKeys"), setup=cells_selected, cleanup=undo_cut_and_restore)

    async def click_drag_options(_ctx):
        t.click("timeline.dragOptions")
        return "a real click on the Timeline toolbar's Drag Options"

    row("Drag and Drop Options", f"04 §11.14; {spec_line('timeline.transferOptions.set')}",
        ["Timeline", "toolbar", "Drag Options"], click_drag_options, "timelineTransfer",
        toggled("timelineTransfer.autoExtend"),
        lambda ctx: [("timeline.transferOptions.set", {"operation": "dragDrop"}, [])],
        options_of("dragDrop"), toggle_effect("autoExtend"), cleanup=restore("dragDrop"))
    return rows


async def fixture(t: Flows) -> None:
    cmd = t.cmd
    flow, other = t.work / "Flow.animpkg", t.work / "Other.animpkg"
    t.fx.update({"flow": flow, "other": other, "otherPristine": t.work / "Other-pristine.animpkg"})
    await cmd("prefs.set", {"values": {"files.backups.count": 5}})
    # Two projects of their own names (Open Recent lists each by its project's name): Other, then Flow.
    for name, path in (("Other", other), ("Flow", flow)):
        await cmd("project.create", {"name": name, "onUnsaved": "fail"})
        bob = ((await cmd("element.create", {"type": "drawing", "name": "Bob"})).get("elementIds") or [None])[0]
        await cmd("element.drawing.create", {"elementId": bob, "name": "1", "exposeFrom": 1, "exposeTo": 24})
        await cmd("app.context.set", {"elementId": bob, "frame": 1})
        await cmd("draw.rectangle.add", {"from": {"x": -50, "y": -50}, "to": {"x": 50, "y": 50}})
        await cmd("project.saveAs", {"path": str(path), "onExists": "overwrite"})
    shutil.copyfile(other, t.fx["otherPristine"])
    # Backups: each save over the file keeps the one before (files.backups.count).
    for _ in range(2):
        await asyncio.sleep(1.1)
        await t.dirty()
        await cmd("project.save", {})
    await cmd("workspace.select", {"name": "Default"})
    await asyncio.sleep(1.0)
    t.cdp = d39.Cdp(d39.CDP_PORT)
    t.cdp.record()


async def session(app: str, run, out: Path, work: Path, group: str, only: set, sink: list) -> None:
    """One application session; its rows are appended to `sink` as they run (a Quit row ends the application, and the
    MCP client's closing may then fail without losing them)."""
    user = work / f"user-{group}"
    (user / "Settings").mkdir(parents=True)
    seed = {"agent.policy.defaults": {"policy": "autonomous", "maxJobs": 2, "maxBatchSteps": 1000, "maxInlineImagePx": 2048,
                                      "maxInlineBytes": 52428800, "askInApp": True, "letAgentMoveView": True, "highlightAgentChanges": True}}
    (user / "Settings" / "preferences.json").write_text(json.dumps({"format": "toontown-settings", "kind": "preferences", "formatVersion": "1.0", "data": seed}), encoding="utf-8")
    discovery = work / f"discovery-{group}.json"
    os.environ["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = f"--remote-debugging-port={d39.CDP_PORT}"
    os.environ["WEBVIEW2_USER_DATA_FOLDER"] = str(work / f"webview2-{group}")
    sub = work / group
    sub.mkdir()
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
            await e2e.gui_tree(s, run, lambda r: e2e.by_id(r, "startScreen") is not None, f"the GUI is up ({group})")
            t = Flows(s, run, out, sub, proc)
            t.discovery = discovery
            t.rows = sink
            await fixture(t)
            if group == "main":
                rows = [*unsaved_rows(t, None), *save_as_rows(t), *keymap_rows(t), *workspace_rows(t), *toolbar_rows(t), *undo_back_rows(t), *preferences_rows(t),
                        *form_rows(t), *dismiss_rows(t), *default_rows(t), *modal_kind_rows(t), *timeline_rows(t), jobs_row(t)]
            elif group == "token":
                rows = token_rows(t)
            elif group == "confirm":
                rows = confirm_rows(t)
            else:
                rows = unsaved_rows(t, group.removeprefix("quit-"))
            for r in rows:
                if only and not any(o in r["name"] for o in only):
                    continue
                await t.flow_row(r)
            if group == "main":
                if not only or any("recovery" in o for o in only):
                    for _, button in CHOICES:
                        t.named(f"recovery open › {button}", f"01 §3.5, §3.7; {spec_line('project.recovery.open')}",
                                "its only entry, the start screen, shows only without a project, so the unsaved-changes question "
                                "never comes up in the window; shell/unsavedFlows.test.tsx covers the three choices")
                    for button in ("Restart", "Cancel"):
                        t.named(f"engine restart › {button}", "01 §12.1 (engine not responding)",
                                "it needs an engine that stops answering; App.test.tsx covers the question and its choices")
    finally:
        with contextlib.suppress(Exception):
            e2e.stop(proc)


def main() -> int:
    e6.dpi_aware()
    ap = argparse.ArgumentParser()
    ap.add_argument("app")
    ap.add_argument("out")
    ap.add_argument("--only", default="")
    a = ap.parse_args()
    a.app = str(Path(a.app).resolve())
    if sys.platform != "win32":
        print("D-44's flow table drives the real window through Windows input.")
        return 0
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    work = Path(tempfile.mkdtemp(prefix="tt-d44-"))
    only = {x for x in a.only.split(",") if x}
    started = time.monotonic()
    rows: list = []
    try:
        for group in ("main", "quit-discard", "quit-save", "confirm", "token"):
            if group.startswith("quit-") and only and not any("Quit" in o for o in only):
                continue
            if group == "token" and only and not any("Regenerate" in o for o in only):
                continue
            if group == "confirm" and only and not any("Confirmation" in o for o in only):
                continue
            before = len(rows)
            try:
                asyncio.run(session(a.app, run, out, work, group, only, rows))
            except Exception as ex:  # noqa: BLE001 - reported as a failure with its text
                import traceback
                quit_ended = group.startswith("quit-") and len(rows) > before
                if not quit_ended:
                    traceback.print_exception(ex)
                    run.check(False, f"the {group} session ran", repr(ex))
            run.save("rows.json", rows)
    finally:
        shutil.rmtree(work, ignore_errors=True)
    # C43B-R3S-03: every row the coverage names ran (a dialog's choice cannot be listed and not exercised).
    if not only:
        cov = _load("c43d44coverage", "c43_d44_coverage.py")
        ran = {r.get("name") for r in rows}
        missing = sorted(cov.covered_rows() - ran)
        run.check(not missing and not cov.problems(), "D-44 coverage: every dialog has its rows or a reason, and every row ran",
                  {"missing": missing, "problems": cov.problems()})
    manifest = {"kind": "toontown.c43b.d44-flows", "seconds": round(time.monotonic() - started, 1), "rows": len(rows),
                "named": [r["name"] for r in rows if r.get("notRun")], "failed": [r.get("name") for r in rows if not r.get("ok")],
                "failures": run.failures}
    run.save("rows.json", rows)
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps(manifest, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
