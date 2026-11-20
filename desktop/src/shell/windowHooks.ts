// Behaviour every window of the shell shares (the main window and each floating-panel window; ruling O-2): the mirror
// store over the numbered engine events, the key dispatch through the kernel's keymap, the text-focus sensor and the
// rendered UI tree with the event number it reflects. Each window reports its own facts; the host combines them.
import { useEffect } from 'react';

import { GENERATION_EVENT } from '../ipc';
import { isUnmodified, sequenceOf } from '../keys/sequence';
import type { ShellServices } from '../state/context';
import type { Binding } from '../state/types';
import type { ActionsApi } from './actionsApi';
import { anyModalOpen } from './modalFocus';
import { collectUiTree, type UiNode } from './uiTree';

/** How long after the last change the rendered UI tree is reported (a frame or two; product value). */
export const TREE_REPORT_MS = 30;

/** Browser accelerators the webview must not act on outside text fields (reload, find, print, zoom, navigation). */
export function isBrowserAccelerator(seq: string): boolean {
  return (
    seq.includes('Primary+') || seq.startsWith('Alt+') || /^(Shift\+)?F\d+$/.test(seq) || seq === 'Backspace'
  );
}

export function isTextField(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  if (el.isContentEditable) return true;
  // Editable by its own attribute (also where `isContentEditable` is not computed).
  const editable = el.getAttribute('contenteditable');
  if (editable !== null && editable !== 'false') return true;
  if (el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement) return true;
  return (
    el instanceof HTMLInputElement && !['checkbox', 'radio', 'button', 'submit', 'reset'].includes(el.type)
  );
}

const ARROWS = ['Left', 'Right', 'Up', 'Down'];

/** The platform's text-editing chords (01 §8.4: "platform text-editing shortcuts act on the focused text field"; ruling
 *  D-66 item 2): select all, cut, copy, paste, undo and redo of the field's own text, word and line navigation and
 *  selection, word deletion. Keys without Primary, Ctrl or Alt (characters, Shift with a character, Home, End, Delete,
 *  Backspace, the arrows, Enter, Escape) are the field's anyway. */
const TEXT_EDITING: Readonly<Record<'mac' | 'other', ReadonlySet<string>>> = {
  other: new Set([
    ...['A', 'X', 'C', 'V', 'Z', 'Y', 'Insert', 'Backspace', 'Delete', 'Home', 'End', ...ARROWS].map(
      (k) => `Primary+${k}`,
    ),
    ...['Z', 'Home', 'End', ...ARROWS].map((k) => `Primary+Shift+${k}`),
    'Alt+Backspace',
  ]),
  mac: new Set([
    ...['A', 'X', 'C', 'V', 'Z', 'Backspace', 'Delete', ...ARROWS].map((k) => `Primary+${k}`),
    ...['Z', ...ARROWS].map((k) => `Primary+Shift+${k}`),
    ...['Backspace', 'Delete', ...ARROWS].map((k) => `Alt+${k}`),
    ...ARROWS.map((k) => `Alt+Shift+${k}`),
    // The Cocoa text system's Control keys (line start and end, kill, character and line moves).
    ...['A', 'E', 'K', 'B', 'F', 'N', 'P', 'D', 'H', 'T', 'Y', 'O'].map((k) => `Ctrl+${k}`),
    ...['A', 'E', 'B', 'F', 'N', 'P'].map((k) => `Ctrl+Shift+${k}`),
  ]),
};

/** Whether a key pressed in `target` belongs to the focused text-entry element and never reaches the keymap (ruling
 *  D-66 item 2, C44A-R2-01): in a text field (the numeric field, text inputs, text areas, an inline rename, anything
 *  editable) the keys without Primary, Ctrl or Alt, a character typed with AltGr (or Option on macOS), and the
 *  platform's text-editing chords. Escape and Enter keep the field's own rules (its handlers act on them). Every other
 *  chord (Save, for one) still reaches the keymap. */
export function fieldOwnsKey(
  target: EventTarget | null,
  seq: string,
  e: Pick<KeyboardEvent, 'key' | 'altKey' | 'ctrlKey' | 'metaKey'> & {
    getModifierState?: (k: string) => boolean;
  },
  mac: boolean,
): boolean {
  if (!isTextField(target)) return false;
  if (isUnmodified(seq)) return true;
  // A list box is not text entry: only its typing-ahead (the keys above) is its own.
  if (target instanceof HTMLSelectElement) return false;
  const typed = e.key.length === 1 && e.key !== ' ';
  if (typed && e.getModifierState?.('AltGraph') === true) return true;
  if (typed && mac && e.altKey && !e.metaKey && !e.ctrlKey) return true;
  return TEXT_EDITING[mac ? 'mac' : 'other'].has(seq);
}

