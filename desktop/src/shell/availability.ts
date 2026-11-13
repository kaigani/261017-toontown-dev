// Availability of command references (ruling O-10): whether each can run now, its check mark and its state-dependent
// label, from the kernel (`availability`), the one source the native menu, toolbars, the tool rail and the key
// dispatch share. The front end never derives enablement itself: until the kernel has answered, an item is disabled.

/** A command reference a GUI surface shows. */
export interface CommandRef {
  readonly commandId: string;
  readonly params: Readonly<Record<string, unknown>>;
}

/** The kernel's answer for one reference. */
export interface Availability {
  readonly commandId: string;
  readonly params: unknown;
  readonly enabled: boolean;
  readonly reason?: string;
  readonly checked?: boolean;
  readonly label?: string;
  readonly dialog: boolean;
}

function stable(v: unknown): string {
  if (v === undefined) return 'null';
  if (v === null || typeof v !== 'object') return JSON.stringify(v);
  if (Array.isArray(v)) return `[${v.map(stable).join(',')}]`;
  const o = v as Record<string, unknown>;
  return `{${Object.keys(o)
    .sort()
    .map((k) => `${JSON.stringify(k)}:${stable(o[k])}`)
    .join(',')}}`;
}

/** The key of a reference in the store's `availability` slice. */
export function refKey(ref: CommandRef): string {
  return `${ref.commandId} ${stable(ref.params)}`;
}

/** Whether a value is one availability answer. */
export function isAvailability(v: unknown): v is Availability {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  return (
    typeof o['commandId'] === 'string' &&
    typeof o['enabled'] === 'boolean' &&
    typeof o['dialog'] === 'boolean'
  );
}
