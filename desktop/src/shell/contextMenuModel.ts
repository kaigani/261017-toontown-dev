// The context menu's items and where it opens (M4 C4.4a, design note 1 §9); the component is `ContextMenu.tsx`.

/** One item: a command reference with its label; `check` shows the kernel's check mark. */
export interface MenuCommand {
  readonly id: string;
  readonly label: string;
  readonly command: string;
  readonly params?: Readonly<Record<string, unknown>>;
  readonly check?: boolean;
  /** A later checkpoint that owns an item the spec requires but no command performs yet (04 §11.9 "add default
   *  keyframe"): shown disabled, "arrives with <owner>", never performed or asked of the kernel (`command` is empty).
   *  An item whose command exists takes its owner from the deferred list or the kernel instead. */
  readonly owner?: string;
  /** An item that collects a value first (a dialog): chosen, it runs this instead of performing the reference, which
   *  stays its availability and its parity binding (the dialog completes it). */
  readonly onSelect?: () => void;
}

/** A group of items, with a heading when it has a name (02 §4.4: "Arrange", "Transform"). */
export interface MenuGroup {
  readonly label?: string;
  readonly items: readonly MenuCommand[];
}

export interface MenuAt {
  readonly x: number;
  readonly y: number;
}

/** Where a `contextmenu` event opens a menu: the pointer, or the target's corner when the keyboard opened it. */
export function menuAt(e: { clientX: number; clientY: number; currentTarget: Element }): MenuAt {
  if (e.clientX !== 0 || e.clientY !== 0) return { x: e.clientX, y: e.clientY };
  const r = e.currentTarget.getBoundingClientRect();
  return { x: r.left + 8, y: r.top + 8 };
}

/** The menu's natural size and the window's client area, in CSS pixels. */
export interface Size {
  readonly width: number;
  readonly height: number;
}

/** Where a menu is drawn: its top-left corner and, when it does not fit, the bounds it scrolls within. */
export interface MenuPlacement {
  readonly x: number;
  readonly y: number;
  /** Set when the menu is taller than the client area: it scrolls within this height. */
  readonly maxHeight: number | null;
  /** Set when the menu is wider than the client area. */
  readonly maxWidth: number | null;
}

/** The gap kept between a menu and the client area's edges. */
export const MENU_MARGIN = 4;

/** One axis: open toward the side with room from `at`; flip to end at `at` when it would cross the far
 *  edge; shift inside when neither side has room; the start edge when it is longer than the area. */
function axis(
  at: number,
  length: number,
  area: number,
  margin: number,
): { start: number; max: number | null } {
  const room = area - 2 * margin;
  if (length > room) return { start: margin, max: Math.max(room, 0) };
  // A pointer past an edge (a press that began outside, a keyboard opening near an edge) counts as at the edge.
  at = Math.min(Math.max(at, margin), area - margin);
  if (at + length <= area - margin) return { start: Math.max(at, margin), max: null };
  if (at - length >= margin) return { start: at - length, max: null };
  return { start: Math.min(Math.max(at, margin), area - margin - length), max: null };
}

/** D-60 item 1 (C44A-R1-01): a context menu is always fully inside the window's client area. It opens
 *  toward the side with room, flips or shifts when it would cross an edge, and when it is taller than the
 *  window it scrolls (the component shows scroll affordances). A submenu is placed by the same function
 *  from its parent item. */
export function placeMenu(at: MenuAt, menu: Size, area: Size, margin = MENU_MARGIN): MenuPlacement {
  const h = axis(at.x, menu.width, area.width, margin);
  const v = axis(at.y, menu.height, area.height, margin);
  return { x: h.start, y: v.start, maxHeight: v.max, maxWidth: h.max };
}
