# M4 design note: the window's questions and modals (ruling D-49)

Status: C4.3b round 3. Rulings D-49 items 1 to 3 (C43B-R3-01 to -03), D-46 item 3 (SI-161), D-44 (the flow table).
Built in `desktop/src/shell/makeActions.tsx`, `messages.tsx`, `modalFocus.ts`, `dialogs.tsx`, `windowHooks.ts`;
checked by `scripts/c43-d44-flows.py` (real window) and the Vitest files named below.

## 1. One replacement decision per operation (D-49 item 1; 01 §9.5)

**The native save picker always asks.** The window's save pickers are `tauri-plugin-dialog`'s, which uses `rfd` 0.17.2
(`Cargo.lock`).

- **Windows:** `rfd` builds the save dialog as an `IFileSaveDialog`. It sets filters, start folder, file name and title,
  and never calls `SetOptions` (`rfd-0.17.2/src/backend/win_cid/file_dialog/dialog_ffi.rs`, `build_save_file`, lines
  301-310). The dialog therefore keeps Windows' default save options, which include `FOS_OVERWRITEPROMPT` (Microsoft's
  `FILEOPENDIALOGOPTIONS` reference: the default for a Save dialog). The round 3 live session observed that question
  ("Confirm Save As", Yes / No).
- **macOS:** `rfd` uses an `NSSavePanel` (`rfd-0.17.2/src/backend/macos/file_dialog/panel_ffi.rs`). AppKit's save panel
  asks whether to replace an existing file before it returns the URL; no option turns that off.

**The paths in the window that reach a replacement decision:**

| Path | Where the path comes from | The decision | What the window sends |
|---|---|---|---|
| File › Save As | the native save picker | the picker's own question | `project.saveAs {path, onExists: overwrite}`, no second question |
| File › Save of a read-only or untitled project | goes to Save As | as above | as above |
| Preferences › Keyboard › Export | the native save picker | the picker's own question | `keymap.export {path, onExists: overwrite}` (unchanged) |
| Window › Workspace › Export Workspace | the native save picker | the picker's own question | `workspace.export {path, onExists: overwrite}` (unchanged) |

No path in the window reaches `project.saveAs` without the picker. **The application's own Replace / Cancel question is
therefore removed from the window.** The kernel keeps its `project.exists` refusal for agents, who pass `onExists`
themselves. D-44 rows: `Save As (existing file) › native Yes` (one question, the file saved over) and `› native No`
(nothing sent). A Vitest (`saveAsFlows.test.tsx`) checks that the window sends `onExists: overwrite` and shows no
question, and that a cancelled picker sends nothing.

## 2. Every message box opens on a focused default (D-49 item 2)

When a message box opens, its focused button is the one with the `default` role, else its Cancel, else its first
button. Enter or Space takes the focused button: keys inside a modal are the modal's, and the keymap receives none
(§3). A destructive question with no ruled default opens on Cancel.

| Box | Default | Why |
|---|---|---|
| Unsaved changes (Save / Don't Save / Cancel) | Save | The affirmative destroys nothing; the platforms' convention (the destructive choice is Don't Save, never the default). The live matrix passed with Enter → Save. |
| Keymap Import (Merge / Replace / Cancel) | Merge | D-46 item 3, SI-161: the choice that destroys nothing. |
| Reset all (Reset All / Cancel) | Cancel | Destructive, no ruled default. |
| Engine restart (Restart / Cancel) | Cancel | Destructive (work since the last autosave is lost), no ruled default. |
| Shortcut conflict (Override / Cancel), inline in Preferences | Cancel | Destructive (the other command loses its key). |
| Error and information boxes (OK) | OK | One button. |

The D-44 table records each default in `scripts/c43_d44_coverage.py`. Rows press Enter on the unsaved-changes question,
Reset all and Import.

## 3. Modals contain the focus (D-49 item 3)

One implementation, `desktop/src/shell/modalFocus.ts`, used by every application modal: the message box (`messages.tsx`)
and the dialog component (`dialogs.tsx`: Preferences, New Project, Project Properties, the result dialogs, the name
prompts, the job progress dialog). While a modal is open:

- **The focus stays inside.** Tab and Shift+Tab cycle through its controls.
- **The rest of the window is inert.** Every sibling of the modal and of its ancestors gets the `inert` attribute. A
  modal opened over another modal makes its own ancestors live again, and gives them back when it closes.
- **Escape takes the top modal's Cancel**, wherever the focus is (a capture-phase listener on the window).
- **No key reaches the keymap** while any modal is open. The shortcut handler reads `anyModalOpen()`, so the decision is
  made by a modal being open, not by the event's target. The page's own accelerators (reload, print) stay off.
- **The exception:** a control that takes keys as data while it listens (the shortcut editor's capture,
  `data-modal-keys="own"` while capturing) receives Escape and Tab itself. Escape then ends the capture, as 01 §8.3
  asks, and a second Escape cancels the dialog.

Tests: `modalFocus.test.tsx` (the cycle, Escape from outside the modal, the inert background, no keymap while a modal is
open, a message box over a form). D-44 rows, one per modal kind: `modal (message box) › Tab past the last control,
Escape` and `modal (form) › …`.
