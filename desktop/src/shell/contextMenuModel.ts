// The context menu's items and where it opens (M4 C4.4a, design note 1 §9); the component is `ContextMenu.tsx`.

/** One item: a command reference with its label; `check` shows the kernel's check mark. */
export interface MenuCommand {
  readonly id: string;
  readonly label: string;
  readonly command: string;
  readonly params?: Readonly<Record<string, unknown>>;
  readonly check?: boolean;
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
