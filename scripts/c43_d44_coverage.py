"""The D-44 flow table's coverage of every dialog and question the window can show (C43B-R3S-03, ruling D-48).

`inventory()` generates the list from the front end's source (non-test files under desktop/src): each dialog opened by
title (`dialogs.open('Title', ...)`), each job progress dialog (`waitJob(..., 'Title', ...)`), each name prompt
(`prompt('id', 'Title', ...)`), each message box question (`messageId: '...'`), each inline choice group, each
`role="dialog"` popover with a literal label, and the classes the code shows (error boxes from `messages.failure`, the
native picker from `pickPath`, non-modal notifications from `messages.notify`). `COVERAGE` maps every item to the rows
of `scripts/c43-d44-flows.py` that exercise its choices, or to the reason it has none. `problems()` names every item
without an entry and every entry without an item: a new dialog without a row or a reason fails
(`scripts/test_c43_d44_inventory.py`, run in CI). The flow script checks at the end that every row named here ran.
Pure: no application, no MCP.
"""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "desktop" / "src"

PATTERNS = [
    ("dialog", re.compile(r"dialogs\.open(?:<(?:[^<>]|<[^<>]*>)*>)?\(\s*'([^']+)'")),
    ("progress", re.compile(r"waitJob\(.*?,\s*'([^']+)',\s*(?:true|false)\)")),
    ("prompt", re.compile(r"\bprompt\(\s*'[^']+',\s*'([^']+)'")),
    ("message", re.compile(r"messageId:\s*'([^']+)'")),
    ("inline", re.compile(r"data-tt-id=\"(\w+\.conflict)\"")),
    ("popover", re.compile(r"role=\"dialog\"(?:(?!>).)*?aria-label=\"([^\"]+)\"", re.S)),
]
CLASSES = [
    ("class:error box", re.compile(r"messages\.failure\(")),
    ("class:native picker", re.compile(r"\.pickPath\(")),
    ("class:notification", re.compile(r"messages\.notify\(")),
]

#: What no product path shows in M4: listed so the class has its reason.
ALWAYS = ["class:information box", "class:tip with suppression box", "class:native picker replace question",
          "kind:message box", "kind:form"]


def _product_files():
    for p in sorted(SRC.rglob("*")):
        rel = p.relative_to(SRC).as_posix()
        if p.suffix not in (".ts", ".tsx") or ".test." in p.name:
            continue
        if rel.startswith(("test/", "test-support/", "spikes/", "ipc/generated/", "__probes__/")):
            continue
        yield p


def inventory() -> list[str]:
    found: set[str] = set(ALWAYS)
    for p in _product_files():
        text = p.read_text(encoding="utf-8")
        for kind, pat in PATTERNS:
            for m in pat.finditer(text):
                found.add(f"{kind}:{m.group(1)}")
        for name, pat in CLASSES:
            if pat.search(text):
                found.add(name)
    return sorted(found)


UNSAVED = [f"{c} › {b}" for c in ("New", "Open", "Open Recent", "Open Backup", "Close", "Quit")
           for b in ("Save", "Don't Save", "Cancel")]
NO_COMMAND = "no command: dismissing it sends nothing"