/** Engine events applied in order; a resync at start and after a gap (IPC freshness). */
export function useEngineMirror(services: ShellServices, reportScreens: boolean): void {
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    let chain: Promise<void> = Promise.resolve();
    void services.engine
      .onEvent((e) => {
        chain = chain.then(() => services.store.apply(e)).catch(() => undefined);
      })
      .then((u) => {
        if (disposed) u();
        else unlisten = u;
      });
    void services.store.resync();
    if (reportScreens) {
      void services.engine
        .report({
          attached: true,
          screens: [
            {
              x: 0,
              y: 0,
              width: Math.max(1, window.screen.availWidth || window.innerWidth),
              height: Math.max(1, window.screen.availHeight || window.innerHeight),
              scale: window.devicePixelRatio || 1,
            },
          ],
        })
        .catch(() => undefined);
    }
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [services, reportScreens]);
}

/** Whether the key event's target is a control Enter or Space activates (a button, a link, a check box, a radio). */
function isActivatable(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  if (el instanceof HTMLButtonElement || el instanceof HTMLAnchorElement) return true;
  if (el instanceof HTMLInputElement)
    return ['checkbox', 'radio', 'button', 'submit', 'reset'].includes(el.type);
  return ['button', 'menuitem', 'menuitemcheckbox', 'option', 'tab', 'checkbox', 'switch'].includes(
    el.getAttribute('role') ?? '',
  );
}

/** Shortcuts (01 §8.4): the kernel's keymap decides; a text field keeps its keys (`fieldOwnsKey`, D-66 item 2: the
 *  platform's text-editing keys act on the field and never reach the keymap, in a dialog too). Keys inside a modal dialog
 *  or message box are the dialog's (its buttons, its Escape), and Enter or Space on a focused control activates it: the
 *  keymap's Enter (Play) once took a message box's default button's Enter (found by the D-39 table, C4.3b round 3). */
export function useShortcuts(
  services: ShellServices,
  actions: ActionsApi,
  keymap: readonly Binding[],
  mac: boolean,
): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented) return;
      const seq = sequenceOf(e, mac);
      if (!seq) return;
      // The field's own: not consumed, so the browser's text editing acts on it (Primary+A selects its text).
      if (fieldOwnsKey(e.target, seq, e, mac)) return;
      // D-49 item 3: no key reaches the keymap while an application modal is open, wherever the focus is (decided by
      // the modal being open, not by the event's target); the page's own accelerators (reload, print) stay off.
      if (anyModalOpen()) {
        if (isBrowserAccelerator(seq)) e.preventDefault();
        return;
      }
      if ((seq === 'Enter' || seq === 'Space') && isActivatable(e.target)) return;
      // D-66 item 3: Shift+F10 opens the focused control's context menu, as the context-menu key does (the platform's
      // keyboard reach, 01 §5.4). The function-key guard below would cancel the browser's own `contextmenu` for it, so
      // the same event is sent to the focused control here (no pointer position: the menu opens at the control).
      if (seq === 'Shift+F10') {
        e.preventDefault();
        const at = e.target instanceof Element ? e.target : document.activeElement;
        at?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 0, clientY: 0 }));
        return;
      }
      const bound = keymap.some((b) => b.keys === seq);
      if (!bound && !isBrowserAccelerator(seq)) return;
      e.preventDefault();
      if (!bound) return;
      void services.engine
        .dispatchKey(seq)
        .then(async (outcome) => {
          if (outcome.outcome !== 'command' || !outcome.commandId) return;
          await actions.perform(outcome.commandId, outcome.params ?? {});
        })
        // A key bound to a disabled command answers its `command.disabled` (C41-R2-09): the key is consumed silently,
        // never with a message box (01 §8.4).
        .catch(() => undefined);
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, [actions, keymap, mac, services]);
}

/** Text focus is a sensor fact (the kernel's key dispatch and the `key` tool respect it). */
export function useTextFocusReport(services: ShellServices): void {
  useEffect(() => {
    const report = () => {
      void services.engine.report({ textFocus: isTextField(document.activeElement) }).catch(() => undefined);
    };
    window.addEventListener('focusin', report);
    window.addEventListener('focusout', report);
    // A new engine generation starts without this window's facts (ruling O-9, O-14).
    window.addEventListener(GENERATION_EVENT, report);
    return () => {
      window.removeEventListener('focusin', report);
      window.removeEventListener('focusout', report);
      window.removeEventListener(GENERATION_EVENT, report);
    };
  }, [services]);
}

/**
 * The rendered UI tree, with the last engine event it reflects (inspect_ui freshness): reported after any change of
 * the document (dialogs included, which render outside the shell's root) or of the store, a frame or two later.
 */
export function useTreeReporter(services: ShellServices, rootNode: () => UiNode): void {
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    const report = () => {
      timer = null;
      const st = services.store.getState();
      const tree = collectUiTree(document.body, rootNode());
      void services.engine.report({ uiTree: tree, renderedSeq: st.seq }).catch(() => undefined);
    };
    const schedule = () => {
      timer ??= setTimeout(report, TREE_REPORT_MS);
    };
    const observer = new MutationObserver(schedule);
    observer.observe(document.body, {
      subtree: true,
      childList: true,
      attributes: true,
      characterData: true,
    });
    const unsubscribe = services.store.subscribe(schedule);
    schedule();
    return () => {
      observer.disconnect();
      unsubscribe();
      if (timer !== null) clearTimeout(timer);
    };
  }, [services, rootNode]);
}
