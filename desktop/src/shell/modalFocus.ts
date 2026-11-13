// Focus of modal windows (M4 C4.1 round 1): a dialog or message box takes the keyboard focus when it opens, so its
// keys (Escape cancels, Enter submits) work at once, whoever opened it (a menu, a shortcut or an agent's
// `ui.preferences.open`); the focus returns to where it was when it closes.
import { useEffect, type RefObject } from 'react';

export function useModalFocus(ref: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const before = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (!el.contains(document.activeElement)) el.focus();
    return () => {
      // The dialog's element may already be gone: focus then sits on the body.
      const now = document.activeElement;
      if (before?.isConnected && (now === null || now === document.body || el.contains(now))) before.focus();
    };
  }, [ref]);
}
