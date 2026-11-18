// Application modals (message boxes and forms such as Preferences, New Project, Project Properties): one shared
// implementation of ruling D-49 items 2 and 3 (C43B-R3-02, C43B-R3-03).
//
// While a modal is open: the focus stays inside the top modal (Tab and Shift+Tab cycle through its controls), the rest
// of the window is inert, Escape takes the top modal's Cancel wherever the focus is, and no key reaches the keymap
// (`anyModalOpen()`, which the shortcut handler reads: decided by a modal being open, not by the event's target). When
// the modal closes, the focus returns to where it was. One exception: a control that takes keys as data while it
// listens (the shortcut editor's capture, `data-modal-keys="own"`) receives Escape and Tab itself.
import { useEffect, useRef, type RefObject } from 'react';

interface OpenModal {
  readonly el: HTMLElement;
  readonly cancel: () => void;
  /** Elements this modal made inert (restored when it closes). */
  readonly inerted: HTMLElement[];
  /** Its own ancestors an earlier modal had made inert, made live while it is open (inert again when it closes). */
  readonly revived: HTMLElement[];
}

const stack: OpenModal[] = [];

/** Whether an application modal is open (the keymap receives no key then; D-49 item 3). */
export function anyModalOpen(): boolean {
  return stack.length > 0;
}

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), ' +
  'summary, [tabindex]:not([tabindex="-1"])';

function focusables(el: HTMLElement): HTMLElement[] {
  return Array.from(el.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((e) => !e.closest('[inert]'));
}

/** Makes every element outside `el` inert: each sibling of `el` and of each of its ancestors up to the body. */
function inertOutside(el: HTMLElement): HTMLElement[] {
  const out: HTMLElement[] = [];
  let node: HTMLElement = el;
  while (node !== document.body) {
    const parent: HTMLElement | null = node.parentElement;
    if (!parent) break;
    for (const sib of Array.from(parent.children)) {
      if (sib !== node && sib instanceof HTMLElement && !sib.hasAttribute('inert')) {
        sib.setAttribute('inert', '');
        out.push(sib);
      }
    }
    node = parent;
  }
  return out;
}

/** The ancestors of `el` that are inert (an earlier modal's doing): made live, and returned to be restored. */
function revive(el: HTMLElement): HTMLElement[] {
  const out: HTMLElement[] = [];
  for (let a: HTMLElement | null = el; a; a = a.parentElement) {
    if (a.hasAttribute('inert')) {
      a.removeAttribute('inert');
      out.push(a);
    }
  }
  return out;
}

function ownsKeys(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest('[data-modal-keys="own"]') !== null;
}

function onKey(e: KeyboardEvent): void {
  const top = stack[stack.length - 1];
  if (!top || ownsKeys(e.target)) return;
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    top.cancel();
    return;
  }
  if (e.key !== 'Tab') return;
  const items = focusables(top.el);
  const active = document.activeElement;
  const inside = active instanceof HTMLElement && top.el.contains(active);
  if (items.length === 0) {
    e.preventDefault();
    top.el.focus();
    return;
  }
  const first = items[0];
  const last = items[items.length - 1];
  if (e.shiftKey && (!inside || active === first || active === top.el)) {
    e.preventDefault();
    last?.focus();
  } else if (!e.shiftKey && (!inside || active === last)) {
    e.preventDefault();
    first?.focus();
  }
}

let listening = false;
function listen(): void {
  if (listening || typeof window === 'undefined') return;
  // Capture phase: before any control's own handler and before the keymap's window listener.
  window.addEventListener('keydown', onKey, true);
  listening = true;
}

/** Makes `ref`'s element an application modal for as long as it is mounted; `onCancel` is its Cancel (Escape). The
 *  element is focused unless the focus is already inside it (a message box's default button has `autoFocus`). */
export function useModalFocus(ref: RefObject<HTMLElement | null>, onCancel?: () => void): void {
  const cancelRef = useRef(onCancel);
  useEffect(() => {
    cancelRef.current = onCancel;
  });
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    listen();
    const before = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const revived = revive(el);
    const entry: OpenModal = {
      el,
      cancel: () => {
        cancelRef.current?.();
      },
      inerted: inertOutside(el),
      revived,
    };
    stack.push(entry);
    if (!el.contains(document.activeElement)) el.focus();
    return () => {
      const i = stack.indexOf(entry);
      if (i >= 0) stack.splice(i, 1);
      for (const e of entry.inerted) e.removeAttribute('inert');
      // The modal below is still open: what it made inert is inert again.
      if (stack.length > 0) for (const e of entry.revived) if (e.isConnected) e.setAttribute('inert', '');
      // The dialog's element may already be gone: focus then sits on the body.
      const now = document.activeElement;
      if (before?.isConnected && (now === null || now === document.body || el.contains(now))) before.focus();
    };
  }, [ref]);
}
