// Key events → platform-neutral key sequences (01 §12.1 rule 17), the notation the kernel's keymap parses
// (`crates/executor/src/keymap.rs`): modifiers in the order Primary+Ctrl+Alt+Shift, then the key. Primary is Command
// on macOS and Control elsewhere; Ctrl is the physical Control key on macOS only. Letters follow the layout
// (`KeyboardEvent.key`); digits and punctuation follow the physical key (`KeyboardEvent.code`), so Shift+1 is
// "Shift+1", as the kernel spells it.

export interface KeyLike {
  readonly key: string;
  readonly code: string;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly altKey: boolean;
  readonly shiftKey: boolean;
}

const CODE_KEYS: Readonly<Record<string, string>> = {
  Comma: ',',
  Period: '.',
  Slash: '/',
  Backslash: '\\',
  Semicolon: ';',
  Quote: "'",
  BracketLeft: '[',
  BracketRight: ']',
  Minus: '-',
  Equal: '=',
  Backquote: '`',
  Space: 'Space',
};

const NAMED: Readonly<Record<string, string>> = {
  Enter: 'Enter',
  Tab: 'Tab',
  Escape: 'Escape',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Insert: 'Insert',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ' ': 'Space',
};

const MODIFIER_KEYS = new Set(['Shift', 'Control', 'Alt', 'Meta', 'OS', 'AltGraph', 'CapsLock', 'Fn']);

/** The canonical sequence of a key event, or null for a bare modifier (01 §8.3 item 4: not accepted). */
export function sequenceOf(e: KeyLike, mac: boolean): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null;
  let key: string | undefined;
  if (/^Digit[0-9]$/.test(e.code)) key = e.code.slice(5);
  else if (/^Numpad[0-9]$/.test(e.code)) key = e.code.slice(6);
  else if (CODE_KEYS[e.code]) key = CODE_KEYS[e.code];
  else if (/^F([1-9]|1[0-9]|2[0-4])$/.test(e.key)) key = e.key;
  else if (NAMED[e.key]) key = NAMED[e.key];
  else if (/^[a-zA-Z]$/.test(e.key)) key = e.key.toUpperCase();
  else if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
  if (!key) return null;
  const parts: string[] = [];
  const primary = mac ? e.metaKey : e.ctrlKey;
  if (primary) parts.push('Primary');
  if (mac && e.ctrlKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');
  parts.push(key);
  return parts.join('+');
}

/** Native notation for menus and tooltips (01 §0): ⌃⌥⇧⌘ on macOS, Ctrl+Alt+Shift elsewhere. */
export function nativeNotation(sequence: string, mac: boolean): string {
  const parts = sequence.split('+');
  const key = parts.pop() ?? '';
  const has = (m: string) => parts.includes(m);
  if (mac) {
    return `${has('Ctrl') ? '⌃' : ''}${has('Alt') ? '⌥' : ''}${has('Shift') ? '⇧' : ''}${has('Primary') ? '⌘' : ''}${key}`;
  }
  const out: string[] = [];
  if (has('Primary')) out.push('Ctrl');
  if (has('Alt')) out.push('Alt');
  if (has('Shift')) out.push('Shift');
  out.push(key);
  return out.join('+');
}

/** Whether a sequence is an unmodified key that a focused text field keeps (01 §8.4). */
export function isUnmodified(sequence: string): boolean {
  const parts = sequence.split('+');
  return !parts.includes('Primary') && !parts.includes('Alt') && !parts.includes('Ctrl');
}

/** Whether the platform is macOS (the webview's own report). */
export function isMac(): boolean {
  const nav = typeof navigator === 'undefined' ? undefined : navigator;
  const platform = nav?.platform ?? '';
  return /Mac|iPhone|iPad/.test(platform);
}
