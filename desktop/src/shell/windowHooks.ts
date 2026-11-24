// Behaviour every window of the shell shares (the main window and each floating-panel window; ruling O-2): the mirror
// store over the numbered engine events, the key dispatch through the kernel's keymap, the text-focus sensor and the
// rendered UI tree with the event number it reflects. Each window reports its own facts; the host combines them.
import { useEffect } from 'react';

import { GENERATION_EVENT, toHostError } from '../ipc';
import { isUnmodified, sequenceOf } from '../keys/sequence';
import TEXT_FIELD_KEYS from '../keys/text-field-keys.json';
import type { ShellServices } from '../state/context';
import type { Binding } from '../state/types';
import type { ActionsApi } from './actionsApi';
import { anyModalOpen } from './modalFocus';
import { collectUiTree, type UiNode } from './uiTree';

/** How long after the last change the rendered UI tree is reported (a frame or two; product value). */
export const TREE_REPORT_MS = 30;

/** The browser's own navigation keys (D-70 item 2b): never the page's to act on, in a text field or not. */
const BROWSER_KEYS = new Set([
  'BrowserBack',
  'BrowserForward',
  'BrowserRefresh',
  'BrowserHome',
  'BrowserStop',
]);

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

/** The one rule of which keys a focused text-entry element owns (ruling D-73 item 2): its data,
 *  `keys/text-field-keys.json`, is read here and by the kernel's key resolution (`keymap.rs` `text_field_owns`), which
 *  the window's `dispatch_key` and the agents' `key` tool share. */
const TEXT_EDITING: Readonly<Record<'mac' | 'other', ReadonlySet<string>>> = {
  other: new Set(TEXT_FIELD_KEYS.textEditing.other),
  mac: new Set(TEXT_FIELD_KEYS.textEditing.mac),
};
const FUNCTION_KEYS_KEPT: ReadonlySet<string> = new Set(TEXT_FIELD_KEYS.functionKeysKept);
/** SI-193 (finding C44B-R1S-04): the chords the canvas's text editor gives to the document (undo and redo). */
const CANVAS_TEXT_DOCUMENT_KEYS: Readonly<Record<'mac' | 'other', ReadonlySet<string>>> = {
  other: new Set(TEXT_FIELD_KEYS.fieldKinds.canvasText.documentKeys.other),
  mac: new Set(TEXT_FIELD_KEYS.fieldKinds.canvasText.documentKeys.mac),
};

/** The kind of a text field (the shared rule's `fieldKinds`): the canvas's text editor is marked
 *  `data-tt-field-kind="canvasText"`; every other field is ordinary. */
export type FieldKind = 'canvasText' | 'ordinary';

export function fieldKindOf(el: EventTarget | null): FieldKind {
  return el instanceof HTMLElement && el.dataset['ttFieldKind'] === 'canvasText' ? 'canvasText' : 'ordinary';
}

/** The rule over a key sequence alone (D-73 item 2), as the kernel applies it: a function key, plain or with Shift, is
 *  the keymap's unless the field keeps it (Shift+F10); any other key without Primary, Ctrl or Alt is the field's; a
 *  chord with them is the field's when it is one of the platform's text-editing chords. */
export function textFieldOwnsSequence(seq: string, mac: boolean, kind: FieldKind = 'ordinary'): boolean {
  // SI-193: in the canvas's text editor undo and redo are the document's (the kernel's `text_field_owns_kind`).
  if (kind === 'canvasText' && CANVAS_TEXT_DOCUMENT_KEYS[mac ? 'mac' : 'other'].has(seq)) return false;
  if (/^(Shift\+)?F\d+$/.test(seq)) return FUNCTION_KEYS_KEPT.has(seq);
  if (isUnmodified(seq)) return true;
  return TEXT_EDITING[mac ? 'mac' : 'other'].has(seq);
}

