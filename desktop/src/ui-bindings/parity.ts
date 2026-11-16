// The parity gate's collector (M4 C4.3b; parity note 9, ruling D-25): every element a person can act on in a rendered
// surface, with the command or commands it sends (`data-tt-command`, `data-tt-params`), or the reason it sends none
// (`data-tt-gui-only`, SI-155). The generated manifest (`parity.json`) is what this collects from every surface, plus
// the menu manifest, the keymap's defaults and the kernel's gesture table.

/** What a person can act on (parity note §3). */
export const ACTIONABLE =
  'button, [role="button"], [role="menuitem"], [role="menuitemcheckbox"], [role="tab"], [role="checkbox"], [role="switch"], [role="option"], input, select, textarea, a[href]';

/** SI-155: the reasons an actionable element sends no command. */
export const GUI_ONLY_REASONS = [
  'geometry',
  'scroll',
  'typing',
  'hover',
  'focus',
  'dismiss',
  'navigate',
  'placeholder',
  // Opens or closes a popover or a section (the agent sessions popover, 11 §6.4): nothing changes in the document or
  // the session; what the popover then shows is its own surface (D-35).
  'disclose',
  // A message box's button answers the question of the action that asked it (01 §12.1): the action's own command is
  // bound where the action is; an informational message's OK changes nothing (`dismiss`).
  'answer',
] as const;
export type GuiOnlyReason = (typeof GUI_ONLY_REASONS)[number];

export interface Action {
  /** The action's stable id: its `data-tt-id`. */
  id: string;
  surface: string;
  /** The commands it sends, in order (`data-tt-command`, several separated by spaces). */
  commands: string[];
  /** Its fixed parameters, when it has them. */
  params?: unknown;
}

export interface Collected {
  actions: Action[];
  guiOnly: { id: string; surface: string; reason: string }[];
  /** Actionable elements with neither a command nor a GUI-only reason, or without an id: the gate's failures (PA1). */
  unbound: { surface: string; what: string }[];
}

function describe(el: Element): string {
  const label = el.getAttribute('aria-label') ?? el.getAttribute('title') ?? el.textContent;
  return `<${el.tagName.toLowerCase()}${el.getAttribute('role') ? ` role=${el.getAttribute('role') ?? ''}` : ''}> ${label.trim().slice(0, 60)}`;
}

/** Collects the actions of `root` (a rendered surface) as `surface`. An element inside a `data-tt-surface` region is
 *  attributed to that surface (a dialog, a float, the status bar). */
export function collect(root: Element, surface: string): Collected {
  const out: Collected = { actions: [], guiOnly: [], unbound: [] };
  for (const el of Array.from(root.querySelectorAll(ACTIONABLE))) {
    const region = el.closest('[data-tt-surface]')?.getAttribute('data-tt-surface') ?? surface;
    const id = el.getAttribute('data-tt-id');
    const command = el.getAttribute('data-tt-command');
    // A third-party container around a declared element (the dock's tab wrapper around its declared tab) is that
    // element's: it is not counted twice.
    if (!id && !command && el.querySelector('[data-tt-id][data-tt-command], [data-tt-id][data-tt-gui-only]'))
      continue;
    // SI-155: a field inside a form is typing; the form's submit sends the command its value is for.
    const field =
      /^(INPUT|SELECT|TEXTAREA)$/.test(el.tagName) &&
      !['button', 'submit'].includes(el.getAttribute('type') ?? '');
    const gui =
      el.getAttribute('data-tt-gui-only') ?? (field && !command && el.closest('form') ? 'typing' : null);
    if (!id) {
      out.unbound.push({ surface: region, what: `${describe(el)} (no data-tt-id)` });
      continue;
    }
    if (command) {
      const params = el.getAttribute('data-tt-params');
      const a: Action = { id, surface: region, commands: command.split(/\s+/).filter(Boolean) };
      if (params) a.params = JSON.parse(params) as unknown;
      out.actions.push(a);
    } else if (gui) {
      out.guiOnly.push({ id, surface: region, reason: gui });
    } else {
      out.unbound.push({ surface: region, what: `${id}: ${describe(el)}` });
    }
  }
  return out;
}
