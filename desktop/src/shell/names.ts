// The shared name rule (01 §9.6), mirrored for inline validation; the executor applies the same rule on every command.

const FORBIDDEN = '/\\:*?"<>|';

/** The problem with a name, or null when it is valid. */
export function nameProblem(name: string): string | null {
  if (name.length === 0) return 'A name cannot be empty.';
  if (Array.from(name).length > 100) return 'A name can have at most 100 characters.';
  if (name.trim() !== name) return 'A name cannot start or end with spaces.';
  if (name.endsWith('.')) return 'A name cannot end with a dot.';
  for (const ch of name) {
    const code = ch.codePointAt(0) ?? 0;
    if (FORBIDDEN.includes(ch) || code < 0x20 || code === 0x7f) {
      return `A name cannot contain ${FORBIDDEN.split('').join(' ')} or control characters.`;
    }
  }
  return null;
}