/** Whether a key pressed in `target` belongs to the focused text-entry element and never reaches the keymap (ruling
 *  D-66 item 2, C44A-R2-01): in a text field (the numeric field, text inputs, text areas, an inline rename, anything
 *  editable) the keys without Primary, Ctrl or Alt, a character typed with AltGr (or Option on macOS), and the
 *  platform's text-editing chords. Escape and Enter keep the field's own rules (its handlers act on them). Every other
 *  chord (Save, for one) still reaches the keymap.
 *  D-69 item 2 (C44A-R3S-01): a dead key (`key: "Dead"`, macOS Option+E), every key during a composition (`isComposing`,
 *  key code 229) and the key that completes it belong to the field, whatever their modifiers, so accents and input
 *  methods type. Function keys, plain or with Shift, type nothing: they reach the keymap from a field too, except
 *  Shift+F10, which the field keeps (its own context menu). */
export function fieldOwnsKey(
  target: EventTarget | null,
  seq: string,
  e: Pick<KeyboardEvent, 'key' | 'altKey' | 'ctrlKey' | 'metaKey'> & {
    getModifierState?: (k: string) => boolean;
    isComposing?: boolean;
    keyCode?: number;
  },
  mac: boolean,
): boolean {
  if (!isTextField(target)) return false;
  if (isComposition(e)) return true;
  // A list box is not text entry: only its typing-ahead (keys without Primary, Ctrl or Alt) is its own.
  if (target instanceof HTMLSelectElement) return isUnmodified(seq) && !/^(Shift\+)?F\d+$/.test(seq);
  const typed = e.key.length === 1 && e.key !== ' ';
  if (typed && e.getModifierState?.('AltGraph') === true) return true;
  if (typed && mac && e.altKey && !e.metaKey && !e.ctrlKey) return true;
  return textFieldOwnsSequence(seq, mac, fieldKindOf(target));
}

/** A dead key, or a key of an input method's composition (D-69 item 2): the browser composes the character. */
export function isComposition(e: { key: string; isComposing?: boolean; keyCode?: number }): boolean {
  return e.key === 'Dead' || e.key === 'Process' || e.isComposing === true || e.keyCode === 229;
}

/** The input types whose element is text entry (the webview's text-editing menu belongs to them). */
const TEXT_INPUTS = new Set(['', 'text', 'search', 'url', 'tel', 'email', 'password', 'number']);

/** D-70 item 2a: whether the webview's own context menu belongs to `el`: a text-entry element (a text-like input,
 *  read-only ones included, a text area, editable content), where it offers text editing. A list box, a slider or any
 *  other control is not one: there the webview would show the page's menu (Back, Reload, Save as, Print). */
export function isTextEntry(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  if (el.isContentEditable) return true;
  const editable = el.getAttribute('contenteditable');
  if (editable !== null && editable !== 'false') return true;
  if (el instanceof HTMLTextAreaElement) return true;
  return el instanceof HTMLInputElement && TEXT_INPUTS.has((el.getAttribute('type') ?? '').toLowerCase());
}

/** An agent's Shift+F10 (11 §7.6: simulated input takes the real input's path; M5 ruling E-48): the platform's keyboard
 *  `contextmenu` (no pointer position) to the control the notice names by its window id, or to the focused control, as
 *  the window's own Shift+F10 sends it (D-66 item 3). The control opens its menu where the keyboard is: a Timeline
 *  surface at the current row, cell or frame. */
export function openContextMenuFor(payload: unknown): void {
  const target =
    typeof payload === 'object' && payload !== null ? (payload as { target?: unknown }).target : undefined;
  const at =
    typeof target === 'string'
      ? document.querySelector(`[data-tt-id=${JSON.stringify(target)}]`)
      : document.activeElement;
  if (!(at instanceof HTMLElement)) return;
  at.focus({ preventScroll: true });
  at.dispatchEvent(
    new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 0, clientY: 0 }),
  );
}

