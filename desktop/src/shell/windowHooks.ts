// Behaviour every window of the shell shares (the main window and each floating-panel window; ruling O-2): the mirror
// store over the numbered engine events, the key dispatch through the kernel's keymap, the text-focus sensor and the
// rendered UI tree with the event number it reflects. Each window reports its own facts; the host combines them.
import { useEffect } from 'react';

import { GENERATION_EVENT } from '../ipc';
import { isUnmodified, sequenceOf } from '../keys/sequence';
import type { ShellServices } from '../state/context';
import type { Binding } from '../state/types';
import type { ActionsApi } from './actionsApi';
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
  if (el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement) return true;
  return (
    el instanceof HTMLInputElement && !['checkbox', 'radio', 'button', 'submit', 'reset'].includes(el.type)
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

/** Shortcuts (01 §8.4): the kernel's keymap decides; a text field keeps unmodified keys. */
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
      const field = isTextField(e.target);
      if (field && isUnmodified(seq)) return;
      const bound = keymap.some((b) => b.keys === seq);
      if (!bound && !isBrowserAccelerator(seq)) return;
      e.preventDefault();
      if (!bound) return;
      void services.engine
        .dispatchKey(seq)
        .then(async (outcome) => {
          if (outcome.outcome !== 'command' || !outcome.commandId) return;
          // A disabled command consumes its key silently (01 §8.4).
          if (outcome.enabled === false) return;
          await actions.perform(outcome.commandId, outcome.params ?? {});
        })
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