#: Every inventory item: {"rows": [row names of scripts/c43-d44-flows.py]} or {"reason": why it has no row}.
COVERAGE: dict[str, dict] = {
    "dialog:Preferences": {"rows": ["Preferences › OK", "Preferences › Cancel"]},
    "kind:message box": {"rows": ["modal (message box) › Tab past the last control, Escape"]},
    "kind:form": {"rows": ["modal (form) › Tab past the last control, Escape"]},
    "dialog:Help": {"rows": ["Help › Close"], "class": NO_COMMAND},
    "dialog:About Toontown": {"rows": ["About › Close"], "class": NO_COMMAND},
    "dialog:Project Info": {"rows": ["Project Info › Close"], "class": NO_COMMAND},
    "dialog:New Project": {"rows": ["New Project › Create", "New Project › Cancel", "New › Save", "New › Don't Save"]},
    "dialog:Project Properties": {"rows": ["Project Properties › OK (set as default)", "Project Properties › Cancel"]},
    "dialog:Check Project": {"rows": ["Check Project › Repair", "Check Project › Close"]},
    "dialog:Open Backup": {"rows": ["backup chooser › Open", "backup chooser › Cancel", "Open Backup › Save",
                                    "Open Backup › Don't Save"]},
    "progress:Opening project": {"reason": "its Cancel sends job.cancel only before the open's commit point; the fixture "
                                           "projects open faster than a person can press it, so the window's progress "
                                           "Cancel is exercised on the progress window's row (an agent's long render) "
                                           "and the dialog's own Cancel by Vitest (App.test.tsx)"},
    "progress:Recovering project": {"reason": "as Opening project: a recovery open of the fixture finishes before a "
                                              "Cancel could land; recovery itself is named in the table"},
    "progress:Saving": {"reason": "no choice: a save is not cancellable (its window has no Cancel)"},
    "prompt:New Workspace": {"rows": ["New Workspace › Create", "New Workspace › Cancel"]},
    "prompt:Rename Workspace": {"rows": ["Rename Workspace › Rename", "Rename Workspace › Cancel"]},
    "message:project.unsavedChanges": {"rows": UNSAVED + ["unsaved changes › Enter (default Save)"], "default": "Save"},
    "class:native picker replace question": {"rows": ["Save As (existing file) › native Yes",
                                                       "Save As (existing file) › native No"],
                                              "default": "No (the platform's)"},
    "message:keymap.importMode": {"rows": ["shortcut editor › Import › Merge", "shortcut editor › Import › Replace",
                                           "shortcut editor › Import › Cancel",
                                           "shortcut editor › Import › Enter (default Merge)"], "default": "Merge"},
    "message:keymap.resetAll": {"rows": ["shortcut editor › Reset all › Reset All",
                                         "shortcut editor › Reset all › Cancel",
                                         "shortcut editor › Reset all › Enter (default Cancel)"], "default": "Cancel"},
    "message:engine.restartConfirm": {"reason": "needs an engine that stops answering; App.test.tsx covers the question "
                                                "and both choices (named in the table)", "default": "Cancel"},
    "inline:keymap.conflict": {"rows": ["shortcut editor › conflict › Override", "shortcut editor › conflict › Cancel"],
                               "default": "Cancel"},
    "popover:Jobs": {"rows": ["progress window › Cancel"]},
    "popover:Agent sessions": {"reason": "no question: Pause, Resume and Stop are single actions, each a D-39 row"},
    "popover:Start screen": {"reason": "no question: each entry is a single action (New, Open, a recent project, a "
                                       "recovery entry), each a D-39 row"},
    "class:error box": {"rows": ["error box › OK"], "class": NO_COMMAND},
    "class:native picker": {"rows": ["native picker › Cancel"], "class": NO_COMMAND},
    "class:notification": {"reason": "non-modal (01 §12.1 rule 6): it offers no choice and holds nothing; dismissing "
                                     "it sends nothing"},
    "class:information box": {"reason": "no product path shows a modal information box in M4 (messages.show is called "
                                        "only with the questions above; agent messages are notifications)",
                              "class": NO_COMMAND},
    "class:tip with suppression box": {"reason": "no product path shows a suppressible tip in M4 (no messages.show call "
                                                 "sets `suppressible`)", "class": NO_COMMAND},
}


def problems(items: list[str] | None = None) -> list[str]:
    items = inventory() if items is None else items
    out = [f"{i}: a dialog or question without a D-44 row or a reason" for i in items if i not in COVERAGE]
    out += [f"{k}: listed in the D-44 coverage but no longer in the window's source" for k in COVERAGE if k not in items]
    for k, v in COVERAGE.items():
        if not v.get("rows") and not v.get("reason"):
            out.append(f"{k}: neither rows nor a reason")
    return out


def covered_rows() -> set[str]:
    return {r for v in COVERAGE.values() for r in v.get("rows", [])}


if __name__ == "__main__":
    import json
    import sys
    print(json.dumps(inventory(), indent=1))
    p = problems()
    print("\n".join(p) or "every dialog and question has its rows or reason")
    sys.exit(1 if p else 0)