/** Engine events applied in order; a resync at start and after a gap (IPC freshness). */
export function useEngineMirror(services: ShellServices, reportScreens: boolean): void {
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    let chain: Promise<void> = Promise.resolve();
    void services.engine
      .onEvent((e) => {
        if (e.name === 'shell.contextMenu') openContextMenuFor(e.payload);
        chain = chain.then(() => services.store.apply(e)).catch(() => undefined);
      })
      .then((u) => {
        if (disposed) u();
        else unlisten = u;
        // Read after subscribing (D-86 item 2): nothing between the read and the subscription is lost.
        if (!disposed) void services.store.resync();
      })
      .catch(() => {
        void services.store.resync();
      });
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
      // D-70 item 2b: the person cannot reload or navigate the page by any key: the browser's own keys are cancelled
      // everywhere; F5, Ctrl+R, Ctrl+Shift+R, Ctrl+F5, Alt+Left, Alt+Right, Backspace and their macOS forms are not the
      // field's (`fieldOwnsKey`) and are cancelled by the accelerator guard below unless the keymap binds them.
      if (BROWSER_KEYS.has(e.key)) {
        e.preventDefault();
        return;
      }
      // D-69 item 2: a dead key or a composition in a text field is the field's, never cancelled or dispatched.
      if (isTextField(e.target) && isComposition(e)) return;
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
        at?.dispatchEvent(
          new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 0, clientY: 0 }),
        );
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
        // A key bound to a disabled command answers its `command.disabled` (C41-R2-09), a key whose edit the
        // interactive-target rule refuses answers that refusal (D-83): never a message box (01 §8.4); the status line
        // says why, as for every input (ruling D-87).
        .catch((err: unknown) => {
          // The host rejects with its error's JSON text (a transport may wrap it in an Error).
          const e = toHostError(err instanceof Error ? err.message : err);
          actions.refused({ code: e.code, message: e.message, details: e.details }, seq);
        });
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, [actions, keymap, mac, services]);
}

/** D-70 item 2a: in the product the webview's own context menu opens only on text-entry elements (where it offers
 *  text editing); a secondary click anywhere else opens the application's own menu (its handler cancels the default
 *  first) or nothing, so no browser entry (Back, Forward, Reload, Save as, Print, Inspect, View source) is ever shown.
 *  The listener is the window's, in the bubbling phase, so the application's menus have handled the event first. The
 *  mouse's Back and Forward buttons navigate nothing either (D-70 item 2b). */
export function useNoBrowserMenus(): void {
  useEffect(() => {
    const onMenu = (e: MouseEvent) => {
      if (!isTextEntry(e.target)) e.preventDefault();
    };
    const onButton = (e: MouseEvent) => {
      if (e.button === 3 || e.button === 4) e.preventDefault();
    };
    window.addEventListener('contextmenu', onMenu);
    window.addEventListener('mouseup', onButton);
    window.addEventListener('auxclick', onButton);
    return () => {
      window.removeEventListener('contextmenu', onMenu);
      window.removeEventListener('mouseup', onButton);
      window.removeEventListener('auxclick', onButton);
    };
  }, []);
}

/** Whether this document has told the host it painted (once per document: a later report would retire the deliveries
 *  of a token the page shows). */
let pageReadySent = false;
/** D-72 item 1: how often, and how many more times, a report the host did not accept is sent again. */
const PAGE_READY_RETRY_MS = 200;
const PAGE_READY_TRIES = 50;

/** D-69 item 1: once this window's document has painted after its bootstrap (two frames after the first commit), the
 *  host is told (`page_ready`): it retires the token deliveries of the document this one replaced and lets captures
 *  of the window through again. Once per document. */
export function usePageReady(services: ShellServices): void {
  useEffect(() => {
    let stopped = false;
    // D-72 item 1: the host takes the report only once its own page-load event says the load finished; a report that
    // came earlier is ignored, so it is sent again (every 200 ms, up to 50 times) until the host accepts it.
    const report = (left: number) => {
      if (stopped || pageReadySent) return;
      void services.engine
        .pageReady()
        .then((accepted) => {
          if (accepted) pageReadySent = true;
          else if (left > 0)
            setTimeout(() => {
              report(left - 1);
            }, PAGE_READY_RETRY_MS);
        })
        .catch(() => {
          if (left > 0)
            setTimeout(() => {
              report(left - 1);
            }, PAGE_READY_RETRY_MS);
        });
    };
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        report(PAGE_READY_TRIES);
      });
    });
    return () => {
      stopped = true;
    };
  }, [services]);
}

/** Text focus is a sensor fact (the kernel's key dispatch and the `key` tool respect it). */
export function useTextFocusReport(services: ShellServices): void {
  useEffect(() => {
    const report = () => {
      const el = document.activeElement;
      // The field's kind goes with the fact, so the kernel's key resolution applies the same rule (SI-193).
      void services.engine
        .report({ textFocus: isTextField(el), textFieldKind: fieldKindOf(el) })
        .catch(() => undefined);
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
